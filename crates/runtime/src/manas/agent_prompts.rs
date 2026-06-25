use std::path::{Path, PathBuf};

use crate::state_machine::agent_management::{AgentManagement, AgentPromptItem};

pub(super) fn load_agent_prompt_messages(
    agent: &AgentManagement,
) -> Result<Vec<serde_json::Value>, String> {
    let mut messages = Vec::new();

    for prompt_item in &agent.agent_prompt {
        for prompt_path in ordered_agent_prompt_paths(prompt_item) {
            let content = std::fs::read_to_string(&prompt_path).map_err(|err| {
                format!(
                    "failed to read agent prompt {}: {err}",
                    prompt_path.display()
                )
            })?;
            messages.push(serde_json::json!({
                "role": "system",
                "content": content,
            }));
        }
    }

    Ok(messages)
}

pub(super) fn load_agent_system_prompt_messages(
    agent: &AgentManagement,
) -> Result<Vec<serde_json::Value>, String> {
    let mut messages = load_session_persona_messages(agent)?;
    messages.extend(load_agent_prompt_messages(agent)?);
    Ok(messages)
}

fn ordered_agent_prompt_paths(prompt_item: &AgentPromptItem) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    let standard_path = prompt_item.prompt_directory.join("prompt.md");
    if standard_path.exists() {
        paths.push(standard_path);
        return paths;
    }

    let legacy_path = prompt_item.prompt_directory.join("prompt");
    if legacy_path.exists() {
        paths.push(legacy_path);
        return paths;
    }

    let fallback_path = prompt_item.prompt_directory.join("fallback_agent.md");
    if fallback_path.exists() {
        paths.push(fallback_path);
    }

    paths
}

fn load_session_persona_messages(
    agent: &AgentManagement,
) -> Result<Vec<serde_json::Value>, String> {
    if frontend_source_is_cli() {
        let Some(project_root) = project_root_for_persona(agent) else {
            return Ok(Vec::new());
        };
        if let Some(content) = load_shared_cli_communication_style(&project_root) {
            return Ok(vec![system_message(content)]);
        }
        return Ok(Vec::new());
    }

    let Some(persona_id) = session_persona_id() else {
        return Ok(Vec::new());
    };
    let Some(project_root) = project_root_for_persona(agent) else {
        return Ok(Vec::new());
    };
    let Some(persona) = tura_persona::store::load_persona(&project_root, &persona_id) else {
        return Ok(Vec::new());
    };

    let mut messages = Vec::new();
    if let Some(content) = persona.persona.filter(|value| !value.trim().is_empty()) {
        messages.push(system_message(content));
    }
    if let Some(content) = persona
        .communication_style
        .filter(|value| !value.trim().is_empty())
    {
        messages.push(system_message(content));
    }
    Ok(messages)
}

fn system_message(content: String) -> serde_json::Value {
    serde_json::json!({
        "role": "system",
        "content": content,
    })
}

fn load_shared_cli_communication_style(project_root: &Path) -> Option<String> {
    std::fs::read_to_string(
        project_root
            .join(tura_persona::store::STATIC_PERSONAS_DIR)
            .join(tura_persona::store::COMMUNICATION_STYLE_DIR)
            .join(tura_persona::store::CLI_COMMUNICATION_STYLE_FILE),
    )
    .ok()
    .filter(|value| !value.trim().is_empty())
}

fn session_persona_id() -> Option<String> {
    std::env::var("TURA_SESSION_PERSONA")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn frontend_source_is_cli() -> bool {
    std::env::var("TURA_FRONTEND_SOURCE")
        .ok()
        .map(|value| value.trim().to_ascii_lowercase())
        .is_some_and(|value| value == "cli")
}

fn project_root_for_persona(agent: &AgentManagement) -> Option<PathBuf> {
    if let Some(root) = std::env::var_os("TURA_PROJECT_ROOT") {
        let path = PathBuf::from(root);
        if path.join("personas").exists() {
            return Some(path);
        }
    }
    agent
        .agent_directory
        .ancestors()
        .find(|candidate| has_persona_root(candidate))
        .map(Path::to_path_buf)
}

fn has_persona_root(candidate: &Path) -> bool {
    candidate.join("personas").join("src").is_dir() || candidate.join("personas").is_dir()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state_machine::agent_management::{
        AgentPromptItem, ProviderConfig, ToolChoice, ValidatorConfig,
    };
    use chrono::Utc;
    use std::ffi::OsString;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn loader_includes_configured_session_persona_before_agent_prompt() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|err| err.into_inner());
        let previous_persona = std::env::var_os("TURA_SESSION_PERSONA");
        let previous_root = std::env::var_os("TURA_PROJECT_ROOT");
        let previous_frontend_source = std::env::var_os("TURA_FRONTEND_SOURCE");
        let run_id = format!(
            "tura-session-persona-prompt-test-{}",
            Utc::now().timestamp_nanos_opt().unwrap_or_default()
        );
        let root = std::env::temp_dir().join(run_id);
        let agent_dir = root.join("agents").join("src").join("coding_agent");
        let prompt_dir = root
            .join("personas")
            .join("src")
            .join("guide")
            .join("prompt");
        let shared_style_dir = root
            .join("personas")
            .join("src")
            .join("communication_style");
        std::fs::create_dir_all(&agent_dir).expect("agent prompt dir should be created");
        std::fs::create_dir_all(&prompt_dir).expect("persona prompt dir should be created");
        std::fs::create_dir_all(&shared_style_dir).expect("shared style dir should be created");
        std::fs::write(agent_dir.join("prompt.md"), "agent prompt")
            .expect("agent prompt should be written");
        std::fs::write(prompt_dir.join("persona.md"), "persona prompt")
            .expect("persona prompt should be written");
        std::fs::write(
            shared_style_dir.join("communication_style.md"),
            "communication style prompt",
        )
        .expect("communication style should be written");
        std::fs::write(
            root.join("personas")
                .join("src")
                .join("guide")
                .join("persona_config.json"),
            serde_json::to_string_pretty(&serde_json::json!({
                "persona_name": "guide",
                "display_name": "Guide",
                "description": "Guide persona",
                "short_description": "Guide",
                "default_config": true,
                "persona_directory": "personas/src/guide",
                "prompt_directory": "personas/src/guide/prompt",
                "media": null,
                "metadata": {}
            }))
            .expect("persona config should encode"),
        )
        .expect("persona config should be written");

        std::env::set_var("TURA_SESSION_PERSONA", "guide");
        std::env::set_var("TURA_PROJECT_ROOT", &root);
        std::env::remove_var("TURA_FRONTEND_SOURCE");

        let mut agent = test_agent(&agent_dir, "coding_agent");
        let now = Utc::now();
        agent.add_prompt(
            AgentPromptItem {
                agent_prompt: "coding_agent".to_string(),
                prompt_directory: agent_dir,
            },
            now,
        );

        let messages =
            load_agent_system_prompt_messages(&agent).expect("system prompts should load");
        assert_eq!(
            message_contents(&messages),
            vec![
                "persona prompt",
                "communication style prompt",
                "agent prompt"
            ]
        );

        restore_env("TURA_SESSION_PERSONA", previous_persona);
        restore_env("TURA_PROJECT_ROOT", previous_root);
        restore_env("TURA_FRONTEND_SOURCE", previous_frontend_source);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn cli_loader_skips_persona_and_uses_cli_communication_style() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|err| err.into_inner());
        let previous_persona = std::env::var_os("TURA_SESSION_PERSONA");
        let previous_root = std::env::var_os("TURA_PROJECT_ROOT");
        let previous_frontend_source = std::env::var_os("TURA_FRONTEND_SOURCE");
        let run_id = format!(
            "tura-cli-persona-prompt-test-{}",
            Utc::now().timestamp_nanos_opt().unwrap_or_default()
        );
        let root = std::env::temp_dir().join(run_id);
        let agent_dir = root.join("agents").join("src").join("coding_agent");
        let prompt_dir = root
            .join("personas")
            .join("src")
            .join("guide")
            .join("prompt");
        let shared_style_dir = root
            .join("personas")
            .join("src")
            .join("communication_style");
        std::fs::create_dir_all(&agent_dir).expect("agent prompt dir should be created");
        std::fs::create_dir_all(&prompt_dir).expect("persona prompt dir should be created");
        std::fs::create_dir_all(&shared_style_dir).expect("shared style dir should be created");
        std::fs::write(agent_dir.join("prompt.md"), "agent prompt")
            .expect("agent prompt should be written");
        std::fs::write(prompt_dir.join("persona.md"), "persona prompt")
            .expect("persona prompt should be written");
        std::fs::write(
            shared_style_dir.join("communication_style.md"),
            "communication style prompt",
        )
        .expect("communication style should be written");
        std::fs::write(
            shared_style_dir.join("cli_communication_style.md"),
            "cli communication style prompt",
        )
        .expect("cli communication style should be written");

        std::env::remove_var("TURA_SESSION_PERSONA");
        std::env::set_var("TURA_PROJECT_ROOT", &root);
        std::env::set_var("TURA_FRONTEND_SOURCE", "cli");

        let mut agent = test_agent(&agent_dir, "coding_agent");
        let now = Utc::now();
        agent.add_prompt(
            AgentPromptItem {
                agent_prompt: "coding_agent".to_string(),
                prompt_directory: agent_dir,
            },
            now,
        );

        let messages =
            load_agent_system_prompt_messages(&agent).expect("system prompts should load");
        assert_eq!(
            message_contents(&messages),
            vec!["cli communication style prompt", "agent prompt"]
        );

        restore_env("TURA_SESSION_PERSONA", previous_persona);
        restore_env("TURA_PROJECT_ROOT", previous_root);
        restore_env("TURA_FRONTEND_SOURCE", previous_frontend_source);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn runtime_environment_selects_persona_and_communication_prompts() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|err| err.into_inner());
        let previous_persona = std::env::var_os("TURA_SESSION_PERSONA");
        let previous_root = std::env::var_os("TURA_PROJECT_ROOT");
        let previous_frontend_source = std::env::var_os("TURA_FRONTEND_SOURCE");
        let run_id = format!(
            "tura-persona-environment-prompt-test-{}",
            Utc::now().timestamp_nanos_opt().unwrap_or_default()
        );
        let root = std::env::temp_dir().join(run_id);
        let agent_dir = root.join("agents").join("src").join("coding_agent");
        let prompt_dir = root
            .join("personas")
            .join("src")
            .join("guide")
            .join("prompt");
        let shared_style_dir = root
            .join("personas")
            .join("src")
            .join("communication_style");
        std::fs::create_dir_all(&agent_dir).expect("agent prompt dir should be created");
        std::fs::create_dir_all(&prompt_dir).expect("persona prompt dir should be created");
        std::fs::create_dir_all(&shared_style_dir).expect("shared style dir should be created");
        std::fs::write(agent_dir.join("prompt.md"), "agent prompt")
            .expect("agent prompt should be written");
        std::fs::write(prompt_dir.join("persona.md"), "persona prompt")
            .expect("persona prompt should be written");
        std::fs::write(
            shared_style_dir.join("communication_style.md"),
            "gui communication style",
        )
        .expect("gui communication style should be written");
        std::fs::write(
            shared_style_dir.join("cli_communication_style.md"),
            "cli communication style",
        )
        .expect("cli communication style should be written");
        std::fs::write(
            root.join("personas")
                .join("src")
                .join("guide")
                .join("persona_config.json"),
            serde_json::to_string_pretty(&serde_json::json!({
                "persona_name": "guide",
                "display_name": "Guide",
                "description": "Guide persona",
                "short_description": "Guide",
                "default_config": true,
                "persona_directory": "personas/src/guide",
                "prompt_directory": "personas/src/guide/prompt",
                "media": null,
                "metadata": {}
            }))
            .expect("persona config should encode"),
        )
        .expect("persona config should be written");

        let mut agent = test_agent(&agent_dir, "coding_agent");
        let now = Utc::now();
        agent.add_prompt(
            AgentPromptItem {
                agent_prompt: "coding_agent".to_string(),
                prompt_directory: agent_dir,
            },
            now,
        );

        std::env::set_var("TURA_PROJECT_ROOT", &root);

        std::env::remove_var("TURA_SESSION_PERSONA");
        std::env::remove_var("TURA_FRONTEND_SOURCE");
        assert_eq!(
            message_contents(
                &load_agent_system_prompt_messages(&agent).expect("gui without persona")
            ),
            vec!["agent prompt"]
        );

        std::env::set_var("TURA_SESSION_PERSONA", "guide");
        std::env::remove_var("TURA_FRONTEND_SOURCE");
        assert_eq!(
            message_contents(&load_agent_system_prompt_messages(&agent).expect("gui persona")),
            vec!["persona prompt", "gui communication style", "agent prompt"]
        );

        std::env::remove_var("TURA_SESSION_PERSONA");
        std::env::set_var("TURA_FRONTEND_SOURCE", "cli");
        assert_eq!(
            message_contents(&load_agent_system_prompt_messages(&agent).expect("cli no persona")),
            vec!["cli communication style", "agent prompt"]
        );

        std::env::set_var("TURA_SESSION_PERSONA", "guide");
        std::env::set_var("TURA_FRONTEND_SOURCE", "cli");
        assert_eq!(
            message_contents(&load_agent_system_prompt_messages(&agent).expect("cli persona")),
            vec!["cli communication style", "agent prompt"]
        );

        restore_env("TURA_SESSION_PERSONA", previous_persona);
        restore_env("TURA_PROJECT_ROOT", previous_root);
        restore_env("TURA_FRONTEND_SOURCE", previous_frontend_source);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn loader_uses_legacy_prompt_when_prompt_md_is_absent() {
        let run_id = format!(
            "tura-legacy-agent-prompt-test-{}",
            Utc::now().timestamp_nanos_opt().unwrap_or_default()
        );
        let root = std::env::temp_dir().join(run_id);
        let prompt_dir = root.join("modes").join("code");
        std::fs::create_dir_all(&prompt_dir).expect("legacy prompt dir should be created");
        std::fs::write(prompt_dir.join("prompt"), "legacy prompt")
            .expect("legacy prompt should be written");

        let mut agent = test_agent(&root, "coding_agent");
        let now = Utc::now();
        agent.add_prompt(
            AgentPromptItem {
                agent_prompt: "legacy_agent".to_string(),
                prompt_directory: prompt_dir,
            },
            now,
        );

        let messages = load_agent_prompt_messages(&agent).expect("prompt loading should succeed");
        let contents = message_contents(&messages);

        assert_eq!(contents, vec!["legacy prompt"]);

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn loader_uses_fallback_agent_md_when_prompt_md_and_legacy_prompt_are_absent() {
        let run_id = format!(
            "tura-fallback-agent-prompt-test-{}",
            Utc::now().timestamp_nanos_opt().unwrap_or_default()
        );
        let root = std::env::temp_dir().join(run_id);
        let prompt_dir = root.join("modes").join("code");
        std::fs::create_dir_all(&prompt_dir).expect("fallback prompt dir should be created");
        std::fs::write(
            prompt_dir.join("fallback_agent.md"),
            "fallback agent prompt",
        )
        .expect("fallback prompt should be written");

        let mut agent = test_agent(&root, "coding_agent");
        let now = Utc::now();
        agent.add_prompt(
            AgentPromptItem {
                agent_prompt: "fallback_agent".to_string(),
                prompt_directory: prompt_dir,
            },
            now,
        );

        let messages = load_agent_prompt_messages(&agent).expect("prompt loading should succeed");
        let contents = message_contents(&messages);

        assert_eq!(contents, vec!["fallback agent prompt"]);

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn loader_does_not_read_command_prompt_files_from_tool_directories() {
        let run_id = format!(
            "tura-agent-prompt-command-isolation-test-{}",
            Utc::now().timestamp_nanos_opt().unwrap_or_default()
        );
        let root = std::env::temp_dir().join(run_id);
        let agent_prompt_dir = root.join("agents").join("coding_agent");
        let command_prompt_dir = root
            .join("crates")
            .join("tools")
            .join("src")
            .join("command_run");
        std::fs::create_dir_all(&agent_prompt_dir).expect("agent prompt dir should be created");
        std::fs::create_dir_all(&command_prompt_dir).expect("command prompt dir should be created");
        std::fs::write(agent_prompt_dir.join("prompt.md"), "agent prompt")
            .expect("agent prompt should be written");
        std::fs::write(
            command_prompt_dir.join("prompt.md"),
            "common command_run prompt",
        )
        .expect("command prompt should be written");

        let mut agent = test_agent(&root, "coding_agent");
        let now = Utc::now();
        agent.add_prompt(
            AgentPromptItem {
                agent_prompt: "coding_agent".to_string(),
                prompt_directory: agent_prompt_dir,
            },
            now,
        );

        let messages = load_agent_prompt_messages(&agent).expect("prompt loading should succeed");
        let joined = message_contents(&messages).join("\n");

        assert!(joined.contains("agent prompt"));
        assert!(!joined.contains("common command_run prompt"));

        let _ = std::fs::remove_dir_all(root);
    }

    fn test_agent(root: &std::path::Path, name: &str) -> AgentManagement {
        AgentManagement::new(
            "agent".to_string(),
            name.to_string(),
            root.to_path_buf(),
            None,
            true,
            false,
            false,
            ProviderConfig {
                tura_llm_name: "test".to_string(),
                default_model_tier: None,
                current_model: None,
                stream: false,
                temperature: 0.0,
                max_tokens: 0,
                tool_choice: ToolChoice::Auto,
                time_out_ms: 1000,
            },
            ValidatorConfig {
                need_validator: false,
                validator_name: None,
            },
            Utc::now(),
        )
    }

    fn message_contents(messages: &[serde_json::Value]) -> Vec<&str> {
        messages
            .iter()
            .filter_map(|message| message.get("content").and_then(|content| content.as_str()))
            .collect()
    }

    fn restore_env(key: &str, value: Option<OsString>) {
        if let Some(value) = value {
            std::env::set_var(key, value);
        } else {
            std::env::remove_var(key);
        }
    }
}

#[cfg(test)]
mod prompt_resource_tests {
    use super::super::constants::COMMAND_RUN_TOOL;
    use super::load_agent_prompt_messages;
    use crate::state_machine::agent_management::{
        AgentCapabilityItem, AgentManagement, AgentPromptItem, ProviderConfig, ToolChoice,
        ValidatorConfig,
    };
    #[test]
    fn prompt_loading_only_includes_agent_prompt() {
        let now = chrono::Utc::now();
        let unique = format!(
            "mano-prompt-test-{:x}",
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default()
        );
        let root = std::env::temp_dir().join(unique);
        let agent_prompt_dir = root.join("agent-prompts");
        let tool_dir = root.join("tools");
        std::fs::create_dir_all(&agent_prompt_dir).expect("agent prompt dir should be created");
        std::fs::write(agent_prompt_dir.join("prompt.md"), "agent prompt")
            .expect("agent prompt should be written");

        let provider = ProviderConfig {
            tura_llm_name: "test".to_string(),
            default_model_tier: None,
            current_model: None,
            stream: false,
            temperature: 0.0,
            max_tokens: 0,
            tool_choice: ToolChoice::Auto,
            time_out_ms: 1_000,
        };
        let validator = ValidatorConfig {
            need_validator: false,
            validator_name: None,
        };
        let mut agent = AgentManagement::new(
            "agent-id".to_string(),
            "agent".to_string(),
            root.clone(),
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
                agent_prompt: "agent".to_string(),
                prompt_directory: agent_prompt_dir,
            },
            now,
        );
        agent.add_capability(
            AgentCapabilityItem {
                capability_name: COMMAND_RUN_TOOL.to_string(),
                capability_directory: tool_dir,
            },
            now,
        );

        let messages = load_agent_prompt_messages(&agent).expect("prompt loading should succeed");
        let content = messages
            .iter()
            .filter_map(|message| message.get("content").and_then(|content| content.as_str()))
            .collect::<Vec<_>>()
            .join("\n");

        assert!(content.contains("agent prompt"));
        assert!(!content.contains("command_run prompt"));

        let _ = std::fs::remove_dir_all(root);
    }
}
