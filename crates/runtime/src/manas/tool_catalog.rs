use std::collections::BTreeSet;
use std::path::PathBuf;

use crate::state_machine::agent_management::AgentManagement;

use super::constants::{
    COMMAND_RUN_TOOL, DISABLE_EXECUTE_TOOLS_TOOL_ENV, DISABLE_PLANNING_TOOL_ENV, PROJECT_ROOT_ENV,
};

#[cfg(test)]
use super::constants::PLANNING_TOOL;

pub(super) fn load_agent_capabilities(
    agent: &AgentManagement,
) -> Result<Vec<serde_json::Value>, String> {
    let Some(command_run_directory) = command_run_capability_directory(agent)? else {
        return Ok(Vec::new());
    };
    let interface_path = command_run_directory
        .join(COMMAND_RUN_TOOL)
        .join("schema.json");
    if !interface_path.exists() {
        return Ok(Vec::new());
    }

    let content = std::fs::read_to_string(&interface_path)
        .map_err(|e| format!("failed to read tool interface: {}", e))?;
    let interface = serde_json::from_str::<serde_json::Value>(&content)
        .map_err(|e| format!("failed to parse tool interface: {}", e))?;

    Ok(vec![tool_interface_to_provider_schema_for_agent(
        interface, agent,
    )])
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

fn command_run_capability_directory(agent: &AgentManagement) -> Result<Option<PathBuf>, String> {
    if let Some(capability) = agent
        .agent_capabilities
        .iter()
        .find(|capability| capability.capability_name == COMMAND_RUN_TOOL)
    {
        return Ok(Some(capability.capability_directory.clone()));
    }

    if let Some(capability) = agent.agent_capabilities.first() {
        return Ok(Some(capability.capability_directory.clone()));
    }

    let project_directory = project_directory_with_tools()?;
    Ok(Some(
        project_directory.join("crates").join("tools").join("src"),
    ))
}

#[cfg(test)]
pub(super) fn tool_interface_to_provider_schema(interface: serde_json::Value) -> serde_json::Value {
    tool_interface_to_provider_schema_with_commands(interface, None)
}

pub(super) fn tool_interface_to_provider_schema_for_agent(
    interface: serde_json::Value,
    agent: &AgentManagement,
) -> serde_json::Value {
    let allowed_commands = command_run_commands_for_agent(agent);
    tool_interface_to_provider_schema_with_commands(interface, Some(&allowed_commands))
}

pub(super) fn command_run_commands_for_agent(agent: &AgentManagement) -> BTreeSet<String> {
    let mut commands = agent
        .agent_capabilities
        .iter()
        .filter_map(|capability| {
            let name = code_tools::commands::canonical_command(&capability.capability_name);
            (name != COMMAND_RUN_TOOL).then_some(name)
        })
        .collect::<BTreeSet<_>>();

    if commands.is_empty() {
        commands = default_command_run_commands();
    }
    commands
}

fn default_command_run_commands() -> BTreeSet<String> {
    [
        "apply_patch",
        active_shell_command_name(),
        "read_media",
        "web_discover",
        "compact_context",
        "task_status",
    ]
    .into_iter()
    .map(str::to_string)
    .collect::<BTreeSet<_>>()
}

fn tool_interface_to_provider_schema_with_commands(
    interface: serde_json::Value,
    allowed_commands: Option<&BTreeSet<String>>,
) -> serde_json::Value {
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
        description = command_run_description_for_active_shell(&description, allowed_commands);
    }
    let mut input_schema = sanitize_provider_schema(
        interface
            .get("input_schema")
            .cloned()
            .unwrap_or_else(|| serde_json::json!({ "type": "object" })),
    );
    if name == COMMAND_RUN_TOOL {
        if let Some(commands) = allowed_commands {
            input_schema = restrict_command_run_schema(input_schema, commands);
        }
    }
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

fn restrict_command_run_schema(
    mut schema: serde_json::Value,
    commands: &BTreeSet<String>,
) -> serde_json::Value {
    let active = active_shell_command_name();
    let command_names = command_list_for_description(commands, active)
        .into_iter()
        .map(serde_json::Value::String)
        .collect::<Vec<_>>();
    if let Some(command_type) = schema
        .pointer_mut("/properties/commands/items/properties/command_type")
        .and_then(serde_json::Value::as_object_mut)
    {
        command_type.insert("enum".to_string(), serde_json::Value::Array(command_names));
    }
    schema
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

fn command_run_description_for_active_shell(
    original: &str,
    allowed_commands: Option<&BTreeSet<String>>,
) -> String {
    let active = active_shell_command_name();
    let default_commands;
    let allowed_commands = match allowed_commands {
        Some(commands) => commands,
        None => {
            default_commands = default_command_run_commands();
            &default_commands
        }
    };
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
    let mut command_lines = Vec::new();
    if allowed_commands.contains("apply_patch") {
        command_lines.push(format!(
            "- apply_patch: {}",
            current_apply_patch_command_format()
        ));
    }
    if allowed_commands.contains(active) {
        let shell_prompt = if active == "shell_command" {
            code_tools::commands::shell_command::PROMPT
        } else {
            code_tools::commands::bash::PROMPT
        };
        command_lines.push(format!(
            "- {active}: {}",
            current_shell_command_format(active, shell_prompt)
        ));
    }
    if allowed_commands.contains("read_media") {
        command_lines.push(format!(
            "- read_media: {} Schema: {}",
            compact_prompt(code_tools::commands::read_media::PROMPT),
            compact_schema(code_tools::commands::read_media::SCHEMA),
        ));
    }
    if allowed_commands.contains("web_discover") {
        command_lines.push(format!(
            "- web_discover: {} Schema: {}",
            compact_prompt(code_tools::commands::web_discover::PROMPT),
            compact_schema(code_tools::commands::web_discover::SCHEMA),
        ));
    }
    if allowed_commands.contains("compact_context") {
        command_lines.push(format!(
            "- compact_context: {} Schema: {}",
            compact_prompt(code_tools::commands::compact_context::PROMPT),
            compact_schema(code_tools::commands::compact_context::SCHEMA),
        ));
    }
    if allowed_commands.contains("task_status") {
        command_lines.push(format!(
            "- task_status: {} Schema: {}",
            compact_prompt(code_tools::commands::task_status::PROMPT),
            compact_schema(code_tools::commands::task_status::SCHEMA),
        ));
    }
    if allowed_commands.contains("planning") {
        command_lines.push(format!(
            "- planning: {} Schema: {}",
            compact_prompt(code_tools::commands::planning::PROMPT),
            compact_schema(code_tools::commands::planning::SCHEMA),
        ));
    }
    format!(
        "{prefix} Available commands: {}.\nCommand run patterns:\n{}\nCommand line formats:\n{}",
        command_list_for_description(allowed_commands, active).join(", "),
        command_run_usage_patterns(allowed_commands),
        command_lines.join("\n"),
    )
}

fn command_list_for_description(commands: &BTreeSet<String>, active_shell: &str) -> Vec<String> {
    let order = [
        "apply_patch",
        active_shell,
        "read_media",
        "web_discover",
        "compact_context",
        "task_status",
        "planning",
    ];
    order
        .into_iter()
        .filter(|name| commands.contains(*name))
        .map(str::to_string)
        .collect()
}

fn command_run_usage_patterns(allowed_commands: &BTreeSet<String>) -> String {
    let mut patterns = vec![
        "- Batch investigation: use early commands for the specific discovery, searches, and file reads needed to understand the failure surface.",
        "- Keep related path listing, targeted search, and candidate file reads in the same command_run batch; independent commands with no output dependency may share one step.",
        "- Do not run test/probe invocations before you have read the relevant code and determined the actual CLI command set.",
        "- Use steps to express execution order and dependency relationships. Commands in the same step may run together; later steps should depend on earlier steps only when their inputs are already known before the batch is created.",
        "- Code repair loop: after discovery has produced enough facts, use one step for coordinated edits and later steps for already-known tests or focused validation.",
        "- Avoid embedding long generated source code or complex quoting directly in shell/bash command lines; for complex logic, invoke a script/interpreter from shell/bash rather than encoding the logic in shell syntax.",
        "- Verification: run the relevant test or build command after edits in the same command_run only when the verification command is already known.",
        "- Failure handling: inspect each failed item and change the next command based on that failure instead of retrying the same command.",
        "- Context compaction: after a meaningful phase completes, or when context is near 200,000 tokens and feels crowded, put `compact_context` as the final command in the highest step with a concise handoff summary for the next turn.",
        "- Example investigation batch: step 1 groups independent `rg --files`, targeted `rg -n`, and candidate file reads.",
        "- Example repair batch: step 1 `apply_patch` across related files, step 2 write or update a focused test script when needed, step 3 run the narrow test and focused validation searches.",
        "- Example frontend batch: step 1 write or reuse the focused frontend test script, step 2 run that script and inspect generated textual outputs.",
    ];
    if allowed_commands.contains("read_media") || allowed_commands.contains("web_discover") {
        patterns.push("- Example media batch: step 1 use `web_discover` or generation to collect the needed media, docs, or repo artifacts, step 2 use `read_media` or focused reads to verify the resulting media or repo content.");
    }
    patterns.join("\n")
}

fn current_apply_patch_command_format() -> String {
    let grammar = "start: begin_patch hunk+ end_patch\nbegin_patch: \"*** Begin Patch\" LF\nend_patch: \"*** End Patch\" LF?\n\nhunk: add_hunk | delete_hunk | update_hunk\nadd_hunk: \"*** Add File: \" filename LF add_line+\ndelete_hunk: \"*** Delete File: \" filename LF\nupdate_hunk: \"*** Update File: \" filename LF change_move? change?\n\nfilename: /(.+)/\nadd_line: \"+\" /(.*)/ LF -> line\n\nchange_move: \"*** Move to: \" filename LF\nchange: (change_context | change_line)+ eof_line?\nchange_context: (\"@@\" | \"@@ \" /(.+)/) LF\nchange_line: (\"+\" | \"-\" | \" \") /(.*)/ LF\neof_line: \"*** End of File\" LF\n\n%import common.LF\n";
    format!(
        "Use one patch for coordinated multi-file source edits after reads. Patches validate context and fail on mismatch. Raw freeform body. Format type `grammar`, syntax `lark`. Definition: {grammar}"
    )
}

fn current_shell_command_format(active: &str, shell_prompt: &str) -> String {
    let guidance = format!(
        "Use for tests, builds, scripts, package tools, and host-shell behavior. Default timeout is 15 seconds; set timeout_ms explicitly for legitimate long-running one-shot commands. Put verification after edits in a later step only when that verification command is already known. {} {}",
        compact_prompt(shell_prompt),
        long_running_service_guidance(active),
    );
    let schema = "{\"type\":\"object\",\"properties\":{\"command\":{\"type\":\"string\",\"description\":\"The shell script to execute in the user's default shell\"},\"workdir\":{\"type\":\"string\",\"description\":\"The working directory to execute the command in\"},\"timeout_ms\":{\"type\":\"number\",\"description\":\"The timeout for the command in milliseconds\"}},\"required\":[\"command\"],\"additionalProperties\":false}";
    format!("{guidance} JSON object string matching this schema: {schema}")
}

fn long_running_service_guidance(active: &str) -> &'static str {
    let _ = active;
    "Persistent services must never be used as blocking foreground commands. If a command can keep running after readiness, it must be backgrounded or wrapped in a persisted startup script with bounded readiness checks and cleanup; otherwise the command is considered hung and incorrect."
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

    fn command_run_agent_with_capabilities(capabilities: &[&str]) -> AgentManagement {
        let now = Utc::now();
        let mut agent = AgentManagement::new(
            "agent-1".to_string(),
            "thinking-planning".to_string(),
            std::path::PathBuf::from("agents/src/thinking-planning"),
            None,
            true,
            true,
            ProviderConfig {
                tura_llm_name: "flagship_thinking".to_string(),
                stream: true,
                temperature: 0.2,
                max_tokens: 0,
                tool_choice: ToolChoice::Auto,
                time_out_ms: 120_000,
            },
            ValidatorConfig {
                need_validator: false,
                validator_name: None,
            },
            now,
        );
        for capability_name in capabilities {
            agent.add_capability(
                AgentCapabilityItem {
                    capability_name: (*capability_name).to_string(),
                    capability_directory: std::path::PathBuf::from("crates/tools/src"),
                },
                now,
            );
        }
        agent
    }

    fn command_run_interface() -> serde_json::Value {
        serde_json::json!({
            "name": COMMAND_RUN_TOOL,
            "description": "Run tools as a pure batch+step command runner. Use assistant content only for concise reasoning, progress, and conclusions. Available commands: apply_patch, bash, shell_command.\nCommand line formats:\n- apply_patch: patch\n- bash: bash details\n- shell_command: shell details",
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
    fn command_run_description_injects_task_status_command_prompt() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        let schema = tool_interface_to_provider_schema(command_run_interface());
        let description = schema
            .pointer("/function/description")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();

        // task_status is advertised as an available command.
        assert!(
            description.contains("task_status"),
            "description missing task_status command"
        );
        assert!(
            description
                .contains("Reminder: settle the task state with the last task_status command."),
            "description missing task_status reminder"
        );
        // The schema enum is injected too.
        assert!(
            description.contains("\"enum\":[\"question\",\"done\"]"),
            "description missing task_status schema enum"
        );
    }

    #[test]
    fn planning_command_does_not_replace_task_status_prompt() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        let mut commands = default_command_run_commands();
        commands.insert("planning".to_string());
        let schema = tool_interface_to_provider_schema_with_commands(
            command_run_interface(),
            Some(&commands),
        );
        let description = schema
            .pointer("/function/description")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();

        assert!(description.contains("task_status"));
        assert!(description
            .contains("Reminder: settle the task state with the last task_status command."));
        assert!(!description.contains("Continue working toward the active thread goal."));
        assert!(!description.contains("[current objective]:"));
        assert!(!description.to_ascii_lowercase().contains("budget"));
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
    fn planning_capability_adds_command_for_configured_agent_capabilities() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|err| err.into_inner());
        let agent = command_run_agent_with_capabilities(&[
            "command_run",
            "apply_patch",
            "shell_command",
            "task_status",
            "planning",
        ]);

        let commands = command_run_commands_for_agent(&agent);

        assert!(commands.contains("planning"));
        assert!(commands.contains("task_status"));
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
        assert!(parameters["properties"].get("task_status").is_none());
        assert!(command_required.contains(&serde_json::json!("command_type")));
        assert!(command_required.contains(&serde_json::json!("step")));
        std::env::remove_var("TURA_COMMAND_RUN_STRICT_JSON");
    }

    #[test]
    fn command_run_provider_description_exposes_only_shell_command_surface() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|err| err.into_inner());
        std::env::set_var("TURA_COMMAND_RUN_SHELL", "shell_command");

        let schema = tool_interface_to_provider_schema(command_run_interface());
        let description = schema["function"]["description"]
            .as_str()
            .unwrap_or_default();

        assert!(description.contains(
            "Available commands: apply_patch, shell_command, read_media, web_discover, compact_context, task_status."
        ));
        assert!(description.contains(
            "Use assistant content only for concise reasoning, progress, and conclusions."
        ));
        assert!(description.contains("- shell_command:"));
        assert!(description.contains("- read_media:"));
        assert!(description.contains("- web_discover:"));
        assert!(description.contains("- compact_context:"));
        assert!(description.contains("- task_status:"));
        assert!(!description.contains("- planning:"));
        assert!(description.contains("\"command\":{\"type\":\"string\""));
        assert!(description.contains("\"workdir\":{\"type\":\"string\""));
        assert!(description.contains("\"timeout_ms\":{\"type\":\"number\""));
        assert!(description.contains("Default timeout is 15 seconds"));
        assert!(description
            .contains("Persistent services must never be used as blocking foreground commands"));
        assert!(description.contains(
            "monitor early process exit while waiting for readiness and fail immediately"
        ));
        assert!(description.contains("check for process exit on every readiness poll"));
        assert!(description.contains(
            "If the service exits before readiness, immediately kill/clear only that process tree"
        ));
        assert!(description.contains("otherwise the command is considered hung and incorrect"));
        assert!(!description.contains("Start-Process -WindowStyle Hidden -PassThru"));
        assert!(!description.contains("Stop-Process -Id $p1.Id,$p2.Id -Force"));
        assert!(!description.contains("p1=$(node server.mjs 4173"));
        assert!(!description.contains("Available commands: apply_patch, bash"));
        assert!(!description.contains("- bash:"));

        std::env::remove_var("TURA_COMMAND_RUN_SHELL");
    }

    #[test]
    fn command_run_provider_description_exposes_only_bash_surface() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|err| err.into_inner());
        std::env::set_var("TURA_COMMAND_RUN_SHELL", "bash");

        let schema = tool_interface_to_provider_schema(command_run_interface());
        let description = schema["function"]["description"]
            .as_str()
            .unwrap_or_default();

        assert!(description.contains(
            "Available commands: apply_patch, bash, read_media, web_discover, compact_context, task_status."
        ));
        assert!(description.contains(
            "Use assistant content only for concise reasoning, progress, and conclusions."
        ));
        assert!(description.contains("- bash:"));
        assert!(description.contains("- read_media:"));
        assert!(description.contains("- web_discover:"));
        assert!(description.contains("- compact_context:"));
        assert!(description.contains("- task_status:"));
        assert!(!description.contains("- planning:"));
        assert!(description.contains("\"command\":{\"type\":\"string\""));
        assert!(description.contains("\"workdir\":{\"type\":\"string\""));
        assert!(description.contains("\"timeout_ms\":{\"type\":\"number\""));
        assert!(description.contains("Default timeout is 15 seconds"));
        assert!(description
            .contains("Persistent services must never be used as blocking foreground commands"));
        assert!(description.contains(
            "monitor early process exit while waiting for readiness and fail immediately"
        ));
        assert!(description.contains("check for process exit on every readiness poll"));
        assert!(description.contains(
            "If the service exits before readiness, immediately kill/clear only that process tree"
        ));
        assert!(description.contains("otherwise the command is considered hung and incorrect"));
        assert!(!description.contains("Start-Process -WindowStyle Hidden -PassThru"));
        assert!(!description.contains("Stop-Process -Id $p1.Id,$p2.Id -Force"));
        assert!(!description.contains("p1=$(node server.mjs 4173"));
        assert!(!description.contains("shell_command"));

        std::env::remove_var("TURA_COMMAND_RUN_SHELL");
    }

    #[test]
    fn command_run_provider_description_injects_planning_only_when_enabled() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|err| err.into_inner());
        std::env::set_var("TURA_COMMAND_RUN_SHELL", "shell_command");
        let mut commands = default_command_run_commands();
        commands.insert("planning".to_string());

        let schema = tool_interface_to_provider_schema_with_commands(
            command_run_interface(),
            Some(&commands),
        );
        let description = schema["function"]["description"]
            .as_str()
            .unwrap_or_default();

        assert!(description.contains(
            "Available commands: apply_patch, shell_command, read_media, web_discover, compact_context, task_status, planning."
        ));
        assert!(description.contains("- planning:"));

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
            now,
        );
        agent.add_capability(
            AgentCapabilityItem {
                capability_name: COMMAND_RUN_TOOL.to_string(),
                capability_directory: root.clone(),
            },
            now,
        );

        let tools = load_agent_capabilities(&agent).expect("tool loading should succeed");
        let descriptions = tools
            .iter()
            .filter_map(|tool| {
                tool.get("function")
                    .and_then(|function| function.get("description"))
                    .and_then(|description| description.as_str())
            })
            .collect::<Vec<_>>()
            .join("\n");

        assert!(!descriptions.contains("common command_run prompt"));
        assert!(!descriptions.contains("apply_patch prompt"));
        assert!(!descriptions.contains("shell command_run prompt"));
        assert!(!descriptions.contains("bash command_run prompt"));

        let _ = std::fs::remove_dir_all(root);
    }
}
