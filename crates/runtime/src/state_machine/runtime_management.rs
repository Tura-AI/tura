use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::agent_management::{AgentId, ProviderConfig};
use super::session_management::{ContextTokenStats, SessionId};

/// UTC timestamp with millisecond precision.
pub type UtcDateTimeMs = DateTime<Utc>;

/// Runtime-scoped hexadecimal identifier.
pub type RuntimeId = String;

/// Free-form reasoning text.
pub type ReasoningText = String;

/// Hash of the reasoning text or reasoning artifact.
pub type ReasoningHash = String;

/// Assistant text output for this runtime call.
pub type OutputText = String;

/// Report attached to one individual tool call.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolCallRecord {
    /// Name of the tool that was called.
    pub tool_called_name: String,
    /// JSON payload passed to the tool.
    pub tool_called_input: serde_json::Value,
    /// Provider-specific metadata required to replay tool-call history.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_metadata: Option<serde_json::Value>,
    /// Time the full tool call was received.
    pub tool_received_at: UtcDateTimeMs,
    /// Time execution of the tool call started.
    pub tool_executed_at: UtcDateTimeMs,
    /// Time the tool calldata was received.
    pub tool_calldata_received_at: UtcDateTimeMs,
    /// Whether the tool itself returned success.
    pub tool_reported_success: bool,
    /// Whether the agent believes the whole local tool execution succeeded.
    pub agent_reported_success: bool,
    /// Whether the agent believes the tool result is helpful.
    pub agent_reported_helpful: bool,
    /// Agent-side summary of the tool execution result.
    pub agent_reported_summary: String,
    /// Validator result for the full subtask.
    pub validator_reported_success: Option<bool>,
}

/// Rich provider information captured on each runtime call.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RuntimeProviderConfig {
    /// Shared provider config inherited from the agent.
    #[serde(flatten)]
    pub base: ProviderConfig,
    /// Whether hidden reasoning/thinking was enabled.
    pub thinking: bool,
    /// Provider name.
    pub provider_name: String,
    /// Exact model name.
    pub model_name: String,
    /// Provider URL alias/name.
    pub provider_url_name: String,
    /// Underlying LLM provider name, such as openai, google, or anthropic.
    pub llm_provider_name: String,
}

/// Error object returned by the model/provider.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeError {
    /// Provider/model error code.
    pub error_code: Option<String>,
    /// Human-readable failure message.
    pub error_text: Option<String>,
    /// Whether retry is allowed.
    pub retry_allowed: bool,
    /// Whether fallback is allowed.
    pub fallback_allowed: bool,
    /// Runtime identifier that this call may fall back to.
    pub fallback_to_id: Option<RuntimeId>,
}

/// Token usage and cost report.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UsageReport {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub total_tokens: u64,
    pub cached_input_tokens: u64,
    pub cache_write_tokens: u64,
    pub reasoning_tokens: u64,
    pub attachment_input_tokens: u64,
    pub input_cost: f64,
    pub output_cost: f64,
    pub total_cost: f64,
    pub currency: String,
    pub pricing_source: String,
    pub latency_ms: u64,
    pub time_to_first_token_ms: u64,
    pub token_per_second: f64,
}

/// Final result of a runtime call.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RuntimeCallResultStatus {
    Pending,
    Streaming,
    Succeeded,
    Failed,
    TimedOut,
    Cancelled,
}

/// Runtime state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RuntimeState {
    Created,
    Dispatching,
    WaitingFirstToken,
    Streaming,
    Finished,
    Failed,
}

impl RuntimeState {
    /// Returns true if transitioning from `self` to `next` is allowed.
    pub fn can_transition_to(self, next: RuntimeState) -> bool {
        use RuntimeState::*;

        match (self, next) {
            (Created, Dispatching | Failed) => true,
            (Dispatching, WaitingFirstToken | Failed) => true,
            (WaitingFirstToken, Streaming | Finished | Failed) => true,
            (Streaming, Finished | Failed) => true,
            (Finished | Failed, _) => false,
            _ if self == next => true,
            _ => false,
        }
    }
}

/// Runtime-owned session sync status for one provider call.
///
/// Gateway uses this payload to decide whether a callback belongs in the live
/// overlay or should trigger one session DB refresh. The decision stays tied to
/// the runtime state machine instead of individual tool/message status fields.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeSessionSyncStatus {
    pub runtime_id: RuntimeId,
    pub state: RuntimeState,
    pub call_result_status: RuntimeCallResultStatus,
    pub live: bool,
    pub session_db_refresh_required: bool,
}

impl RuntimeSessionSyncStatus {
    pub fn live_overlay_active(&self) -> bool {
        self.live && !self.session_db_refresh_required
    }

    pub fn should_refresh_session_db(&self) -> bool {
        self.session_db_refresh_required || !self.live
    }
}

/// Full runtime record for one LLM call.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RuntimeManagement {
    /// Runtime identifier.
    pub runtime_id: RuntimeId,
    /// Runtime creation timestamp.
    pub created_at: UtcDateTimeMs,
    /// Time the call started consuming provider resources.
    pub called_at: Option<UtcDateTimeMs>,
    /// Time the first token was received.
    pub first_token_at: Option<UtcDateTimeMs>,
    /// Time the full callback finished.
    pub call_finished_at: Option<UtcDateTimeMs>,
    /// Final provider callback result status.
    pub call_result_status: RuntimeCallResultStatus,
    /// If this runtime is a fallback, reference the failed runtime identifier.
    pub fallback_from_id: Option<RuntimeId>,
    /// Direct session identifier.
    pub session_id: SessionId,
    /// Direct agent identifier.
    pub agent_id: AgentId,
    /// Provider configuration captured at runtime.
    pub provider: RuntimeProviderConfig,
    /// Optional provider/model error details.
    pub error: Option<RuntimeError>,
    /// Hidden reasoning text or summary.
    pub reasoning: Option<ReasoningText>,
    /// Hash for the reasoning field.
    pub reasoning_hash: Option<ReasoningHash>,
    /// Full request payload sent to the provider for this runtime call.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input: Option<serde_json::Value>,
    /// Full provider response payload received for this runtime call.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output: Option<serde_json::Value>,
    /// Assistant text output.
    pub text: OutputText,
    /// Tool call reports.
    pub tool_call: Vec<ToolCallRecord>,
    /// Latest provider-reported input token count for this runtime.
    #[serde(default)]
    pub context_tokens: ContextTokenStats,
    /// Usage and billing report.
    pub usage: Option<UsageReport>,
    /// Current runtime state.
    pub state: RuntimeState,
}

impl RuntimeManagement {
    /// Creates a new runtime record in `Created` state.
    pub fn new(
        runtime_id: RuntimeId,
        session_id: SessionId,
        agent_id: AgentId,
        provider: RuntimeProviderConfig,
        created_at: UtcDateTimeMs,
    ) -> Self {
        Self {
            runtime_id,
            created_at,
            called_at: None,
            first_token_at: None,
            call_finished_at: None,
            call_result_status: RuntimeCallResultStatus::Pending,
            fallback_from_id: None,
            session_id,
            agent_id,
            provider,
            error: None,
            reasoning: None,
            reasoning_hash: None,
            input: None,
            output: None,
            text: String::new(),
            tool_call: Vec::new(),
            context_tokens: ContextTokenStats::default(),
            usage: None,
            state: RuntimeState::Created,
        }
    }

    /// Applies a validated runtime state transition.
    pub fn transition(&mut self, next: RuntimeState) -> Result<(), String> {
        if !self.state.can_transition_to(next) {
            return Err(format!(
                "invalid runtime state transition: {:?} -> {:?}",
                self.state, next
            ));
        }

        self.state = next;
        Ok(())
    }

    /// Marks the runtime as dispatched to the provider.
    pub fn mark_called(&mut self, called_at: UtcDateTimeMs) -> Result<(), String> {
        self.transition(RuntimeState::Dispatching)?;
        self.called_at = Some(called_at);
        Ok(())
    }

    /// Marks that the request is now waiting for the first token.
    pub fn mark_waiting_first_token(&mut self) -> Result<(), String> {
        self.transition(RuntimeState::WaitingFirstToken)
    }

    /// Marks the runtime as streaming and records first-token time.
    pub fn mark_first_token(&mut self, first_token_at: UtcDateTimeMs) -> Result<(), String> {
        self.transition(RuntimeState::Streaming)?;
        self.first_token_at = Some(first_token_at);
        self.call_result_status = RuntimeCallResultStatus::Streaming;
        Ok(())
    }

    /// Appends model text while the call is streaming.
    pub fn append_text(&mut self, chunk: impl AsRef<str>) {
        self.text.push_str(chunk.as_ref());
    }

    /// Stores the exact request payload that was sent to the provider.
    pub fn set_input(&mut self, input: serde_json::Value) {
        self.input = Some(input);
    }

    /// Stores the full provider response payload.
    pub fn set_output(&mut self, output: serde_json::Value) {
        self.output = Some(output);
    }

    /// Adds one tool call record.
    pub fn push_tool_call(&mut self, record: ToolCallRecord) {
        self.tool_call.push(record);
    }

    /// True while gateway should keep callback payloads in the active live
    /// overlay for this runtime call.
    pub fn live_overlay_active(&self) -> bool {
        matches!(
            self.state,
            RuntimeState::Created
                | RuntimeState::Dispatching
                | RuntimeState::WaitingFirstToken
                | RuntimeState::Streaming
        ) && matches!(
            self.call_result_status,
            RuntimeCallResultStatus::Pending | RuntimeCallResultStatus::Streaming
        )
    }

    /// True once gateway should drop this runtime's live overlay and refresh
    /// the canonical session DB history.
    pub fn session_db_refresh_required(&self) -> bool {
        !self.live_overlay_active()
    }

    pub fn session_sync_status(&self) -> RuntimeSessionSyncStatus {
        RuntimeSessionSyncStatus {
            runtime_id: self.runtime_id.clone(),
            state: self.state,
            call_result_status: self.call_result_status,
            live: self.live_overlay_active(),
            session_db_refresh_required: self.session_db_refresh_required(),
        }
    }

    /// Runtime-owned assistant message timestamps shared by gateway callbacks
    /// and persisted session snapshots.
    pub fn assistant_message_timestamps(&self) -> (i64, i64) {
        let created_at = self
            .first_token_at
            .or(self.called_at)
            .unwrap_or(self.created_at);
        let updated_at = self.call_finished_at.unwrap_or(created_at);
        (created_at.timestamp_millis(), updated_at.timestamp_millis())
    }

    /// Marks the runtime as successful.
    pub fn finish_success(
        &mut self,
        finished_at: UtcDateTimeMs,
        usage: Option<UsageReport>,
    ) -> Result<(), String> {
        self.transition(RuntimeState::Finished)?;
        self.call_finished_at = Some(finished_at);
        self.call_result_status = RuntimeCallResultStatus::Succeeded;
        self.usage = usage;
        Ok(())
    }

    /// Marks the runtime as failed and stores the error payload.
    pub fn finish_failure(
        &mut self,
        finished_at: UtcDateTimeMs,
        error: RuntimeError,
        status: RuntimeCallResultStatus,
        usage: Option<UsageReport>,
    ) -> Result<(), String> {
        self.transition(RuntimeState::Failed)?;
        self.call_finished_at = Some(finished_at);
        self.call_result_status = status;
        self.error = Some(error);
        self.usage = usage;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{
        RuntimeCallResultStatus, RuntimeError, RuntimeManagement, RuntimeProviderConfig,
        RuntimeState, UsageReport,
    };
    use crate::state_machine::agent_management::{ProviderConfig, ToolChoice};
    use chrono::{Duration, Utc};

    fn provider_config() -> RuntimeProviderConfig {
        RuntimeProviderConfig {
            base: ProviderConfig {
                tura_llm_name: "fast".to_string(),
                default_model_tier: None,
                current_model: None,
                stream: true,
                temperature: 0.0,
                max_tokens: 1024,
                tool_choice: ToolChoice::Auto,
                time_out_ms: 30_000,
            },
            thinking: false,
            provider_name: "openai".to_string(),
            model_name: "gpt-test".to_string(),
            provider_url_name: "openai".to_string(),
            llm_provider_name: "openai".to_string(),
        }
    }

    fn runtime() -> RuntimeManagement {
        RuntimeManagement::new(
            "runtime-test".to_string(),
            "session-test".to_string(),
            "agent-test".to_string(),
            provider_config(),
            Utc::now(),
        )
    }

    #[test]
    fn runtime_state_transition_matrix_rejects_illegal_and_terminal_edges() {
        use RuntimeState::*;

        let states = [
            Created,
            Dispatching,
            WaitingFirstToken,
            Streaming,
            Finished,
            Failed,
        ];
        for from in states {
            for to in states {
                let expected = matches!(
                    (from, to),
                    (Created, Created | Dispatching | Failed)
                        | (Dispatching, Dispatching | WaitingFirstToken | Failed)
                        | (
                            WaitingFirstToken,
                            WaitingFirstToken | Streaming | Finished | Failed
                        )
                        | (Streaming, Streaming | Finished | Failed)
                );
                assert_eq!(
                    from.can_transition_to(to),
                    expected,
                    "unexpected RuntimeState transition verdict for {from:?} -> {to:?}"
                );
            }
        }
    }

    #[test]
    fn runtime_mark_methods_apply_ordered_state_and_timestamps() {
        let mut runtime = runtime();
        let called_at = runtime.created_at + Duration::milliseconds(10);
        let first_token_at = called_at + Duration::milliseconds(25);

        runtime
            .mark_called(called_at)
            .expect("mark_called should transition Created -> Dispatching");
        assert_eq!(runtime.state, RuntimeState::Dispatching);
        assert_eq!(runtime.called_at, Some(called_at));

        runtime
            .mark_waiting_first_token()
            .expect("mark_waiting_first_token should transition Dispatching -> WaitingFirstToken");
        assert_eq!(runtime.state, RuntimeState::WaitingFirstToken);

        runtime
            .mark_first_token(first_token_at)
            .expect("mark_first_token should transition WaitingFirstToken -> Streaming");
        assert_eq!(runtime.state, RuntimeState::Streaming);
        assert_eq!(runtime.first_token_at, Some(first_token_at));
        assert_eq!(
            runtime.call_result_status,
            RuntimeCallResultStatus::Streaming
        );
    }

    #[test]
    fn runtime_session_sync_status_is_derived_from_runtime_state_machine() {
        let mut runtime = runtime();
        let created_status = runtime.session_sync_status();
        assert!(created_status.live_overlay_active());
        assert!(!created_status.should_refresh_session_db());
        assert_eq!(created_status.state, RuntimeState::Created);

        let called_at = runtime.created_at + Duration::milliseconds(10);
        runtime.mark_called(called_at).expect("mark called");
        runtime
            .mark_waiting_first_token()
            .expect("mark waiting first token");
        let waiting_status = runtime.session_sync_status();
        assert!(waiting_status.live_overlay_active());
        assert!(!waiting_status.session_db_refresh_required);

        let first_token_at = called_at + Duration::milliseconds(25);
        runtime
            .mark_first_token(first_token_at)
            .expect("mark first token");
        let streaming_status = runtime.session_sync_status();
        assert!(streaming_status.live_overlay_active());
        assert_eq!(
            streaming_status.call_result_status,
            RuntimeCallResultStatus::Streaming
        );

        let finished_at = first_token_at + Duration::milliseconds(80);
        runtime
            .finish_success(finished_at, None)
            .expect("finish success");
        let finished_status = runtime.session_sync_status();
        assert!(!finished_status.live_overlay_active());
        assert!(finished_status.should_refresh_session_db());
        assert_eq!(finished_status.state, RuntimeState::Finished);
        assert_eq!(
            finished_status.call_result_status,
            RuntimeCallResultStatus::Succeeded
        );
    }

    #[test]
    fn assistant_message_timestamps_are_runtime_owned() {
        let mut runtime = runtime();
        let called_at = runtime.created_at + Duration::milliseconds(10);
        let first_token_at = called_at + Duration::milliseconds(20);
        let finished_at = first_token_at + Duration::milliseconds(30);

        assert_eq!(
            runtime.assistant_message_timestamps(),
            (
                runtime.created_at.timestamp_millis(),
                runtime.created_at.timestamp_millis()
            )
        );

        runtime.mark_called(called_at).expect("mark called");
        assert_eq!(
            runtime.assistant_message_timestamps(),
            (called_at.timestamp_millis(), called_at.timestamp_millis())
        );

        runtime
            .mark_waiting_first_token()
            .expect("mark waiting first token");
        runtime
            .mark_first_token(first_token_at)
            .expect("mark first token");
        assert_eq!(
            runtime.assistant_message_timestamps(),
            (
                first_token_at.timestamp_millis(),
                first_token_at.timestamp_millis()
            )
        );

        runtime
            .finish_success(finished_at, None)
            .expect("finish success");
        assert_eq!(
            runtime.assistant_message_timestamps(),
            (
                first_token_at.timestamp_millis(),
                finished_at.timestamp_millis()
            )
        );
    }

    #[test]
    fn runtime_finish_success_requires_reachable_finished_state() {
        let mut runtime = runtime();
        let error = runtime
            .finish_success(runtime.created_at, None)
            .expect_err("Created -> Finished should be rejected");
        assert!(error.contains("invalid runtime state transition"));
        assert_eq!(runtime.state, RuntimeState::Created);

        runtime
            .mark_called(runtime.created_at)
            .expect("mark_called should succeed");
        runtime
            .mark_waiting_first_token()
            .expect("mark_waiting_first_token should succeed");
        runtime
            .mark_first_token(runtime.created_at)
            .expect("mark_first_token should succeed");
        let usage = UsageReport {
            input_tokens: 10,
            output_tokens: 5,
            total_tokens: 15,
            cached_input_tokens: 0,
            cache_write_tokens: 0,
            reasoning_tokens: 0,
            attachment_input_tokens: 0,
            input_cost: 0.0,
            output_cost: 0.0,
            total_cost: 0.0,
            currency: "USD".to_string(),
            pricing_source: "test".to_string(),
            latency_ms: 20,
            time_to_first_token_ms: 5,
            token_per_second: 250.0,
        };

        runtime
            .finish_success(runtime.created_at, Some(usage.clone()))
            .expect("Streaming -> Finished should succeed");
        assert_eq!(runtime.state, RuntimeState::Finished);
        assert_eq!(
            runtime.call_result_status,
            RuntimeCallResultStatus::Succeeded
        );
        assert_eq!(runtime.usage, Some(usage));

        let terminal_error = runtime
            .transition(RuntimeState::Failed)
            .expect_err("Finished should be terminal");
        assert!(terminal_error.contains("Finished -> Failed"));
    }

    #[test]
    fn runtime_finish_failure_sets_error_status_usage_and_terminal_state() {
        let mut runtime = runtime();
        let usage = UsageReport {
            input_tokens: 2,
            output_tokens: 1,
            total_tokens: 3,
            cached_input_tokens: 0,
            cache_write_tokens: 0,
            reasoning_tokens: 0,
            attachment_input_tokens: 0,
            input_cost: 0.0,
            output_cost: 0.0,
            total_cost: 0.0,
            currency: "USD".to_string(),
            pricing_source: "provider".to_string(),
            latency_ms: 1000,
            time_to_first_token_ms: 0,
            token_per_second: 1.0,
        };
        let error = RuntimeError {
            error_code: Some("CALL_TIMED_OUT".to_string()),
            error_text: Some("runtime call timed out after 1000 ms".to_string()),
            retry_allowed: true,
            fallback_allowed: true,
            fallback_to_id: None,
        };

        runtime
            .finish_failure(
                runtime.created_at,
                error.clone(),
                RuntimeCallResultStatus::TimedOut,
                Some(usage.clone()),
            )
            .expect("Created -> Failed is the allowed failure shortcut");
        assert_eq!(runtime.state, RuntimeState::Failed);
        assert_eq!(
            runtime.call_result_status,
            RuntimeCallResultStatus::TimedOut
        );
        assert_eq!(runtime.error, Some(error));
        assert_eq!(runtime.usage, Some(usage));

        let terminal_error = runtime
            .mark_called(runtime.created_at)
            .expect_err("Failed should be terminal");
        assert!(terminal_error.contains("Failed -> Dispatching"));
    }
}
