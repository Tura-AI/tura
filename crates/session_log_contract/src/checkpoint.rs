//! Durable checkpoint model owned by the session DB service.
//!
//! Runtime must ACK mutating command checkpoints through SessionDbClient before
//! continuing execution.

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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

impl CheckpointType {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::TurnStarted => "turn_started",
            Self::ProviderCallStarted => "provider_call_started",
            Self::CommandRunStarted => "command_run_started",
            Self::CommandReady => "command_ready",
            Self::CommandStarted => "command_started",
            Self::CommandFinished => "command_finished",
            Self::CommandFailed => "command_failed",
            Self::CommandRunFinished => "command_run_finished",
            Self::ProviderCallFinished => "provider_call_finished",
            Self::TurnFinished => "turn_finished",
            Self::TurnFailed => "turn_failed",
            Self::TurnInterrupted => "turn_interrupted",
        }
    }
}

impl std::fmt::Display for CheckpointType {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandCheckpoint {
    pub session_id: String,
    pub runtime_id: String,
    pub runtime_worker_id: Option<String>,
    pub provider_call_id: Option<String>,
    pub command_run_id: Option<String>,
    pub command_id: Option<String>,
    #[serde(default)]
    pub event_seq: Option<i64>,
    pub command_type: Option<String>,
    pub command_line: Option<String>,
    pub checkpoint_type: CheckpointType,
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
            self.runtime_id,
            self.runtime_worker_id.as_deref().unwrap_or("runtime"),
            self.command_run_id.as_deref().unwrap_or("command_run"),
            self.command_id.as_deref().unwrap_or("none"),
            self.event_seq
                .map(|value| value.to_string())
                .unwrap_or_else(|| "none".to_string()),
            self.checkpoint_type.as_str()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::{CheckpointType, CommandCheckpoint};
    use serde_json::json;

    fn checkpoint() -> CommandCheckpoint {
        CommandCheckpoint {
            session_id: "session-1".to_string(),
            runtime_id: "runtime-1".to_string(),
            runtime_worker_id: Some("worker-1".to_string()),
            provider_call_id: Some("provider-1".to_string()),
            command_run_id: Some("run-1".to_string()),
            command_id: Some("cmd-1".to_string()),
            event_seq: Some(7),
            command_type: Some("shell_command".to_string()),
            command_line: Some("echo ok".to_string()),
            checkpoint_type: CheckpointType::CommandFinished,
            output_summary: Some("ok".to_string()),
            changes: json!({ "files": [] }),
            started_at: Some("2026-06-11T00:00:00Z".to_string()),
            finished_at: Some("2026-06-11T00:00:01Z".to_string()),
        }
    }

    #[test]
    fn checkpoint_type_serde_uses_snake_case_contract() {
        let cases = [
            (CheckpointType::TurnStarted, "\"turn_started\""),
            (
                CheckpointType::ProviderCallStarted,
                "\"provider_call_started\"",
            ),
            (CheckpointType::CommandRunStarted, "\"command_run_started\""),
            (CheckpointType::CommandReady, "\"command_ready\""),
            (CheckpointType::CommandStarted, "\"command_started\""),
            (CheckpointType::CommandFinished, "\"command_finished\""),
            (CheckpointType::CommandFailed, "\"command_failed\""),
            (
                CheckpointType::CommandRunFinished,
                "\"command_run_finished\"",
            ),
            (
                CheckpointType::ProviderCallFinished,
                "\"provider_call_finished\"",
            ),
            (CheckpointType::TurnFinished, "\"turn_finished\""),
            (CheckpointType::TurnFailed, "\"turn_failed\""),
            (CheckpointType::TurnInterrupted, "\"turn_interrupted\""),
        ];

        for (kind, text) in cases {
            assert_eq!(serde_json::to_string(&kind).expect("serialize"), text);
            assert!(serde_json::from_str::<CheckpointType>(text).is_ok());
        }
        assert!(serde_json::from_str::<CheckpointType>("\"CommandFinished\"").is_err());
        assert!(serde_json::from_str::<CheckpointType>("\"command-finished\"").is_err());
    }

    #[test]
    fn idempotency_key_contains_stable_execution_identity() {
        let checkpoint = checkpoint();

        assert_eq!(
            checkpoint.idempotency_key(),
            "session-1:runtime-1:worker-1:run-1:cmd-1:7:command_finished"
        );
    }

    #[test]
    fn idempotency_key_uses_explicit_none_placeholders() {
        let mut checkpoint = checkpoint();
        checkpoint.runtime_worker_id = None;
        checkpoint.command_run_id = None;
        checkpoint.command_id = None;
        checkpoint.event_seq = None;
        checkpoint.checkpoint_type = CheckpointType::TurnInterrupted;

        assert_eq!(
            checkpoint.idempotency_key(),
            "session-1:runtime-1:runtime:command_run:none:none:turn_interrupted"
        );
    }

    #[test]
    fn command_checkpoint_round_trips_without_losing_changes_or_timestamps() {
        let checkpoint = checkpoint();
        let encoded = serde_json::to_string(&checkpoint).expect("encode checkpoint");
        let decoded: CommandCheckpoint = serde_json::from_str(&encoded).expect("decode checkpoint");

        assert_eq!(decoded.idempotency_key(), checkpoint.idempotency_key());
        assert_eq!(decoded.provider_call_id.as_deref(), Some("provider-1"));
        assert_eq!(decoded.command_type.as_deref(), Some("shell_command"));
        assert_eq!(decoded.changes, json!({ "files": [] }));
        assert_eq!(decoded.finished_at.as_deref(), Some("2026-06-11T00:00:01Z"));
    }
}
