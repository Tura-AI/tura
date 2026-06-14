use std::path::PathBuf;

use crate::state_machine::agent_management::{AgentManagement, AgentPersonaItem, AgentPromptItem};

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
    let mut messages = load_agent_persona_messages(agent)?;
    messages.extend(load_agent_prompt_messages(agent)?);
    Ok(messages)
}

pub(super) fn load_agent_persona_messages(
    agent: &AgentManagement,
) -> Result<Vec<serde_json::Value>, String> {
    let mut messages = Vec::new();

    for persona_item in &agent.agent_persona {
        for prompt_path in ordered_persona_prompt_paths(persona_item) {
            let content = std::fs::read_to_string(&prompt_path).map_err(|err| {
                format!(
                    "failed to read agent persona prompt {}: {err}",
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

fn ordered_persona_prompt_paths(persona_item: &AgentPersonaItem) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Some(path) = first_existing_persona_prompt_path(persona_item, "persona.md") {
        paths.push(path);
    }
    if let Some(path) = shared_communication_style_prompt_path(persona_item)
        .or_else(|| first_existing_persona_prompt_path(persona_item, "communication_style.md"))
        .or_else(|| first_existing_persona_prompt_path(persona_item, "communication_stlye.md"))
    {
        paths.push(path);
    }
    paths
}

fn shared_communication_style_prompt_path(persona_item: &AgentPersonaItem) -> Option<PathBuf> {
    persona_item
        .persona_directory
        .ancestors()
        .find_map(|ancestor| {
            [
                ancestor
                    .join("communication_style")
                    .join("communication_style.md"),
                ancestor
                    .join("personas")
                    .join("src")
                    .join("communication_style")
                    .join("communication_style.md"),
            ]
            .into_iter()
            .find(|path| path.exists())
        })
}

fn first_existing_persona_prompt_path(
    persona_item: &AgentPersonaItem,
    prompt_name: &str,
) -> Option<PathBuf> {
    [
        persona_item.persona_directory.join(prompt_name),
        persona_item
            .persona_directory
            .join("prompt")
            .join(prompt_name),
    ]
    .into_iter()
    .find(|path| path.exists())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state_machine::agent_management::{
        AgentPersonaItem, AgentPromptItem, ProviderConfig, ToolChoice, ValidatorConfig,
    };
    use chrono::Utc;

    #[test]
    fn loader_includes_persona_and_communication_style_from_persona_binding() {
        let run_id = format!(
            "tura-agent-prompt-supplemental-test-{}",
            Utc::now().timestamp_nanos_opt().unwrap_or_default()
        );
        let root = std::env::temp_dir().join(run_id);
        let prompt_dir = root.join("agents").join("src").join("coding_agent");
        let persona_prompt_dir = root
            .join("personas")
            .join("src")
            .join("tura")
            .join("prompt");
        let shared_communication_style_dir = root
            .join("personas")
            .join("src")
            .join("communication_style");
        std::fs::create_dir_all(&prompt_dir).expect("prompt dir should be created");
        std::fs::create_dir_all(&persona_prompt_dir).expect("persona prompt dir should be created");
        std::fs::create_dir_all(&shared_communication_style_dir)
            .expect("shared communication style dir should be created");
        std::fs::write(persona_prompt_dir.join("persona.md"), "persona prompt")
            .expect("persona prompt should be written");
        std::fs::write(
            shared_communication_style_dir.join("communication_style.md"),
            "communication style prompt",
        )
        .expect("communication style prompt should be written");
        std::fs::write(prompt_dir.join("prompt.md"), "main prompt")
            .expect("main prompt should be written");

        let mut agent = test_agent(&root, "coding_agent");
        let now = Utc::now();
        agent.add_prompt(
            AgentPromptItem {
                agent_prompt: "coding_agent".to_string(),
                prompt_directory: prompt_dir,
            },
            now,
        );
        agent.add_persona(
            AgentPersonaItem {
                persona_name: "tura".to_string(),
                persona_directory: persona_prompt_dir,
            },
            now,
        );

        let mut messages =
            load_agent_persona_messages(&agent).expect("persona loading should succeed");
        messages.extend(load_agent_prompt_messages(&agent).expect("prompt loading should succeed"));
        let contents = message_contents(&messages);

        assert_eq!(
            contents,
            vec![
                "persona prompt",
                "communication style prompt",
                "main prompt"
            ]
        );

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
            ProviderConfig {
                tura_llm_name: "test".to_string(),
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
