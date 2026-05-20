use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::agent_management::{AgentId, ProviderConfig};
use super::session_management::SessionId;

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
    /// Internal provider router name.
    pub provider_router_name: String,
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
