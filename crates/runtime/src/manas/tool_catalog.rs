use std::path::PathBuf;

use crate::state_machine::agent_management::AgentManagement;

use super::constants::{
    CODING_AGENT_CONTEXT_EXCLUDED_TOOLS, COMMAND_RUN_TOOL, DISABLE_EXECUTE_TOOLS_PLANNING_ENV,
    DISABLE_EXECUTE_TOOLS_TOOL_ENV, DISABLE_PLANNING_GATE_ENV, DISABLE_PLANNING_TOOL_ENV,
    FORCE_EXECUTE_TOOLS_PLANNING_ENV, FORCE_PLANNING_ENV, PROJECT_ROOT_ENV,
};

#[cfg(test)]
use super::constants::PLANNING_TOOL;

pub(super) fn load_agent_capabilities(
    agent: &AgentManagement,
) -> Result<Vec<serde_json::Value>, String> {
    let mut tools = Vec::new();

    for capability in &agent.agent_capabilities {
        if should_hide_from_coding_agent_context(agent, &capability.capability_name) {
            continue;
        }
        let interface_path = capability
            .capability_directory
            .join(&capability.capability_name)
            .join("schema.json");

        if interface_path.exists() {
            let content = std::fs::read_to_string(&interface_path)
                .map_err(|e| format!("failed to read tool interface: {}", e))?;

            if let Ok(interface) = serde_json::from_str::<serde_json::Value>(&content) {
                tools.push(tool_interface_to_provider_schema(interface));
            }
        }
    }

    Ok(tools)
}

pub(super) fn filter_tools_for_turn(
    tools: Vec<serde_json::Value>,
    is_final_turn: bool,
    force_no_tools: bool,
    is_planning_child: bool,
    planning_mode_enabled: bool,
) -> Result<Vec<serde_json::Value>, String> {
    if force_no_tools {
        return Ok(Vec::new());
    }

    if is_final_turn {
        return Ok(Vec::new());
    }

    let _ = (is_planning_child, planning_mode_enabled);
    Ok(keep_command_run_only(tools))
}

#[cfg(test)]
pub(super) fn require_planning_tool_for_planning_mode(
    tools: Vec<serde_json::Value>,
) -> Result<Vec<serde_json::Value>, String> {
    if !tools
        .iter()
        .any(|tool| tool_schema_name(tool) == Some(PLANNING_TOOL))
    {
        return Err("planning mode requested but planning is unavailable".to_string());
    }

    Ok(tools)
}

#[cfg(test)]
pub(super) fn remove_tool(
    tools: Vec<serde_json::Value>,
    tool_name: &str,
) -> Vec<serde_json::Value> {
    tools
        .into_iter()
        .filter(|tool| tool_schema_name(tool) != Some(tool_name))
        .collect()
}

pub(super) fn keep_command_run_only(tools: Vec<serde_json::Value>) -> Vec<serde_json::Value> {
    tools
        .into_iter()
        .filter(|tool| tool_schema_name(tool) == Some(COMMAND_RUN_TOOL))
        .collect()
}

pub(super) fn tool_schema_name(tool: &serde_json::Value) -> Option<&str> {
    tool.get("function")
        .and_then(|function| function.get("name"))
        .and_then(|name| name.as_str())
}

pub(super) fn env_flag(name: &str) -> bool {
    std::env::var(name)
        .ok()
        .map(|value| {
            let value = value.trim().to_ascii_lowercase();
            matches!(value.as_str(), "1" | "true" | "yes" | "on")
        })
        .unwrap_or(false)
}

pub(super) fn planning_env_enabled() -> bool {
    env_flag(FORCE_PLANNING_ENV) || env_flag(FORCE_EXECUTE_TOOLS_PLANNING_ENV)
}

pub(super) fn planning_gate_disabled() -> bool {
    env_flag(DISABLE_PLANNING_GATE_ENV) || env_flag(DISABLE_EXECUTE_TOOLS_PLANNING_ENV)
}

pub(super) fn planning_tool_disabled() -> bool {
    env_flag(DISABLE_PLANNING_TOOL_ENV) || env_flag(DISABLE_EXECUTE_TOOLS_TOOL_ENV)
}

pub(super) fn planning_child_depth() -> usize {
    std::env::var("TURA_PLANNING_DEPTH")
        .or_else(|_| std::env::var("TURA_EXECUTE_TOOLS_DEPTH"))
        .ok()
        .and_then(|value| value.trim().parse::<usize>().ok())
        .unwrap_or(0)
}

pub(super) fn project_directory_with_tools() -> Result<PathBuf, String> {
    if let Ok(root) = std::env::var(PROJECT_ROOT_ENV) {
        let root = PathBuf::from(root);
        if root
            .join("crates")
            .join("tools")
            .join("src")
            .join("command_run")
            .join("schema.json")
            .exists()
        {
            return Ok(root);
        }
    }

    let current = std::env::current_dir()
        .map_err(|err| format!("failed to resolve project directory: {err}"))?;
    for candidate in current.ancestors() {
        if candidate
            .join("crates")
            .join("tools")
            .join("src")
            .join("command_run")
            .join("schema.json")
            .exists()
        {
            return Ok(candidate.to_path_buf());
        }
    }
    Ok(current)
}

pub(super) fn load_agent_prompt_messages(
    agent: &AgentManagement,
    active_tool_names: &std::collections::HashSet<String>,
) -> Result<Vec<serde_json::Value>, String> {
    let mut messages = Vec::new();
    let mut loaded_prompt_paths = std::collections::HashSet::new();

    for prompt_item in &agent.agent_prompt {
        let prompt_path = {
            let standard_path = prompt_item.prompt_directory.join("prompt.md");
            if standard_path.exists() {
                standard_path
            } else {
                prompt_item.prompt_directory.join("prompt")
            }
        };

        if prompt_path.exists() {
            let content = std::fs::read_to_string(&prompt_path).map_err(|err| {
                format!(
                    "failed to read agent prompt {}: {err}",
                    prompt_path.display()
                )
            })?;
            loaded_prompt_paths.insert(prompt_path.clone());
            messages.push(serde_json::json!({
                "role": "system",
                "content": content,
            }));
        }
    }

    let _ = active_tool_names;

    Ok(messages)
}

fn should_hide_from_coding_agent_context(agent: &AgentManagement, tool_name: &str) -> bool {
    matches!(
        agent.agent_name.as_str(),
        "coding_agent" | "coding_agent_fast"
    ) && CODING_AGENT_CONTEXT_EXCLUDED_TOOLS.contains(&tool_name)
}

pub(super) fn tool_interface_to_provider_schema(interface: serde_json::Value) -> serde_json::Value {
    let name = interface
        .get("name")
        .and_then(|value| value.as_str())
        .unwrap_or("unknown_tool");
    let mut description = interface
        .get("description")
        .and_then(|value| value.as_str())
        .unwrap_or("")
        .to_string();
    if name == COMMAND_RUN_TOOL {
        description = command_run_description_for_active_shell(&description);
    }
    let input_schema = sanitize_provider_schema(
        interface
            .get("input_schema")
            .cloned()
            .unwrap_or_else(|| serde_json::json!({ "type": "object" })),
    );
    let mut parameters = if input_schema.get("type").and_then(|value| value.as_str()) == Some("array") {
        serde_json::json!({
            "type": "object",
            "required": ["requests"],
            "properties": {
                "requests": input_schema
            }
        })
    } else {
        input_schema
    };
    let strict = !env_flag("TURA_COMMAND_RUN_DISABLE_STRICT_JSON");
    if strict {
        parameters = strict_provider_schema(parameters);
    }

    serde_json::json!({
        "type": "function",
        "function": {
            "name": name,
            "description": description,
            "parameters": parameters,
            "strict": strict
        }
    })
}

fn strict_provider_schema(mut value: serde_json::Value) -> serde_json::Value {
    match &mut value {
        serde_json::Value::Object(object) => {
            if object.get("type").and_then(serde_json::Value::as_str) == Some("object") {
                object
                    .entry("additionalProperties".to_string())
                    .or_insert(serde_json::Value::Bool(false));
                if let Some(properties) = object
                    .get("properties")
                    .and_then(serde_json::Value::as_object)
                {
                    let mut required = properties
                        .keys()
                        .cloned()
                        .map(serde_json::Value::String)
                        .collect::<Vec<_>>();
                    required.sort_by(|left, right| left.as_str().cmp(&right.as_str()));
                    object.insert("required".to_string(), serde_json::Value::Array(required));
                }
            }
            for child in object.values_mut() {
                *child = strict_provider_schema(std::mem::take(child));
            }
        }
        serde_json::Value::Array(items) => {
            for child in items {
                *child = strict_provider_schema(std::mem::take(child));
            }
        }
        _ => {}
    }
    value
}

fn active_shell_command_name() -> &'static str {
    match std::env::var("TURA_COMMAND_RUN_SHELL")
        .ok()
        .map(|value| value.trim().to_ascii_lowercase())
        .as_deref()
    {
        Some("bash") => "bash",
        Some("shell") | Some("shell_command") | Some("shll") | Some("shall") => "shell_command",
        _ if cfg!(windows) => "shell_command",
        _ => "bash",
    }
}

fn command_run_description_for_active_shell(original: &str) -> String {
    let active = active_shell_command_name();
    let shell_schema = "{\"type\":\"object\",\"required\":[\"command\"],\"additionalProperties\":false,\"properties\":{\"command\":{\"type\":\"string\",\"description\":\"The shell script to execute in the active session directory\"}}}";
    let shell_description = if active == "shell_command" {
        "Use for tests, builds, scripts, package tools, and host-shell behavior. Put verification after edits in a later step. "
    } else {
        ""
    };

    let prefix = original
        .split("\nCommand line formats:\n")
        .next()
        .unwrap_or(original)
        .replace(
            "Available commands: apply_patch, bash, shell_command.",
            &format!("Available commands: apply_patch, {active}."),
        )
        .replace(
            "Supported internal commands are shell_command, bash, and apply_patch.",
            &format!("Supported internal commands are {active} and apply_patch."),
        );

    format!(
        "{prefix}\nCommand line formats:\n- apply_patch: Use one patch for coordinated multi-file source edits after reads. Patches validate context and fail on mismatch. Raw freeform body beginning with `*** Begin Patch`.\n- {active}: {shell_description}JSON object string matching this schema: {shell_schema}"
    )
}

fn sanitize_provider_schema(mut value: serde_json::Value) -> serde_json::Value {
    match &mut value {
        serde_json::Value::Object(object) => {
            for child in object.values_mut() {
                *child = sanitize_provider_schema(std::mem::take(child));
            }
        }
        serde_json::Value::Array(items) => {
            for child in items {
                *child = sanitize_provider_schema(std::mem::take(child));
            }
        }
        _ => {}
    }
    value
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state_machine::agent_management::{
        AgentCapabilityItem, ProviderConfig, ToolChoice, ValidatorConfig,
    };
    use chrono::Utc;
    use std::collections::HashSet;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn tool(name: &str) -> serde_json::Value {
        serde_json::json!({
            "type": "function",
            "function": {
                "name": name,
                "description": "",
                "parameters": { "type": "object" }
            }
        })
    }

    fn names(tools: Vec<serde_json::Value>) -> Vec<String> {
        tools
            .iter()
            .filter_map(|tool| tool_schema_name(tool).map(str::to_string))
            .collect()
    }

    fn command_run_interface() -> serde_json::Value {
        serde_json::json!({
            "name": COMMAND_RUN_TOOL,
            "description": "Run Codex tools as a pure batch+step command runner. Available commands: apply_patch, bash, shell_command.\nCommand line formats:\n- apply_patch: patch\n- bash: bash details\n- shell_command: shell details",
            "input_schema": {
                "type": "object",
                "properties": {
                    "commands": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "command": { "type": "string" },
                                "command_line": { "type": "string" }
                            }
                        }
                    }
                }
            }
        })
    }

    #[test]
    fn default_non_final_turn_keeps_only_command_run() {
        let filtered = filter_tools_for_turn(
            vec![
                tool(COMMAND_RUN_TOOL),
                tool(PLANNING_TOOL),
                tool("apply_diff"),
                tool("web_search"),
            ],
            false,
            false,
            false,
            false,
        )
        .expect("filter should succeed");

        assert_eq!(names(filtered), vec![COMMAND_RUN_TOOL]);
    }

    #[test]
    fn planning_mode_still_keeps_only_command_run() {
        let filtered = filter_tools_for_turn(
            vec![
                tool(COMMAND_RUN_TOOL),
                tool(PLANNING_TOOL),
                tool("apply_diff"),
            ],
            false,
            false,
            false,
            true,
        )
        .expect("filter should succeed");

        assert_eq!(names(filtered), vec![COMMAND_RUN_TOOL]);
    }

    #[test]
    fn provider_schema_preserves_additional_properties_recursively() {
        let schema = tool_interface_to_provider_schema(serde_json::json!({
            "name": "example",
            "description": "example",
            "input_schema": {
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "items": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "additionalProperties": false,
                            "properties": {
                                "name": { "type": "string" }
                            }
                        }
                    }
                }
            }
        }));

        assert!(schema.to_string().contains("additionalProperties"));
        assert_eq!(schema["function"]["strict"], false);
    }

    #[test]
    fn strict_json_env_requires_all_object_properties() {
        let _guard = ENV_LOCK.lock().expect("env lock");
        std::env::set_var("TURA_COMMAND_RUN_STRICT_JSON", "1");

        let schema = tool_interface_to_provider_schema(command_run_interface());
        let parameters = &schema["function"]["parameters"];

        assert_eq!(schema["function"]["strict"], true);
        assert_eq!(parameters["required"], serde_json::json!(["commands"]));
        assert_eq!(
            parameters["properties"]["commands"]["items"]["required"],
            serde_json::json!(["command", "command_line"])
        );

        std::env::remove_var("TURA_COMMAND_RUN_STRICT_JSON");
    }

    #[test]
    fn command_run_provider_description_exposes_only_shell_command_surface() {
        let _guard = ENV_LOCK.lock().expect("env lock");
        std::env::set_var("TURA_COMMAND_RUN_SHELL", "shell_command");

        let schema = tool_interface_to_provider_schema(command_run_interface());
        let description = schema["function"]["description"]
            .as_str()
            .unwrap_or_default();

        assert!(description.contains("Available commands: apply_patch, shell_command."));
        assert!(description.contains("- shell_command:"));
        assert!(!description.contains("workdir"));
        assert!(!description.contains("timeout_ms"));
        assert!(!description.contains("Available commands: apply_patch, bash"));
        assert!(!description.contains("- bash:"));

        std::env::remove_var("TURA_COMMAND_RUN_SHELL");
    }

    #[test]
    fn command_run_provider_description_exposes_only_bash_surface() {
        let _guard = ENV_LOCK.lock().expect("env lock");
        std::env::set_var("TURA_COMMAND_RUN_SHELL", "bash");

        let schema = tool_interface_to_provider_schema(command_run_interface());
        let description = schema["function"]["description"]
            .as_str()
            .unwrap_or_default();

        assert!(description.contains("Available commands: apply_patch, bash."));
        assert!(description.contains("- bash:"));
        assert!(!description.contains("workdir"));
        assert!(!description.contains("timeout_ms"));
        assert!(!description.contains("shell_command"));

        std::env::remove_var("TURA_COMMAND_RUN_SHELL");
    }

    #[test]
    fn command_run_prompt_loading_excludes_capability_prompts_from_model_context() {
        let run_id = format!(
            "tura-command-run-prompt-test-{}",
            Utc::now().timestamp_nanos_opt().unwrap_or_default()
        );
        let root = std::env::temp_dir().join(run_id);
        let command_run_dir = root.join("command_run");
        let shell_dir = root.join("commands").join("shell_command");
        let bash_dir = root.join("commands").join("bash");
        let apply_patch_dir = root.join("commands").join("apply_patch");
        std::fs::create_dir_all(&command_run_dir).expect("command_run dir should be created");
        std::fs::create_dir_all(&shell_dir).expect("shell dir should be created");
        std::fs::create_dir_all(&bash_dir).expect("bash dir should be created");
        std::fs::create_dir_all(&apply_patch_dir).expect("apply_patch dir should be created");
        std::fs::write(
            command_run_dir.join("prompt.md"),
            "common command_run prompt",
        )
        .expect("common prompt should be written");
        std::fs::write(shell_dir.join("prompt.md"), "shell command_run prompt")
            .expect("shell prompt should be written");
        std::fs::write(bash_dir.join("prompt.md"), "bash command_run prompt")
            .expect("bash prompt should be written");
        std::fs::write(apply_patch_dir.join("prompt.md"), "apply_patch prompt")
            .expect("apply_patch prompt should be written");

        let now = Utc::now();
        let mut agent = AgentManagement::new(
            "agent".to_string(),
            "general".to_string(),
            root.clone(),
            None,
            true,
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
            now,
        );
        agent.add_capability(
            AgentCapabilityItem {
                capability_name: COMMAND_RUN_TOOL.to_string(),
                capability_directory: root.clone(),
            },
            now,
        );

        let messages =
            load_agent_prompt_messages(&agent, &HashSet::from([COMMAND_RUN_TOOL.to_string()]))
                .expect("prompt loading should succeed");
        let joined = messages
            .iter()
            .filter_map(|message| message.get("content").and_then(|content| content.as_str()))
            .collect::<Vec<_>>()
            .join("\n");

        assert!(!joined.contains("common command_run prompt"));
        assert!(!joined.contains("apply_patch prompt"));
        assert!(!joined.contains("shell command_run prompt"));
        assert!(!joined.contains("bash command_run prompt"));

        let _ = std::fs::remove_dir_all(root);
    }
}
