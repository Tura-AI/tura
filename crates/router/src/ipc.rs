//! Typed JSONL IPC shared by gateway, router, and router-owned services.
//!
//! The router IPC surface is restricted to supervision and execution methods.
//! Normal session DB read/write payloads are intentionally outside this module.

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
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

pub type IpcNotificationSender = tokio::sync::mpsc::UnboundedSender<IpcNotification>;
