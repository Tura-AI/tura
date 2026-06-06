//! Durable checkpoint model owned by the session DB service.
//!
//! Runtime must ACK mutating command checkpoints through SessionDbClient before
//! continuing execution.

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckpointType {
    TurnStarted,
    ProviderCallStarted,
    CommandRunStarted,
    CommandReady,
    CommandStarted,
    CommandFinished,
    CommandFailed,
    CommandRunFinished,
    ProviderCallFinished,
    TurnFinished,
    TurnFailed,
    TurnInterrupted,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandCheckpoint {
    pub session_id: String,
    pub turn_id: String,
    pub runtime_worker_id: Option<String>,
    pub provider_call_id: Option<String>,
    pub command_run_id: Option<String>,
    pub command_id: Option<String>,
    #[serde(default)]
    pub event_seq: Option<i64>,
    pub command_type: Option<String>,
    pub command_line: Option<String>,
    pub status: String,
    pub output_summary: Option<String>,
    pub changes: Value,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
}

impl CommandCheckpoint {
    pub fn idempotency_key(&self) -> String {
        format!(
            "{}:{}:{}:{}:{}:{}:{}",
            self.session_id,
            self.turn_id,
            self.runtime_worker_id.as_deref().unwrap_or("runtime"),
            self.command_run_id.as_deref().unwrap_or("command_run"),
            self.command_id.as_deref().unwrap_or("none"),
            self.event_seq
                .map(|value| value.to_string())
                .unwrap_or_else(|| "none".to_string()),
            self.status
        )
    }
}
