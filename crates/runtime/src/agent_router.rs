mod activate_agent;

use crate::state_machine::agent_management::{
    AgentCapabilityItem, AgentManagement, AgentPromptItem, AgentState, ProviderConfig, ToolChoice,
    ValidatorConfig,
};
use crate::state_machine::session_management::SessionManagement;
use chrono::Utc;
use serde::Deserialize;
use std::path::{Path, PathBuf};
use tura_agents::coding_agent::{CodingAgent, CodingAgentProviderConfig, CodingAgentToolChoice};

const PROJECT_ROOT_ENV: &str = "TURA_PROJECT_ROOT";

pub use activate_agent::{
    activate_agent, build_agent_with_capabilities, load_agent_capabilities_config,
    load_capability_interface, load_capability_prompt, AgentCapabilitiesConfig,
    CapabilityDefinition, CapabilityPrompt,
};

pub fn activate_agent_with_loader(
    session: &SessionManagement,
    agent_loader: fn(&SessionManagement) -> Result<Vec<AgentManagement>, String>,
) -> Result<Vec<AgentManagement>, String> {
    agent_loader(session)
}

pub fn activate_agents_by_session_type(
    session: &SessionManagement,
) -> Result<Vec<AgentManagement>, String> {
    let project_directory = project_directory_with_agent_registry()?;

    let capabilities_directory = project_directory.join("crates").join("tools").join("src");
    let coding_prompts_directory = capabilities_directory.join("modes").join("code");

    let mut agents = Vec::new();
    let agent = if let Some(agent_name) = session.input.agent.as_deref() {
        create_agent_by_name(
            agent_name,
            &project_directory,
            &capabilities_directory,
            &coding_prompts_directory,
        )?
    } else {
        match session.session_topic.as_str() {
            "coding" | "programming" | "development" | "testing" => create_coding_agent(
                &project_directory,
                &capabilities_directory,
                &coding_prompts_directory,
            )?,
            "general" => create_general_agent(
                &project_directory,
                &capabilities_directory,
                &coding_prompts_directory,
            )?,
            _ => create_general_agent(
                &project_directory,
                &capabilities_directory,
                &coding_prompts_directory,
            )?,
        }
    };
    agents.push(agent);
    Ok(agents)
}

fn create_agent_by_name(
    agent_name: &str,
    project_directory: &Path,
    capabilities_directory: &Path,
    prompts_directory: &Path,
) -> Result<AgentManagement, String> {
    if let Some(agent) = load_agent_from_registry(project_directory, agent_name)? {
        return Ok(agent);
    }
    match agent_name {
        "coding_agent" | "coding" => {
            create_coding_agent(project_directory, capabilities_directory, prompts_directory)
        }
        "general" | "general_agent" => {
            create_general_agent(project_directory, capabilities_directory, prompts_directory)
        }
        other => Err(format!("unknown agent `{other}`")),
    }
}

fn create_coding_agent(
    project_directory: &Path,
    capabilities_directory: &Path,
    prompts_directory: &Path,
) -> Result<AgentManagement, String> {
    if let Some(agent) = load_agent_from_registry(project_directory, "coding_agent")? {
        return Ok(agent);
    }

    let now = Utc::now();
    let provider = provider_config_from_coding_agent(CodingAgent::provider());

    let validator = ValidatorConfig {
        need_validator: false,
        validator_name: None,
    };

    let mut agent = AgentManagement::new(
        format!(
            "coding-agent-{:x}",
            Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ),
        CodingAgent::name().to_string(),
        project_directory.to_path_buf(),
        None,
        true,
        provider,
        validator,
        now,
    );
    for capability_name in CodingAgent::capabilities() {
        agent.add_capability(
            AgentCapabilityItem {
                capability_name,
                capability_directory: capabilities_directory.to_path_buf(),
            },
            now,
        );
    }

    for prompt_name in CodingAgent::prompts() {
        agent.add_prompt(
            AgentPromptItem {
                agent_prompt: prompt_name,
                prompt_directory: prompts_directory.to_path_buf(),
            },
            now,
        );
    }

    Ok(agent)
}

fn create_general_agent(
    project_directory: &Path,
    capabilities_directory: &Path,
    prompts_directory: &Path,
) -> Result<AgentManagement, String> {
    if let Some(agent) = load_agent_from_registry(project_directory, "general")? {
        return Ok(agent);
    }

    let now = Utc::now();
    let provider = ProviderConfig {
        tura_llm_name: "tura_general".to_string(),
        stream: false,
        temperature: 0.7,
        max_tokens: 0,
        tool_choice: ToolChoice::Auto,
        time_out_ms: 120_000,
    };
    let validator = ValidatorConfig {
        need_validator: true,
        validator_name: Some("tura_validator".to_string()),
    };

    let mut agent = AgentManagement::new(
        generate_agent_id("general"),
        "general".to_string(),
        project_directory.to_path_buf(),
        None,
        true,
        provider,
        validator,
        now,
    );
    agent.add_capability(
        AgentCapabilityItem {
            capability_name: "command_run".to_string(),
            capability_directory: capabilities_directory.to_path_buf(),
        },
        now,
    );

    agent.add_prompt(
        AgentPromptItem {
            agent_prompt: "general".to_string(),
            prompt_directory: prompts_directory.to_path_buf(),
        },
        now,
    );

    Ok(agent)
}

pub fn coding_agent_provider_name() -> String {
    CodingAgent::provider().tura_llm_name
}

pub(crate) fn provider_config_from_coding_agent(
    config: CodingAgentProviderConfig,
) -> ProviderConfig {
    ProviderConfig {
        tura_llm_name: config.tura_llm_name,
        stream: config.stream,
        temperature: config.temperature,
        max_tokens: config.max_tokens,
        tool_choice: match config.tool_choice {
            CodingAgentToolChoice::Auto => ToolChoice::Auto,
            CodingAgentToolChoice::Strict => ToolChoice::Strict,
            CodingAgentToolChoice::Disable => ToolChoice::Disable,
        },
        time_out_ms: config.time_out_ms,
    }
}

#[derive(Debug, Clone, Deserialize)]
struct AgentRegistryEntry {
    agent_name: String,
    agent_directory: PathBuf,
    parent_agent_id: Option<String>,
    report_to_user: bool,
    provider: ProviderConfig,
    agent_prompt: Vec<AgentPromptItem>,
    agent_capabilities: Vec<AgentCapabilityItem>,
    validator: ValidatorConfig,
}

fn load_agent_from_registry(
    project_directory: &Path,
    agent_name: &str,
) -> Result<Option<AgentManagement>, String> {
    let standard_registry_path = project_directory
        .join("crates")
        .join("agents")
        .join("src")
        .join(agent_name)
        .join("agent_config.json");
    let legacy_standard_registry_path = project_directory
        .join("crates")
        .join("agents")
        .join("src")
        .join(agent_name)
        .join("agent_config");
    let crate_registry_path = project_directory
        .join("crates")
        .join("agents")
        .join(agent_name)
        .join("interface")
        .join(format!("I{}.json", agent_name));
    let registry_path = if standard_registry_path.exists() {
        standard_registry_path
    } else if legacy_standard_registry_path.exists() {
        legacy_standard_registry_path
    } else if crate_registry_path.exists() {
        crate_registry_path
    } else {
        project_directory
            .join("agents")
            .join("interface")
            .join(format!("I{}.json", agent_name))
    };

    if !registry_path.exists() {
        return Ok(None);
    }

    let content = std::fs::read_to_string(&registry_path).map_err(|err| {
        format!(
            "failed to read agent registry {}: {err}",
            registry_path.display()
        )
    })?;
    let entry: AgentRegistryEntry = serde_json::from_str(&content).map_err(|err| {
        format!(
            "failed to parse agent registry {}: {err}",
            registry_path.display()
        )
    })?;

    let now = Utc::now();
    let mut agent = AgentManagement::new(
        generate_agent_id(&entry.agent_name),
        entry.agent_name,
        resolve_project_path(project_directory, entry.agent_directory),
        entry.parent_agent_id,
        entry.report_to_user,
        entry.provider,
        entry.validator,
        now,
    );

    for prompt in entry.agent_prompt {
        agent.add_prompt(
            AgentPromptItem {
                agent_prompt: prompt.agent_prompt,
                prompt_directory: resolve_project_path(project_directory, prompt.prompt_directory),
            },
            now,
        );
    }

    for capability in entry.agent_capabilities {
        agent.add_capability(
            AgentCapabilityItem {
                capability_name: capability.capability_name,
                capability_directory: resolve_project_path(
                    project_directory,
                    capability.capability_directory,
                ),
            },
            now,
        );
    }

    Ok(Some(agent))
}

fn resolve_project_path(project_directory: &Path, path: PathBuf) -> PathBuf {
    if path.is_absolute() {
        path
    } else {
        project_directory.join(path)
    }
}

fn project_directory_with_agent_registry() -> Result<PathBuf, String> {
    if let Ok(root) = std::env::var(PROJECT_ROOT_ENV) {
        let root = PathBuf::from(root);
        if tura_project_root_is_valid(&root) {
            return Ok(root);
        }
    }

    let current = std::env::current_dir()
        .map_err(|err| format!("failed to resolve project directory: {err}"))?;
    for candidate in current.ancestors() {
        if tura_project_root_is_valid(candidate) {
            return Ok(candidate.to_path_buf());
        }
    }
    Ok(current)
}

fn tura_project_root_is_valid(path: &Path) -> bool {
    path.join("crates")
        .join("tools")
        .join("src")
        .join("command_run")
        .join("schema.json")
        .exists()
}

fn generate_agent_id(agent_name: &str) -> String {
    format!(
        "{}-{:x}",
        agent_name,
        Utc::now().timestamp_nanos_opt().unwrap_or_default()
    )
}

pub fn initialize_agent_state_machine(
    agents: &mut [AgentManagement],
    session: &SessionManagement,
) -> Result<(), String> {
    let now = chrono::Utc::now();

    for agent in agents.iter_mut() {
        match session.state {
            crate::state_machine::session_management::SessionState::Created => {
                agent.state = AgentState::Idle;
            }
            crate::state_machine::session_management::SessionState::Running => {
                agent.state = AgentState::Idle;
            }
            crate::state_machine::session_management::SessionState::Paused => {
                agent.state = AgentState::Waiting;
            }
            crate::state_machine::session_management::SessionState::Completed => {
                agent.state = AgentState::Completed;
            }
            crate::state_machine::session_management::SessionState::Failed => {
                agent.state = AgentState::Failed;
            }
            crate::state_machine::session_management::SessionState::Cancelled => {
                agent.state = AgentState::Failed;
            }
        }
        agent.updated_at = now;
    }

    Ok(())
}
