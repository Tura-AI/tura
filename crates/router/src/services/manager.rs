use std::collections::HashSet;
use std::{collections::HashMap, path::Path, sync::Arc};

use anyhow::{anyhow, Result};
use parking_lot::RwLock;
use tracing::{info, warn};

use super::{
    models::{CallContext, WorkerHandle, WorkerStatus},
    rust_service::prepare_service,
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

    pub async fn ensure_service_ready(&self, service_dir: &Path) -> Result<WorkerHandle> {
        let canonical_service_key = service_dir.canonicalize()?.display().to_string();

        let existing_worker_id = {
            let service_to_worker = self.service_to_worker.read();
            service_to_worker.get(&canonical_service_key).cloned()
        };

        if let Some(existing_id) = existing_worker_id {
            let worker = {
                let workers = self.workers.read();
                workers.get(&existing_id).cloned()
            };

            if let Some(worker) = worker {
                let is_alive = worker.is_alive().await;
                if is_alive {
                    info!(
                        worker_id = existing_id,
                        service = worker.service_name,
                        "reusing existing worker"
                    );
                    return Ok(WorkerHandle {
                        worker_id: existing_id.clone(),
                        service_name: worker.service_name.clone(),
                        url: service_url(&worker.service_name, &existing_id),
                    });
                }

                warn!(
                    worker_id = existing_id,
                    service = worker.service_name,
                    "worker record existed but process is gone, recreating"
                );
                self.workers.write().remove(&existing_id);
                self.service_to_worker
                    .write()
                    .retain(|_, worker_id| worker_id != &existing_id);
            }
        }

        let prepared = prepare_service(service_dir).await?;

        let worker_id = uuid::Uuid::new_v4().to_string();
        let worker = WorkerProcess::start(
            worker_id.clone(),
            prepared.service_name.clone(),
            &prepared.executable_path,
        )
        .await?;

        self.workers.write().insert(worker_id.clone(), worker);
        self.service_to_worker
            .write()
            .insert(canonical_service_key, worker_id.clone());

        Ok(WorkerHandle {
            worker_id: worker_id.clone(),
            service_name: prepared.service_name.clone(),
            url: service_url(&prepared.service_name, &worker_id),
        })
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

    pub async fn statuses(&self) -> Vec<WorkerStatus> {
        let active_worker_ids = self
            .service_to_worker
            .read()
            .values()
            .cloned()
            .collect::<HashSet<_>>();
        let workers = self
            .workers
            .read()
            .iter()
            .filter(|(id, _)| active_worker_ids.contains(*id))
            .map(|(id, worker)| (id.clone(), worker.clone()))
            .collect::<Vec<_>>();

        let mut statuses = Vec::with_capacity(workers.len());
        let mut stale = Vec::new();
        for (worker_id, worker) in workers {
            let alive = worker.is_alive().await;
            if !alive {
                stale.push(worker_id.clone());
                continue;
            }
            statuses.push(WorkerStatus {
                worker_id: worker_id.clone(),
                service_name: worker.service_name.clone(),
                url: service_url(&worker.service_name, &worker_id),
                alive,
                pid: worker.pid().await,
                executable_path: worker.executable_path.display().to_string(),
            });
        }
        if !stale.is_empty() {
            let mut workers = self.workers.write();
            let mut service_to_worker = self.service_to_worker.write();
            for worker_id in stale {
                workers.remove(&worker_id);
                service_to_worker.retain(|_, mapped| mapped != &worker_id);
            }
        }
        statuses
    }
}

fn service_url(service_name: &str, worker_id: &str) -> String {
    format!("/services/{service_name}/{worker_id}")
}
