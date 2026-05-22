use std::path::PathBuf;

use crate::state_machine::agent_management::AgentManagement;

use super::constants::{
    CODING_AGENT_CONTEXT_EXCLUDED_TOOLS, COMMAND_RUN_TOOL, DISABLE_EXECUTE_TOOLS_TOOL_ENV,
    DISABLE_MULTIPLE_TASKS_TOOL_ENV, FORCE_EXECUTE_TOOLS_MULTIPLE_TASKS_ENV,
    FORCE_MULTIPLE_TASKS_ENV, PROJECT_ROOT_ENV, TASK_DELIVERED_TOOL,
};

#[cfg(test)]
use super::constants::MULTIPLE_TASKS_TOOL;

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
    is_multiple_tasks_child: bool,
    multiple_tasks_mode_enabled: bool,
) -> Result<Vec<serde_json::Value>, String> {
    if force_no_tools {
        return Ok(Vec::new());
    }

    if is_final_turn {
        return Ok(Vec::new());
    }

    let _ = is_multiple_tasks_child;
    if multiple_tasks_mode_enabled {
        return Ok(keep_command_run_and_task_delivered(tools));
    }
    Ok(keep_command_run_only(tools))
}

#[cfg(test)]
pub(super) fn require_multiple_tasks_tool_for_multiple_tasks_mode(
    tools: Vec<serde_json::Value>,
) -> Result<Vec<serde_json::Value>, String> {
    if !tools
        .iter()
        .any(|tool| tool_schema_name(tool) == Some(MULTIPLE_TASKS_TOOL))
    {
        return Err("multiple-tasks mode requested but multiple_tasks is unavailable".to_string());
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

pub(super) fn keep_command_run_and_task_delivered(
    tools: Vec<serde_json::Value>,
) -> Vec<serde_json::Value> {
    tools
        .into_iter()
        .filter(|tool| {
            matches!(
                tool_schema_name(tool),
                Some(COMMAND_RUN_TOOL) | Some(TASK_DELIVERED_TOOL)
            )
        })
        .collect()
}

pub(super) fn task_delivered_provider_schema() -> Result<serde_json::Value, String> {
    let path = project_directory_with_tools()?
        .join("crates")
        .join("tools")
        .join("src")
        .join(TASK_DELIVERED_TOOL)
        .join("schema.json");
    let content = std::fs::read_to_string(&path).map_err(|err| {
        format!(
            "failed to read task_delivered schema {}: {err}",
            path.display()
        )
    })?;
    let interface = serde_json::from_str::<serde_json::Value>(&content)
        .map_err(|err| format!("failed to parse task_delivered schema: {err}"))?;
    Ok(tool_interface_to_provider_schema(interface))
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

pub(super) fn multiple_tasks_env_enabled() -> bool {
    env_flag(FORCE_MULTIPLE_TASKS_ENV) || env_flag(FORCE_EXECUTE_TOOLS_MULTIPLE_TASKS_ENV)
}

pub(super) fn multiple_tasks_tool_disabled() -> bool {
    env_flag(DISABLE_MULTIPLE_TASKS_TOOL_ENV) || env_flag(DISABLE_EXECUTE_TOOLS_TOOL_ENV)
}

pub(super) fn multiple_tasks_child_depth() -> usize {
    std::env::var("TURA_MULTIPLE_TASKS_DEPTH")
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
    let mut parameters =
        if input_schema.get("type").and_then(|value| value.as_str()) == Some("array") {
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
    let strict = env_flag("TURA_COMMAND_RUN_STRICT_JSON");
    if strict {
        parameters = strict_provider_schema(parameters);
    }
    parameters = strip_tura_schema_extensions(parameters);

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

fn strip_tura_schema_extensions(mut value: serde_json::Value) -> serde_json::Value {
    match &mut value {
        serde_json::Value::Object(object) => {
            object.remove("x-tura-optional");
            for child in object.values_mut() {
                *child = strip_tura_schema_extensions(std::mem::take(child));
            }
        }
        serde_json::Value::Array(items) => {
            for child in items {
                *child = strip_tura_schema_extensions(std::mem::take(child));
            }
        }
        _ => {}
    }
    value
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
                    let mut required = object
                        .get("required")
                        .and_then(serde_json::Value::as_array)
                        .cloned()
                        .unwrap_or_default();
                    for key in properties.keys() {
                        let key_value = serde_json::Value::String(key.clone());
                        if !required.contains(&key_value) {
                            required.push(key_value);
                        }
                    }
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
    let prefix = original
        .split("\nAvailable command details")
        .next()
        .unwrap_or(original)
        .split("\nCommand line formats:\n")
        .next()
        .unwrap_or(original)
        .replace("Available commands: apply_patch, bash, shell_command.", "")
        .trim_end()
        .to_string();
    let shell_prompt = if active == "shell_command" {
        code_tools::commands::shell_command::PROMPT
    } else {
        code_tools::commands::bash::PROMPT
    };
    let mut description = format!(
        "{prefix} Available commands: apply_patch, {active}, read_media, web_discover, compact_context.\nCommand run patterns:\n{}\nCommand line formats:\n- apply_patch: {}\n- {active}: {}\n- read_media: {} Schema: {}\n- web_discover: {} Schema: {}\n- compact_context: {} Schema: {}",
        command_run_usage_patterns(),
        current_apply_patch_command_format(),
        current_shell_command_format(active, shell_prompt),
        compact_prompt(code_tools::commands::read_media::PROMPT),
        compact_schema(code_tools::commands::read_media::SCHEMA),
        compact_prompt(code_tools::commands::web_discover::PROMPT),
        compact_schema(code_tools::commands::web_discover::SCHEMA),
        compact_prompt(code_tools::commands::compact_context::PROMPT),
        compact_schema(code_tools::commands::compact_context::SCHEMA),
    );
    if multiple_tasks_env_enabled() {
        description.push_str(&format!(
            "\n- multiple_tasks: {} Schema: {}",
            compact_prompt(code_tools::commands::multiple_tasks::PROMPT),
            compact_schema(code_tools::commands::multiple_tasks::SCHEMA),
        ));
        description = description.replace(
            &format!("Available commands: apply_patch, {active}, read_media, web_discover, compact_context."),
            &format!("Available commands: apply_patch, {active}, read_media, web_discover, compact_context, multiple_tasks."),
        );
    }
    description
}

fn command_run_usage_patterns() -> &'static str {
    r#"- Batch investigation: use early commands for the specific discovery, searches, and file reads needed to understand the failure surface.
- Keep related path listing, targeted search, and candidate file reads in the same command_run batch and same read-only step when they are independent.
- Parallel reads: put independent safe read-only commands in the same step when they do not depend on each other.
- Code repair loop: use early steps for needed discovery/reads, a later step for one multi-file `apply_patch`, and final steps in the same command_run for tests plus focused validation searches.
- Avoid embedding long generated source code or complex quoting directly in shell/bash command lines; for complex logic, invoke a script/interpreter from shell/bash rather than encoding the logic in shell syntax.
- Verification: run the relevant test or build command after edits in the same command_run when the edit target is already clear.
- Failure handling: inspect each failed item and change the next command based on that failure instead of retrying the same command.
- Context compaction: after a meaningful phase completes, or when context is near 200,000 tokens and feels crowded, put `compact_context` as the final command in the highest step with a concise handoff summary for the next turn.
- Example investigation batch: step 1 needed `rg --files`, targeted `rg -n`, and candidate file reads.
- Example repair batch: step 1 `apply_patch` across related files, step 2 write or update a focused test script when needed, step 3 run the narrow test and focused validation searches.
- Example media batch: step 1 use `web_discover` or generation to collect the needed media, docs, or repo artifacts, step 2 use `read_media` or focused reads to verify the resulting media or repo content."#
}

fn current_apply_patch_command_format() -> String {
    let grammar = "start: begin_patch hunk+ end_patch\nbegin_patch: \"*** Begin Patch\" LF\nend_patch: \"*** End Patch\" LF?\n\nhunk: add_hunk | delete_hunk | update_hunk\nadd_hunk: \"*** Add File: \" filename LF add_line+\ndelete_hunk: \"*** Delete File: \" filename LF\nupdate_hunk: \"*** Update File: \" filename LF change_move? change?\n\nfilename: /(.+)/\nadd_line: \"+\" /(.*)/ LF -> line\n\nchange_move: \"*** Move to: \" filename LF\nchange: (change_context | change_line)+ eof_line?\nchange_context: (\"@@\" | \"@@ \" /(.+)/) LF\nchange_line: (\"+\" | \"-\" | \" \") /(.*)/ LF\neof_line: \"*** End of File\" LF\n\n%import common.LF\n";
    format!(
        "Use one patch for coordinated multi-file source edits after reads. Patches validate context and fail on mismatch. Raw freeform body. Format type `grammar`, syntax `lark`. Definition: {grammar}"
    )
}

fn current_shell_command_format(active: &str, shell_prompt: &str) -> String {
    let guidance = format!(
        "Use for tests, builds, scripts, package tools, and host-shell behavior. Put verification after edits in a later step. {}",
        long_running_service_guidance(active)
    );
    let schema = "{\"type\":\"object\",\"properties\":{\"command\":{\"type\":\"string\",\"description\":\"The shell script to execute in the user's default shell\"},\"workdir\":{\"type\":\"string\",\"description\":\"The working directory to execute the command in\"},\"timeout_ms\":{\"type\":\"number\",\"description\":\"The timeout for the command in milliseconds\"}},\"required\":[\"command\"],\"additionalProperties\":false}";
    let _ = shell_prompt;
    format!("{guidance} JSON object string matching this schema: {schema}")
}

fn long_running_service_guidance(active: &str) -> &'static str {
    if active == "shell_command" && cfg!(windows) {
        "For long-running local servers, use Start-Process -WindowStyle Hidden -PassThru, wait for readiness, run probes, then stop it with try/finally."
    } else {
        "For long-running local servers, start them in the background, wait for readiness, run probes, then clean them up."
    }
}

fn compact_prompt(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn compact_schema(text: &str) -> String {
    serde_json::from_str::<serde_json::Value>(text)
        .map(|value| value.to_string())
        .unwrap_or_else(|_| compact_prompt(text))
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
                "required": ["commands"],
                "properties": {
                    "commands": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "required": ["command_type", "command_line"],
                            "properties": {
                                "command_type": { "type": "string" },
                                "command_line": { "type": "string" },
                                "step": { "type": "integer", "x-tura-optional": true }
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
                tool(MULTIPLE_TASKS_TOOL),
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
    fn multiple_tasks_mode_still_keeps_only_command_run() {
        let filtered = filter_tools_for_turn(
            vec![
                tool(COMMAND_RUN_TOOL),
                tool(MULTIPLE_TASKS_TOOL),
                tool(TASK_DELIVERED_TOOL),
                tool("apply_diff"),
            ],
            false,
            false,
            false,
            true,
        )
        .expect("filter should succeed");

        assert_eq!(names(filtered), vec![COMMAND_RUN_TOOL, TASK_DELIVERED_TOOL]);
    }

    #[test]
    fn provider_schema_preserves_additional_properties_recursively() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|err| err.into_inner());
        std::env::set_var("TURA_COMMAND_RUN_DISABLE_STRICT_JSON", "1");

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

        std::env::remove_var("TURA_COMMAND_RUN_DISABLE_STRICT_JSON");
    }

    #[test]
    fn strict_json_env_requires_all_provider_object_fields() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|err| err.into_inner());
        std::env::set_var("TURA_COMMAND_RUN_STRICT_JSON", "1");

        let schema = tool_interface_to_provider_schema(command_run_interface());
        let parameters = &schema["function"]["parameters"];

        assert_eq!(schema["function"]["strict"], true);
        assert_eq!(parameters["required"], serde_json::json!(["commands"]));
        assert_eq!(
            parameters["properties"]["commands"]["items"]["required"],
            serde_json::json!(["command_type", "command_line", "step"])
        );

        std::env::remove_var("TURA_COMMAND_RUN_STRICT_JSON");
    }

    #[test]
    fn real_command_run_provider_schema_is_openai_strict_compatible() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|err| err.into_inner());
        std::env::set_var("TURA_COMMAND_RUN_STRICT_JSON", "1");

        let interface = serde_json::from_str::<serde_json::Value>(include_str!(
            "../../../tools/src/command_run/schema.json"
        ))
        .expect("command_run schema should parse");
        let schema = tool_interface_to_provider_schema(interface);
        let parameters = &schema["function"]["parameters"];
        let command_required = parameters["properties"]["commands"]["items"]["required"]
            .as_array()
            .expect("commands item required should be an array");

        assert_eq!(schema["function"]["strict"], true);
        assert_eq!(parameters["required"], serde_json::json!(["commands"]));
        assert_eq!(
            parameters["properties"]["commands"]["items"]["required"],
            serde_json::json!(["command_type", "command_line", "step"])
        );
        assert!(parameters["properties"].get("task_delivered").is_none());
        assert!(command_required.contains(&serde_json::json!("command_type")));
        assert!(command_required.contains(&serde_json::json!("step")));
        std::env::remove_var("TURA_COMMAND_RUN_STRICT_JSON");
    }

    #[test]
    fn task_delivered_tool_is_optional_even_under_strict_json() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|err| err.into_inner());
        std::env::set_var("TURA_COMMAND_RUN_STRICT_JSON", "1");

        let interface = serde_json::from_str::<serde_json::Value>(include_str!(
            "../../../tools/src/task_delivered/schema.json"
        ))
        .expect("task_delivered schema should parse");
        let schema = tool_interface_to_provider_schema(interface);
        let parameters = &schema["function"]["parameters"];

        assert_eq!(schema["function"]["name"], TASK_DELIVERED_TOOL);
        assert_eq!(schema["function"]["strict"], true);
        assert_eq!(
            parameters["required"],
            serde_json::json!(["task_delivered"])
        );
        assert!(parameters["properties"].get("task_delivered").is_some());

        std::env::remove_var("TURA_COMMAND_RUN_STRICT_JSON");
    }

    #[test]
    fn command_run_provider_description_exposes_only_shell_command_surface() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|err| err.into_inner());
        std::env::remove_var("TURA_FORCE_MULTIPLE_TASKS");
        std::env::remove_var("TURA_FORCE_EXECUTE_TOOLS_MULTIPLE_TASKS");
        std::env::set_var("TURA_COMMAND_RUN_SHELL", "shell_command");

        let schema = tool_interface_to_provider_schema(command_run_interface());
        let description = schema["function"]["description"]
            .as_str()
            .unwrap_or_default();

        assert!(description.contains(
            "Available commands: apply_patch, shell_command, read_media, web_discover, compact_context."
        ));
        assert!(description.contains("- shell_command:"));
        assert!(description.contains("- read_media:"));
        assert!(description.contains("- web_discover:"));
        assert!(description.contains("- compact_context:"));
        assert!(!description.contains("- multiple_tasks:"));
        assert!(description.contains("\"command\":{\"type\":\"string\""));
        assert!(description.contains("\"workdir\":{\"type\":\"string\""));
        assert!(description.contains("\"timeout_ms\":{\"type\":\"number\""));
        if cfg!(windows) {
            assert!(description.contains("Start-Process -WindowStyle Hidden -PassThru"));
            assert!(description.contains("try/finally"));
        } else {
            assert!(description.contains("start them in the background"));
        }
        assert!(!description.contains("Available commands: apply_patch, bash"));
        assert!(!description.contains("- bash:"));

        std::env::remove_var("TURA_COMMAND_RUN_SHELL");
    }

    #[test]
    fn command_run_provider_description_exposes_only_bash_surface() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|err| err.into_inner());
        std::env::remove_var("TURA_FORCE_MULTIPLE_TASKS");
        std::env::remove_var("TURA_FORCE_EXECUTE_TOOLS_MULTIPLE_TASKS");
        std::env::set_var("TURA_COMMAND_RUN_SHELL", "bash");

        let schema = tool_interface_to_provider_schema(command_run_interface());
        let description = schema["function"]["description"]
            .as_str()
            .unwrap_or_default();

        assert!(description.contains(
            "Available commands: apply_patch, bash, read_media, web_discover, compact_context."
        ));
        assert!(description.contains("- bash:"));
        assert!(description.contains("- read_media:"));
        assert!(description.contains("- web_discover:"));
        assert!(description.contains("- compact_context:"));
        assert!(!description.contains("- multiple_tasks:"));
        assert!(description.contains("\"command\":{\"type\":\"string\""));
        assert!(description.contains("\"workdir\":{\"type\":\"string\""));
        assert!(description.contains("\"timeout_ms\":{\"type\":\"number\""));
        assert!(description.contains("start them in the background"));
        assert!(!description.contains("shell_command"));

        std::env::remove_var("TURA_COMMAND_RUN_SHELL");
    }

    #[test]
    fn command_run_provider_description_injects_multiple_tasks_only_when_enabled() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|err| err.into_inner());
        std::env::set_var("TURA_COMMAND_RUN_SHELL", "shell_command");
        std::env::set_var("TURA_FORCE_EXECUTE_TOOLS_MULTIPLE_TASKS", "1");

        let schema = tool_interface_to_provider_schema(command_run_interface());
        let description = schema["function"]["description"]
            .as_str()
            .unwrap_or_default();

        assert!(description.contains(
            "Available commands: apply_patch, shell_command, read_media, web_discover, compact_context, multiple_tasks."
        ));
        assert!(description.contains("- multiple_tasks:"));

        std::env::remove_var("TURA_FORCE_EXECUTE_TOOLS_MULTIPLE_TASKS");
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
