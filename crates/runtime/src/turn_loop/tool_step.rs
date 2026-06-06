use crate::context::compact_session_context;
use crate::manas::COMMAND_RUN_TOOL;
use crate::state_machine::session_management::SessionManagement;
use crate::tool_router::execute_tool::ToolExecutionResult;

pub(crate) fn apply_compact_context_results(
    session: &mut SessionManagement,
    tool_results: &mut [ToolExecutionResult],
) -> Result<(), String> {
    for tool_result in tool_results.iter_mut() {
        if tool_result.tool_name != COMMAND_RUN_TOOL {
            continue;
        }
        let Some(summary) = compact_context_summary_from_command_run(&tool_result.result) else {
            continue;
        };
        compact_session_context(session, &summary)?;
        strip_compact_context_from_command_run(&mut tool_result.arguments, &mut tool_result.result);
        tool_result.success = command_run_result_success_value(&tool_result.result);
        tool_result.error = command_run_result_error_value(&tool_result.result);
    }
    Ok(())
}

fn compact_context_summary_from_command_run(result: &serde_json::Value) -> Option<String> {
    result
        .get("results")
        .and_then(serde_json::Value::as_array)?
        .iter()
        .find(|item| {
            item.get("command_type")
                .or_else(|| item.get("command"))
                .and_then(serde_json::Value::as_str)
                == Some("compact_context")
                && item.get("success").and_then(serde_json::Value::as_bool) == Some(true)
        })
        .and_then(|item| {
            item.get("output")
                .and_then(|output| {
                    output
                        .get("compact_context")
                        .and_then(serde_json::Value::as_str)
                        .or_else(|| output.as_str())
                })
                .map(ToString::to_string)
        })
}

fn strip_compact_context_from_command_run(
    arguments: &mut serde_json::Value,
    result: &mut serde_json::Value,
) {
    if let Some(commands) = arguments
        .get_mut("commands")
        .and_then(serde_json::Value::as_array_mut)
    {
        commands.retain(|command| {
            command
                .get("command_type")
                .or_else(|| command.get("command"))
                .and_then(serde_json::Value::as_str)
                .map(canonical_command_name)
                .as_deref()
                != Some("compact_context")
        });
    }
    if let Some(results) = result
        .get_mut("results")
        .and_then(serde_json::Value::as_array_mut)
    {
        results.retain(|item| {
            item.get("command_type")
                .or_else(|| item.get("command"))
                .and_then(serde_json::Value::as_str)
                != Some("compact_context")
        });
    }
}

fn canonical_command_name(name: &str) -> String {
    name.trim().to_ascii_lowercase().replace('-', "_")
}

pub(crate) fn command_run_results_empty(result: &serde_json::Value) -> bool {
    result
        .get("results")
        .and_then(serde_json::Value::as_array)
        .is_some_and(|results| results.is_empty())
}

fn command_run_result_success_value(result: &serde_json::Value) -> bool {
    result
        .get("results")
        .and_then(serde_json::Value::as_array)
        .map(|results| {
            results
                .iter()
                .all(|item| item.get("success").and_then(serde_json::Value::as_bool) == Some(true))
        })
        .unwrap_or(true)
}

fn command_run_result_error_value(result: &serde_json::Value) -> Option<String> {
    if command_run_result_success_value(result) {
        return None;
    }
    result
        .get("results")
        .and_then(serde_json::Value::as_array)
        .and_then(|results| {
            results.iter().find_map(|item| {
                if item.get("success").and_then(serde_json::Value::as_bool) == Some(false) {
                    item.get("error")
                        .and_then(serde_json::Value::as_str)
                        .map(ToString::to_string)
                } else {
                    None
                }
            })
        })
}
