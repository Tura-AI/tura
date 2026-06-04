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

/// 声明式 worker 描述：可执行 + 启动参数 + env 契约。
/// 不绑定任何具体服务；以「key→worker」复用、探活、自愈。
#[derive(Debug, Clone)]
pub struct WorkerSpec {
    /// 复用维度的 key（session/agent 维度，非服务目录）。
    pub key: String,
    /// 用于 url/状态展示的逻辑名。
    pub service_name: String,
    pub executable: std::path::PathBuf,
    pub args: Vec<String>,
    pub env: Vec<(String, String)>,
}
