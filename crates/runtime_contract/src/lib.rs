#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const WORKER_KIND_CALL: &str = "call";
pub const WORKER_KIND_HEALTH_CHECK: &str = "health_check";
pub const GATEWAY_CALLBACK_KIND: &str = "gateway.callback";
pub const GATEWAY_AGENT_MESSAGE_METHOD: &str = "session.agent_message";
pub const GATEWAY_AGENT_STREAM_METHOD: &str = "session.agent_stream";

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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GatewayCallbackPayload {
    pub session_id: String,
    pub body: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GatewayCallbackFrame {
    pub kind: String,
    pub method: String,
    pub payload: GatewayCallbackPayload,
}

impl GatewayCallbackFrame {
    pub fn new(method: impl Into<String>, session_id: impl Into<String>, body: Value) -> Self {
        Self {
            kind: GATEWAY_CALLBACK_KIND.to_string(),
            method: method.into(),
            payload: GatewayCallbackPayload {
                session_id: session_id.into(),
                body,
            },
        }
    }
}
