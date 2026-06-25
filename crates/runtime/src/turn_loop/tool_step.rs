use crate::manas::COMMAND_RUN_TOOL;
use crate::manas::{user_visible_runtime_output_text, user_visible_runtime_text};
use crate::state_machine::runtime_management::RuntimeManagement;
use crate::tool_router::execute_tool::ToolExecutionResult;
use chrono::{DateTime, Utc};

#[derive(Debug, Clone)]
pub(crate) struct PendingCompactContext {
    pub summary: String,
    pub agent_message_content: Option<String>,
    pub agent_message_timestamp: DateTime<Utc>,
}

pub(crate) fn extract_compact_context_results(
    tool_results: &mut [ToolExecutionResult],
    runtime: Option<&RuntimeManagement>,
) -> Vec<PendingCompactContext> {
    let mut pending = Vec::new();
    for tool_result in tool_results.iter_mut() {
        if tool_result.tool_name != COMMAND_RUN_TOOL {
            continue;
        }
        let Some(summary) = compact_context_summary_from_command_run(&tool_result.result) else {
            continue;
        };
        pending.push(PendingCompactContext {
            summary,
            agent_message_content: runtime.and_then(runtime_visible_text),
            agent_message_timestamp: runtime
                .and_then(|runtime| {
                    runtime
                        .call_finished_at
                        .or(runtime.first_token_at)
                        .or(runtime.called_at)
                })
                .unwrap_or_else(Utc::now),
        });
        strip_compact_context_from_command_run(&mut tool_result.arguments, &mut tool_result.result);
        tool_result.success = command_run_result_success_value(&tool_result.result);
        tool_result.error = command_run_result_error_value(&tool_result.result);
    }
    pending
}

fn compact_context_summary_from_command_run(result: &serde_json::Value) -> Option<String> {
    result
        .get("results")
        .and_then(serde_json::Value::as_array)?
        .iter()
        .find_map(compact_context_summary_from_result_item)
}

fn compact_context_summary_from_result_item(item: &serde_json::Value) -> Option<String> {
    if item.get("success").and_then(serde_json::Value::as_bool) != Some(true) {
        return None;
    }
    let command_type = item
        .get("command_type")
        .or_else(|| item.get("command"))
        .and_then(serde_json::Value::as_str)?;
    let output = item.get("output")?;
    match command_type {
        "task_status" => output
            .get("task_status")
            .and_then(|status| status.get("compact_context"))
            .and_then(serde_json::Value::as_str)
            .map(ToString::to_string),
        _ => None,
    }
}

fn strip_compact_context_from_command_run(
    arguments: &mut serde_json::Value,
    result: &mut serde_json::Value,
) {
    if let Some(commands) = arguments
        .get_mut("commands")
        .and_then(serde_json::Value::as_array_mut)
    {
        commands.retain_mut(|command| {
            strip_task_status_compact_context(command);
            true
        });
    }
    if let Some(results) = result
        .get_mut("results")
        .and_then(serde_json::Value::as_array_mut)
    {
        results.retain_mut(|item| {
            strip_task_status_compact_context(item);
            true
        });
    }
}

fn runtime_visible_text(runtime: &RuntimeManagement) -> Option<String> {
    user_visible_runtime_text(&runtime.text)
        .or_else(|| {
            runtime
                .output
                .as_ref()
                .and_then(user_visible_runtime_output_text)
        })
        .map(|text| text.trim().to_string())
        .filter(|text| !text.is_empty())
}

fn strip_task_status_compact_context(value: &mut serde_json::Value) {
    let command_type = value
        .get("command_type")
        .or_else(|| value.get("command"))
        .and_then(serde_json::Value::as_str)
        .map(canonical_command_name);
    if command_type.as_deref() != Some("task_status") {
        return;
    }
    if let Some(object) = value.as_object_mut() {
        object.remove("compact_context");
        strip_task_status_compact_context_from_string_field(object, "command_line");
        strip_task_status_compact_context_from_string_field(object, "command");
        if let Some(inline) = object
            .get_mut("inline_arguments")
            .and_then(serde_json::Value::as_object_mut)
        {
            inline.remove("compact_context");
        }
        if let Some(output_status) = object
            .get_mut("output")
            .and_then(|output| output.get_mut("task_status"))
            .and_then(serde_json::Value::as_object_mut)
        {
            output_status.remove("compact_context");
        }
        if let Some(status) = object
            .get_mut("task_status")
            .and_then(serde_json::Value::as_object_mut)
        {
            status.remove("compact_context");
        }
    }

    fn strip_task_status_compact_context_from_string_field(
        object: &mut serde_json::Map<String, serde_json::Value>,
        field: &str,
    ) {
        let Some(text) = object.get(field).and_then(serde_json::Value::as_str) else {
            return;
        };
        let trimmed = text.trim();
        if !trimmed.starts_with('{') {
            return;
        }
        let Ok(mut value) = serde_json::from_str::<serde_json::Value>(trimmed) else {
            return;
        };
        let Some(payload) = value.as_object_mut() else {
            return;
        };
        if payload.remove("compact_context").is_none() {
            return;
        }
        if let Ok(encoded) = serde_json::to_string(&value) {
            object.insert(field.to_string(), serde_json::Value::String(encoded));
        }
    }
    if let Some(commands) = value
        .get_mut("commands")
        .and_then(serde_json::Value::as_array_mut)
    {
        for command in commands {
            strip_task_status_compact_context(command);
        }
    }
    if let Some(results) = value
        .get_mut("results")
        .and_then(serde_json::Value::as_array_mut)
    {
        for item in results {
            strip_task_status_compact_context(item);
        }
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
    use super::extract_compact_context_results;
    use crate::context::{
        build_messages_from_session, compact_session_context_with_agent_message,
        CompactContextAgentMessage,
    };
    use crate::manas::COMMAND_RUN_TOOL;
    use crate::state_machine::agent_management::{ProviderConfig, ToolChoice};
    use crate::state_machine::runtime_management::{RuntimeManagement, RuntimeProviderConfig};
    use crate::state_machine::session_management::{SessionInput, SessionManagement};
    use crate::tool_router::execute_tool::ToolExecutionResult;
    use chrono::Utc;
    use serde_json::json;
    use std::path::PathBuf;

    fn runtime(session: &SessionManagement) -> RuntimeManagement {
        RuntimeManagement::new(
            "runtime-compact-tool-step".to_string(),
            session.session_id.clone(),
            "agent".to_string(),
            RuntimeProviderConfig {
                base: ProviderConfig {
                    tura_llm_name: "provider".to_string(),
                    default_model_tier: None,
                    current_model: None,
                    stream: false,
                    temperature: 0.0,
                    max_tokens: 0,
                    tool_choice: ToolChoice::Auto,
                    time_out_ms: 120_000,
                },
                thinking: false,
                provider_name: "provider".to_string(),
                model_name: "model".to_string(),
                provider_url_name: "provider".to_string(),
                llm_provider_name: "provider".to_string(),
            },
            Utc::now(),
        )
    }

    #[test]
    fn extract_compact_context_results_accepts_task_status_handoff_and_keeps_status() {
        let mut session = SessionManagement::new(
            "session-task-status-compact".to_string(),
            "task status compact".to_string(),
            PathBuf::from("C:/workspace/task-status-compact"),
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
                    {"command_type": "task_status", "compact_context": "handoff"}
                ]
            }),
            result: json!({
                "results": [
                    {
                        "command_type": "task_status",
                        "success": true,
                        "output": {
                            "task_status": {
                                "task_group": "compact",
                                "status": "doing",
                                "compact_context": "task_status handoff"
                            }
                        }
                    }
                ]
            }),
            success: true,
            error: None,
        }];

        let mut runtime = runtime(&session);
        runtime.text = "Visible runtime reply before checkpoint".to_string();
        let pending = extract_compact_context_results(&mut tool_results, Some(&runtime));

        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].summary, "task_status handoff");
        assert_eq!(
            tool_results[0].result["results"][0]["output"]["task_status"]["status"],
            "doing"
        );
        assert!(
            tool_results[0].result["results"][0]["output"]["task_status"]
                .get("compact_context")
                .is_none()
        );
        compact_session_context_with_agent_message(
            &mut session,
            &pending[0].summary,
            pending[0]
                .agent_message_content
                .as_deref()
                .map(|content| CompactContextAgentMessage {
                    content,
                    timestamp: pending[0].agent_message_timestamp,
                }),
        )
        .expect("apply compact");
        let joined = build_messages_from_session(&session)
            .iter()
            .map(serde_json::Value::to_string)
            .collect::<Vec<_>>()
            .join("\n");
        assert!(session.session_log.iter().any(|entry| {
            let value = serde_json::from_str::<serde_json::Value>(entry).ok();
            value
                .as_ref()
                .and_then(|value| value.get("content"))
                .and_then(serde_json::Value::as_str)
                .is_some_and(|content| {
                    content.contains("task_status handoff")
                        && content.contains("Visible runtime reply before checkpoint")
                        && content.contains("Context rebuild before this checkpoint")
                })
        }));
        assert!(joined.contains("task_status handoff"));
    }
}
