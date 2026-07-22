use lifecycle::{AgentId, AgentName, ProviderConfig};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// One prompt resource attached to an agent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentPromptItem {
    /// Natural-language prompt name.
    pub agent_prompt: String,
    /// Absolute path to the prompt directory.
    pub prompt_directory: PathBuf,
}

/// One command capability resource attached to an agent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentCapabilityItem {
    /// Natural-language capability name.
    pub capability_name: String,
    /// Absolute path to the capability directory.
    #[serde(default = "default_capability_directory")]
    pub capability_directory: PathBuf,
}

fn default_capability_directory() -> PathBuf {
    PathBuf::from("crates/tools/src")
}

/// Validation configuration for an agent deliverable.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidatorConfig {
    /// Whether the agent output must be validated.
    pub need_validator: bool,
    /// Validator name.
    pub validator_name: Option<String>,
}

fn default_op_manual() -> bool {
    true
}

/// Runtime configuration for one agent.
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
    /// Whether this agent receives operation manuals when task types are active.
    #[serde(default = "default_op_manual")]
    pub op_manual: bool,
    /// Whether this agent receives per-turn self-reflection tail prompts.
    #[serde(default)]
    pub self_reflection: bool,
    /// Provider/LLM configuration.
    pub provider: ProviderConfig,
    /// Prompts bound to this agent.
    pub agent_prompt: Vec<AgentPromptItem>,
    /// Capabilities bound to this agent.
    pub agent_capabilities: Vec<AgentCapabilityItem>,
    /// Validator configuration.
    pub validator: ValidatorConfig,
}

impl AgentManagement {
    /// Creates an agent configuration with no prompt or capability bindings.
    #[expect(
        clippy::too_many_arguments,
        reason = "agent construction mirrors the serialized configuration fields"
    )]
    pub fn new(
        agent_id: AgentId,
        agent_name: AgentName,
        agent_directory: PathBuf,
        parent_agent_id: Option<AgentId>,
        report_to_user: bool,
        default_config: bool,
        reflection: bool,
        self_reflection: bool,
        provider: ProviderConfig,
        validator: ValidatorConfig,
    ) -> Self {
        Self {
            agent_id,
            agent_name,
            agent_directory,
            parent_agent_id,
            report_to_user,
            default_config,
            reflection,
            op_manual: true,
            self_reflection,
            provider,
            agent_prompt: Vec::new(),
            agent_capabilities: Vec::new(),
            validator,
        }
    }

    pub fn add_prompt(&mut self, prompt: AgentPromptItem) {
        self.agent_prompt.push(prompt);
    }

    pub fn add_capability(&mut self, capability: AgentCapabilityItem) {
        self.agent_capabilities.push(capability);
    }
}
