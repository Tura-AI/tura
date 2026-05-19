use std::fs;
use std::path::PathBuf;

use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::state_machine::agent_management::{
    AgentCapabilityItem, AgentManagement, AgentPromptItem, ValidatorConfig,
};
use crate::state_machine::session_management::SessionManagement;
use tura_agents::coding_agent::CodingAgent;

const CODING_AGENT_NAME: &str = "coding_agent";
const TOOLS_DIR: &str = "crates/tools/src";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
    pub output_schema: serde_json::Value,
    pub safety: Option<String>,
    #[serde(default)]
    pub language_support: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityPrompt {
    pub name: String,
    pub content: String,
}

#[derive(Debug, Clone, Default)]
pub struct AgentCapabilitiesConfig {
    pub capabilities: Vec<CapabilityDefinition>,
    pub capability_prompts: Vec<CapabilityPrompt>,
}

pub fn load_capability_interface(
    capability_name: &str,
    base_dir: &PathBuf,
) -> Option<CapabilityDefinition> {
    let interface_path = base_dir
        .join(TOOLS_DIR)
        .join(capability_name)
        .join("schema.json");
    if interface_path.exists() {
        let content = fs::read_to_string(&interface_path).ok()?;
        serde_json::from_str::<CapabilityDefinition>(&content).ok()
    } else {
        None
    }
}

pub fn load_capability_prompt(
    capability_name: &str,
    base_dir: &PathBuf,
) -> Option<CapabilityPrompt> {
    let prompt_path = base_dir
        .join(TOOLS_DIR)
        .join(capability_name)
        .join("prompt.md");
    if prompt_path.exists() {
        let content = fs::read_to_string(&prompt_path).ok()?;
        Some(CapabilityPrompt {
            name: capability_name.to_string(),
            content,
        })
    } else {
        None
    }
}

pub fn load_agent_capabilities_config(
    agent_capabilities: &[AgentCapabilityItem],
    base_dir: &PathBuf,
) -> AgentCapabilitiesConfig {
    let mut config = AgentCapabilitiesConfig::default();

    for capability_item in agent_capabilities {
        if let Some(capability_def) =
            load_capability_interface(&capability_item.capability_name, base_dir)
        {
            config.capabilities.push(capability_def);
        }
        if let Some(capability_prompt) =
            load_capability_prompt(&capability_item.capability_name, base_dir)
        {
            config.capability_prompts.push(capability_prompt);
        }
    }

    config
}

pub fn activate_agent(_session: &SessionManagement) -> Result<Vec<AgentManagement>, String> {
    let project_directory = std::env::current_dir()
        .map_err(|err| format!("failed to resolve project directory: {err}"))?;
    let now = Utc::now();
    let coding_capabilities = CodingAgent::capabilities();
    let capability_names = coding_capabilities
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();

    let coding_agent = build_agent(
        CODING_AGENT_NAME,
        true,
        &capability_names,
        &project_directory,
        now,
    );

    Ok(vec![coding_agent])
}

fn build_agent(
    agent_name: &str,
    report_to_user: bool,
    capability_names: &[&str],
    agent_directory: &PathBuf,
    now: chrono::DateTime<Utc>,
) -> AgentManagement {
    let provider = super::provider_config_from_coding_agent(CodingAgent::provider());

    let validator = ValidatorConfig {
        need_validator: false,
        validator_name: None,
    };

    let mut agent = AgentManagement::new(
        generate_agent_id(agent_name),
        agent_name.to_string(),
        agent_directory.clone(),
        None,
        report_to_user,
        provider,
        validator,
        now,
    );

    agent.add_prompt(
        AgentPromptItem {
            agent_prompt: agent_name.to_string(),
            prompt_directory: agent_directory.join("crates/tools/src/modes/code"),
        },
        now,
    );

    for capability_name in capability_names {
        agent.add_capability(
            AgentCapabilityItem {
                capability_name: (*capability_name).to_string(),
                capability_directory: agent_directory.join(TOOLS_DIR),
            },
            now,
        );
    }

    agent
}

pub fn build_agent_with_capabilities(
    agent_name: &str,
    report_to_user: bool,
    capability_items: &[AgentCapabilityItem],
    prompt_items: &[AgentPromptItem],
    agent_directory: &PathBuf,
    _project_directory: &PathBuf,
    now: chrono::DateTime<Utc>,
) -> AgentManagement {
    let provider = super::provider_config_from_coding_agent(CodingAgent::provider());

    let validator = ValidatorConfig {
        need_validator: false,
        validator_name: None,
    };

    let mut agent = AgentManagement::new(
        generate_agent_id(agent_name),
        agent_name.to_string(),
        agent_directory.clone(),
        None,
        report_to_user,
        provider,
        validator,
        now,
    );

    for prompt_item in prompt_items {
        agent.add_prompt(prompt_item.clone(), now);
    }

    for capability_item in capability_items {
        agent.add_capability(capability_item.clone(), now);
    }

    agent
}

fn generate_agent_id(agent_name: &str) -> String {
    format!(
        "{}-{:x}",
        agent_name,
        Utc::now().timestamp_nanos_opt().unwrap_or_default()
    )
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use crate::state_machine::session_management::{SessionInput, SessionManagement};

    use super::*;

    #[test]
    fn coding_agent_preset_includes_development_tools_planning_and_send_message() {
        let session = SessionManagement::new(
            "test-session".to_string(),
            "test-session".to_string(),
            std::env::current_dir().expect("current dir should resolve"),
            false,
            "coding".to_string(),
            SessionInput {
                user_input: "test".to_string(),
                file_input: Vec::new(),
                agent: None,
                runtime_context: None,
            },
            "test".to_string(),
            Utc::now(),
        );

        let agents = activate_agent(&session).expect("agents should activate");
        let expected = CodingAgent::capabilities()
            .into_iter()
            .collect::<HashSet<_>>();

        assert_eq!(agents.len(), 1);
        assert_eq!(agents[0].agent_name, CODING_AGENT_NAME);
        let actual = agents[0]
            .agent_capabilities
            .iter()
            .map(|capability| capability.capability_name.clone())
            .collect::<HashSet<_>>();
        assert_eq!(
            actual, expected,
            "coding_agent should receive configured coding capabilities"
        );
        assert_eq!(actual.len(), 1, "coding_agent exposes only command_run");
        assert!(
            !actual.contains("write_file"),
            "coding_agent should keep write_file behind command_run"
        );
        assert!(
            !actual.contains("apply_diff"),
            "coding_agent should keep apply_diff behind command_run"
        );
        assert!(
            !actual.contains("delete_file"),
            "coding_agent should keep delete_file behind command_run"
        );
        assert!(
            !actual.contains("planning"),
            "planning is not model-visible"
        );
        assert!(
            !actual.contains("send_message_to_user"),
            "send_message_to_user is not model-visible"
        );
        assert!(
            actual.contains("command_run"),
            "coding_agent should include command_run"
        );
    }
}
