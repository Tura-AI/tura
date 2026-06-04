use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// UTC timestamp with millisecond precision.
pub type UtcDateTimeMs = DateTime<Utc>;

/// Runtime-scoped hexadecimal identifier.
pub type AgentId = String;

/// Natural-language agent name.
pub type AgentName = String;

/// Natural-language prompt name.
pub type PromptName = String;

/// Natural-language persona name.
pub type PersonaName = String;

/// Natural-language capability name.
pub type CapabilityName = String;

/// One prompt resource attached to an agent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentPromptItem {
    /// Natural-language prompt name.
    pub agent_prompt: PromptName,
    /// Absolute path to the prompt directory.
    pub prompt_directory: PathBuf,
}

/// One persona prompt resource attached to an agent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentPersonaItem {
    /// Natural-language persona name.
    pub persona_name: PersonaName,
    /// Absolute path to the persona prompt directory.
    pub persona_directory: PathBuf,
}

/// One command capability resource attached to an agent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentCapabilityItem {
    /// Natural-language capability name.
    pub capability_name: CapabilityName,
    /// Absolute path to the capability directory.
    pub capability_directory: PathBuf,
}

/// LLM/provider configuration used by the agent.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProviderConfig {
    /// Internal LLM configuration name.
    pub tura_llm_name: String,
    /// Whether streaming is enabled.
    pub stream: bool,
    /// Sampling temperature.
    pub temperature: f32,
    /// Max output tokens allowed for a call.
    pub max_tokens: u32,
    /// Tool selection policy.
    pub tool_choice: ToolChoice,
    /// Call timeout in milliseconds.
    pub time_out_ms: u64,
}

/// Tool selection mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ToolChoice {
    Auto,
    Strict,
    Disable,
}

/// Validation configuration for an agent deliverable.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidatorConfig {
    /// Whether the agent output must be validated.
    pub need_validator: bool,
    /// Validator name.
    pub validator_name: Option<String>,
}

/// State machine for agent execution lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentState {
    /// Agent definition exists but has not started work.
    Idle,
    /// Agent is processing a task.
    Running,
    /// Agent is waiting on downstream work or input.
    Waiting,
    /// Agent completed successfully.
    Completed,
    /// Agent failed.
    Failed,
}

impl AgentState {
    /// Returns true if transitioning from `self` to `next` is allowed.
    pub fn can_transition_to(self, next: AgentState) -> bool {
        use AgentState::*;

        match (self, next) {
            (Idle, Running) => true,
            (Running, Waiting | Completed | Failed) => true,
            (Waiting, Running | Failed) => true,
            (Completed | Failed, _) => false,
            _ if self == next => true,
            _ => false,
        }
    }
}

/// Root agent state object.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgentManagement {
    /// Runtime-scoped agent identifier.
    pub agent_id: AgentId,
    /// Natural-language agent name.
    pub agent_name: AgentName,
    /// Absolute directory path of the agent.
    pub agent_directory: PathBuf,
    /// Upstream parent agent identifier, if any.
    pub parent_agent_id: Option<AgentId>,
    /// Whether this agent reports directly to the user.
    pub report_to_user: bool,
    /// Whether this is a protected built-in/default configuration.
    pub default_config: bool,
    /// Provider/LLM configuration.
    pub provider: ProviderConfig,
    /// Prompts bound to this agent.
    pub agent_prompt: Vec<AgentPromptItem>,
    /// Persona prompt resources bound to this agent.
    #[serde(default)]
    pub agent_persona: Vec<AgentPersonaItem>,
    /// Capabilities bound to this agent.
    pub agent_capabilities: Vec<AgentCapabilityItem>,
    /// Validator configuration.
    pub validator: ValidatorConfig,
    /// Current lifecycle state.
    pub state: AgentState,
    /// Creation timestamp in UTC.
    pub created_at: UtcDateTimeMs,
    /// Last state update timestamp in UTC.
    pub updated_at: UtcDateTimeMs,
}

impl AgentManagement {
    /// Creates a new agent in `Idle` state.
    #[expect(
        clippy::too_many_arguments,
        reason = "agent state construction mirrors the serialized state-machine fields"
    )]
    pub fn new(
        agent_id: AgentId,
        agent_name: AgentName,
        agent_directory: PathBuf,
        parent_agent_id: Option<AgentId>,
        report_to_user: bool,
        default_config: bool,
        provider: ProviderConfig,
        validator: ValidatorConfig,
        now: UtcDateTimeMs,
    ) -> Self {
        Self {
            agent_id,
            agent_name,
            agent_directory,
            parent_agent_id,
            report_to_user,
            default_config,
            provider,
            agent_prompt: Vec::new(),
            agent_persona: Vec::new(),
            agent_capabilities: Vec::new(),
            validator,
            state: AgentState::Idle,
            created_at: now,
            updated_at: now,
        }
    }

    /// Applies a validated state transition.
    pub fn transition(&mut self, next: AgentState, now: UtcDateTimeMs) -> Result<(), String> {
        if !self.state.can_transition_to(next) {
            return Err(format!(
                "invalid agent state transition: {:?} -> {:?}",
                self.state, next
            ));
        }

        self.state = next;
        self.updated_at = now;
        Ok(())
    }

    /// Adds a prompt binding to the agent.
    pub fn add_prompt(&mut self, prompt: AgentPromptItem, now: UtcDateTimeMs) {
        self.agent_prompt.push(prompt);
        self.updated_at = now;
    }

    /// Adds a persona prompt binding to the agent.
    pub fn add_persona(&mut self, persona: AgentPersonaItem, now: UtcDateTimeMs) {
        self.agent_persona.push(persona);
        self.updated_at = now;
    }

    /// Adds a capability binding to the agent.
    pub fn add_capability(&mut self, capability: AgentCapabilityItem, now: UtcDateTimeMs) {
        self.agent_capabilities.push(capability);
        self.updated_at = now;
    }
}
