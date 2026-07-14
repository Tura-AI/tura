use chrono::Utc;
use tracing::error;

use crate::runtime::types::ToolCallData;
use crate::state_machine::agent_management::AgentManagement;
use crate::state_machine::runtime_management::RuntimeManagement;
use crate::state_machine::session_management::SessionManagement;
use crate::tool_flow::command_run_result::{
    apply_task_attribution_to_streamed_result, record_streamed_command_events,
};
use crate::tool_flow::task_status::apply_tool_result_session_state_update;
use crate::tool_router::execute_tool::{execute_tool, ExecuteToolInput, ToolExecutionResult};

use crate::gateway_events::{
    publish_step_summary, publish_task_plan_todos, publish_tool_call_record,
    publish_tool_call_started,
};
use crate::manas::constants::COMMAND_RUN_TOOL;
use crate::manas::tool_arguments::normalize_tool_arguments_for_tool;
use crate::manas::tool_catalog::{
    command_run_commands_for_agent, extend_command_run_commands_with_capabilities,
    project_directory_with_tools,
};

use super::permission::{permission_denial_for_tool, request_command_run_sandbox_bypass};

pub(crate) fn execute_tool_calls(
    tool_calls: &[ToolCallData],
    agent: Option<&AgentManagement>,
    session: &mut SessionManagement,
    runtime: &RuntimeManagement,
    _redis_url: &str,
) -> Result<Vec<ToolExecutionResult>, String> {
    let mut results = Vec::new();
    let require_startup_task_state = session.task_type.is_empty();
    let project_directory = project_directory_with_tools()?;
    let tools_directory = project_directory.join("crates").join("tools").join("src");
    let allowed_command_run_commands = agent.map(|agent| {
        let mut commands = command_run_commands_for_agent(agent);
        extend_command_run_commands_with_capabilities(
            &mut commands,
            session.session_capabilities.iter().map(String::as_str),
        );
        commands
    });

    for tool_call in tool_calls {
        let tool_started_at = Utc::now();
        let normalized_arguments = normalize_tool_arguments_for_tool(
            &tool_call.tool_name,
            tool_call.arguments.clone(),
            &session.session_directory,
        );
        let execution_arguments = command_run_arguments_before_startup_apply_patch(
            &tool_call.tool_name,
            normalized_arguments.clone(),
            require_startup_task_state,
        );
        let startup_apply_patch_discarded = startup_apply_patch_was_discarded(
            &tool_call.tool_name,
            &normalized_arguments,
            &execution_arguments,
            require_startup_task_state,
        );
        let execution_tool_call = ToolCallData {
            tool_name: tool_call.tool_name.clone(),
            arguments: execution_arguments.clone(),
            provider_metadata: tool_call.provider_metadata.clone(),
        };
        let has_streamed_command_run_result = tool_call.tool_name == COMMAND_RUN_TOOL
            && streamed_command_run_result(runtime).is_some();
        if !has_streamed_command_run_result {
            publish_tool_call_started(
                session,
                runtime,
                &execution_tool_call,
                execution_arguments.clone(),
                tool_started_at,
            );
            publish_step_summary(session, runtime, &execution_tool_call);
        }
        if startup_apply_patch_discarded && command_run_arguments_are_empty(&execution_arguments) {
            let execution_result = ToolExecutionResult {
                tool_name: tool_call.tool_name.clone(),
                arguments: execution_arguments.clone(),
                result: serde_json::json!({ "results": [] }),
                success: true,
                error: None,
            };
            publish_tool_call_record(
                session,
                runtime,
                &execution_tool_call,
                execution_arguments,
                &execution_result.result,
                true,
                None,
                tool_started_at,
            );
            results.push(execution_result);
            continue;
        }
        if let Some(blocked_result) =
            permission_denial_for_tool(&tool_call.tool_name, &normalized_arguments, runtime)
        {
            publish_tool_call_record(
                session,
                runtime,
                &execution_tool_call,
                execution_arguments,
                &blocked_result.result,
                false,
                blocked_result.error.as_deref(),
                tool_started_at,
            );
            results.push(blocked_result);
            continue;
        }
        if tool_call.tool_name == COMMAND_RUN_TOOL {
            if let Some(streamed_result) = streamed_command_run_result(runtime) {
                let mut streamed_result = streamed_result;
                apply_task_attribution_to_streamed_result(session, &mut streamed_result);
                record_streamed_command_events(session, runtime, &streamed_result);
                let streamed_arguments =
                    streamed_command_run_arguments(&normalized_arguments, &streamed_result);
                let execution_result = ToolExecutionResult {
                    tool_name: tool_call.tool_name.clone(),
                    arguments: streamed_arguments.clone(),
                    success: command_run_result_success(&streamed_result),
                    error: command_run_result_error(&streamed_result),
                    result: streamed_result,
                };
                let mut execution_result = execution_result;
                if apply_tool_result_session_state_update(
                    session,
                    &tool_call.tool_name,
                    &mut execution_result.result,
                ) {
                    publish_task_plan_todos(session);
                    execution_result.success = command_run_result_success(&execution_result.result);
                    execution_result.error = command_run_result_error(&execution_result.result);
                }
                publish_tool_call_record(
                    session,
                    runtime,
                    &execution_tool_call,
                    streamed_arguments,
                    &execution_result.result,
                    execution_result.success,
                    execution_result.error.as_deref(),
                    tool_started_at,
                );
                results.push(execution_result);
                continue;
            }
        }
        let execute_input = ExecuteToolInput {
            tool_name: tool_call.tool_name.clone(),
            arguments: execution_arguments.clone(),
            session_id: session.session_id.clone(),
            runtime_id: runtime.runtime_id.clone(),
            session_directory: session.session_directory.clone(),
            tools_directory: tools_directory.clone(),
            disable_permission_restrictions: session.disable_permission_restrictions,
            allowed_command_run_commands: allowed_command_run_commands.clone(),
        };

        let mut result = tokio::runtime::Runtime::new()
            .map_err(|e| format!("failed to create runtime: {e}"))?
            .block_on(execute_tool(execute_input.clone()));
        if command_run_hit_workspace_sandbox(&tool_call.tool_name, &result)
            && !session.disable_permission_restrictions
        {
            let reason = result
                .as_ref()
                .ok()
                .and_then(|execution_result| execution_result.error.as_deref())
                .unwrap_or("command_run requested access outside the session workspace");
            match request_command_run_sandbox_bypass(runtime, reason) {
                Ok(true) => {
                    let mut approved_input = execute_input.clone();
                    approved_input.disable_permission_restrictions = true;
                    result = tokio::runtime::Runtime::new()
                        .map_err(|e| format!("failed to create runtime: {e}"))?
                        .block_on(execute_tool(approved_input));
                }
                Ok(false) => {
                    result = Ok(ToolExecutionResult {
                        tool_name: tool_call.tool_name.clone(),
                        arguments: execution_arguments.clone(),
                        result: serde_json::json!({
                            "ok": false,
                            "blocked": true,
                            "error": "permission denied by user",
                        }),
                        success: false,
                        error: Some("permission denied by user".to_string()),
                    });
                }
                Err(error) => {
                    let error_message = error.clone();
                    result = Ok(ToolExecutionResult {
                        tool_name: tool_call.tool_name.clone(),
                        arguments: execution_arguments.clone(),
                        result: serde_json::json!({
                            "ok": false,
                            "blocked": true,
                            "error": error_message,
                        }),
                        success: false,
                        error: Some(error),
                    });
                }
            }
        }

        match result {
            Ok(mut execution_result) => {
                if apply_tool_result_session_state_update(
                    session,
                    &tool_call.tool_name,
                    &mut execution_result.result,
                ) {
                    publish_task_plan_todos(session);
                    execution_result.success = command_run_result_success(&execution_result.result);
                    execution_result.error = command_run_result_error(&execution_result.result);
                }
                publish_tool_call_record(
                    session,
                    runtime,
                    &execution_tool_call,
                    execution_arguments,
                    &execution_result.result,
                    execution_result.success,
                    execution_result.error.as_deref(),
                    tool_started_at,
                );
                results.push(execution_result);
            }
            Err(e) => {
                error!(tool_name = %tool_call.tool_name, error = %e, "tool execution failed");
                publish_tool_call_record(
                    session,
                    runtime,
                    &execution_tool_call,
                    execution_arguments.clone(),
                    &serde_json::Value::Null,
                    false,
                    Some(e.as_str()),
                    tool_started_at,
                );
                results.push(ToolExecutionResult {
                    tool_name: tool_call.tool_name.clone(),
                    arguments: execution_arguments,
                    result: serde_json::Value::Null,
                    success: false,
                    error: Some(e),
                });
            }
        }
    }

    Ok(results)
}

fn streamed_command_run_result(runtime: &RuntimeManagement) -> Option<serde_json::Value> {
    runtime
        .output
        .as_ref()?
        .get("streamed_command_run_result")
        .cloned()
}

fn streamed_command_run_arguments(
    fallback: &serde_json::Value,
    streamed_result: &serde_json::Value,
) -> serde_json::Value {
    streamed_result
        .get("commands")
        .filter(|commands| commands.as_array().is_some_and(|items| !items.is_empty()))
        .map(|commands| {
            serde_json::json!({
                "commands": commands,
            })
        })
        .unwrap_or_else(|| fallback.clone())
}

fn command_run_arguments_before_startup_apply_patch(
    tool_name: &str,
    arguments: serde_json::Value,
    require_startup_task_state: bool,
) -> serde_json::Value {
    if !require_startup_task_state || tool_name != COMMAND_RUN_TOOL {
        return arguments;
    }
    truncate_command_run_arguments_before_apply_patch(arguments)
}

fn truncate_command_run_arguments_before_apply_patch(
    mut arguments: serde_json::Value,
) -> serde_json::Value {
    let Some(commands) = arguments
        .get_mut("commands")
        .and_then(serde_json::Value::as_array_mut)
    else {
        return arguments;
    };
    if let Some(index) = commands.iter().position(command_is_apply_patch) {
        commands.truncate(index);
    }
    arguments
}

fn startup_apply_patch_was_discarded(
    tool_name: &str,
    original_arguments: &serde_json::Value,
    execution_arguments: &serde_json::Value,
    require_startup_task_state: bool,
) -> bool {
    require_startup_task_state
        && tool_name == COMMAND_RUN_TOOL
        && command_run_arguments_len(execution_arguments)
            < command_run_arguments_len(original_arguments)
}

fn command_run_arguments_are_empty(arguments: &serde_json::Value) -> bool {
    command_run_arguments_len(arguments) == 0
}

fn command_run_arguments_len(arguments: &serde_json::Value) -> usize {
    arguments
        .get("commands")
        .and_then(serde_json::Value::as_array)
        .map(Vec::len)
        .unwrap_or(0)
}

fn command_is_apply_patch(command: &serde_json::Value) -> bool {
    command
        .get("command")
        .or_else(|| command.get("command_type"))
        .and_then(serde_json::Value::as_str)
        .map(code_tools::commands::canonical_command)
        .as_deref()
        == Some("apply_patch")
}

fn command_run_result_success(output: &serde_json::Value) -> bool {
    output
        .get("results")
        .and_then(serde_json::Value::as_array)
        .map(|results| {
            results.iter().all(|result| {
                result.get("success").and_then(serde_json::Value::as_bool) == Some(true)
            })
        })
        .unwrap_or(true)
}

fn command_run_result_error(output: &serde_json::Value) -> Option<String> {
    if command_run_result_success(output) {
        return None;
    }
    output
        .get("results")
        .and_then(serde_json::Value::as_array)
        .and_then(|results| {
            results.iter().find_map(|result| {
                if result.get("success").and_then(serde_json::Value::as_bool) == Some(false) {
                    result
                        .get("error")
                        .and_then(serde_json::Value::as_str)
                        .or_else(|| result.get("output").and_then(serde_json::Value::as_str))
                        .map(ToString::to_string)
                } else {
                    None
                }
            })
        })
}

fn command_run_hit_workspace_sandbox(
    tool_name: &str,
    result: &Result<ToolExecutionResult, String>,
) -> bool {
    if tool_name != COMMAND_RUN_TOOL {
        return false;
    }
    let text = match result {
        Ok(result) => format!(
            "{} {}",
            result.error.as_deref().unwrap_or_default(),
            result.result
        ),
        Err(error) => error.clone(),
    };
    text.contains("command denied by sandbox policy") || text.contains("path outside workspace")
}

#[cfg(test)]
mod tests {
    use super::{
        command_run_arguments_are_empty, command_run_arguments_before_startup_apply_patch,
        command_run_hit_workspace_sandbox, command_run_result_error, command_run_result_success,
        startup_apply_patch_was_discarded, streamed_command_run_arguments,
        streamed_command_run_result,
    };
    use crate::state_machine::agent_management::{ProviderConfig, ToolChoice};
    use crate::state_machine::runtime_management::{RuntimeManagement, RuntimeProviderConfig};
    use crate::tool_router::execute_tool::ToolExecutionResult;
    use chrono::Utc;
    use serde_json::json;

    fn runtime_with_output(output: serde_json::Value) -> RuntimeManagement {
        let mut runtime = RuntimeManagement::new(
            "runtime-tool-flow".to_string(),
            "session-tool-flow".to_string(),
            "agent-tool-flow".to_string(),
            RuntimeProviderConfig {
                base: ProviderConfig {
                    tura_llm_name: "fast".to_string(),
                    default_model_tier: None,
                    current_model: None,
                    stream: true,
                    temperature: 0.0,
                    max_tokens: 1024,
                    tool_choice: ToolChoice::Auto,
                    time_out_ms: 30_000,
                },
                thinking: false,
                provider_name: "openai".to_string(),
                model_name: "gpt-test".to_string(),
                provider_url_name: "openai".to_string(),
                llm_provider_name: "openai".to_string(),
            },
            Utc::now(),
        );
        runtime.set_output(output);
        runtime
    }

    #[test]
    fn streamed_command_run_result_is_read_from_runtime_output() {
        let runtime = runtime_with_output(json!({
            "streamed_command_run_result": {
                "commands": [{ "command": "echo ok" }],
                "results": [{ "success": true }]
            }
        }));

        assert_eq!(
            streamed_command_run_result(&runtime),
            Some(json!({
                "commands": [{ "command": "echo ok" }],
                "results": [{ "success": true }]
            }))
        );
    }

    #[test]
    fn streamed_command_arguments_prefer_completed_commands_over_fallback() {
        let fallback = json!({ "command": "fallback" });
        let streamed = json!({
            "commands": [{ "command": "echo streamed" }],
            "results": [{ "success": true }]
        });

        assert_eq!(
            streamed_command_run_arguments(&fallback, &streamed),
            json!({ "commands": [{ "command": "echo streamed" }] })
        );
        assert_eq!(
            streamed_command_run_arguments(&fallback, &json!({ "commands": [] })),
            fallback
        );
    }

    #[test]
    fn startup_task_state_command_run_arguments_stop_before_apply_patch() {
        let original = json!({
            "commands": [
                { "command_type": "task_status", "command_line": "{\"task_type\":[\"debug\"]}" },
                { "command_type": "apply_patch", "command_line": "ignored patch body" },
                { "command_type": "shell_command", "command_line": "should not run" }
            ]
        });

        let truncated =
            command_run_arguments_before_startup_apply_patch("command_run", original.clone(), true);

        assert_eq!(
            truncated,
            json!({
                "commands": [
                    { "command_type": "task_status", "command_line": "{\"task_type\":[\"debug\"]}" }
                ]
            })
        );
        assert!(startup_apply_patch_was_discarded(
            "command_run",
            &original,
            &truncated,
            true,
        ));
        assert!(!command_run_arguments_are_empty(&truncated));
    }

    #[test]
    fn startup_task_state_leading_apply_patch_becomes_empty_command_run() {
        let original = json!({
            "commands": [
                { "command_type": "apply_patch", "command_line": "ignored patch body" },
                { "command_type": "shell_command", "command_line": "should not run" }
            ]
        });

        let truncated =
            command_run_arguments_before_startup_apply_patch("command_run", original.clone(), true);

        assert_eq!(truncated, json!({ "commands": [] }));
        assert!(command_run_arguments_are_empty(&truncated));
        assert!(startup_apply_patch_was_discarded(
            "command_run",
            &original,
            &truncated,
            true,
        ));
    }

    #[test]
    fn command_run_result_success_requires_all_result_items_to_succeed() {
        assert!(command_run_result_success(&json!({
            "results": [{ "success": true }, { "success": true }]
        })));
        assert!(!command_run_result_success(&json!({
            "results": [{ "success": true }, { "success": false }]
        })));
        assert!(command_run_result_success(&json!({})));
    }

    #[test]
    fn command_run_result_error_reports_first_failing_result_context() {
        let with_error = json!({
            "results": [
                { "success": true },
                { "success": false, "error": "first failure" },
                { "success": false, "output": "second failure" }
            ]
        });
        assert_eq!(
            command_run_result_error(&with_error).as_deref(),
            Some("first failure")
        );

        let with_output = json!({
            "results": [{ "success": false, "output": "stderr fallback" }]
        });
        assert_eq!(
            command_run_result_error(&with_output).as_deref(),
            Some("stderr fallback")
        );
    }

    #[test]
    fn workspace_sandbox_detection_is_limited_to_command_run() {
        let denied = Ok(ToolExecutionResult {
            tool_name: "command_run".to_string(),
            arguments: json!({ "command": "cat ../secret" }),
            result: json!({ "error": "path outside workspace" }),
            success: false,
            error: Some("command denied by sandbox policy".to_string()),
        });

        assert!(command_run_hit_workspace_sandbox("command_run", &denied));
        assert!(!command_run_hit_workspace_sandbox("planning", &denied));

        let failed: Result<ToolExecutionResult, String> =
            Err("path outside workspace: ../secret".to_string());
        assert!(command_run_hit_workspace_sandbox("command_run", &failed));
    }
}
