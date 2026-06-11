use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallContext {
    pub request_id: String,
    pub method: String,
    pub path: String,
    pub input: Value,
}

impl CallContext {
    pub fn new(method: String, path: String, input: Value) -> Self {
        Self {
            request_id: uuid::Uuid::new_v4().to_string(),
            method,
            path,
            input,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerEnvelope {
    pub kind: String,
    pub payload: Value,
}

#[derive(Debug, Clone)]
pub struct WorkerHandle {
    pub worker_id: String,
}

/// Declarative worker description: executable, startup arguments, and env.
/// Workers are reused by key and can be replaced after liveness checks fail.
#[derive(Debug, Clone)]
pub struct WorkerSpec {
    /// Reuse key, usually at session or agent scope.
    pub key: String,
    /// Logical name used in status output and URLs.
    pub service_name: String,
    pub executable: std::path::PathBuf,
    pub args: Vec<String>,
    pub env: Vec<(String, String)>,
}
