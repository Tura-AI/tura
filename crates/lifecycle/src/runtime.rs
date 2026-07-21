use chrono::{DateTime, Utc};
use serde::de::Error as _;
use serde::ser::SerializeStruct;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::session::SessionId;

/// UTC timestamp with millisecond precision.
pub type UtcDateTimeMs = DateTime<Utc>;

pub type RuntimeId = String;

/// Runtime-scoped agent identifier.
pub type AgentId = String;

pub const DEFAULT_CONTEXT_TOKEN_LIMIT: u64 = 260_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ToolChoice {
    Auto,
    Strict,
    Disable,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub tura_llm_name: String,
    #[serde(default)]
    pub default_model_tier: Option<String>,
    #[serde(default)]
    pub current_model: Option<String>,
    pub stream: bool,
    pub temperature: f32,
    pub max_tokens: u32,
    pub tool_choice: ToolChoice,
    pub time_out_ms: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextTokenStats {
    #[serde(default)]
    pub input: u64,
    #[serde(default = "default_context_token_limit")]
    pub limit: u64,
}

impl Default for ContextTokenStats {
    fn default() -> Self {
        Self {
            input: 0,
            limit: DEFAULT_CONTEXT_TOKEN_LIMIT,
        }
    }
}

fn default_context_token_limit() -> u64 {
    DEFAULT_CONTEXT_TOKEN_LIMIT
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RuntimeCallResultStatus {
    Pending,
    Streaming,
    Succeeded,
    Failed,
    TimedOut,
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RuntimeState {
    Created,
    Dispatching,
    WaitingFirstToken,
    Streaming,
    Finished,
    Failed,
    TimedOut,
    Cancelled,
}

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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "command", rename_all = "snake_case", deny_unknown_fields)]
pub enum RuntimeCommand {
    Transition {
        next: RuntimeState,
    },
    CallStarted {
        called_at: UtcDateTimeMs,
    },
    WaitingFirstToken,
    FirstTokenReceived {
        first_token_at: UtcDateTimeMs,
    },
    AppendText {
        chunk: String,
    },
    CaptureInput {
        input: serde_json::Value,
    },
    CaptureOutput {
        output: serde_json::Value,
    },
    RecordToolCall {
        record: ToolCallRecord,
    },
    UpdateContextTokens {
        context_tokens: ContextTokenStats,
    },
    UpdateUsage {
        usage: Option<UsageReport>,
    },
    UpdateReasoning {
        reasoning: Option<ReasoningText>,
        reasoning_hash: Option<ReasoningHash>,
    },
    FinishSuccess {
        finished_at: UtcDateTimeMs,
        usage: Option<UsageReport>,
    },
    FinishFailure {
        finished_at: UtcDateTimeMs,
        error: RuntimeError,
        terminal_state: RuntimeState,
        usage: Option<UsageReport>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case", deny_unknown_fields)]
pub enum RuntimeEvent {
    RuntimeCreated {
        session_id: SessionId,
        agent_id: AgentId,
        provider: RuntimeProviderConfig,
        created_at: UtcDateTimeMs,
        #[serde(deserialize_with = "Option::deserialize")]
        fallback_from_id: Option<RuntimeId>,
        context_tokens: ContextTokenStats,
    },
    StateChanged {
        state: RuntimeState,
    },
    CallStarted {
        called_at: UtcDateTimeMs,
    },
    WaitingFirstToken,
    FirstTokenReceived {
        first_token_at: UtcDateTimeMs,
    },
    TextAppended {
        chunk: String,
    },
    InputCaptured {
        input: serde_json::Value,
    },
    OutputCaptured {
        output: serde_json::Value,
    },
    ToolCallRecorded {
        record: ToolCallRecord,
    },
    ContextTokensUpdated {
        context_tokens: ContextTokenStats,
    },
    UsageUpdated {
        usage: Option<UsageReport>,
    },
    ReasoningUpdated {
        reasoning: Option<ReasoningText>,
        reasoning_hash: Option<ReasoningHash>,
    },
    RuntimeFinished {
        finished_at: UtcDateTimeMs,
        usage: Option<UsageReport>,
    },
    RuntimeFailed {
        finished_at: UtcDateTimeMs,
        error: RuntimeError,
        state: RuntimeState,
        usage: Option<UsageReport>,
    },
}

impl RuntimeEvent {
    fn as_command(&self) -> Option<RuntimeCommand> {
        match self {
            Self::RuntimeCreated { .. } => None,
            Self::StateChanged { state } => Some(RuntimeCommand::Transition { next: *state }),
            Self::CallStarted { called_at } => Some(RuntimeCommand::CallStarted {
                called_at: *called_at,
            }),
            Self::WaitingFirstToken => Some(RuntimeCommand::WaitingFirstToken),
            Self::FirstTokenReceived { first_token_at } => {
                Some(RuntimeCommand::FirstTokenReceived {
                    first_token_at: *first_token_at,
                })
            }
            Self::TextAppended { chunk } => Some(RuntimeCommand::AppendText {
                chunk: chunk.clone(),
            }),
            Self::InputCaptured { input } => Some(RuntimeCommand::CaptureInput {
                input: input.clone(),
            }),
            Self::OutputCaptured { output } => Some(RuntimeCommand::CaptureOutput {
                output: output.clone(),
            }),
            Self::ToolCallRecorded { record } => Some(RuntimeCommand::RecordToolCall {
                record: record.clone(),
            }),
            Self::ContextTokensUpdated { context_tokens } => {
                Some(RuntimeCommand::UpdateContextTokens {
                    context_tokens: *context_tokens,
                })
            }
            Self::UsageUpdated { usage } => Some(RuntimeCommand::UpdateUsage {
                usage: usage.clone(),
            }),
            Self::ReasoningUpdated {
                reasoning,
                reasoning_hash,
            } => Some(RuntimeCommand::UpdateReasoning {
                reasoning: reasoning.clone(),
                reasoning_hash: reasoning_hash.clone(),
            }),
            Self::RuntimeFinished { finished_at, usage } => Some(RuntimeCommand::FinishSuccess {
                finished_at: *finished_at,
                usage: usage.clone(),
            }),
            Self::RuntimeFailed {
                finished_at,
                error,
                state,
                usage,
            } => Some(RuntimeCommand::FinishFailure {
                finished_at: *finished_at,
                error: error.clone(),
                terminal_state: *state,
                usage: usage.clone(),
            }),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "query", rename_all = "snake_case", deny_unknown_fields)]
pub enum RuntimeQuery {
    Lifecycle,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeProjection {
    pub runtime_id: RuntimeId,
    pub state: RuntimeState,
    pub call_result_status: RuntimeCallResultStatus,
    pub live: bool,
    pub session_db_refresh_required: bool,
}

impl RuntimeProjection {
    pub fn new(runtime_id: RuntimeId, state: RuntimeState) -> Self {
        Self {
            runtime_id,
            state,
            call_result_status: state.call_result_status(),
            live: state.is_live(),
            session_db_refresh_required: !state.is_live(),
        }
    }

    pub fn call_result_status(&self) -> RuntimeCallResultStatus {
        self.call_result_status
    }

    pub fn live_overlay_active(&self) -> bool {
        self.live
    }

    pub fn should_refresh_session_db(&self) -> bool {
        self.session_db_refresh_required
    }
}

impl<'de> Deserialize<'de> for RuntimeProjection {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Wire {
            runtime_id: RuntimeId,
            state: RuntimeState,
            call_result_status: RuntimeCallResultStatus,
            live: bool,
            session_db_refresh_required: bool,
        }

        let wire = Wire::deserialize(deserializer)?;
        let projection = RuntimeProjection::new(wire.runtime_id, wire.state);
        if wire.call_result_status != projection.call_result_status
            || wire.live != projection.live
            || wire.session_db_refresh_required != projection.session_db_refresh_required
        {
            return Err(D::Error::custom(
                "runtime lifecycle projection contradicts runtime state",
            ));
        }
        Ok(projection)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeTransitionError {
    pub previous: RuntimeState,
    pub next: RuntimeState,
}

impl std::fmt::Display for RuntimeTransitionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "invalid runtime state transition: {:?} -> {:?}",
            self.previous, self.next
        )
    }
}

impl std::error::Error for RuntimeTransitionError {}

/// Canonical state for one provider invocation.
#[derive(Debug, Clone)]
pub struct RuntimeAggregate {
    pub runtime_id: RuntimeId,
    pub session_id: SessionId,
    pub state: RuntimeState,
    /// Runtime creation timestamp.
    pub created_at: UtcDateTimeMs,
    /// Time the call started consuming provider resources.
    pub called_at: Option<UtcDateTimeMs>,
    /// Time the first token was received.
    pub first_token_at: Option<UtcDateTimeMs>,
    /// Time the full callback finished.
    pub call_finished_at: Option<UtcDateTimeMs>,
    /// If this runtime is a fallback, reference the failed runtime identifier.
    pub fallback_from_id: Option<RuntimeId>,
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
    pub input: Option<serde_json::Value>,
    /// Full provider response payload received for this runtime call.
    pub output: Option<serde_json::Value>,
    /// Assistant text output.
    pub text: OutputText,
    /// Tool call reports.
    pub tool_call: Vec<ToolCallRecord>,
    /// Latest provider-reported input token count for this runtime.
    pub context_tokens: ContextTokenStats,
    /// Usage and billing report.
    pub usage: Option<UsageReport>,
    /// Events produced locally but not yet acknowledged by the Session service.
    /// This transport buffer is deliberately excluded from the aggregate wire form.
    uncommitted_events: Vec<RuntimeEvent>,
}

impl PartialEq for RuntimeAggregate {
    fn eq(&self, other: &Self) -> bool {
        self.runtime_id == other.runtime_id
            && self.session_id == other.session_id
            && self.state == other.state
            && self.created_at == other.created_at
            && self.called_at == other.called_at
            && self.first_token_at == other.first_token_at
            && self.call_finished_at == other.call_finished_at
            && self.fallback_from_id == other.fallback_from_id
            && self.agent_id == other.agent_id
            && self.provider == other.provider
            && self.error == other.error
            && self.reasoning == other.reasoning
            && self.reasoning_hash == other.reasoning_hash
            && self.input == other.input
            && self.output == other.output
            && self.text == other.text
            && self.tool_call == other.tool_call
            && self.context_tokens == other.context_tokens
            && self.usage == other.usage
    }
}

impl Serialize for RuntimeAggregate {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let field_count =
            18 + usize::from(self.input.is_some()) + usize::from(self.output.is_some());
        let mut wire = serializer.serialize_struct("RuntimeAggregate", field_count)?;
        wire.serialize_field("runtime_id", &self.runtime_id)?;
        wire.serialize_field("created_at", &self.created_at)?;
        wire.serialize_field("called_at", &self.called_at)?;
        wire.serialize_field("first_token_at", &self.first_token_at)?;
        wire.serialize_field("call_finished_at", &self.call_finished_at)?;
        wire.serialize_field("call_result_status", &self.call_result_status())?;
        wire.serialize_field("fallback_from_id", &self.fallback_from_id)?;
        wire.serialize_field("session_id", &self.session_id)?;
        wire.serialize_field("agent_id", &self.agent_id)?;
        wire.serialize_field("provider", &self.provider)?;
        wire.serialize_field("error", &self.error)?;
        wire.serialize_field("reasoning", &self.reasoning)?;
        wire.serialize_field("reasoning_hash", &self.reasoning_hash)?;
        if let Some(input) = &self.input {
            wire.serialize_field("input", input)?;
        }
        if let Some(output) = &self.output {
            wire.serialize_field("output", output)?;
        }
        wire.serialize_field("text", &self.text)?;
        wire.serialize_field("tool_call", &self.tool_call)?;
        wire.serialize_field("context_tokens", &self.context_tokens)?;
        wire.serialize_field("usage", &self.usage)?;
        wire.serialize_field("state", &self.state)?;
        wire.end()
    }
}

impl<'de> Deserialize<'de> for RuntimeAggregate {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Wire {
            runtime_id: RuntimeId,
            created_at: UtcDateTimeMs,
            called_at: Option<UtcDateTimeMs>,
            first_token_at: Option<UtcDateTimeMs>,
            call_finished_at: Option<UtcDateTimeMs>,
            call_result_status: RuntimeCallResultStatus,
            #[serde(deserialize_with = "Option::deserialize")]
            fallback_from_id: Option<RuntimeId>,
            session_id: SessionId,
            agent_id: AgentId,
            provider: RuntimeProviderConfig,
            error: Option<RuntimeError>,
            reasoning: Option<ReasoningText>,
            reasoning_hash: Option<ReasoningHash>,
            #[serde(default)]
            input: Option<serde_json::Value>,
            #[serde(default)]
            output: Option<serde_json::Value>,
            text: OutputText,
            tool_call: Vec<ToolCallRecord>,
            #[serde(default)]
            context_tokens: ContextTokenStats,
            usage: Option<UsageReport>,
            state: RuntimeState,
        }

        let wire = Wire::deserialize(deserializer)?;
        if wire.call_result_status != wire.state.call_result_status() {
            return Err(D::Error::custom(
                "runtime call result projection contradicts runtime state",
            ));
        }
        Ok(Self {
            runtime_id: wire.runtime_id,
            session_id: wire.session_id,
            state: wire.state,
            created_at: wire.created_at,
            called_at: wire.called_at,
            first_token_at: wire.first_token_at,
            call_finished_at: wire.call_finished_at,
            fallback_from_id: wire.fallback_from_id,
            agent_id: wire.agent_id,
            provider: wire.provider,
            error: wire.error,
            reasoning: wire.reasoning,
            reasoning_hash: wire.reasoning_hash,
            input: wire.input,
            output: wire.output,
            text: wire.text,
            tool_call: wire.tool_call,
            context_tokens: wire.context_tokens,
            usage: wire.usage,
            uncommitted_events: Vec::new(),
        })
    }
}

impl RuntimeAggregate {
    /// Creates a new runtime record in `Created` state.
    pub fn new(
        runtime_id: RuntimeId,
        session_id: SessionId,
        agent_id: AgentId,
        provider: RuntimeProviderConfig,
        created_at: UtcDateTimeMs,
    ) -> Self {
        Self::new_with_fallback(runtime_id, session_id, agent_id, provider, created_at, None)
            .expect("runtime without fallback source is valid")
    }

    pub fn new_with_fallback(
        runtime_id: RuntimeId,
        session_id: SessionId,
        agent_id: AgentId,
        provider: RuntimeProviderConfig,
        created_at: UtcDateTimeMs,
        fallback_from_id: Option<RuntimeId>,
    ) -> Result<Self, String> {
        validate_fallback_source(&runtime_id, fallback_from_id.as_ref())?;
        let context_tokens = ContextTokenStats::default();
        let created = RuntimeEvent::RuntimeCreated {
            session_id: session_id.clone(),
            agent_id: agent_id.clone(),
            provider: provider.clone(),
            created_at,
            fallback_from_id: fallback_from_id.clone(),
            context_tokens,
        };
        Ok(Self {
            runtime_id,
            session_id,
            state: RuntimeState::Created,
            created_at,
            called_at: None,
            first_token_at: None,
            call_finished_at: None,
            fallback_from_id,
            agent_id,
            provider,
            error: None,
            reasoning: None,
            reasoning_hash: None,
            input: None,
            output: None,
            text: String::new(),
            tool_call: Vec::new(),
            context_tokens,
            usage: None,
            uncommitted_events: vec![created],
        })
    }

    pub fn execute(&mut self, command: RuntimeCommand) -> Result<RuntimeEvent, String> {
        let event = self.decide(command)?;
        self.apply(&event);
        self.uncommitted_events.push(event.clone());
        Ok(event)
    }

    /// Rebuilds canonical runtime state from its ordered event stream.
    pub fn replay(
        runtime_id: RuntimeId,
        events: impl IntoIterator<Item = RuntimeEvent>,
    ) -> Result<Self, String> {
        let mut events = events.into_iter();
        let first = events
            .next()
            .ok_or_else(|| format!("runtime {runtime_id} has no creation event"))?;
        let RuntimeEvent::RuntimeCreated {
            session_id,
            agent_id,
            provider,
            created_at,
            fallback_from_id,
            context_tokens,
        } = first
        else {
            return Err(format!(
                "runtime {runtime_id} event stream must begin with runtime_created"
            ));
        };
        validate_fallback_source(&runtime_id, fallback_from_id.as_ref())?;
        let mut aggregate = Self {
            runtime_id,
            session_id,
            state: RuntimeState::Created,
            created_at,
            called_at: None,
            first_token_at: None,
            call_finished_at: None,
            fallback_from_id,
            agent_id,
            provider,
            error: None,
            reasoning: None,
            reasoning_hash: None,
            input: None,
            output: None,
            text: String::new(),
            tool_call: Vec::new(),
            context_tokens,
            usage: None,
            uncommitted_events: Vec::new(),
        };
        for event in events {
            aggregate.apply_committed(&event)?;
        }
        Ok(aggregate)
    }

    /// Applies one event received from the canonical ordered stream.
    pub fn apply_committed(&mut self, event: &RuntimeEvent) -> Result<(), String> {
        let command = event
            .as_command()
            .ok_or_else(|| "runtime_created may only be the first runtime event".to_string())?;
        let expected = self.decide(command)?;
        if expected != *event {
            return Err("runtime event does not match the canonical reducer result".to_string());
        }
        self.apply(event);
        Ok(())
    }

    pub fn take_uncommitted_events(&mut self) -> Vec<RuntimeEvent> {
        std::mem::take(&mut self.uncommitted_events)
    }

    pub fn next_uncommitted_event(&self) -> Option<&RuntimeEvent> {
        self.uncommitted_events.first()
    }

    pub fn acknowledge_uncommitted_event(&mut self) -> Option<RuntimeEvent> {
        if self.uncommitted_events.is_empty() {
            None
        } else {
            Some(self.uncommitted_events.remove(0))
        }
    }

    pub fn has_uncommitted_events(&self) -> bool {
        !self.uncommitted_events.is_empty()
    }

    pub fn decide(&self, command: RuntimeCommand) -> Result<RuntimeEvent, String> {
        let transition = |next| {
            if self.state.can_transition_to(next) {
                Ok(())
            } else {
                Err(RuntimeTransitionError {
                    previous: self.state,
                    next,
                }
                .to_string())
            }
        };
        match command {
            RuntimeCommand::Transition { next } => {
                transition(next)?;
                Ok(RuntimeEvent::StateChanged { state: next })
            }
            RuntimeCommand::CallStarted { called_at } => {
                transition(RuntimeState::Dispatching)?;
                Ok(RuntimeEvent::CallStarted { called_at })
            }
            RuntimeCommand::WaitingFirstToken => {
                transition(RuntimeState::WaitingFirstToken)?;
                Ok(RuntimeEvent::WaitingFirstToken)
            }
            RuntimeCommand::FirstTokenReceived { first_token_at } => {
                transition(RuntimeState::Streaming)?;
                Ok(RuntimeEvent::FirstTokenReceived { first_token_at })
            }
            RuntimeCommand::AppendText { chunk } => {
                self.require_live_command("append_text")?;
                Ok(RuntimeEvent::TextAppended { chunk })
            }
            RuntimeCommand::CaptureInput { input } => {
                self.require_live_command("capture_input")?;
                Ok(RuntimeEvent::InputCaptured { input })
            }
            RuntimeCommand::CaptureOutput { output } => {
                self.require_live_command("capture_output")?;
                Ok(RuntimeEvent::OutputCaptured { output })
            }
            RuntimeCommand::RecordToolCall { record } => {
                self.require_live_command("record_tool_call")?;
                Ok(RuntimeEvent::ToolCallRecorded { record })
            }
            RuntimeCommand::UpdateContextTokens { context_tokens } => {
                self.require_live_command("update_context_tokens")?;
                Ok(RuntimeEvent::ContextTokensUpdated { context_tokens })
            }
            RuntimeCommand::UpdateUsage { usage } => {
                self.require_live_command("update_usage")?;
                Ok(RuntimeEvent::UsageUpdated { usage })
            }
            RuntimeCommand::UpdateReasoning {
                reasoning,
                reasoning_hash,
            } => {
                self.require_live_command("update_reasoning")?;
                Ok(RuntimeEvent::ReasoningUpdated {
                    reasoning,
                    reasoning_hash,
                })
            }
            RuntimeCommand::FinishSuccess { finished_at, usage } => {
                transition(RuntimeState::Finished)?;
                Ok(RuntimeEvent::RuntimeFinished { finished_at, usage })
            }
            RuntimeCommand::FinishFailure {
                finished_at,
                error,
                terminal_state,
                usage,
            } => {
                if !matches!(
                    terminal_state,
                    RuntimeState::Failed | RuntimeState::TimedOut | RuntimeState::Cancelled
                ) {
                    return Err(format!(
                        "runtime failure requires a failure terminal state, got {terminal_state:?}"
                    ));
                }
                transition(terminal_state)?;
                Ok(RuntimeEvent::RuntimeFailed {
                    finished_at,
                    error,
                    state: terminal_state,
                    usage,
                })
            }
        }
    }

    pub fn apply(&mut self, event: &RuntimeEvent) {
        match event {
            RuntimeEvent::RuntimeCreated {
                session_id,
                agent_id,
                provider,
                created_at,
                fallback_from_id,
                context_tokens,
            } => {
                self.session_id.clone_from(session_id);
                self.agent_id.clone_from(agent_id);
                self.provider.clone_from(provider);
                self.created_at = *created_at;
                self.fallback_from_id.clone_from(fallback_from_id);
                self.context_tokens = *context_tokens;
            }
            RuntimeEvent::StateChanged { state } => self.state = *state,
            RuntimeEvent::CallStarted { called_at } => {
                self.state = RuntimeState::Dispatching;
                self.called_at = Some(*called_at);
            }
            RuntimeEvent::WaitingFirstToken => self.state = RuntimeState::WaitingFirstToken,
            RuntimeEvent::FirstTokenReceived { first_token_at } => {
                self.state = RuntimeState::Streaming;
                self.first_token_at = Some(*first_token_at);
            }
            RuntimeEvent::TextAppended { chunk } => self.text.push_str(chunk),
            RuntimeEvent::InputCaptured { input } => self.input = Some(input.clone()),
            RuntimeEvent::OutputCaptured { output } => self.output = Some(output.clone()),
            RuntimeEvent::ToolCallRecorded { record } => self.tool_call.push(record.clone()),
            RuntimeEvent::ContextTokensUpdated { context_tokens } => {
                self.context_tokens = *context_tokens;
            }
            RuntimeEvent::UsageUpdated { usage } => self.usage = usage.clone(),
            RuntimeEvent::ReasoningUpdated {
                reasoning,
                reasoning_hash,
            } => {
                self.reasoning = reasoning.clone();
                self.reasoning_hash = reasoning_hash.clone();
            }
            RuntimeEvent::RuntimeFinished { finished_at, usage } => {
                self.state = RuntimeState::Finished;
                self.call_finished_at = Some(*finished_at);
                self.usage = usage.clone();
                if let Some(input_tokens) = usage
                    .as_ref()
                    .map(|usage| usage.input_tokens)
                    .filter(|input_tokens| *input_tokens > 0)
                {
                    self.context_tokens.input = input_tokens;
                }
            }
            RuntimeEvent::RuntimeFailed {
                finished_at,
                error,
                state,
                usage,
            } => {
                self.state = *state;
                self.call_finished_at = Some(*finished_at);
                self.error = Some(error.clone());
                self.usage = usage.clone();
                if let Some(input_tokens) = usage
                    .as_ref()
                    .map(|usage| usage.input_tokens)
                    .filter(|input_tokens| *input_tokens > 0)
                {
                    self.context_tokens.input = input_tokens;
                }
            }
        }
    }

    fn require_live_command(&self, command: &str) -> Result<(), String> {
        if self.state.is_live() {
            Ok(())
        } else {
            Err(format!(
                "runtime command {command} rejected in terminal state {:?}",
                self.state
            ))
        }
    }

    /// Applies a validated runtime state transition.
    pub fn transition(&mut self, next: RuntimeState) -> Result<(), String> {
        self.execute(RuntimeCommand::Transition { next })
            .map(|_| ())
    }

    /// Marks the runtime as dispatched to the provider.
    pub fn mark_called(&mut self, called_at: UtcDateTimeMs) -> Result<(), String> {
        self.execute(RuntimeCommand::CallStarted { called_at })
            .map(|_| ())
    }

    /// Marks that the request is now waiting for the first token.
    pub fn mark_waiting_first_token(&mut self) -> Result<(), String> {
        self.execute(RuntimeCommand::WaitingFirstToken).map(|_| ())
    }

    /// Marks the runtime as streaming and records first-token time.
    pub fn mark_first_token(&mut self, first_token_at: UtcDateTimeMs) -> Result<(), String> {
        self.execute(RuntimeCommand::FirstTokenReceived { first_token_at })
            .map(|_| ())
    }

    /// Appends model text while the call is streaming.
    pub fn append_text(&mut self, chunk: impl Into<String>) -> Result<(), String> {
        self.execute(RuntimeCommand::AppendText {
            chunk: chunk.into(),
        })
        .map(|_| ())
    }

    /// Stores the exact request payload that was sent to the provider.
    pub fn set_input(&mut self, input: serde_json::Value) -> Result<(), String> {
        self.execute(RuntimeCommand::CaptureInput { input })
            .map(|_| ())
    }

    /// Stores the full provider response payload.
    pub fn set_output(&mut self, output: serde_json::Value) -> Result<(), String> {
        self.execute(RuntimeCommand::CaptureOutput { output })
            .map(|_| ())
    }

    /// Adds one tool call record.
    pub fn push_tool_call(&mut self, record: ToolCallRecord) -> Result<(), String> {
        self.execute(RuntimeCommand::RecordToolCall { record })
            .map(|_| ())
    }

    pub fn update_context_tokens(
        &mut self,
        context_tokens: ContextTokenStats,
    ) -> Result<(), String> {
        self.execute(RuntimeCommand::UpdateContextTokens { context_tokens })
            .map(|_| ())
    }

    pub fn update_usage(&mut self, usage: Option<UsageReport>) -> Result<(), String> {
        self.execute(RuntimeCommand::UpdateUsage { usage })
            .map(|_| ())
    }

    pub fn update_reasoning(
        &mut self,
        reasoning: Option<ReasoningText>,
        reasoning_hash: Option<ReasoningHash>,
    ) -> Result<(), String> {
        self.execute(RuntimeCommand::UpdateReasoning {
            reasoning,
            reasoning_hash,
        })
        .map(|_| ())
    }

    /// True while gateway should keep callback payloads in the active live
    /// overlay for this runtime call.
    pub fn live_overlay_active(&self) -> bool {
        self.state.is_live()
    }

    /// True once gateway should drop this runtime's live overlay and refresh
    /// the canonical session DB history.
    pub fn session_db_refresh_required(&self) -> bool {
        !self.live_overlay_active()
    }

    pub fn lifecycle_projection(&self) -> RuntimeProjection {
        self.query(RuntimeQuery::Lifecycle)
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
        self.execute(RuntimeCommand::FinishSuccess { finished_at, usage })
            .map(|_| ())
    }

    /// Marks the runtime as failed and stores the error payload.
    pub fn finish_failure(
        &mut self,
        finished_at: UtcDateTimeMs,
        error: RuntimeError,
        terminal_state: RuntimeState,
        usage: Option<UsageReport>,
    ) -> Result<(), String> {
        self.execute(RuntimeCommand::FinishFailure {
            finished_at,
            error,
            terminal_state,
            usage,
        })
        .map(|_| ())
    }

    pub fn call_result_status(&self) -> RuntimeCallResultStatus {
        self.state.call_result_status()
    }

    pub fn query(&self, query: RuntimeQuery) -> RuntimeProjection {
        match query {
            RuntimeQuery::Lifecycle => RuntimeProjection::new(self.runtime_id.clone(), self.state),
        }
    }
}

fn validate_fallback_source(
    runtime_id: &RuntimeId,
    fallback_from_id: Option<&RuntimeId>,
) -> Result<(), String> {
    let Some(source) = fallback_from_id else {
        return Ok(());
    };
    if source.trim().is_empty() {
        return Err("runtime fallback source must be non-empty".to_string());
    }
    if source == runtime_id {
        return Err("runtime cannot fall back from itself".to_string());
    }
    Ok(())
}

impl RuntimeState {
    pub fn can_transition_to(self, next: Self) -> bool {
        use RuntimeState::*;

        match (self, next) {
            (Created, Dispatching | Failed | TimedOut | Cancelled) => true,
            (Dispatching, WaitingFirstToken | Failed | TimedOut | Cancelled) => true,
            (WaitingFirstToken, Streaming | Finished | Failed | TimedOut | Cancelled) => true,
            (Streaming, Finished | Failed | TimedOut | Cancelled) => true,
            (Finished | Failed | TimedOut | Cancelled, _) => false,
            _ if self == next => true,
            _ => false,
        }
    }

    pub fn call_result_status(self) -> RuntimeCallResultStatus {
        match self {
            Self::Created | Self::Dispatching | Self::WaitingFirstToken => {
                RuntimeCallResultStatus::Pending
            }
            Self::Streaming => RuntimeCallResultStatus::Streaming,
            Self::Finished => RuntimeCallResultStatus::Succeeded,
            Self::Failed => RuntimeCallResultStatus::Failed,
            Self::TimedOut => RuntimeCallResultStatus::TimedOut,
            Self::Cancelled => RuntimeCallResultStatus::Cancelled,
        }
    }

    pub fn is_live(self) -> bool {
        matches!(
            self,
            Self::Created | Self::Dispatching | Self::WaitingFirstToken | Self::Streaming
        )
    }

    pub fn is_terminal(self) -> bool {
        !self.is_live()
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ContextTokenStats, ProviderConfig, RuntimeAggregate, RuntimeCallResultStatus, RuntimeError,
        RuntimeEvent, RuntimeProjection, RuntimeProviderConfig, RuntimeState, ToolChoice,
        UsageReport,
    };
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

    fn runtime() -> RuntimeAggregate {
        RuntimeAggregate::new(
            "runtime-test".to_string(),
            "session-test".to_string(),
            "agent-test".to_string(),
            provider_config(),
            Utc::now(),
        )
    }

    fn usage_report() -> UsageReport {
        UsageReport {
            input_tokens: 0,
            output_tokens: 0,
            total_tokens: 0,
            cached_input_tokens: 0,
            cache_write_tokens: 0,
            reasoning_tokens: 0,
            attachment_input_tokens: 0,
            input_cost: 0.0,
            output_cost: 0.0,
            total_cost: 0.0,
            currency: "USD".to_string(),
            pricing_source: "test".to_string(),
            latency_ms: 0,
            time_to_first_token_ms: 0,
            token_per_second: 0.0,
        }
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
            TimedOut,
            Cancelled,
        ];
        for from in states {
            for to in states {
                let expected = matches!(
                    (from, to),
                    (
                        Created,
                        Created | Dispatching | Failed | TimedOut | Cancelled
                    ) | (
                        Dispatching,
                        Dispatching | WaitingFirstToken | Failed | TimedOut | Cancelled
                    ) | (
                        WaitingFirstToken,
                        WaitingFirstToken | Streaming | Finished | Failed | TimedOut | Cancelled
                    ) | (
                        Streaming,
                        Streaming | Finished | Failed | TimedOut | Cancelled
                    )
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
            runtime.call_result_status(),
            RuntimeCallResultStatus::Streaming
        );
    }

    #[test]
    fn runtime_replay_rebuilds_state_from_ordered_events() {
        let mut runtime = runtime();
        let called_at = runtime.created_at + Duration::milliseconds(10);
        let first_token_at = called_at + Duration::milliseconds(20);
        let finished_at = first_token_at + Duration::milliseconds(30);

        runtime.mark_called(called_at).expect("mark called");
        runtime
            .mark_waiting_first_token()
            .expect("mark waiting first token");
        runtime
            .mark_first_token(first_token_at)
            .expect("mark first token");
        runtime.append_text("hello").expect("append text");
        runtime
            .set_input(serde_json::json!({ "prompt": "hello" }))
            .expect("capture input");
        runtime
            .set_output(serde_json::json!({ "text": "hello" }))
            .expect("capture output");
        runtime
            .update_context_tokens(ContextTokenStats {
                input: 12,
                limit: 260_000,
            })
            .expect("update context tokens");
        runtime
            .update_reasoning(Some("reasoning".to_string()), Some("hash".to_string()))
            .expect("update reasoning");
        runtime
            .finish_success(finished_at, None)
            .expect("finish runtime");

        let events = runtime.take_uncommitted_events();
        let replayed = RuntimeAggregate::replay(runtime.runtime_id.clone(), events)
            .expect("ordered events should replay");

        assert_eq!(replayed, runtime);
        assert!(!replayed.has_uncommitted_events());
    }

    #[test]
    fn runtime_fallback_source_is_immutable_creation_metadata() {
        let mut retry_runtime = RuntimeAggregate::new_with_fallback(
            "runtime-retry".to_string(),
            "session-test".to_string(),
            "agent-test".to_string(),
            provider_config(),
            Utc::now(),
            Some("runtime-failed".to_string()),
        )
        .expect("valid fallback runtime");
        assert_eq!(
            retry_runtime.fallback_from_id.as_deref(),
            Some("runtime-failed")
        );

        let events = retry_runtime.take_uncommitted_events();
        assert_eq!(events.len(), 1, "fallback metadata belongs to creation");
        let encoded = serde_json::to_value(&events[0]).expect("serialize creation event");
        assert_eq!(encoded["fallback_from_id"], "runtime-failed");
        let replayed = RuntimeAggregate::replay(retry_runtime.runtime_id.clone(), events)
            .expect("fallback source should replay");
        assert_eq!(replayed, retry_runtime);

        assert!(RuntimeAggregate::new_with_fallback(
            "runtime-self".to_string(),
            "session-test".to_string(),
            "agent-test".to_string(),
            provider_config(),
            Utc::now(),
            Some("runtime-self".to_string()),
        )
        .is_err());

        let mut invalid_events = runtime().take_uncommitted_events();
        let RuntimeEvent::RuntimeCreated {
            fallback_from_id, ..
        } = &mut invalid_events[0]
        else {
            panic!("runtime fixture should start with creation");
        };
        *fallback_from_id = Some("runtime-test".to_string());
        assert!(RuntimeAggregate::replay("runtime-test".to_string(), invalid_events).is_err());

        let normal_created = runtime().take_uncommitted_events().remove(0);
        let normal_encoded =
            serde_json::to_value(normal_created).expect("serialize creation event");
        assert_eq!(normal_encoded["fallback_from_id"], serde_json::Value::Null);
        let mut missing_source = normal_encoded;
        missing_source
            .as_object_mut()
            .expect("runtime event is an object")
            .remove("fallback_from_id");
        assert!(serde_json::from_value::<RuntimeEvent>(missing_source).is_err());
    }

    #[test]
    fn runtime_session_sync_status_is_derived_from_runtime_state_machine() {
        let mut runtime = runtime();
        let created_status = runtime.lifecycle_projection();
        assert!(created_status.live_overlay_active());
        assert!(!created_status.should_refresh_session_db());
        assert_eq!(created_status.state, RuntimeState::Created);

        let called_at = runtime.created_at + Duration::milliseconds(10);
        runtime.mark_called(called_at).expect("mark called");
        runtime
            .mark_waiting_first_token()
            .expect("mark waiting first token");
        let waiting_status = runtime.lifecycle_projection();
        assert!(waiting_status.live_overlay_active());
        assert!(!waiting_status.should_refresh_session_db());

        let first_token_at = called_at + Duration::milliseconds(25);
        runtime
            .mark_first_token(first_token_at)
            .expect("mark first token");
        let streaming_status = runtime.lifecycle_projection();
        assert!(streaming_status.live_overlay_active());
        assert_eq!(
            streaming_status.call_result_status(),
            RuntimeCallResultStatus::Streaming
        );

        let finished_at = first_token_at + Duration::milliseconds(80);
        runtime
            .finish_success(finished_at, None)
            .expect("finish success");
        let finished_status = runtime.lifecycle_projection();
        assert!(!finished_status.live_overlay_active());
        assert!(finished_status.should_refresh_session_db());
        assert_eq!(finished_status.state, RuntimeState::Finished);
        assert_eq!(
            finished_status.call_result_status(),
            RuntimeCallResultStatus::Succeeded
        );
    }

    #[test]
    fn terminal_event_applies_provider_input_tokens_without_post_terminal_mutation() {
        let mut runtime = runtime();
        runtime
            .mark_called(runtime.created_at)
            .expect("mark called");
        runtime
            .mark_waiting_first_token()
            .expect("mark waiting first token");
        let mut usage = usage_report();
        usage.input_tokens = 42;

        runtime
            .finish_success(runtime.created_at, Some(usage))
            .expect("finish success");

        assert_eq!(runtime.context_tokens.input, 42);
        assert!(runtime
            .update_context_tokens(runtime.context_tokens)
            .is_err());
        assert!(matches!(
            runtime.next_uncommitted_event(),
            Some(RuntimeEvent::RuntimeCreated { .. })
        ));
        assert!(matches!(
            runtime.take_uncommitted_events().last(),
            Some(RuntimeEvent::RuntimeFinished { .. })
        ));
    }

    #[test]
    fn runtime_session_sync_status_rejects_contradictory_wire_projection() {
        let contradictory = serde_json::json!({
            "runtime_id": "runtime-contradictory",
            "state": "Finished",
            "call_result_status": "Streaming",
            "live": true,
            "session_db_refresh_required": false
        });

        assert!(serde_json::from_value::<RuntimeProjection>(contradictory).is_err());
    }

    #[test]
    fn runtime_aggregate_rejects_contradictory_wire_projection() {
        let mut contradictory = serde_json::to_value(runtime()).expect("serialize runtime");
        contradictory["state"] = serde_json::json!("Finished");
        contradictory["call_result_status"] = serde_json::json!("Streaming");

        assert!(serde_json::from_value::<RuntimeAggregate>(contradictory).is_err());
    }

    #[test]
    fn runtime_aggregate_rejects_unknown_wire_fields() {
        let mut encoded = serde_json::to_value(runtime()).expect("serialize runtime");
        encoded["extra"] = serde_json::json!(true);

        assert!(serde_json::from_value::<RuntimeAggregate>(encoded).is_err());
    }

    #[test]
    fn runtime_aggregate_requires_nullable_fallback_source_field() {
        let mut encoded = serde_json::to_value(runtime()).expect("serialize runtime");
        encoded
            .as_object_mut()
            .expect("runtime wire value is an object")
            .remove("fallback_from_id");

        assert!(serde_json::from_value::<RuntimeAggregate>(encoded).is_err());
    }

    #[test]
    fn runtime_wire_projection_preserves_existing_json_shape() {
        let mut runtime = runtime();
        runtime
            .mark_called(runtime.created_at)
            .expect("mark runtime called");
        runtime
            .mark_waiting_first_token()
            .expect("mark waiting first token");
        runtime
            .mark_first_token(runtime.created_at)
            .expect("mark first token");

        let sync = serde_json::to_value(runtime.lifecycle_projection()).expect("serialize sync");
        assert_eq!(
            sync,
            serde_json::json!({
                "runtime_id": "runtime-test",
                "state": "Streaming",
                "call_result_status": "Streaming",
                "live": true,
                "session_db_refresh_required": false
            })
        );

        let encoded = serde_json::to_value(&runtime).expect("serialize runtime");
        assert_eq!(encoded["state"], "Streaming");
        assert_eq!(encoded["call_result_status"], "Streaming");
        assert_eq!(encoded["fallback_from_id"], serde_json::Value::Null);
        assert!(encoded.get("input").is_none());
        assert!(encoded.get("output").is_none());
        assert_eq!(
            serde_json::from_value::<RuntimeAggregate>(encoded).expect("round trip runtime"),
            runtime
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
            runtime.call_result_status(),
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
                RuntimeState::TimedOut,
                Some(usage.clone()),
            )
            .expect("Created -> TimedOut is the allowed failure shortcut");
        assert_eq!(runtime.state, RuntimeState::TimedOut);
        assert_eq!(
            runtime.call_result_status(),
            RuntimeCallResultStatus::TimedOut
        );
        assert_eq!(runtime.error, Some(error));
        assert_eq!(runtime.usage, Some(usage));

        let terminal_error = runtime
            .mark_called(runtime.created_at)
            .expect_err("TimedOut should be terminal");
        assert!(terminal_error.contains("TimedOut -> Dispatching"));
    }

    #[test]
    fn runtime_cancelled_terminal_state_drives_status_and_wire_projection() {
        let mut runtime = runtime();
        let error = RuntimeError {
            error_code: Some("COMMAND_RUN_CANCELLED".to_string()),
            error_text: Some("command run cancelled".to_string()),
            retry_allowed: false,
            fallback_allowed: false,
            fallback_to_id: None,
        };

        runtime
            .finish_failure(runtime.created_at, error, RuntimeState::Cancelled, None)
            .expect("Created -> Cancelled should be allowed");

        assert_eq!(runtime.state, RuntimeState::Cancelled);
        assert_eq!(
            runtime.call_result_status(),
            RuntimeCallResultStatus::Cancelled
        );
        assert!(!runtime.live_overlay_active());
        assert!(runtime.session_db_refresh_required());
        let encoded = serde_json::to_value(&runtime).expect("serialize cancelled runtime");
        assert_eq!(encoded["state"], "Cancelled");
        assert_eq!(encoded["call_result_status"], "Cancelled");
    }
}
