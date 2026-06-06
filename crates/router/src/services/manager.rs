use std::{collections::HashMap, sync::Arc};

use anyhow::{anyhow, Result};
use parking_lot::RwLock;
use tracing::{info, warn};

use super::{
    models::{CallContext, WorkerHandle, WorkerSpec},
    worker_process::WorkerProcess,
};

#[derive(Clone)]
pub struct ServiceManager {
    workers: Arc<RwLock<HashMap<String, Arc<WorkerProcess>>>>,
    service_to_worker: Arc<RwLock<HashMap<String, String>>>,
}

impl ServiceManager {
    pub fn new() -> Self {
        Self {
            workers: Arc::new(RwLock::new(HashMap::new())),
            service_to_worker: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 声明式拉起任意 worker：按 `spec.key` 复用与探活，进程已亡则重建（自愈）。
    /// 不绑定具体服务；供 runtime worker / 子 session 派发等场景复用。
    pub async fn ensure_worker(&self, spec: WorkerSpec) -> Result<WorkerHandle> {
        let existing_worker_id = {
            let service_to_worker = self.service_to_worker.read();
            service_to_worker.get(&spec.key).cloned()
        };

        if let Some(existing_id) = existing_worker_id {
            let worker = {
                let workers = self.workers.read();
                workers.get(&existing_id).cloned()
            };
            if let Some(worker) = worker {
                if worker.is_alive().await {
                    info!(
                        worker_id = existing_id,
                        key = spec.key,
                        "reusing existing worker"
                    );
                    return Ok(WorkerHandle {
                        worker_id: existing_id.clone(),
                    });
                }
                warn!(
                    worker_id = existing_id,
                    key = spec.key,
                    "worker gone, recreating"
                );
                self.workers.write().remove(&existing_id);
                self.service_to_worker
                    .write()
                    .retain(|_, worker_id| worker_id != &existing_id);
            }
        }

        let worker_id = uuid::Uuid::new_v4().to_string();
        let worker = WorkerProcess::start_with(
            worker_id.clone(),
            spec.service_name.clone(),
            &spec.executable,
            &spec.args,
            &spec.env,
        )
        .await?;

        self.workers.write().insert(worker_id.clone(), worker);
        self.service_to_worker
            .write()
            .insert(spec.key.clone(), worker_id.clone());

        Ok(WorkerHandle {
            worker_id: worker_id.clone(),
        })
    }

    /// 统计当前以 `key_prefix` 开头注册的活跃 worker 数量（用于并发上限防 fork 爆炸）。
    pub fn count_workers_with_prefix(&self, key_prefix: &str) -> usize {
        self.service_to_worker
            .read()
            .keys()
            .filter(|key| key.starts_with(key_prefix))
            .count()
    }

    pub async fn call_worker(
        &self,
        worker_id: &str,
        ctx: CallContext,
    ) -> Result<serde_json::Value> {
        let worker = {
            let workers = self.workers.read();
            workers
                .get(worker_id)
                .cloned()
                .ok_or_else(|| anyhow!("worker not found: {worker_id}"))?
        };

        worker.invoke(ctx).await
    }

    pub async fn stop_worker(&self, worker_id: &str) -> bool {
        let worker = self.workers.write().remove(worker_id);
        self.service_to_worker
            .write()
            .retain(|_, mapped_worker_id| mapped_worker_id != worker_id);
        if let Some(worker) = worker {
            worker.stop().await;
            true
        } else {
            false
        }
    }

    pub async fn stop_worker_by_key(&self, key: &str) -> bool {
        let worker_id = self.service_to_worker.write().remove(key);
        if let Some(worker_id) = worker_id {
            self.stop_worker(&worker_id).await
        } else {
            false
        }
    }

    pub async fn stop_workers_with_prefix(&self, key_prefix: &str) -> usize {
        let worker_ids = {
            let mut service_to_worker = self.service_to_worker.write();
            let keys: Vec<String> = service_to_worker
                .keys()
                .filter(|key| key.starts_with(key_prefix))
                .cloned()
                .collect();
            keys.into_iter()
                .filter_map(|key| service_to_worker.remove(&key))
                .collect::<Vec<_>>()
        };
        let mut stopped = 0;
        for worker_id in worker_ids {
            if self.stop_worker(&worker_id).await {
                stopped += 1;
            }
        }
        stopped
    }
}
