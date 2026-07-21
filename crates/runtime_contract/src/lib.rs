#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

use lifecycle::SessionState;

pub const WORKER_KIND_CALL: &str = "call";
pub const WORKER_KIND_HEALTH_CHECK: &str = "health_check";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorkerEnvelope {
    pub kind: String,
    #[serde(default)]
    pub payload: Value,
}

impl WorkerEnvelope {
    pub fn health_check() -> Self {
        Self {
            kind: WORKER_KIND_HEALTH_CHECK.to_string(),
            payload: Value::Object(Default::default()),
        }
    }

    pub fn call(context: CallContext) -> Self {
        Self {
            kind: WORKER_KIND_CALL.to_string(),
            payload: serde_json::json!({ "input": context }),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct RunAgentRequest {
    pub runtime_id: String,
    pub lease_id: String,
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
    pub no_op_manual: bool,
    #[serde(default)]
    pub return_log: bool,
    #[serde(default)]
    pub worker_env: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct RuntimeWorkerResponse {
    pub ok: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_state: Option<SessionState>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message_count: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turn_started_at_ms: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub final_text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub session_log: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage: Option<Value>,
}
