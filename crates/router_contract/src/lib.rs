#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

pub const IPC_KIND_CALL: &str = "call";
pub const IPC_KIND_HEALTH_CHECK: &str = "health_check";
pub const METHOD_HEALTH_CHECK: &str = "health_check";
pub const METHOD_ENQUEUE_TURN: &str = "execution.enqueue_turn";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RouterEndpoint {
    pub addr: String,
    pub version: String,
    pub pid: Option<u32>,
    pub process_start_time: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IpcRequest {
    pub request_id: String,
    pub kind: String,
    #[serde(default)]
    pub method: String,
    #[serde(default)]
    pub payload: Value,
    #[serde(default)]
    pub deadline_ms: Option<u64>,
}

impl IpcRequest {
    pub fn call(request_id: impl Into<String>, method: impl Into<String>, payload: Value) -> Self {
        Self {
            request_id: request_id.into(),
            kind: IPC_KIND_CALL.to_string(),
            method: method.into(),
            payload,
            deadline_ms: None,
        }
    }

    pub fn health_check(request_id: impl Into<String>, deadline_ms: u64) -> Self {
        Self {
            request_id: request_id.into(),
            kind: IPC_KIND_HEALTH_CHECK.to_string(),
            method: METHOD_HEALTH_CHECK.to_string(),
            payload: Value::Object(Default::default()),
            deadline_ms: Some(deadline_ms),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IpcResponse {
    pub request_id: String,
    pub ok: bool,
    #[serde(default)]
    pub payload: Value,
    #[serde(default)]
    pub error: Option<String>,
}

impl IpcResponse {
    pub fn ok(request_id: impl Into<String>, payload: Value) -> Self {
        Self {
            request_id: request_id.into(),
            ok: true,
            payload,
            error: None,
        }
    }

    pub fn error(request_id: impl Into<String>, error: impl Into<String>) -> Self {
        Self {
            request_id: request_id.into(),
            ok: false,
            payload: Value::Null,
            error: Some(error.into()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IpcNotification {
    pub request_id: String,
    pub kind: String,
    #[serde(default)]
    pub method: String,
    #[serde(default)]
    pub payload: Value,
}

impl IpcNotification {
    pub fn new(
        request_id: impl Into<String>,
        kind: impl Into<String>,
        method: impl Into<String>,
        payload: Value,
    ) -> Self {
        Self {
            request_id: request_id.into(),
            kind: kind.into(),
            method: method.into(),
            payload,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct RunAgentRequest {
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default)]
    pub directory: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub agent: Option<String>,
    #[serde(default)]
    pub session_type: Option<String>,
    #[serde(default)]
    pub prompt: Option<String>,
    #[serde(default)]
    pub message: Option<String>,
    #[serde(default)]
    pub input: Option<Value>,
    #[serde(default)]
    pub parent_session_id: Option<String>,
    #[serde(default)]
    pub depth: Option<usize>,
    #[serde(default)]
    pub runtime_context: Option<String>,
    #[serde(default)]
    pub planning_mode_override: Option<bool>,
    #[serde(default)]
    pub return_log: bool,
    #[serde(default)]
    pub worker_env: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EnqueueTurnRequest {
    pub turn_id: String,
    pub session_id: String,
    pub payload: Value,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProbeSessionsRequest {
    #[serde(default)]
    pub session_ids: Vec<String>,
}
