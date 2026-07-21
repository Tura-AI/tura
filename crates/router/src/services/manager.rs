use std::{collections::HashMap, sync::Arc};

use anyhow::{anyhow, Result};
use parking_lot::RwLock;
use tokio::sync::Mutex;
use tracing::{info, warn};

use runtime_contract::CallContext;

use super::{
    models::{WorkerHandle, WorkerSpec},
    worker_process::WorkerProcess,
};

#[derive(Clone)]
pub struct ServiceManager {
    workers: Arc<RwLock<HashMap<String, Arc<WorkerProcess>>>>,
    service_to_worker: Arc<RwLock<HashMap<String, String>>>,
    ensure_locks: Arc<RwLock<HashMap<String, Arc<Mutex<()>>>>>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExistingWorkerPolicy {
    Reuse,
    Reject,
}

#[derive(Debug)]
pub struct WorkerAlreadyRunning {
    pub key: String,
    pub worker_id: String,
}

impl std::fmt::Display for WorkerAlreadyRunning {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "worker key {} is already running as {}",
            self.key, self.worker_id
        )
    }
}

impl std::error::Error for WorkerAlreadyRunning {}

impl ServiceManager {
    pub fn new() -> Self {
        Self {
            workers: Arc::new(RwLock::new(HashMap::new())),
            service_to_worker: Arc::new(RwLock::new(HashMap::new())),
            ensure_locks: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Ensure a worker from a declarative spec, reusing by `spec.key` while the
    /// process is healthy and replacing it after liveness checks fail.
    #[allow(dead_code)]
    pub async fn ensure_worker(&self, spec: WorkerSpec) -> Result<WorkerHandle> {
        self.ensure_worker_inner(spec, ExistingWorkerPolicy::Reuse)
            .await
    }

    /// Start a worker only if no live worker is already registered for `spec.key`.
    pub async fn ensure_exclusive_worker(&self, spec: WorkerSpec) -> Result<WorkerHandle> {
        self.ensure_worker_inner(spec, ExistingWorkerPolicy::Reject)
            .await
    }

    async fn ensure_worker_inner(
        &self,
        spec: WorkerSpec,
        existing_policy: ExistingWorkerPolicy,
    ) -> Result<WorkerHandle> {
        let ensure_lock = self.ensure_lock_for_key(&spec.key);
        let _guard = ensure_lock.lock().await;
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
                    if existing_policy == ExistingWorkerPolicy::Reject {
                        return Err(WorkerAlreadyRunning {
                            key: spec.key,
                            worker_id: existing_id,
                        }
                        .into());
                    }
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

    fn ensure_lock_for_key(&self, key: &str) -> Arc<Mutex<()>> {
        if let Some(lock) = self.ensure_locks.read().get(key).cloned() {
            return lock;
        }
        let mut locks = self.ensure_locks.write();
        Arc::clone(
            locks
                .entry(key.to_string())
                .or_insert_with(|| Arc::new(Mutex::new(()))),
        )
    }

    /// Count active workers under a key prefix for concurrency limits.
    pub fn count_workers_with_prefix(&self, key_prefix: &str) -> usize {
        self.service_to_worker
            .read()
            .keys()
            .filter(|key| key.starts_with(key_prefix))
            .count()
    }

    pub async fn worker_alive_by_key(&self, key: &str) -> bool {
        let worker_id = {
            let service_to_worker = self.service_to_worker.read();
            service_to_worker.get(key).cloned()
        };
        let Some(worker_id) = worker_id else {
            return false;
        };
        let worker = {
            let workers = self.workers.read();
            workers.get(&worker_id).cloned()
        };
        let Some(worker) = worker else {
            self.service_to_worker.write().remove(key);
            return false;
        };
        if worker.is_alive().await {
            return true;
        }
        self.workers.write().remove(&worker_id);
        self.service_to_worker.write().remove(key);
        false
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

        let result = worker.invoke(ctx).await;
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
    use super::super::models::WorkerSpec;
    use super::ServiceManager;
    use runtime_contract::CallContext;
    use serde_json::json;
    use std::path::PathBuf;

    fn missing_worker_spec(key: &str) -> WorkerSpec {
        WorkerSpec {
            key: key.to_string(),
            service_name: "runtime".to_string(),
            executable: PathBuf::from("definitely-missing-runtime-worker-for-manager-test"),
            args: vec!["--serve".to_string()],
            env: vec![("TURA_WORKER_MODE".to_string(), "one-shot".to_string())],
        }
    }

    #[tokio::test]
    async fn ensure_worker_reuses_alive_worker_for_same_key() {
        let manager = ServiceManager::new();
        let first = manager
            .ensure_worker(missing_worker_spec("runtime_worker:session-a"))
            .await
            .expect("explicit one-shot worker should create a reusable handle");
        let second = manager
            .ensure_worker(missing_worker_spec("runtime_worker:session-a"))
            .await
            .expect("same key should reuse existing one-shot worker");

        assert_eq!(first.worker_id, second.worker_id);
        assert_eq!(manager.count_workers_with_prefix("runtime_worker:"), 1);
    }

    #[tokio::test]
    async fn ensure_exclusive_worker_rejects_alive_worker_for_same_key() {
        let manager = ServiceManager::new();
        let first = manager
            .ensure_exclusive_worker(missing_worker_spec("runtime_worker:session-exclusive"))
            .await
            .expect("first worker should be registered");

        let error = manager
            .ensure_exclusive_worker(missing_worker_spec("runtime_worker:session-exclusive"))
            .await
            .expect_err("same key should reject while the first worker is alive");
        let duplicate = error
            .downcast_ref::<super::WorkerAlreadyRunning>()
            .expect("exclusive worker rejection should use a structured error");

        assert_eq!(duplicate.key, "runtime_worker:session-exclusive");
        assert_eq!(duplicate.worker_id, first.worker_id);
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
    async fn worker_alive_by_key_reports_registered_one_shot_worker() {
        let manager = ServiceManager::new();
        manager
            .ensure_worker(missing_worker_spec("runtime_worker:session-live"))
            .await
            .expect("one-shot worker should be registered");

        assert!(
            manager
                .worker_alive_by_key("runtime_worker:session-live")
                .await
        );
        assert!(!manager.worker_alive_by_key("runtime_worker:missing").await);
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
