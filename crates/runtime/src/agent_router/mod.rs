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
const DEFAULT_CODING_AGENT_NAME: &str = "balanced";

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
        create_coding_agent(
            &project_directory,
            &capabilities_directory,
            &coding_prompts_directory,
        )?
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
        "thoughtful"
        | "thinking-planning"
        | "coding_agent_planning"
        | "coding_agent"
        | "coding"
        | "balanced"
        | "thinking"
        | "direct"
        | "fast"
        | "direct-text-only"
        | "fast-text-only"
        | "coding_agent_fast"
        | "coding_agent_thinking" => {
            let canonical_name = match agent_name {
                "thinking-planning" | "coding_agent_planning" | "coding_agent" | "coding" => {
                    "thoughtful"
                }
                "thinking" | "coding_agent_thinking" => "balanced",
                "fast" | "coding_agent_fast" => "direct",
                "fast-text-only" => "direct-text-only",
                other => other,
            };
            if let Some(agent) = load_agent_from_registry(project_directory, canonical_name)? {
                Ok(agent)
            } else {
                let mut agent = create_coding_agent(
                    project_directory,
                    capabilities_directory,
                    prompts_directory,
                )?;
                agent.agent_id = generate_agent_id(canonical_name);
                agent.agent_name = canonical_name.to_string();
                agent.reflection = canonical_name == "thoughtful";
                agent.agent_prompt.clear();
                agent.add_prompt(
                    AgentPromptItem {
                        agent_prompt: canonical_name.to_string(),
                        prompt_directory: prompts_directory.to_path_buf(),
                    },
                    Utc::now(),
                );
                if canonical_name == "balanced" || canonical_name == "thoughtful" {
                    agent
                        .agent_capabilities
                        .retain(|capability| capability.capability_name != "planning");
                }
                if canonical_name == "direct" || canonical_name == "direct-text-only" {
                    agent.provider.tura_llm_name = "fast".to_string();
                    agent.provider.default_model_tier = Some("fast".to_string());
                }
                if canonical_name == "direct-text-only" {
                    agent
                        .agent_capabilities
                        .retain(|capability| capability.capability_name != "read_media");
                }
                Ok(agent)
            }
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
    if let Some(agent) = load_agent_from_registry(project_directory, DEFAULT_CODING_AGENT_NAME)? {
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
        DEFAULT_CODING_AGENT_NAME.to_string(),
        project_directory.to_path_buf(),
        None,
        true,
        false,
        false,
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
    _capabilities_directory: &Path,
    prompts_directory: &Path,
) -> Result<AgentManagement, String> {
    if let Some(agent) = load_agent_from_registry(project_directory, "general")? {
        return Ok(agent);
    }

    let now = Utc::now();
    let provider = ProviderConfig {
        tura_llm_name: "fast".to_string(),
        default_model_tier: Some("fast".to_string()),
        current_model: None,
        stream: true,
        temperature: 0.7,
        max_tokens: 0,
        tool_choice: ToolChoice::Auto,
        time_out_ms: 120_000,
    };
    let validator = ValidatorConfig {
        need_validator: true,
        validator_name: Some("thinking".to_string()),
    };

    let mut agent = AgentManagement::new(
        generate_agent_id("general"),
        "general".to_string(),
        project_directory.to_path_buf(),
        None,
        true,
        false,
        false,
        provider,
        validator,
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
        default_model_tier: config.default_model_tier,
        current_model: config.current_model,
        stream: true,
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
    #[serde(default)]
    default_config: bool,
    #[serde(default)]
    reflection: bool,
    provider: ProviderConfig,
    agent_prompt: Vec<AgentPromptItem>,
    agent_capabilities: Vec<AgentCapabilityItem>,
    validator: ValidatorConfig,
}

fn load_agent_from_registry(
    project_directory: &Path,
    agent_name: &str,
) -> Result<Option<AgentManagement>, String> {
    if let Some(agent) = load_agent_from_router_spec(project_directory, agent_name)? {
        return Ok(Some(agent));
    }

    if let Some(agent) = load_agent_from_agent_store(project_directory, agent_name)? {
        return Ok(Some(agent));
    }

    Ok(None)
}

fn load_agent_from_agent_store(
    project_directory: &Path,
    agent_name: &str,
) -> Result<Option<AgentManagement>, String> {
    let Some(stored_agent) = tura_agents::store::load_agent(project_directory, agent_name) else {
        return Ok(None);
    };
    let config = serde_json::to_value(stored_agent.config)
        .map_err(|err| format!("failed to encode stored agent `{agent_name}` config: {err}"))?;
    let entry: AgentRegistryEntry = serde_json::from_value(config)
        .map_err(|err| format!("failed to parse stored agent `{agent_name}` config: {err}"))?;
    build_agent_from_registry_entry(project_directory, entry).map(Some)
}

fn load_agent_from_router_spec(
    project_directory: &Path,
    agent_name: &str,
) -> Result<Option<AgentManagement>, String> {
    let Ok(raw) = std::env::var("TURA_ROUTER_AGENT_SPEC") else {
        return Ok(None);
    };
    let value: serde_json::Value = serde_json::from_str(&raw)
        .map_err(|err| format!("failed to parse router agent spec: {err}"))?;
    let spec_name = value
        .get("agent_name")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    if !spec_name.eq_ignore_ascii_case(agent_name) {
        return Ok(None);
    }
    let Some(config) = value.get("config").cloned() else {
        return Ok(None);
    };
    let entry: AgentRegistryEntry = serde_json::from_value(config)
        .map_err(|err| format!("failed to parse router agent `{agent_name}` config: {err}"))?;
    build_agent_from_registry_entry(project_directory, entry).map(Some)
}

fn build_agent_from_registry_entry(
    project_directory: &Path,
    entry: AgentRegistryEntry,
) -> Result<AgentManagement, String> {
    let now = Utc::now();
    let mut agent = AgentManagement::new(
        generate_agent_id(&entry.agent_name),
        entry.agent_name,
        resolve_project_path(project_directory, entry.agent_directory),
        entry.parent_agent_id,
        entry.report_to_user,
        entry.default_config,
        entry.reflection,
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

    Ok(agent)
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
            crate::state_machine::session_management::SessionState::Interrupted => {
                agent.state = AgentState::Failed;
            }
        }
        agent.updated_at = now;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        build_agent_from_registry_entry, initialize_agent_state_machine,
        provider_config_from_coding_agent, resolve_project_path, AgentRegistryEntry,
    };
    use crate::state_machine::agent_management::{
        AgentCapabilityItem, AgentManagement, AgentPromptItem, AgentState, ProviderConfig,
        ToolChoice, ValidatorConfig,
    };
    use crate::state_machine::session_management::{SessionInput, SessionManagement, SessionState};
    use chrono::Utc;
    use std::path::{Path, PathBuf};
    use tura_agents::coding_agent::{CodingAgentProviderConfig, CodingAgentToolChoice};

    fn provider_config() -> ProviderConfig {
        ProviderConfig {
            tura_llm_name: "fast".to_string(),
            default_model_tier: Some("fast".to_string()),
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

    fn agent(name: &str) -> AgentManagement {
        let now = Utc::now();
        AgentManagement::new(
            format!("{name}-id"),
            name.to_string(),
            PathBuf::from("agents").join(name),
            None,
            true,
            false,
            false,
            provider_config(),
            validator_config(),
            now,
        )
    }

    fn session_in_state(state: SessionState) -> SessionManagement {
        let now = Utc::now();
        let mut session = SessionManagement::new(
            "session-agent-router".to_string(),
            "Agent Router".to_string(),
            PathBuf::from("workspace"),
            false,
            "coding".to_string(),
            SessionInput {
                user_input: "hello".to_string(),
                file_input: Vec::new(),
                agent: None,
                runtime_context: None,
                planning_mode_override: None,
            },
            "goal".to_string(),
            now,
        );
        session.state = state;
        session
    }

    #[test]
    fn provider_config_from_coding_agent_maps_tool_choice_and_keeps_timeout() {
        for (choice, expected) in [
            (CodingAgentToolChoice::Auto, ToolChoice::Auto),
            (CodingAgentToolChoice::Strict, ToolChoice::Strict),
            (CodingAgentToolChoice::Disable, ToolChoice::Disable),
        ] {
            let config = provider_config_from_coding_agent(CodingAgentProviderConfig {
                tura_llm_name: "thinking".to_string(),
                default_model_tier: Some("thinking".to_string()),
                current_model: None,
                stream: false,
                temperature: 0.3,
                max_tokens: 4096,
                tool_choice: choice,
                time_out_ms: 1234,
            });

            assert_eq!(config.tura_llm_name, "thinking");
            assert_eq!(config.default_model_tier.as_deref(), Some("thinking"));
            assert!(
                config.stream,
                "runtime provider config must force streaming"
            );
            assert_eq!(config.temperature, 0.3);
            assert_eq!(config.max_tokens, 4096);
            assert_eq!(config.tool_choice, expected);
            assert_eq!(config.time_out_ms, 1234);
        }
    }

    #[test]
    fn registry_entry_build_resolves_relative_paths_without_persona_binding() {
        let project = Path::new("C:/repo/tura");
        let entry = AgentRegistryEntry {
            agent_name: "custom".to_string(),
            agent_directory: PathBuf::from("agents/custom"),
            parent_agent_id: Some("parent".to_string()),
            report_to_user: false,
            default_config: true,
            reflection: false,
            provider: provider_config(),
            agent_prompt: vec![AgentPromptItem {
                agent_prompt: "prompt".to_string(),
                prompt_directory: PathBuf::from("prompts/custom"),
            }],
            agent_capabilities: vec![AgentCapabilityItem {
                capability_name: "command_run".to_string(),
                capability_directory: PathBuf::from("crates/tools/src"),
            }],
            validator: validator_config(),
        };

        let agent =
            build_agent_from_registry_entry(project, entry).expect("registry entry should build");

        assert_eq!(agent.agent_name, "custom");
        assert_eq!(agent.parent_agent_id.as_deref(), Some("parent"));
        assert!(!agent.report_to_user);
        assert!(agent.default_config);
        assert_eq!(agent.agent_directory, project.join("agents/custom"));
        assert_eq!(
            agent.agent_prompt[0].prompt_directory,
            project.join("prompts/custom")
        );
        assert_eq!(
            agent.agent_capabilities[0].capability_directory,
            project.join("crates/tools/src")
        );
    }

    #[test]
    fn registry_entry_build_maps_reflection_flag() {
        let project = Path::new("C:/repo/tura");
        let entry = AgentRegistryEntry {
            agent_name: "reflective".to_string(),
            agent_directory: PathBuf::from("agents/reflective"),
            parent_agent_id: None,
            report_to_user: true,
            default_config: false,
            reflection: true,
            provider: provider_config(),
            agent_prompt: Vec::new(),
            agent_capabilities: Vec::new(),
            validator: validator_config(),
        };

        let agent =
            build_agent_from_registry_entry(project, entry).expect("registry entry should build");

        assert!(agent.reflection);
    }

    #[test]
    fn project_path_resolution_keeps_absolute_paths_and_roots_relative_paths() {
        let project = Path::new("C:/repo/tura");
        let absolute = if cfg!(windows) {
            PathBuf::from("C:/external/agent")
        } else {
            PathBuf::from("/external/agent")
        };

        assert_eq!(
            resolve_project_path(project, PathBuf::from("agents/custom")),
            project.join("agents/custom")
        );
        assert_eq!(resolve_project_path(project, absolute.clone()), absolute);
    }

    #[test]
    fn initialize_agent_state_machine_maps_session_states_to_agent_states() {
        for (session_state, expected_agent_state) in [
            (SessionState::Created, AgentState::Idle),
            (SessionState::Running, AgentState::Idle),
            (SessionState::Paused, AgentState::Waiting),
            (SessionState::Completed, AgentState::Completed),
            (SessionState::Failed, AgentState::Failed),
            (SessionState::Cancelled, AgentState::Failed),
            (SessionState::Interrupted, AgentState::Failed),
        ] {
            let mut agents = vec![agent("agent-a"), agent("agent-b")];
            let session = session_in_state(session_state);

            initialize_agent_state_machine(&mut agents, &session)
                .expect("agent state initialization should succeed");

            assert!(
                agents
                    .iter()
                    .all(|agent| agent.state == expected_agent_state),
                "session state {session_state:?} should map all agents to {expected_agent_state:?}"
            );
        }
    }
}
