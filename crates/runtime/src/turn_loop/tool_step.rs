use crate::context::compact_session_context;
use crate::manas::COMMAND_RUN_TOOL;
use crate::state_machine::session_management::SessionManagement;
use crate::tool_router::execute_tool::ToolExecutionResult;

pub(crate) fn apply_compact_context_results(
    session: &mut SessionManagement,
    tool_results: &mut [ToolExecutionResult],
) -> Result<bool, String> {
    let mut compact_applied = false;
    for tool_result in tool_results.iter_mut() {
        if tool_result.tool_name != COMMAND_RUN_TOOL {
            continue;
        }
        let Some(summary) = compact_context_summary_from_command_run(&tool_result.result) else {
            continue;
        };
        compact_session_context(session, &summary)?;
        compact_applied = true;
        strip_compact_context_from_command_run(&mut tool_result.arguments, &mut tool_result.result);
        tool_result.success = command_run_result_success_value(&tool_result.result);
        tool_result.error = command_run_result_error_value(&tool_result.result);
    }
    Ok(compact_applied)
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

#[cfg(test)]
mod tests {
    use super::apply_compact_context_results;
    use crate::manas::COMMAND_RUN_TOOL;
    use crate::state_machine::session_management::{SessionInput, SessionManagement};
    use crate::tool_router::execute_tool::ToolExecutionResult;
    use chrono::Utc;
    use serde_json::json;
    use std::path::PathBuf;

    #[test]
    fn apply_compact_context_results_reports_compaction_and_preserves_other_results() {
        let mut session = SessionManagement::new(
            "session-compact-tool-step".to_string(),
            "compact tool step".to_string(),
            PathBuf::from("C:/workspace/compact-tool-step"),
            false,
            "coding".to_string(),
            SessionInput {
                user_input: "compact".to_string(),
                file_input: Vec::new(),
                agent: None,
                runtime_context: None,
                planning_mode_override: None,
            },
            "compact".to_string(),
            Utc::now(),
        );
        let mut tool_results = vec![ToolExecutionResult {
            tool_name: COMMAND_RUN_TOOL.to_string(),
            arguments: json!({
                "commands": [
                    {"command_type": "shell_command", "command_line": "echo ok"},
                    {"command_type": "compact_context", "compact_context": "handoff"}
                ]
            }),
            result: json!({
                "results": [
                    {
                        "command_type": "shell_command",
                        "success": true,
                        "output": {"stdout": "ok"}
                    },
                    {
                        "command_type": "compact_context",
                        "success": true,
                        "output": {"compact_context": "handoff summary"}
                    }
                ]
            }),
            success: true,
            error: None,
        }];

        let compact_applied =
            apply_compact_context_results(&mut session, &mut tool_results).expect("apply compact");

        assert!(compact_applied);
        assert!(tool_results[0].success);
        assert_eq!(tool_results[0].error, None);
        assert_eq!(
            tool_results[0].arguments["commands"]
                .as_array()
                .expect("command_run arguments should keep remaining commands")
                .len(),
            1
        );
        assert_eq!(
            tool_results[0].result["results"]
                .as_array()
                .expect("command_run result should keep remaining results")
                .len(),
            1
        );
        assert!(session.session_log.iter().any(|entry| {
            let value = serde_json::from_str::<serde_json::Value>(entry).ok();
            value
                .as_ref()
                .and_then(|value| value.get("type"))
                .and_then(serde_json::Value::as_str)
                == Some("context_compaction")
        }));
    }
}
