use std::collections::BTreeSet;
use std::path::PathBuf;

use crate::prompt_style::task_status;
use crate::state_machine::agent_management::AgentManagement;
use crate::state_machine::session_management::SessionManagement;

use super::constants::{
    COMMAND_RUN_TOOL, DISABLE_EXECUTE_TOOLS_TOOL_ENV, DISABLE_PLANNING_TOOL_ENV, PROJECT_ROOT_ENV,
};

pub(super) fn load_agent_capabilities_with_commands(
    agent: &AgentManagement,
    session: &SessionManagement,
    allowed_commands: &BTreeSet<String>,
) -> Result<Vec<serde_json::Value>, String> {
    load_agent_capabilities_for_task_state(agent, allowed_commands, session.task_type.is_empty())
}

fn load_agent_capabilities_for_task_state(
    agent: &AgentManagement,
    allowed_commands: &BTreeSet<String>,
    require_startup_task_state: bool,
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
        .map_err(|e| format!("failed to read tool interface: {e}"))?;
    let interface = serde_json::from_str::<serde_json::Value>(&content)
        .map_err(|e| format!("failed to parse tool interface: {e}"))?;

    Ok(vec![tool_interface_to_provider_schema_with_commands(
        interface,
        Some(allowed_commands),
        require_startup_task_state,
    )])
}

pub(crate) fn filter_tools_for_turn(
    tools: Vec<serde_json::Value>,
    _is_final_turn: bool,
    _force_no_tools: bool,
) -> Result<Vec<serde_json::Value>, String> {
    Ok(keep_command_run_only(tools))
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

pub(crate) fn env_flag(name: &str) -> bool {
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

pub(crate) fn planning_child_depth() -> usize {
    std::env::var("TURA_PLANNING_DEPTH")
        .or_else(|_| std::env::var("TURA_EXECUTE_TOOLS_DEPTH"))
        .ok()
        .and_then(|value| value.trim().parse::<usize>().ok())
        .unwrap_or(0)
}

pub(crate) fn project_directory_with_tools() -> Result<PathBuf, String> {
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
    if agent.agent_capabilities.is_empty() {
        return Ok(None);
    }

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
    tool_interface_to_provider_schema_with_commands(interface, None, false)
}

pub(crate) fn command_run_commands_for_agent(agent: &AgentManagement) -> BTreeSet<String> {
    let mut commands = agent
        .agent_capabilities
        .iter()
        .filter_map(|capability| {
            let name = code_tools::commands::canonical_command(&capability.capability_name);
            (name != COMMAND_RUN_TOOL).then_some(name)
        })
        .collect::<BTreeSet<_>>();

    if commands.is_empty() && !agent.agent_capabilities.is_empty() {
        commands = default_command_run_commands();
    }
    commands
}

pub(crate) fn extend_command_run_commands_with_capabilities<I, S>(
    commands: &mut BTreeSet<String>,
    capabilities: I,
) where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    for capability in capabilities {
        let name = code_tools::commands::canonical_command(capability.as_ref());
        if name != COMMAND_RUN_TOOL {
            commands.insert(name);
        }
    }
}

fn default_command_run_commands() -> BTreeSet<String> {
    [
        "apply_patch",
        active_shell_command_name(),
        "web_discover",
        "mcp",
        "task_status",
    ]
    .into_iter()
    .map(str::to_string)
    .collect::<BTreeSet<_>>()
}

fn tool_interface_to_provider_schema_with_commands(
    interface: serde_json::Value,
    allowed_commands: Option<&BTreeSet<String>>,
    require_startup_task_state: bool,
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
        description = command_run_description_for_active_shell(
            &description,
            allowed_commands,
            require_startup_task_state,
        );
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
        Some("zsh") => "zsh",
        Some("shell") | Some("shell_command") | Some("shll") | Some("shall") => "shell_command",
        _ if cfg!(windows) => "shell_command",
        _ if cfg!(target_os = "macos") => "zsh",
        _ => "bash",
    }
}

fn command_run_description_for_active_shell(
    original: &str,
    allowed_commands: Option<&BTreeSet<String>>,
    require_startup_task_state: bool,
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
    let command_lines = command_list_for_description(allowed_commands, active)
        .into_iter()
        .filter_map(|command| command_run_command_format_line(&command, require_startup_task_state))
        .collect::<Vec<_>>();
    format!(
        "{prefix} Available commands: {}.\nCommand run patterns:\n{}\nCommand line formats:\n{}",
        command_list_for_description(allowed_commands, active).join(", "),
        command_run_usage_patterns(allowed_commands),
        command_lines.join("\n"),
    )
}

pub(crate) fn command_run_command_format_line(
    command_id: &str,
    require_startup_task_state: bool,
) -> Option<String> {
    let command_id = code_tools::commands::canonical_command(command_id);
    let active = active_shell_command_name();
    match command_id.as_str() {
        "apply_patch" => Some(format!(
            "- apply_patch: {}",
            current_apply_patch_command_format()
        )),
        command if command == active => {
            let shell_prompt = command_prompt(active);
            Some(format!(
                "- {active}: {}",
                current_shell_command_format(&shell_prompt)
            ))
        }
        "read_media" | "generate_media" | "web_discover" | "mcp" => Some(format!(
            "- {command_id}: {} Schema: {}",
            compact_prompt(&command_prompt(&command_id)),
            compact_schema(&command_schema(&command_id)),
        )),
        "task_status" => {
            let task_status_schema = task_status::task_status_schema(require_startup_task_state);
            Some(format!(
                "- task_status: {} Schema: {}",
                task_status::task_status_prompt(require_startup_task_state),
                compact_schema(&task_status_schema),
            ))
        }
        "planning" => Some(format!(
            "- planning: {} Schema: {}",
            compact_prompt(&command_prompt("planning")),
            compact_schema(code_tools::commands::planning::SCHEMA),
        )),
        _ => None,
    }
}

fn command_prompt(command_id: &str) -> String {
    read_command_file(command_id, "prompt.md")
        .or_else(|| builtin_command_prompt(command_id).map(str::to_string))
        .unwrap_or_default()
}

fn builtin_command_prompt(command_id: &str) -> Option<&'static str> {
    Some(match command_id {
        "apply_patch" => code_tools::commands::apply_patch::PROMPT,
        "bash" => code_tools::commands::bash::PROMPT,
        "planning" => code_tools::commands::planning::PROMPT,
        "shell_command" => code_tools::commands::shell_command::PROMPT,
        "task_status" => code_tools::commands::task_status::PROMPT,
        "zsh" => code_tools::commands::zsh::PROMPT,
        _ => return None,
    })
}

fn command_schema(command_id: &str) -> String {
    read_command_file(command_id, "schema.json").unwrap_or_else(|| "{}".to_string())
}

fn read_command_file(command_id: &str, file_name: &str) -> Option<String> {
    let root = project_directory_with_tools().ok()?;
    [
        root.join("crates")
            .join("tools")
            .join("src")
            .join("commands")
            .join(command_id)
            .join(file_name),
        root.join("commands").join(command_id).join(file_name),
    ]
    .into_iter()
    .find_map(|path| std::fs::read_to_string(path).ok())
}

fn command_list_for_description(commands: &BTreeSet<String>, active_shell: &str) -> Vec<String> {
    let order = [
        "apply_patch",
        active_shell,
        "generate_media",
        "read_media",
        "web_discover",
        "mcp",
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
        "- Current call schema is mandatory: call `command_run` with a non-empty `commands` array only. Every command object must include `command_type`, `command_line`, and `step`. Historical replay may show `arguments: {}` as a bookkeeping placeholder; never copy that placeholder into a new call.",
        "- Batch investigation: use early commands for the specific discovery, searches, and file reads needed to understand the failure surface.",
        "- Keep related path listing, targeted search, and candidate file reads in the same command_run batch; independent commands with no output dependency must share one step.",
        "- Do not run test/probe invocations before you have read the relevant code and determined the actual CLI command set.",
        "- Use steps as dependency groups, not command indexes. Commands in the same step must have no output dependency on each other and may run together; commands that depend on earlier output must use later unique ordered steps whose inputs are already known before the batch is created.",
        "- Code repair loop: after discovery has produced enough facts, use one step for coordinated edits and later steps for already-known tests or focused validation.",
        "- Avoid embedding long generated source code or complex quoting directly in shell command lines; for complex logic, invoke a script/interpreter from the active shell rather than encoding the logic in shell syntax.",
        "- Verification: run the relevant test or build command after edits in the same command_run only when the verification command is already known.",
        "- Failure handling: inspect each failed item and change the next command based on that failure instead of retrying the same command.",
        "- Example investigation batch: independent `rg --files`, targeted `rg -n`, and candidate file reads all use step 1.",
        "- Example repair batch: step 1 `apply_patch` across related files, step 2 run the known build command and use `apply_patch` to write or modify the testing scripts, step 3 run multiple known test commands in the same step.",
        "- Example frontend batch: step 1 write or reuse the focused frontend test script, step 2 run that script and inspect generated textual outputs.",
        "- Example long-running database check: step 1 run `sleep 60` with `timeout_ms` comfortably above 60000, step 2 run the known database probe script, step 3 read the script output log such as `logs/db-check.log` and summarize the findings.",
    ];
    if allowed_commands.contains("task_status") {
        patterns.push("- Context compaction: after a meaningful phase completes, or when context is near the active context limit and feels crowded, put the handoff summary in `task_status.compact_context` after the work it summarizes.");
    }
    if allowed_commands.contains("read_media") || allowed_commands.contains("generate_media") {
        patterns.push("- Example media batch: step 1 use `web_discover` or `generate_media` to collect the needed media, docs, or repo artifacts, step 2 use `read_media` or focused reads to verify the resulting media or repo content.");
    } else if allowed_commands.contains("web_discover") {
        patterns.push("- Example web discovery batch: step 1 use `web_discover` to collect the needed web docs or references, step 2 use focused reads or probes to verify the resulting repo content.");
    }
    patterns.join("\n")
}

fn current_apply_patch_command_format() -> String {
    let grammar = "start: begin_patch hunk+ end_patch\nbegin_patch: \"*** Begin Patch\" LF\nend_patch: \"*** End Patch\" LF?\n\nhunk: add_hunk | delete_hunk | update_hunk\nadd_hunk: \"*** Add File: \" filename LF add_line+\ndelete_hunk: \"*** Delete File: \" filename LF\nupdate_hunk: \"*** Update File: \" filename LF change_move? change?\n\nfilename: /(.+)/\nadd_line: \"+\" /(.*)/ LF -> line\n\nchange_move: \"*** Move to: \" filename LF\nchange: (change_context | change_line)+ eof_line?\nchange_context: (\"@@\" | \"@@ \" /(.+)/) LF\nchange_line: (\"+\" | \"-\" | \" \") /(.*)/ LF\neof_line: \"*** End of File\" LF\n\n%import common.LF\n";
    let prompt = compact_prompt(&command_prompt("apply_patch"));
    format!(
        "{prompt} Use one patch for coordinated multi-file source edits after reads. Patches validate context and fail on mismatch. Raw freeform body. Format type `grammar`, syntax `lark`. Definition: {grammar}"
    )
}

fn current_shell_command_format(shell_prompt: &str) -> String {
    let guidance = format!(
        "Use for tests, builds, scripts, package tools, and host-shell behavior. Default timeout is 15 seconds; set timeout_ms explicitly for legitimate long-running one-shot commands. Put verification after edits in a later step only when that verification command is already known. Delete commands are allowed only when every delete target is a literal path inside the workspace; variable targets such as `$file.FullName` may be blocked. {} {}",
        compact_prompt(shell_prompt),
        long_running_service_guidance(),
    );
    let schema = "{\"type\":\"object\",\"properties\":{\"command\":{\"type\":\"string\",\"description\":\"The shell script to execute in the user's default shell\"},\"workdir\":{\"type\":\"string\",\"description\":\"The working directory to execute the command in\"},\"timeout_ms\":{\"type\":\"number\",\"description\":\"The timeout for the command in milliseconds\"}},\"required\":[\"command\"],\"additionalProperties\":false}";
    format!("{guidance} JSON object string matching this schema: {schema}")
}

fn long_running_service_guidance() -> &'static str {
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
    use crate::manas::constants::PLANNING_TOOL;
    use crate::state_machine::agent_management::{
        AgentCapabilityItem, ProviderConfig, ToolChoice, ValidatorConfig,
    };
    use crate::state_machine::session_management::SessionInput;
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
            "thoughtful".to_string(),
            std::path::PathBuf::from("agents/src/thoughtful"),
            None,
            true,
            true,
            false,
            false,
            ProviderConfig {
                tura_llm_name: "thinking".to_string(),
                default_model_tier: None,
                current_model: None,
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

    fn session_with_task_type(task_type: Vec<String>) -> SessionManagement {
        SessionManagement::new(
            "session-test".to_string(),
            "Test session".to_string(),
            std::path::PathBuf::from("C:/workspace"),
            false,
            task_type,
            SessionInput {
                user_input: "test task".to_string(),
                file_input: vec![],
                agent: None,
                runtime_context: None,
                planning_mode_override: None,
            },
            "test goal".to_string(),
            Utc::now(),
        )
    }

    fn command_type_enum(schema: &serde_json::Value) -> Vec<String> {
        schema["function"]["parameters"]["properties"]["commands"]["items"]["properties"]
            ["command_type"]["enum"]
            .as_array()
            .expect("command_type enum should be injected")
            .iter()
            .map(|value| {
                value
                    .as_str()
                    .expect("command_type enum value should be a string")
                    .to_string()
            })
            .collect()
    }

    fn assert_command_type_enum(schema: &serde_json::Value, expected: &[&str]) {
        assert_eq!(
            command_type_enum(schema),
            expected
                .iter()
                .map(|value| value.to_string())
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn command_run_schema_injects_task_status_command_and_dynamic_schema() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        let commands = default_command_run_commands();
        let schema = tool_interface_to_provider_schema_with_commands(
            command_run_interface(),
            Some(&commands),
            false,
        );

        assert_command_type_enum(
            &schema,
            &[
                "apply_patch",
                active_shell_command_name(),
                "web_discover",
                "mcp",
                "task_status",
            ],
        );

        let task_status_schema =
            serde_json::from_str::<serde_json::Value>(&task_status::task_status_schema(false))
                .expect("task_status schema should parse");
        assert_eq!(
            task_status_schema["properties"]["status"]["enum"],
            serde_json::json!(["doing", "question", "done"])
        );
        assert_eq!(
            task_status_schema["properties"]["task_type"]["items"]["enum"],
            serde_json::Value::Array(
                crate::prompt_style::runtime_prompt_manual::valid_task_type_ids()
                    .into_iter()
                    .map(serde_json::Value::String)
                    .collect()
            )
        );
    }

    #[test]
    fn command_run_description_warns_not_to_copy_replay_placeholder_arguments() {
        let commands = default_command_run_commands();
        let schema = tool_interface_to_provider_schema_with_commands(
            command_run_interface(),
            Some(&commands),
            false,
        );
        let description = schema["function"]["description"]
            .as_str()
            .expect("command_run description should be a string");

        assert!(
            description.contains("Historical replay may show `arguments: {}`"),
            "{description}"
        );
        assert!(
            description.contains("never copy that placeholder"),
            "{description}"
        );
        assert!(
            description.contains("non-empty `commands` array"),
            "{description}"
        );
    }

    #[test]
    fn task_status_command_format_includes_startup_state_gate_when_required() {
        let line = command_run_command_format_line("task_status", true)
            .expect("task_status command format should exist");

        assert!(
            line.contains(task_status::STARTUP_TASK_STATE_GATE),
            "{line}"
        );
        assert!(line.contains("Schema:"), "{line}");
    }

    #[test]
    fn task_status_command_format_omits_startup_state_gate_when_not_required() {
        let line = command_run_command_format_line("task_status", false)
            .expect("task_status command format should exist");

        assert!(
            !line.contains(task_status::STARTUP_TASK_STATE_GATE),
            "{line}"
        );
        assert!(line.contains("Schema:"), "{line}");
    }

    #[test]
    fn command_run_description_uses_session_task_type_for_startup_gate() {
        let commands = default_command_run_commands();

        let missing_type = tool_interface_to_provider_schema_with_commands(
            command_run_interface(),
            Some(&commands),
            session_with_task_type(Vec::new()).task_type.is_empty(),
        );
        let existing_type = tool_interface_to_provider_schema_with_commands(
            command_run_interface(),
            Some(&commands),
            session_with_task_type(vec!["debug".to_string()])
                .task_type
                .is_empty(),
        );

        let missing_description = missing_type["function"]["description"]
            .as_str()
            .expect("command_run description should be present");
        let existing_description = existing_type["function"]["description"]
            .as_str()
            .expect("command_run description should be present");

        assert!(
            missing_description.contains(task_status::STARTUP_TASK_STATE_GATE),
            "{missing_description}"
        );
        assert!(
            !existing_description.contains(task_status::STARTUP_TASK_STATE_GATE),
            "{existing_description}"
        );
    }

    #[test]
    fn planning_command_extends_task_status_command_schema() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        let mut commands = default_command_run_commands();
        commands.insert("planning".to_string());
        let schema = tool_interface_to_provider_schema_with_commands(
            command_run_interface(),
            Some(&commands),
            false,
        );
        assert!(command_type_enum(&schema).contains(&"task_status".to_string()));
        assert!(command_type_enum(&schema).contains(&"planning".to_string()));
    }

    #[test]
    fn default_non_final_turn_keeps_only_command_run() {
        let filtered = filter_tools_for_turn(
            vec![
                tool(COMMAND_RUN_TOOL),
                tool(PLANNING_TOOL),
                tool("web_search"),
            ],
            false,
            false,
        )
        .expect("filter should succeed");

        assert_eq!(names(filtered), vec![COMMAND_RUN_TOOL]);
    }

    #[test]
    fn planning_mode_still_keeps_only_command_run() {
        let filtered = filter_tools_for_turn(
            vec![tool(COMMAND_RUN_TOOL), tool(PLANNING_TOOL)],
            false,
            false,
        )
        .expect("filter should succeed");

        assert_eq!(names(filtered), vec![COMMAND_RUN_TOOL]);
    }

    #[test]
    fn final_turn_keeps_command_run_schema_for_prompt_cache() {
        let filtered = filter_tools_for_turn(
            vec![
                tool(COMMAND_RUN_TOOL),
                tool(PLANNING_TOOL),
                tool("web_search"),
            ],
            true,
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
            "shells",
            "task_status",
            "planning",
        ]);

        let commands = command_run_commands_for_agent(&agent);

        assert!(commands.contains("planning"));
        assert!(commands.contains("task_status"));
        assert!(commands.contains(active_shell_command_name()));
        assert!(!commands.contains("shells"));
    }

    #[test]
    fn empty_agent_capabilities_do_not_enable_default_command_run_commands() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|err| err.into_inner());
        std::env::set_var("TURA_COMMAND_RUN_SHELL", "shell_command");
        let agent = command_run_agent_with_capabilities(&[]);

        let commands = command_run_commands_for_agent(&agent);

        assert!(
            commands.is_empty(),
            "an agent with no capabilities must not receive default command_run commands"
        );
        std::env::remove_var("TURA_COMMAND_RUN_SHELL");
    }

    #[test]
    fn empty_agent_capabilities_do_not_load_command_run_provider_tool() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|err| err.into_inner());
        std::env::set_var("TURA_COMMAND_RUN_SHELL", "shell_command");
        let agent = command_run_agent_with_capabilities(&[]);
        let session = session_with_task_type(vec!["debug".to_string()]);
        let commands = command_run_commands_for_agent(&agent);

        let tools = load_agent_capabilities_with_commands(&agent, &session, &commands)
            .expect("tool loading should succeed");

        assert!(
            tools.is_empty(),
            "an agent with no capabilities must not receive the command_run provider tool"
        );
        std::env::remove_var("TURA_COMMAND_RUN_SHELL");
    }

    #[test]
    fn runtime_prompt_capabilities_extend_command_run_schema() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|err| err.into_inner());
        std::env::set_var("TURA_COMMAND_RUN_SHELL", "shell_command");
        let mut commands = command_run_commands_for_agent(&command_run_agent_with_capabilities(&[
            "command_run",
            "apply_patch",
            "shells",
            "web_discover",
            "task_status",
        ]));
        extend_command_run_commands_with_capabilities(
            &mut commands,
            ["read_media", "generate_media"],
        );

        let schema = tool_interface_to_provider_schema_with_commands(
            command_run_interface(),
            Some(&commands),
            false,
        );

        assert_command_type_enum(
            &schema,
            &[
                "apply_patch",
                "shell_command",
                "generate_media",
                "read_media",
                "web_discover",
                "task_status",
            ],
        );
        std::env::remove_var("TURA_COMMAND_RUN_SHELL");
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
        assert!(parameters["properties"].get("sandbox").is_none());
        assert!(parameters["properties"].get("task_status").is_none());
        assert!(command_required.contains(&serde_json::json!("command_type")));
        assert!(command_required.contains(&serde_json::json!("step")));
        std::env::remove_var("TURA_COMMAND_RUN_STRICT_JSON");
    }

    #[test]
    fn command_run_provider_schema_exposes_only_shell_command_surface() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|err| err.into_inner());
        std::env::set_var("TURA_COMMAND_RUN_SHELL", "shell_command");

        let commands = default_command_run_commands();
        let schema = tool_interface_to_provider_schema_with_commands(
            command_run_interface(),
            Some(&commands),
            false,
        );
        assert_command_type_enum(
            &schema,
            &[
                "apply_patch",
                "shell_command",
                "web_discover",
                "mcp",
                "task_status",
            ],
        );
        assert_eq!(
            schema["function"]["parameters"]["properties"]["commands"]["items"]["properties"]
                ["command_line"]["type"],
            "string"
        );
        assert_eq!(
            schema["function"]["parameters"]["properties"]["commands"]["items"]["properties"]
                ["step"]["type"],
            "integer"
        );

        std::env::remove_var("TURA_COMMAND_RUN_SHELL");
    }

    #[test]
    fn command_run_provider_schema_exposes_only_bash_surface() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|err| err.into_inner());
        std::env::set_var("TURA_COMMAND_RUN_SHELL", "bash");

        let commands = default_command_run_commands();
        let schema = tool_interface_to_provider_schema_with_commands(
            command_run_interface(),
            Some(&commands),
            false,
        );
        assert_command_type_enum(
            &schema,
            &["apply_patch", "bash", "web_discover", "mcp", "task_status"],
        );

        std::env::remove_var("TURA_COMMAND_RUN_SHELL");
    }

    #[test]
    fn command_run_provider_schema_exposes_only_zsh_surface() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|err| err.into_inner());
        std::env::set_var("TURA_COMMAND_RUN_SHELL", "zsh");

        let commands = default_command_run_commands();
        let schema = tool_interface_to_provider_schema_with_commands(
            command_run_interface(),
            Some(&commands),
            false,
        );
        assert_command_type_enum(
            &schema,
            &["apply_patch", "zsh", "web_discover", "mcp", "task_status"],
        );

        std::env::remove_var("TURA_COMMAND_RUN_SHELL");
    }

    #[test]
    fn command_run_provider_schema_injects_planning_only_when_enabled() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|err| err.into_inner());
        std::env::set_var("TURA_COMMAND_RUN_SHELL", "shell_command");
        let mut commands = default_command_run_commands();
        commands.insert("planning".to_string());

        let schema = tool_interface_to_provider_schema_with_commands(
            command_run_interface(),
            Some(&commands),
            false,
        );

        assert_command_type_enum(
            &schema,
            &[
                "apply_patch",
                "shell_command",
                "web_discover",
                "mcp",
                "task_status",
                "planning",
            ],
        );

        std::env::remove_var("TURA_COMMAND_RUN_SHELL");
    }

    #[test]
    fn command_run_capability_loading_uses_agent_commands_for_schema() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|err| err.into_inner());
        let run_id = format!(
            "tura-command-run-schema-test-{}",
            Utc::now().timestamp_nanos_opt().unwrap_or_default()
        );
        let root = std::env::temp_dir().join(run_id);
        let command_run_dir = root.join("command_run");
        std::fs::create_dir_all(&command_run_dir).expect("command_run dir should be created");
        std::fs::write(
            command_run_dir.join("schema.json"),
            command_run_interface().to_string(),
        )
        .expect("command_run schema should be written");

        let now = Utc::now();
        let mut agent = AgentManagement::new(
            "agent".to_string(),
            "general".to_string(),
            root.clone(),
            None,
            true,
            false,
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
            now,
        );
        agent.add_capability(
            AgentCapabilityItem {
                capability_name: COMMAND_RUN_TOOL.to_string(),
                capability_directory: root.clone(),
            },
            now,
        );
        for capability_name in ["apply_patch", "shells", "task_status", "planning"] {
            agent.add_capability(
                AgentCapabilityItem {
                    capability_name: capability_name.to_string(),
                    capability_directory: root.clone(),
                },
                now,
            );
        }

        std::env::set_var("TURA_COMMAND_RUN_SHELL", "shell_command");
        let session = session_with_task_type(vec!["debug".to_string()]);
        let allowed_commands = command_run_commands_for_agent(&agent);
        let tools = load_agent_capabilities_with_commands(&agent, &session, &allowed_commands)
            .expect("tool loading should succeed");
        let command_run = tools.first().expect("command_run tool should load");
        assert_command_type_enum(
            command_run,
            &["apply_patch", "shell_command", "task_status", "planning"],
        );

        std::env::remove_var("TURA_COMMAND_RUN_SHELL");
        let _ = std::fs::remove_dir_all(root);
    }
}
