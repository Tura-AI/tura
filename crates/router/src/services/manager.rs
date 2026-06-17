use std::{collections::HashMap, sync::Arc};

use anyhow::{anyhow, Result};
use parking_lot::RwLock;
use tokio::sync::Mutex;
use tracing::{info, warn};

use crate::ipc;

use super::{
    models::{CallContext, WorkerHandle, WorkerSpec},
    worker_process::WorkerProcess,
};

#[derive(Clone)]
pub struct ServiceManager {
    workers: Arc<RwLock<HashMap<String, Arc<WorkerProcess>>>>,
    service_to_worker: Arc<RwLock<HashMap<String, String>>>,
    ensure_lock: Arc<Mutex<()>>,
}

impl ServiceManager {
    pub fn new() -> Self {
        Self {
            workers: Arc::new(RwLock::new(HashMap::new())),
            service_to_worker: Arc::new(RwLock::new(HashMap::new())),
            ensure_lock: Arc::new(Mutex::new(())),
        }
    }

    /// Ensure a worker from a declarative spec, reusing by `spec.key` while the
    /// process is healthy and replacing it after liveness checks fail.
    pub async fn ensure_worker(&self, spec: WorkerSpec) -> Result<WorkerHandle> {
        let _guard = self.ensure_lock.lock().await;
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

    /// Count active workers under a key prefix for concurrency limits.
    pub fn count_workers_with_prefix(&self, key_prefix: &str) -> usize {
        self.service_to_worker
            .read()
            .keys()
            .filter(|key| key.starts_with(key_prefix))
            .count()
    }

    #[allow(dead_code)]
    pub async fn call_worker(
        &self,
        worker_id: &str,
        ctx: CallContext,
    ) -> Result<serde_json::Value> {
        self.call_worker_with_notifications(worker_id, ctx, None)
            .await
    }

    pub async fn call_worker_with_notifications(
        &self,
        worker_id: &str,
        ctx: CallContext,
        notifications: Option<ipc::IpcNotificationSender>,
    ) -> Result<serde_json::Value> {
        let worker = {
            let workers = self.workers.read();
            workers
                .get(worker_id)
                .cloned()
                .ok_or_else(|| anyhow!("worker not found: {worker_id}"))?
        };

        let result = worker.invoke_with_notifications(ctx, notifications).await;
        if result.is_err() && !worker.is_alive().await {
            self.workers.write().remove(worker_id);
            self.service_to_worker
                .write()
                .retain(|_, mapped_worker_id| mapped_worker_id != worker_id);
            warn!(
                worker_id,
                "removed unresponsive worker after failed invocation"
            );
        }
        result
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

impl Default for ServiceManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::super::models::{CallContext, WorkerSpec};
    use super::ServiceManager;
    use serde_json::json;
    use std::path::PathBuf;

    fn missing_worker_spec(key: &str) -> WorkerSpec {
        WorkerSpec {
            key: key.to_string(),
            service_name: "runtime".to_string(),
            executable: PathBuf::from("definitely-missing-runtime-worker-for-manager-test"),
            args: vec!["--serve".to_string()],
            env: vec![("TURA_DEBUG_RUNTIME".to_string(), "0".to_string())],
        }
    }

    #[tokio::test]
    async fn ensure_worker_reuses_alive_worker_for_same_key() {
        let manager = ServiceManager::new();
        let first = manager
            .ensure_worker(missing_worker_spec("runtime_worker:session-a"))
            .await
            .expect("missing executable should still create one-shot worker handle");
        let second = manager
            .ensure_worker(missing_worker_spec("runtime_worker:session-a"))
            .await
            .expect("same key should reuse existing one-shot worker");

        assert_eq!(first.worker_id, second.worker_id);
        assert_eq!(manager.count_workers_with_prefix("runtime_worker:"), 1);
    }

    #[tokio::test]
    async fn stop_worker_by_key_removes_worker_and_prefix_count() {
        let manager = ServiceManager::new();
        let handle = manager
            .ensure_worker(missing_worker_spec("runtime_worker:session-b"))
            .await
            .expect("worker should be registered");

        assert!(manager.stop_worker_by_key("runtime_worker:session-b").await);
        assert_eq!(manager.count_workers_with_prefix("runtime_worker:"), 0);
        assert!(!manager.stop_worker(&handle.worker_id).await);
    }

    #[tokio::test]
    async fn stop_workers_with_prefix_removes_only_matching_workers() {
        let manager = ServiceManager::new();
        manager
            .ensure_worker(missing_worker_spec("runtime_worker:one"))
            .await
            .expect("first worker should be registered");
        manager
            .ensure_worker(missing_worker_spec("runtime_worker:two"))
            .await
            .expect("second worker should be registered");
        manager
            .ensure_worker(missing_worker_spec("other_worker:three"))
            .await
            .expect("third worker should be registered");

        assert_eq!(manager.stop_workers_with_prefix("runtime_worker:").await, 2);
        assert_eq!(manager.count_workers_with_prefix("runtime_worker:"), 0);
        assert_eq!(manager.count_workers_with_prefix("other_worker:"), 1);
    }

    #[tokio::test]
    async fn call_worker_reports_missing_worker_id_with_context() {
        let manager = ServiceManager::new();
        let error = manager
            .call_worker(
                "missing-worker",
                CallContext {
                    request_id: "request-missing".to_string(),
                    method: "run".to_string(),
                    path: "/runtime".to_string(),
                    input: json!({}),
                },
            )
            .await
            .expect_err("missing worker id should fail");

        assert!(
            error
                .to_string()
                .contains("worker not found: missing-worker"),
            "missing worker error should include the id: {error}"
        );
    }
}
