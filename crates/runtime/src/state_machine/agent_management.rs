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

/// One command capability resource attached to an agent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentCapabilityItem {
    /// Natural-language capability name.
    pub capability_name: CapabilityName,
    /// Absolute path to the capability directory.
    #[serde(default = "default_capability_directory")]
    pub capability_directory: PathBuf,
}

fn default_capability_directory() -> PathBuf {
    PathBuf::from("crates/tools/src")
}

/// LLM/provider configuration used by the agent.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProviderConfig {
    /// Legacy internal LLM route name. Prefer `default_model_tier` for new agent config.
    pub tura_llm_name: String,
    /// Default model tier used when the agent has no explicit current model.
    #[serde(default)]
    pub default_model_tier: Option<String>,
    /// Explicit provider/model selection, written as `provider/model`.
    #[serde(default)]
    pub current_model: Option<String>,
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
    /// Whether this agent receives reflective task-status prompt style.
    pub reflection: bool,
    /// Provider/LLM configuration.
    pub provider: ProviderConfig,
    /// Prompts bound to this agent.
    pub agent_prompt: Vec<AgentPromptItem>,
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
        reflection: bool,
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
            reflection,
            provider,
            agent_prompt: Vec::new(),
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

    /// Adds a capability binding to the agent.
    pub fn add_capability(&mut self, capability: AgentCapabilityItem, now: UtcDateTimeMs) {
        self.agent_capabilities.push(capability);
        self.updated_at = now;
    }
}

#[cfg(test)]
mod tests {
    use super::{AgentCapabilityItem, AgentManagement, AgentPromptItem};
    use super::{AgentState, ProviderConfig, ToolChoice, ValidatorConfig};
    use chrono::{Duration, Utc};
    use std::path::PathBuf;

    fn provider_config() -> ProviderConfig {
        ProviderConfig {
            tura_llm_name: "fast".to_string(),
            default_model_tier: None,
            current_model: None,
            stream: true,
            temperature: 0.0,
            max_tokens: 1024,
            tool_choice: ToolChoice::Auto,
            time_out_ms: 30_000,
        }
    }

    fn validator_config() -> ValidatorConfig {
        ValidatorConfig {
            need_validator: false,
            validator_name: None,
        }
    }

    fn agent() -> AgentManagement {
        let now = Utc::now();
        AgentManagement::new(
            "agent-test".to_string(),
            "Test Agent".to_string(),
            PathBuf::from("agents/test"),
            None,
            true,
            false,
            false,
            provider_config(),
            validator_config(),
            now,
        )
    }

    #[test]
    fn agent_state_transition_matrix_rejects_illegal_and_terminal_edges() {
        use AgentState::*;

        let states = [Idle, Running, Waiting, Completed, Failed];
        for from in states {
            for to in states {
                let expected = matches!(
                    (from, to),
                    (Idle, Idle | Running)
                        | (Running, Running | Waiting | Completed | Failed)
                        | (Waiting, Waiting | Running | Failed)
                );
                assert_eq!(
                    from.can_transition_to(to),
                    expected,
                    "unexpected AgentState transition verdict for {from:?} -> {to:?}"
                );
            }
        }
    }

    #[test]
    fn agent_transition_updates_state_and_timestamp_only_when_valid() {
        let mut agent = agent();
        let created_at = agent.created_at;
        let running_at = created_at + Duration::seconds(1);
        let completed_at = running_at + Duration::seconds(1);

        agent
            .transition(AgentState::Running, running_at)
            .expect("Idle -> Running should succeed");
        assert_eq!(agent.state, AgentState::Running);
        assert_eq!(agent.updated_at, running_at);

        let invalid = agent
            .transition(AgentState::Idle, completed_at)
            .expect_err("Running -> Idle should be rejected");
        assert!(invalid.contains("Running -> Idle"));
        assert_eq!(agent.state, AgentState::Running);
        assert_eq!(agent.updated_at, running_at);

        agent
            .transition(AgentState::Completed, completed_at)
            .expect("Running -> Completed should succeed");
        assert_eq!(agent.state, AgentState::Completed);
        assert_eq!(agent.updated_at, completed_at);

        let terminal = agent
            .transition(AgentState::Running, completed_at + Duration::seconds(1))
            .expect_err("Completed should be terminal");
        assert!(terminal.contains("Completed -> Running"));
    }

    #[test]
    fn agent_binding_mutators_append_items_and_bump_timestamp() {
        let mut agent = agent();
        let prompt_at = agent.created_at + Duration::milliseconds(1);
        let capability_at = prompt_at + Duration::milliseconds(1);

        agent.add_prompt(
            AgentPromptItem {
                agent_prompt: "coding".to_string(),
                prompt_directory: PathBuf::from("prompts/coding"),
            },
            prompt_at,
        );
        agent.add_capability(
            AgentCapabilityItem {
                capability_name: "command_run".to_string(),
                capability_directory: PathBuf::from("capabilities/command_run"),
            },
            capability_at,
        );

        assert_eq!(agent.agent_prompt.len(), 1);
        assert_eq!(agent.agent_capabilities.len(), 1);
        assert_eq!(agent.updated_at, capability_at);
    }
}
