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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerResponse {
    pub ok: bool,
    pub worker_id: String,
    pub service_name: String,
    pub output: Value,
    pub stderr: String,
    pub exit_code: i32,
    pub mode: String,
}

#[derive(Debug, Clone)]
pub struct WorkerHandle {
    pub worker_id: String,
    pub service_name: String,
    pub url: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct WorkerStatus {
    pub worker_id: String,
    pub service_name: String,
    pub url: String,
    pub alive: bool,
    pub pid: Option<u32>,
    pub executable_path: String,
}
