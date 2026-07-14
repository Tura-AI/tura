use chrono::Utc;
use std::time::Instant;
use tracing::info;

use crate::profile_timings;
use crate::state_machine::runtime_management::RuntimeManagement;
use crate::state_machine::session_management::SessionManagement;
use crate::tool_callback_sanitizer::sanitize_tool_callback_output;

use super::compaction::context_compaction_messages;
use super::types::ContextState;
use super::USER_AGENT_CONTEXT_ROLE;
#[derive(Debug, Clone)]
pub struct ContextInput {
    pub session: SessionManagement,
    pub runtime: RuntimeManagement,
    pub additional_messages: Vec<serde_json::Value>,
}

#[derive(Debug, Clone)]
pub struct ContextOutput {
    pub session: SessionManagement,
    pub messages: Vec<serde_json::Value>,
    pub context_state: ContextState,
}

pub fn build_context(input: ContextInput) -> Result<ContextOutput, String> {
    let total_start = Instant::now();
    let profiling = profile_timings::enabled();
    let build_messages_start = Instant::now();
    let mut messages = build_messages_from_session_with_options(&input.session);
    let build_messages_elapsed = build_messages_start.elapsed();
    let initial_message_count = messages.len();
    let initial_messages_bytes = if profiling {
        profile_timings::json_vec_bytes(&messages)
    } else {
        0
    };
    profile_timings::log_duration(
        "build_context.build_messages_from_session",
        build_messages_elapsed,
        serde_json::json!({
            "session_id": input.session.session_id,
            "session_log_entries": input.session.session_log.len(),
            "message_count": initial_message_count,
            "messages_bytes": initial_messages_bytes,
        }),
    );

    let mut context_state = ContextState {
        session_id: input.session.session_id.clone(),
        messages: Vec::new(),
        tool_results: Vec::new(),
        last_tool_call_response: None,
        reasoning_history: Vec::new(),
    };

    if messages.is_empty() {
        if let Some(reasoning) = &input.runtime.reasoning {
            if !reasoning.is_empty() {
                context_state.reasoning_history.push(reasoning.clone());
                messages.push(serde_json::json!({
                    "role": "system",
                    "type": "reasoning",
                    "content": reasoning,
                }));
            }
        }

        if !input.runtime.text.is_empty() {
            messages.push(serde_json::json!({
                "role": "assistant",
                "content": input.runtime.text,
            }));
        }
    } else if let Some(reasoning) = &input.runtime.reasoning {
        if !reasoning.is_empty() {
            context_state.reasoning_history.push(reasoning.clone());
        }
    }

    for tool_call in &input.runtime.tool_call {
        context_state.tool_results.push(serde_json::json!({
            "tool_name": tool_call.tool_called_name,
            "input": tool_call.tool_called_input,
            "summary": tool_call.agent_reported_summary,
            "success": tool_call.tool_reported_success,
        }));
    }

    if input.session.use_last_tool_call_response {
        if let Some(last_tool_call_response) = last_tool_call_response_from_session(&input.session)
        {
            context_state.last_tool_call_response = Some(last_tool_call_response);
        }
    }

    for msg in &input.additional_messages {
        messages.push(msg.clone());
    }

    let clone_start = Instant::now();
    context_state.messages = messages.clone();
    let clone_elapsed = clone_start.elapsed();
    profile_timings::log_duration(
        "build_context.clone_messages_to_context_state",
        clone_elapsed,
        serde_json::json!({
            "session_id": input.session.session_id,
            "message_count": messages.len(),
            "messages_bytes": if profiling {
                profile_timings::json_vec_bytes(&messages)
            } else {
                0
            },
        }),
    );

    info!(
        session_id = %input.session.session_id,
        message_count = messages.len(),
        tool_result_count = context_state.tool_results.len(),
        "context built"
    );

    let total_elapsed = total_start.elapsed();
    profile_timings::log_duration(
        "build_context.total",
        total_elapsed,
        serde_json::json!({
            "session_id": input.session.session_id,
            "session_log_entries": input.session.session_log.len(),
            "message_count": messages.len(),
            "tool_result_count": context_state.tool_results.len(),
            "messages_bytes": if profiling {
                profile_timings::json_vec_bytes(&messages)
            } else {
                0
            },
        }),
    );

    Ok(ContextOutput {
        session: input.session,
        messages,
        context_state,
    })
}

pub fn accumulate_tool_result(
    session: &mut SessionManagement,
    tool_name: &str,
    tool_input: serde_json::Value,
    tool_output: serde_json::Value,
    tool_success: bool,
    tool_error: Option<String>,
) -> Result<(), String> {
    accumulate_tool_result_with_provider_metadata(
        session,
        tool_name,
        tool_input,
        tool_output,
        tool_success,
        tool_error,
        None,
        None,
    )
}

#[expect(
    clippy::too_many_arguments,
    reason = "tool result checkpoints keep runtime id and provider metadata explicit at the persistence boundary"
)]
pub fn accumulate_tool_result_with_provider_metadata(
    session: &mut SessionManagement,
    tool_name: &str,
    tool_input: serde_json::Value,
    tool_output: serde_json::Value,
    tool_success: bool,
    tool_error: Option<String>,
    runtime_id: Option<&str>,
    provider_metadata: Option<serde_json::Value>,
) -> Result<(), String> {
    let total_start = Instant::now();
    let profiling = profile_timings::enabled();
    let tool_input_bytes = if profiling {
        profile_timings::json_bytes(&tool_input)
    } else {
        0
    };
    let tool_output_bytes = if profiling {
        profile_timings::json_bytes(&tool_output)
    } else {
        0
    };
    let now = Utc::now();
    let sequence_start = Instant::now();
    let sequence = session
        .session_log
        .iter()
        .filter_map(|entry| serde_json::from_str::<serde_json::Value>(entry).ok())
        .filter(|value| value.get("type").and_then(|kind| kind.as_str()) == Some("tool_result"))
        .count()
        + 1;
    profile_timings::log_elapsed(
        "accumulate_tool_result.sequence_scan",
        sequence_start,
        serde_json::json!({
            "session_id": session.session_id,
            "tool_name": tool_name,
            "session_log_entries": session.session_log.len(),
            "sequence": sequence,
        }),
    );
    let strip_input_start = Instant::now();
    let stripped_input = strip_tool_reporting_fields(tool_input);
    let strip_input_elapsed = strip_input_start.elapsed();
    profile_timings::log_duration(
        "accumulate_tool_result.strip_input",
        strip_input_elapsed,
        serde_json::json!({
            "session_id": session.session_id,
            "tool_name": tool_name,
            "input_bytes": tool_input_bytes,
            "stripped_input_bytes": if profiling {
                profile_timings::json_bytes(&stripped_input)
            } else {
                0
            },
        }),
    );
    let base_json_start = Instant::now();
    let mut tool_result_json = serde_json::json!({
        "type": "tool_result",
        "tool_name": tool_name,
        "input": stripped_input,
        "output": tool_output,
        "success": tool_success,
        "error": tool_error,
        "sequence": sequence,
        "timestamp": now.to_rfc3339(),
    });
    let base_json_elapsed = base_json_start.elapsed();
    profile_timings::log_duration(
        "accumulate_tool_result.base_json",
        base_json_elapsed,
        serde_json::json!({
            "session_id": session.session_id,
            "tool_name": tool_name,
            "output_bytes": tool_output_bytes,
            "tool_result_bytes": if profiling {
                profile_timings::json_bytes(&tool_result_json)
            } else {
                0
            },
        }),
    );
    if let Some(runtime_id) = runtime_id {
        tool_result_json["runtime_id"] = serde_json::Value::String(runtime_id.to_string());
    }
    if let Some(provider_metadata) = provider_metadata {
        tool_result_json["provider_metadata"] = provider_metadata;
    }
    let context_cache_start = Instant::now();
    tool_result_json["context_cache"] = tool_result_context_cache(&tool_result_json);
    let context_cache_elapsed = context_cache_start.elapsed();
    profile_timings::log_duration(
        "accumulate_tool_result.context_cache",
        context_cache_elapsed,
        serde_json::json!({
            "session_id": session.session_id,
            "tool_name": tool_name,
            "context_cache_bytes": if profiling {
                profile_timings::json_bytes(&tool_result_json["context_cache"])
            } else {
                0
            },
        }),
    );
    let context_messages_start = Instant::now();
    tool_result_json["context_messages"] =
        serde_json::Value::Array(immutable_tool_result_context_messages(&tool_result_json));
    let context_messages_elapsed = context_messages_start.elapsed();
    profile_timings::log_duration(
        "accumulate_tool_result.context_messages",
        context_messages_elapsed,
        serde_json::json!({
            "session_id": session.session_id,
            "tool_name": tool_name,
            "context_messages_bytes": if profiling {
                profile_timings::json_bytes(&tool_result_json["context_messages"])
            } else {
                0
            },
        }),
    );
    let sanitize_output_start = Instant::now();
    tool_result_json["output"] = sanitize_tool_callback_output(&tool_result_json["output"]);
    profile_timings::log_elapsed(
        "accumulate_tool_result.sanitize_output_for_record",
        sanitize_output_start,
        serde_json::json!({
            "session_id": session.session_id,
            "tool_name": tool_name,
            "output_bytes": if profiling {
                profile_timings::json_bytes(&tool_result_json["output"])
            } else {
                0
            },
        }),
    );

    let serialize_start = Instant::now();
    let serialized = serde_json::to_string(&tool_result_json)
        .unwrap_or_else(|_| format!("tool_result: {tool_name}"));
    profile_timings::log_elapsed(
        "accumulate_tool_result.serialize",
        serialize_start,
        serde_json::json!({
            "session_id": session.session_id,
            "tool_name": tool_name,
            "tool_result_bytes": serialized.len(),
        }),
    );
    let push_start = Instant::now();
    session.push_log(serialized, now);
    profile_timings::log_elapsed(
        "accumulate_tool_result.push_log",
        push_start,
        serde_json::json!({
            "session_id": session.session_id,
            "tool_name": tool_name,
            "session_log_entries": session.session_log.len(),
        }),
    );
    profile_timings::log_elapsed(
        "accumulate_tool_result.total",
        total_start,
        serde_json::json!({
            "session_id": session.session_id,
            "tool_name": tool_name,
            "session_log_entries": session.session_log.len(),
            "tool_result_bytes": session.session_log.last().map(|entry| entry.len()).unwrap_or(0),
        }),
    );

    Ok(())
}

pub fn accumulate_message(
    session: &mut SessionManagement,
    role: &str,
    content: serde_json::Value,
) -> Result<(), String> {
    let now = Utc::now();

    let message_json = serde_json::json!({
        "role": role,
        "content": content,
        "created_at": now.timestamp_millis(),
        "updated_at": now.timestamp_millis(),
        "timestamp": now.to_rfc3339(),
    });

    session.push_log(
        serde_json::to_string(&message_json).unwrap_or_else(|_| format!("message: {role}")),
        now,
    );

    Ok(())
}

const USER_MEDIA_START: &str = "[MEDIA:";
const USER_MEDIA_END: &str = ":MEDIA]";

pub fn user_input_content_value(input: &str) -> serde_json::Value {
    let mut parts = Vec::new();
    let mut cursor = 0usize;
    let mut saw_image = false;

    while let Some(relative_start) = input[cursor..].find(USER_MEDIA_START) {
        let start = cursor + relative_start;
        let data_start = start + USER_MEDIA_START.len();
        let Some(relative_end) = input[data_start..].find(USER_MEDIA_END) else {
            break;
        };
        let end = data_start + relative_end;
        let marker_end = end + USER_MEDIA_END.len();
        let media_url = input[data_start..end].trim();

        if media_url.starts_with("data:image/") {
            push_input_text_part(&mut parts, &input[cursor..start]);
            parts.push(serde_json::json!({
                "type": "input_image",
                "image_url": media_url,
            }));
            saw_image = true;
        } else {
            push_input_text_part(&mut parts, &input[cursor..marker_end]);
        }

        cursor = marker_end;
    }

    if !saw_image {
        return serde_json::Value::String(input.to_string());
    }

    push_input_text_part(&mut parts, &input[cursor..]);
    serde_json::Value::Array(parts)
}

pub fn user_input_content_matches(content: &serde_json::Value, input: &str) -> bool {
    content
        .as_str()
        .is_some_and(|text| text.trim() == input.trim())
        || *content == user_input_content_value(input)
}

fn push_input_text_part(parts: &mut Vec<serde_json::Value>, text: &str) {
    if !text.is_empty() {
        parts.push(serde_json::json!({
            "type": "input_text",
            "text": text,
        }));
    }
}

pub fn build_messages_from_session(session: &SessionManagement) -> Vec<serde_json::Value> {
    build_messages_from_session_with_options(session)
}

fn build_messages_from_session_with_options(session: &SessionManagement) -> Vec<serde_json::Value> {
    let mut messages = Vec::new();
    let mut raw_history_messages = Vec::new();
    let mut saw_context_compaction = false;
    for value in session
        .session_log
        .iter()
        .filter_map(|entry| serde_json::from_str::<serde_json::Value>(entry).ok())
    {
        if value.get("type").and_then(|kind| kind.as_str()) == Some("context_compaction") {
            saw_context_compaction = true;
            messages.clear();
            messages.extend(context_compaction_messages(
                &value,
                session,
                &raw_history_messages,
            ));
            raw_history_messages.push(value);
            continue;
        }
        let entry_messages = immutable_context_messages_from_log_entry(value.clone());
        messages.extend(entry_messages);
        raw_history_messages.push(value);
    }

    let raw_initial_user_input = &session.input.user_input;
    let initial_user_input = raw_initial_user_input.trim();
    if !saw_context_compaction
        && !initial_user_input.is_empty()
        && !messages.iter().any(|message| {
            message.get("role").and_then(|role| role.as_str()) == Some("user")
                && message.get("content").is_some_and(|content| {
                    user_input_content_matches(content, raw_initial_user_input)
                })
        })
    {
        messages.insert(
            0,
            serde_json::json!({
                "role": "user",
                "content": user_input_content_value(initial_user_input),
            }),
        );
    }

    messages
}

fn immutable_context_messages_from_log_entry(value: serde_json::Value) -> Vec<serde_json::Value> {
    let Some(obj) = value.as_object() else {
        return Vec::new();
    };
    if let Some(role) = obj.get("role").and_then(|role| role.as_str()) {
        if role == USER_AGENT_CONTEXT_ROLE
            || role == "user"
            || role == "system"
            || role == "developer"
            || role == "assistant"
        {
            let content = obj.get("content");
            let provider_role = if role == USER_AGENT_CONTEXT_ROLE {
                if content.is_some_and(is_developer_context_injection) {
                    "developer"
                } else {
                    "user"
                }
            } else {
                role
            };
            return obj
                .get("content")
                .map(|content| {
                    vec![serde_json::json!({
                    "role": provider_role,
                    "content": content,
                    })]
                })
                .unwrap_or_default();
        }
    }

    if obj.get("type").and_then(|kind| kind.as_str()) != Some("tool_result") {
        return Vec::new();
    }

    if let Some(messages) = obj
        .get("context_messages")
        .and_then(|messages| messages.as_array())
    {
        if value.get("tool_name").and_then(|name| name.as_str()) != Some("command_run")
            || command_run_cached_context_messages_are_valid(messages)
        {
            return messages
                .iter()
                .cloned()
                .map(strip_context_reporting_fields)
                .collect();
        }
    }

    immutable_tool_result_context_messages(&value)
}

fn is_developer_context_injection(content: &serde_json::Value) -> bool {
    content.as_str().is_some_and(|content| {
        let content = content.trim_start();
        content.starts_with("<WORKSPACE_SNAPSHOT>") || content.starts_with("<environment_context>")
    })
}

use super::tool_results::{
    command_run_cached_context_messages_are_valid, immutable_tool_result_context_messages,
    last_tool_call_response_from_session, strip_context_reporting_fields,
    strip_tool_reporting_fields, tool_result_context_cache,
};

#[cfg(test)]
mod tests {
    use super::{
        accumulate_message, accumulate_tool_result, accumulate_tool_result_with_provider_metadata,
        build_context, build_messages_from_session, ContextInput,
    };
    use crate::context::USER_AGENT_CONTEXT_ROLE;
    use crate::context::{compact_session_context, compact_session_context_automatically};
    use crate::state_machine::agent_management::{ProviderConfig, ToolChoice};
    use crate::state_machine::runtime_management::{RuntimeManagement, RuntimeProviderConfig};
    use crate::state_machine::session_management::{
        PlanStatus, PollInterval, SessionInput, SessionManagement, StartCondition, TaskStep,
    };
    use chrono::{Duration, Utc};
    use serde_json::json;
    use std::path::PathBuf;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());
    const PROMPT_STYLE_BODY_FIXTURE: &str = "Prompt style body fixture";

    fn session() -> SessionManagement {
        let now = Utc::now();
        SessionManagement::new(
            "sess-test".to_string(),
            "test".to_string(),
            PathBuf::from("C:/workspace"),
            false,
            "coding".to_string(),
            SessionInput {
                user_input: "inspect".to_string(),
                file_input: vec![],
                agent: None,
                runtime_context: None,
                planning_mode_override: None,
            },
            "inspect".to_string(),
            now,
        )
    }

    fn runtime(session: &SessionManagement) -> RuntimeManagement {
        let now = Utc::now();
        let provider_name = crate::agent_router::coding_agent_provider_name();
        RuntimeManagement::new(
            "runtime-test".to_string(),
            session.session_id.clone(),
            "agent-test".to_string(),
            RuntimeProviderConfig {
                base: ProviderConfig {
                    tura_llm_name: provider_name.clone(),
                    default_model_tier: None,
                    current_model: None,
                    stream: false,
                    temperature: 0.0,
                    max_tokens: 0,
                    tool_choice: ToolChoice::Auto,
                    time_out_ms: 120_000,
                },
                thinking: false,
                provider_name: provider_name.clone(),
                model_name: String::new(),
                provider_url_name: String::new(),
                llm_provider_name: provider_name,
            },
            now,
        )
    }

    #[test]
    fn accumulate_tool_result_keeps_read_media_payload_only_in_context_messages() {
        let mut session = session();
        let image_url = "data:image/png;base64,AAA";

        accumulate_tool_result(
            &mut session,
            "command_run",
            json!({
                "commands": [
                    { "step": 1, "command_type": "read_media", "command_line": "read_media reference.png" }
                ]
            }),
            json!({
                "results": [{
                    "step": 1,
                    "command_type": "read_media",
                    "success": true,
                    "output": {
                        "summary": "reference image",
                        "visual_preview_count": 1,
                        "visual_previews": [{
                            "type": "image_url",
                            "image_url": { "url": image_url }
                        }]
                    }
                }],
                "command_events": [{
                    "status": "completed",
                    "result": {
                        "output": {
                            "visual_preview_count": 1,
                            "visual_previews": [{
                                "type": "image_url",
                                "image_url": { "url": image_url }
                            }]
                        }
                    }
                }]
            }),
            true,
            None,
        )
        .expect("tool result");

        let value = session
            .session_log
            .last()
            .and_then(|entry| serde_json::from_str::<serde_json::Value>(entry).ok())
            .expect("tool result log entry");
        assert_eq!(
            value["output"]["results"][0]["output"]["visual_previews"]["omitted_from_record"],
            true
        );
        assert_eq!(
            value["output"]["command_events"]["omitted_from_record"],
            true
        );
        assert!(value.get("context_message").is_none());

        let output = serde_json::to_string(&value["output"]).expect("output json");
        assert!(!output.contains(image_url), "{output}");

        let context_messages =
            serde_json::to_string(&value["context_messages"]).expect("context messages json");
        assert_eq!(context_messages.matches(image_url).count(), 1);

        let full_record = serde_json::to_string(&value).expect("full tool record");
        assert_eq!(
            full_record.matches(image_url).count(),
            1,
            "only the provider media channel should retain the media payload: {full_record}"
        );
    }

    #[test]
    fn compact_session_context_keeps_pre_checkpoint_tool_context_and_later_results() {
        let root = tempfile::TempDir::new().expect("tempdir");
        std::fs::create_dir_all(root.path().join("src")).expect("src dir");
        std::fs::write(root.path().join("src").join("lib.rs"), "fn main() {}\n").expect("fixture");
        let mut session = session();
        session.session_directory = root.path().to_path_buf();
        session.context_tokens.input = 209_234;
        session.runtime_usage = json!({"input_tokens": 209_234});

        accumulate_tool_result(
                &mut session,
                "command_run",
                json!({
                    "commands": [
                        { "command_type": "shell_command", "command_line": "echo old" }
                    ]
                }),
                json!({
                    "results": [
                        { "step": 1, "command_type": "shell_command", "success": true, "output": "old-tool-secret" }
                    ]
                }),
                true,
                None,
            )
            .expect("old tool result");
        compact_session_context(
            &mut session,
            "Checkpoint: prior tool history is no longer needed. Continue with src/lib.rs.",
        )
        .expect("compact should write");
        assert_eq!(session.context_tokens.input, 0);
        assert!(session.runtime_usage.is_null());
        accumulate_tool_result(
                &mut session,
                "command_run",
                json!({
                    "commands": [
                        { "command_type": "shell_command", "command_line": "echo new" }
                    ]
                }),
                json!({
                    "results": [
                        { "step": 1, "command_type": "shell_command", "success": true, "output": "new-output" }
                    ]
                }),
                true,
                None,
            )
            .expect("new tool result");

        let output = build_context(ContextInput {
            runtime: runtime(&session),
            session,
            additional_messages: vec![],
        })
        .expect("context should build");
        let joined = output
            .messages
            .iter()
            .map(|message| message.to_string())
            .collect::<Vec<_>>()
            .join("\n");

        assert!(joined.contains("Checkpoint: prior tool history is no longer needed"));
        assert!(joined.contains("<WORKSPACE_SNAPSHOT>"));
        assert!(joined.contains("src/lib.rs"));
        assert!(
            joined.contains("old-tool-secret"),
            "compact should replay the immediately preceding command_run output as context: {joined}"
        );
        assert!(
            !joined.contains("\"type\":\"function_call_output\""),
            "command_run context without provider metadata must not create orphan tool outputs: {joined}"
        );
        assert!(joined.contains("new-output"));
    }

    #[test]
    fn compact_session_context_does_not_replay_non_tail_tool_context() {
        let root = tempfile::TempDir::new().expect("tempdir");
        let mut session = session();
        session.session_directory = root.path().to_path_buf();

        accumulate_tool_result(
            &mut session,
            "command_run",
            json!({
                "commands": [
                    { "command_type": "shell_command", "command_line": "echo stale" }
                ]
            }),
            json!({
                "results": [
                    { "step": 1, "command_type": "shell_command", "success": true, "output": "STALE_TOOL_RESULT_BEFORE_MESSAGE" }
                ]
            }),
            true,
            None,
        )
        .expect("stale tool result");
        accumulate_message(
            &mut session,
            "assistant",
            json!("assistant message after stale tool"),
        )
        .expect("assistant message");

        compact_session_context(&mut session, "handoff after assistant message")
            .expect("compact should write");

        let messages = build_messages_from_session(&session);
        let joined = messages
            .iter()
            .map(serde_json::Value::to_string)
            .collect::<Vec<_>>()
            .join("\n");

        assert!(joined.contains("handoff after assistant message"));
        assert!(
            !joined.contains("STALE_TOOL_RESULT_BEFORE_MESSAGE"),
            "compact should only replay tool_result entries at the immediate pre-checkpoint tail: {joined}"
        );
    }

    #[test]
    fn compact_session_context_rebuilds_next_turn_from_timestamped_user_and_agent_timeline() {
        let root = tempfile::TempDir::new().expect("tempdir");
        let base = Utc::now() - Duration::minutes(20);
        let mut session = session();
        session.session_directory = root.path().to_path_buf();
        session.session_started_at = base + Duration::minutes(10);
        session.push_log(
            serde_json::json!({
                "role": "user",
                "content": "old user requirement kept from retained context",
                "timestamp": (base + Duration::minutes(1)).to_rfc3339(),
                "created_at": (base + Duration::minutes(1)).timestamp_millis(),
                "updated_at": (base + Duration::minutes(1)).timestamp_millis()
            })
            .to_string(),
            base + Duration::minutes(1),
        );
        session.push_log(
            serde_json::json!({
                "type": "context_compaction",
                "content": "earlier agent summary that remains in current context",
                "workspace_snapshot": "<WORKSPACE_SNAPSHOT>\nold\n</WORKSPACE_SNAPSHOT>",
                "environment_context": "<environment_context>old</environment_context>",
                "timestamp": (base + Duration::minutes(2)).to_rfc3339()
            })
            .to_string(),
            base + Duration::minutes(2),
        );
        session.push_log(
            serde_json::json!({
                "role": "user",
                "content": "current run real user request",
                "timestamp": (base + Duration::minutes(11)).to_rfc3339(),
                "created_at": (base + Duration::minutes(11)).timestamp_millis(),
                "updated_at": (base + Duration::minutes(11)).timestamp_millis()
            })
            .to_string(),
            base + Duration::minutes(11),
        );
        session.push_log(
            serde_json::json!({
                "role": "assistant",
                "content": "current run visible agent progress",
                "timestamp": (base + Duration::minutes(12)).to_rfc3339(),
                "created_at": (base + Duration::minutes(12)).timestamp_millis(),
                "updated_at": (base + Duration::minutes(12)).timestamp_millis()
            })
            .to_string(),
            base + Duration::minutes(12),
        );

        compact_session_context(&mut session, "new compact handoff text")
            .expect("compact should succeed");
        let messages = build_messages_from_session(&session);
        let joined = messages
            .iter()
            .map(serde_json::Value::to_string)
            .collect::<Vec<_>>()
            .join("\n");

        assert!(
            joined.contains("Context rebuild before this checkpoint"),
            "{joined}"
        );
        assert!(
            joined.contains(&(base + Duration::minutes(2)).to_rfc3339()),
            "{joined}"
        );
        assert!(
            joined.contains("compact_context/user_instruction: earlier agent summary"),
            "{joined}"
        );
        assert!(
            joined.contains(&(base + Duration::minutes(11)).to_rfc3339()),
            "{joined}"
        );
        assert!(
            joined.contains("current_run/user: current run real user request"),
            "{joined}"
        );
        assert!(
            joined.contains("current_run/agent: current run visible agent progress"),
            "{joined}"
        );
        assert!(joined.contains("Agent compact handoff"), "{joined}");
        assert!(joined.contains("new compact handoff text"), "{joined}");
        assert!(
            joined.contains("other_task/user: old user requirement kept from retained context"),
            "{joined}"
        );
        let summary = joined
            .find("earlier agent summary")
            .expect("summary position");
        let old_user = joined
            .find("old user requirement kept from retained context")
            .expect("old user position");
        let user = joined
            .find("current run real user request")
            .expect("user position");
        let agent = joined
            .find("current run visible agent progress")
            .expect("agent position");
        let agent_handoff = joined
            .find("Agent compact handoff")
            .expect("agent handoff heading");
        let goal = joined
            .find("Goal-mode last user command from session state")
            .expect("goal heading");
        assert!(
            old_user < summary && summary < user && user < agent && agent < agent_handoff && agent_handoff < goal,
            "compact rebuild must order previous compact summaries, user/agent history, agent handoff, then goal: {joined}"
        );
    }

    #[test]
    fn goal_mode_compact_appends_recorded_goal_input_after_agent_handoff() {
        let root = tempfile::TempDir::new().expect("tempdir");
        let mut session = session();
        session.session_directory = root.path().to_path_buf();
        session.goal_mode = true;
        session.last_goal_user_input = "original goal command".to_string();

        compact_session_context(&mut session, "agent handoff").expect("compact should succeed");
        let messages = build_messages_from_session(&session);
        let joined = messages
            .iter()
            .map(serde_json::Value::to_string)
            .collect::<Vec<_>>()
            .join("\n");

        let agent = joined.find("agent handoff").expect("agent handoff");
        let goal = joined
            .find("Goal-mode last user command from session state")
            .expect("goal section");
        assert!(agent < goal, "{joined}");
        assert!(joined.contains("original goal command"), "{joined}");
    }

    #[test]
    fn goal_mode_compact_uses_recorded_goal_or_prompt_style_fallback() {
        let root = tempfile::TempDir::new().expect("tempdir");
        let mut goal_session = session();
        goal_session.session_directory = root.path().to_path_buf();
        goal_session.goal_mode = true;
        goal_session.last_goal_user_input = "resume exact goal".to_string();

        compact_session_context(&mut goal_session, "  ").expect("compact should succeed");
        let joined = build_messages_from_session(&goal_session)
            .iter()
            .map(serde_json::Value::to_string)
            .collect::<Vec<_>>()
            .join("\n");
        assert!(joined.contains("Goal-mode last user command from session state"));
        assert!(joined.contains("resume exact goal"));

        let mut empty_session = session();
        empty_session.session_directory = root.path().to_path_buf();
        empty_session.goal_mode = true;
        compact_session_context(&mut empty_session, "  ").expect("compact should succeed");
        let fallback = build_messages_from_session(&empty_session)
            .iter()
            .map(serde_json::Value::to_string)
            .collect::<Vec<_>>()
            .join("\n");
        assert!(fallback.contains("[goal_mode_prompt_style]"), "{fallback}");
        assert!(fallback.contains("Continue working on the previous goal-mode task"));
    }

    #[test]
    fn repeated_compact_inherits_only_two_previous_compact_summaries() {
        let root = tempfile::TempDir::new().expect("tempdir");
        let mut session = session();
        session.session_directory = root.path().to_path_buf();
        session.goal_mode = true;
        session.last_goal_user_input = "exact active goal".to_string();

        compact_session_context(&mut session, "agent handoff A").expect("compact A");
        compact_session_context(&mut session, "agent handoff B").expect("compact B");
        compact_session_context(&mut session, "agent handoff C").expect("compact C");
        compact_session_context(&mut session, "agent handoff D").expect("compact D");

        let messages = build_messages_from_session(&session);
        let joined = messages
            .iter()
            .map(serde_json::Value::to_string)
            .collect::<Vec<_>>()
            .join("\n");

        assert!(joined.contains("agent handoff D"), "{joined}");
        assert!(joined.contains("agent handoff C"), "{joined}");
        assert!(
            !joined.contains("agent handoff B"),
            "only the current compact handoff and one inherited compact summary should be carried forward: {joined}"
        );
        assert!(
            !joined.contains("agent handoff A"),
            "only the current compact handoff and one inherited compact summary should be carried forward: {joined}"
        );
        assert_eq!(
            joined.matches("[inherited_compact_context]").count(),
            1,
            "only one previous compact summary should be inherited beside the current handoff: {joined}"
        );
    }

    #[test]
    fn compact_summary_limit_preserves_timestamp_interleaving() {
        let root = tempfile::TempDir::new().expect("tempdir");
        let base = Utc::now() - Duration::minutes(30);
        let mut session = session();
        session.session_directory = root.path().to_path_buf();
        session.session_started_at = base;
        session.push_log(
            serde_json::json!({
                "type": "context_compaction",
                "content": "old compact handoff A",
                "workspace_snapshot": "<WORKSPACE_SNAPSHOT>\nold-a\n</WORKSPACE_SNAPSHOT>",
                "environment_context": "<environment_context>old-a</environment_context>",
                "timestamp": (base + Duration::minutes(1)).to_rfc3339()
            })
            .to_string(),
            base + Duration::minutes(1),
        );
        session.push_log(
            serde_json::json!({
                "role": "user",
                "content": "user work before compact B",
                "timestamp": (base + Duration::minutes(2)).to_rfc3339(),
                "created_at": (base + Duration::minutes(2)).timestamp_millis(),
                "updated_at": (base + Duration::minutes(2)).timestamp_millis()
            })
            .to_string(),
            base + Duration::minutes(2),
        );
        session.push_log(
            serde_json::json!({
                "type": "context_compaction",
                "content": "compact handoff B",
                "workspace_snapshot": "<WORKSPACE_SNAPSHOT>\nold-b\n</WORKSPACE_SNAPSHOT>",
                "environment_context": "<environment_context>old-b</environment_context>",
                "timestamp": (base + Duration::minutes(3)).to_rfc3339()
            })
            .to_string(),
            base + Duration::minutes(3),
        );
        session.push_log(
            serde_json::json!({
                "role": "assistant",
                "content": "assistant work between compact B and compact C",
                "timestamp": (base + Duration::minutes(4)).to_rfc3339(),
                "created_at": (base + Duration::minutes(4)).timestamp_millis(),
                "updated_at": (base + Duration::minutes(4)).timestamp_millis()
            })
            .to_string(),
            base + Duration::minutes(4),
        );
        session.push_log(
            serde_json::json!({
                "type": "context_compaction",
                "content": "compact handoff C",
                "workspace_snapshot": "<WORKSPACE_SNAPSHOT>\nold-c\n</WORKSPACE_SNAPSHOT>",
                "environment_context": "<environment_context>old-c</environment_context>",
                "timestamp": (base + Duration::minutes(5)).to_rfc3339()
            })
            .to_string(),
            base + Duration::minutes(5),
        );
        session.push_log(
            serde_json::json!({
                "role": "user",
                "content": "user work after compact C",
                "timestamp": (base + Duration::minutes(6)).to_rfc3339(),
                "created_at": (base + Duration::minutes(6)).timestamp_millis(),
                "updated_at": (base + Duration::minutes(6)).timestamp_millis()
            })
            .to_string(),
            base + Duration::minutes(6),
        );

        compact_session_context(&mut session, "current compact handoff D")
            .expect("compact D should succeed");
        let joined = build_messages_from_session(&session)
            .iter()
            .map(serde_json::Value::to_string)
            .collect::<Vec<_>>()
            .join("\n");

        assert!(!joined.contains("old compact handoff A"), "{joined}");
        let user_before_b = joined
            .find("user work before compact B")
            .expect("user before B");
        let compact_b = joined.find("compact handoff B").expect("compact B");
        let assistant_between = joined
            .find("assistant work between compact B and compact C")
            .expect("assistant between");
        let compact_c = joined.find("compact handoff C").expect("compact C");
        let user_after_c = joined
            .find("user work after compact C")
            .expect("user after C");
        assert!(
            user_before_b < compact_b
                && compact_b < assistant_between
                && assistant_between < compact_c
                && compact_c < user_after_c,
            "retained compact summaries must stay timestamp-interleaved with user/assistant context: {joined}"
        );
    }

    #[test]
    fn automatic_compact_context_preserves_recent_tool_results_and_trims_older_history() {
        let root = tempfile::TempDir::new().expect("tempdir");
        let base = Utc::now() - Duration::minutes(90);
        let mut session = session();
        session.session_directory = root.path().to_path_buf();
        session.session_started_at = base;
        session.context_tokens.limit = 10_000;
        for index in 0..180 {
            let content = format!("old-history-{index:02} {}", "x".repeat(900));
            session.push_log(
                serde_json::json!({
                    "role": "user",
                    "content": content,
                    "timestamp": (base + Duration::seconds(index)).to_rfc3339(),
                    "created_at": (base + Duration::seconds(index)).timestamp_millis(),
                    "updated_at": (base + Duration::seconds(index)).timestamp_millis()
                })
                .to_string(),
                base + Duration::seconds(index),
            );
        }
        session.push_log(
            serde_json::json!({
                "type": "tool_result",
                "tool_name": "command_run",
                "output": {
                    "results": [{
                        "step": 1,
                        "command_type": "shell_command",
                        "success": true,
                        "output": "RECENT_TOOL_RESULT_SENTINEL"
                    }]
                },
                "success": true,
                "timestamp": (base + Duration::minutes(80)).to_rfc3339()
            })
            .to_string(),
            base + Duration::minutes(80),
        );

        compact_session_context_automatically(&mut session, "automatic handoff")
            .expect("automatic compact should succeed");
        assert!(
            session.session_log_retention.omitted_entries >= 180,
            "compact should retain a DB-sync boundary for omitted entries"
        );
        let retained_log = session.session_log.join("\n");
        assert!(
            !retained_log.contains("old-history-00"),
            "compacted runtime state should not keep DB-sync-dead history: {retained_log}"
        );
        let compact = session
            .session_log
            .iter()
            .rev()
            .filter_map(|entry| serde_json::from_str::<serde_json::Value>(entry).ok())
            .find(|value| {
                value.get("type").and_then(serde_json::Value::as_str) == Some("context_compaction")
            })
            .expect("compact record should be present");
        let content = compact
            .get("content")
            .and_then(serde_json::Value::as_str)
            .expect("compact content should be text");

        assert!(content.len() <= 4_000 + 100, "{content}");
        assert!(
            content.contains("older timeline entries omitted"),
            "{content}"
        );
        assert!(
            !content.contains("RECENT_TOOL_RESULT_SENTINEL"),
            "tool outputs must remain in normal tool context messages, not compact summaries: {content}"
        );
        assert!(!content.contains("old-history-00"), "{content}");

        let rebuilt = build_messages_from_session(&session)
            .iter()
            .map(serde_json::Value::to_string)
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            rebuilt.contains("RECENT_TOOL_RESULT_SENTINEL"),
            "retained tail tool context must still be available after trimming: {rebuilt}"
        );
    }

    #[test]
    fn compact_session_context_does_not_append_task_management_state() {
        let mut session = session();
        session.task_plan.plan_summary = "Inspect workspace".to_string();
        session.task_plan.detailed_tasks.push(TaskStep {
            task_id: "compact-task".to_string(),
            step: 1,
            task_summary: "Inspect workspace".to_string(),
            step_deliverable_description: "Find relevant files".to_string(),
            status: PlanStatus::Doing,
            ..TaskStep::default()
        });

        compact_session_context(&mut session, "handoff summary").expect("compact should succeed");
        let messages = build_messages_from_session(&session);
        let last = messages
            .last()
            .and_then(|message| message.get("content"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();

        assert!(!last.starts_with("TASK_MANAGEMENT_STATE:"));
        assert!(!last.contains("\"task_id\":\"compact-task\""));
        assert!(!last.contains("\"status\":\"doing\""));
    }

    #[test]
    fn compact_session_context_does_not_embed_prompt_style_records() {
        let root = tempfile::TempDir::new().expect("tempdir");
        let now = Utc::now();
        let mut session = session();
        session.session_directory = root.path().to_path_buf();
        session.session_started_at = now - Duration::minutes(1);
        session.push_log(
            serde_json::json!({
                "type": "prompt_style",
                "role": "developer",
                "content": PROMPT_STYLE_BODY_FIXTURE,
                "timestamp": now.to_rfc3339(),
                "created_at": now.timestamp_millis(),
                "updated_at": now.timestamp_millis(),
            })
            .to_string(),
            now,
        );

        compact_session_context(&mut session, "handoff summary").expect("compact should succeed");
        let messages = build_messages_from_session(&session);
        let joined = messages
            .iter()
            .map(serde_json::Value::to_string)
            .collect::<Vec<_>>()
            .join("\n");

        assert!(joined.contains("handoff summary"));
        assert!(!joined.contains(PROMPT_STYLE_BODY_FIXTURE));
    }

    #[test]
    fn compact_core_replays_pre_checkpoint_command_context_before_prompt_style() {
        let root = tempfile::TempDir::new().expect("tempdir");
        let now = Utc::now();
        let mut session = session();
        session.session_directory = root.path().to_path_buf();
        accumulate_tool_result_with_provider_metadata(
            &mut session,
            "command_run",
            json!({
                "commands": [
                    { "command_type": "shell_command", "command_line": "echo raw" }
                ]
            }),
            json!({
                "results": [
                    { "step": 1, "command_type": "shell_command", "success": true, "output": "RAW_TOOL_RESULT_BEFORE_COMPACT" }
                ]
            }),
            true,
            None,
            Some("runtime-before-compact"),
            Some(json!({ "id": "call_before_compact" })),
        )
        .expect("tool result before compact");

        compact_session_context(&mut session, "agent handoff after history")
            .expect("compact should succeed");
        session.push_log(
            serde_json::json!({
                "type": "prompt_style",
                "role": "developer",
                "content": PROMPT_STYLE_BODY_FIXTURE,
                "timestamp": (now + Duration::seconds(1)).to_rfc3339(),
                "created_at": (now + Duration::seconds(1)).timestamp_millis(),
                "updated_at": (now + Duration::seconds(1)).timestamp_millis(),
            })
            .to_string(),
            now + Duration::seconds(1),
        );

        let messages = build_messages_from_session(&session);
        let joined = messages
            .iter()
            .map(serde_json::Value::to_string)
            .collect::<Vec<_>>()
            .join("\n");
        let contents = messages
            .iter()
            .filter_map(|message| message.get("content").and_then(serde_json::Value::as_str))
            .collect::<Vec<_>>();
        let snapshot = joined
            .find("<WORKSPACE_SNAPSHOT>")
            .expect("workspace snapshot");
        let environment = joined
            .find("<environment_context>")
            .expect("environment context");
        let compact = joined
            .find("Agent compact handoff:\\nagent handoff after history")
            .expect("compact core");
        let prompt_style = joined
            .find(PROMPT_STYLE_BODY_FIXTURE)
            .expect("prompt style");

        assert!(
            snapshot < environment && environment < compact && compact < prompt_style,
            "compact core must be placed after environment context and before prompt_style: {contents:?}"
        );
        let compact_index = messages
            .iter()
            .position(|message| {
                message
                    .get("content")
                    .and_then(serde_json::Value::as_str)
                    .is_some_and(|content| {
                        content.contains("Agent compact handoff:\nagent handoff after history")
                    })
            })
            .expect("compact core message");
        let function_call_index = messages
            .iter()
            .position(|message| {
                message.get("type").and_then(serde_json::Value::as_str) == Some("function_call")
                    && message.get("name").and_then(serde_json::Value::as_str)
                        == Some("command_run")
            })
            .expect("command_run function call");
        let function_output_index = messages
            .iter()
            .position(|message| {
                message.get("type").and_then(serde_json::Value::as_str)
                    == Some("function_call_output")
            })
            .expect("command_run function output");
        let prompt_style_index = messages
            .iter()
            .position(|message| {
                message
                    .get("content")
                    .and_then(serde_json::Value::as_str)
                    .is_some_and(|content| content.contains(PROMPT_STYLE_BODY_FIXTURE))
            })
            .expect("prompt style");
        assert!(
            compact_index < function_call_index
                && function_call_index < function_output_index
                && function_output_index < prompt_style_index,
            "compact must replay previous command_run pair before prompt_style: {joined}"
        );
        assert_eq!(
            messages[function_call_index]["call_id"], messages[function_output_index]["call_id"],
            "command_run call and output must stay paired: {joined}"
        );
        assert!(
            messages[function_output_index]
                .get("output")
                .and_then(serde_json::Value::as_str)
                .is_some_and(|output| output.contains("RAW_TOOL_RESULT_BEFORE_COMPACT")),
            "function_call_output should carry the pre-checkpoint tool output: {joined}"
        );
        let compact_content = contents
            .iter()
            .find(|content| content.contains("Agent compact handoff:\nagent handoff after history"))
            .expect("compact content");
        assert!(
            !compact_content.contains("RAW_TOOL_RESULT_BEFORE_COMPACT"),
            "compact core must not rewrite command_run output: {compact_content}"
        );
    }

    #[test]
    fn build_context_rebuilds_stale_command_run_context_messages_with_empty_arguments() {
        let now = Utc::now();
        let mut session = session();
        session.push_log(
            serde_json::json!({
                "type": "tool_result",
                "tool_name": "command_run",
                "provider_metadata": { "id": "call_stale_context" },
                "context_messages": [
                    {
                        "type": "function_call",
                        "call_id": "call_stale_context",
                        "name": "command_run",
                        "arguments": "{}"
                    },
                    {
                        "type": "function_call_output",
                        "call_id": "call_stale_context",
                        "output": "{\"results\":[{\"command_type\":\"shell_command\",\"command_line\":\"echo stale\"}]}"
                    }
                ],
                "input": {
                    "commands": [{
                        "step": 1,
                        "command_type": "shell_command",
                        "command_line": "echo current"
                    }]
                },
                "output": {
                    "results": [{
                        "step": 1,
                        "command_type": "shell_command",
                        "success": true,
                        "output": {
                            "ok": true,
                            "exit_code": 0,
                            "stdout": "current\n",
                            "stderr": ""
                        }
                    }]
                },
                "success": true,
                "error": null,
                "timestamp": now.to_rfc3339()
            })
            .to_string(),
            now,
        );

        let messages = build_messages_from_session(&session);
        let function_call = messages
            .iter()
            .find(|message| {
                message.get("type").and_then(serde_json::Value::as_str) == Some("function_call")
                    && message.get("call_id").and_then(serde_json::Value::as_str)
                        == Some("call_stale_context")
            })
            .expect("rebuilt command_run function call");
        let arguments: serde_json::Value = serde_json::from_str(
            function_call["arguments"]
                .as_str()
                .expect("arguments JSON string"),
        )
        .expect("arguments JSON");
        assert_eq!(arguments["commands"][0]["command_line"], "echo current");

        let function_output = messages
            .iter()
            .find(|message| {
                message.get("type").and_then(serde_json::Value::as_str)
                    == Some("function_call_output")
                    && message.get("call_id").and_then(serde_json::Value::as_str)
                        == Some("call_stale_context")
            })
            .expect("rebuilt command_run function output");
        let output: serde_json::Value = serde_json::from_str(
            function_output["output"]
                .as_str()
                .expect("output JSON string"),
        )
        .expect("output JSON");
        assert!(output["results"][0].get("step").is_none());
        assert!(output["results"][0].get("command_type").is_none());
        assert!(output["results"][0].get("command_line").is_none());
        assert_eq!(output["results"][0]["output"]["stdout"], "current\n");
    }

    #[test]
    fn compact_drops_pre_checkpoint_streamed_command_context_after_goal() {
        let root = tempfile::TempDir::new().expect("tempdir");
        let now = Utc::now();
        let mut session = session();
        session.session_directory = root.path().to_path_buf();
        session.goal_mode = true;
        session.last_goal_user_input = "exact goal after compact".to_string();
        session.push_log(
            serde_json::json!({
                "type": "streamed_command_event",
                "status": "completed",
                "event_index": 0,
                "step": 1,
                "command_type": "shell_command",
                "command_line": "python -m py_compile sentinel.py",
                "command": {
                    "step": 1,
                    "command_type": "shell_command",
                    "command_line": "python -m py_compile sentinel.py"
                },
                "result": {
                    "step": 1,
                    "command_type": "shell_command",
                    "success": true,
                    "output": "STREAMED_TOOL_RESULT_SENTINEL"
                },
                "timestamp": now.to_rfc3339(),
                "created_at": now.timestamp_millis(),
                "updated_at": now.timestamp_millis(),
            })
            .to_string(),
            now,
        );

        compact_session_context(&mut session, "agent handoff before streamed context")
            .expect("compact should succeed");
        session.push_log(
            serde_json::json!({
                "type": "prompt_style",
                "role": "developer",
                "content": PROMPT_STYLE_BODY_FIXTURE,
                "timestamp": (now + Duration::seconds(1)).to_rfc3339(),
                "created_at": (now + Duration::seconds(1)).timestamp_millis(),
                "updated_at": (now + Duration::seconds(1)).timestamp_millis(),
            })
            .to_string(),
            now + Duration::seconds(1),
        );

        let messages = build_messages_from_session(&session);
        let joined = messages
            .iter()
            .map(serde_json::Value::to_string)
            .collect::<Vec<_>>()
            .join("\n");
        let contents = messages
            .iter()
            .filter_map(|message| message.get("content").and_then(serde_json::Value::as_str))
            .collect::<Vec<_>>();
        let prompt_style = joined
            .find(PROMPT_STYLE_BODY_FIXTURE)
            .expect("prompt style");
        let compact = joined
            .find("Goal-mode last user command from session state")
            .expect("compact goal section");

        assert!(
            compact < prompt_style,
            "compact/goal must remain before prompt_style: {joined}"
        );
        assert!(
            !joined.contains("\"name\":\"command_run\""),
            "compact must not replay streamed command_run function calls: {joined}"
        );
        assert!(
            !joined.contains("python -m py_compile sentinel.py"),
            "compact must not replay streamed command lines: {joined}"
        );
        assert!(
            !joined.contains("STREAMED_TOOL_RESULT_SENTINEL"),
            "compact must not replay streamed command output: {joined}"
        );
        let compact_content = contents
            .iter()
            .find(|content| {
                content.contains("Agent compact handoff:\nagent handoff before streamed context")
            })
            .expect("compact content");
        assert!(
            !compact_content.contains("STREAMED_TOOL_RESULT_SENTINEL"),
            "compact core must not absorb streamed command output: {compact_content}"
        );
    }

    #[test]
    fn streamed_command_events_do_not_duplicate_final_batch_context() {
        let now = Utc::now();
        let mut session = session();
        for (index, (command_line, output)) in [
            ("apply_patch failing-one", "STREAMED_PER_COMMAND_ONE"),
            ("apply_patch failing-two", "STREAMED_PER_COMMAND_TWO"),
        ]
        .into_iter()
        .enumerate()
        {
            session.push_log(
                serde_json::json!({
                    "type": "streamed_command_event",
                    "status": "completed",
                    "event_index": index,
                    "step": 1,
                    "command_type": "apply_patch",
                    "command_line": command_line,
                    "command": {
                        "step": 1,
                        "command_type": "apply_patch",
                        "command_line": command_line,
                    },
                    "result": {
                        "step": 1,
                        "command_type": "apply_patch",
                        "success": index == 1,
                        "output": output,
                    },
                    "timestamp": (now + Duration::seconds(index as i64)).to_rfc3339(),
                    "created_at": (now + Duration::seconds(index as i64)).timestamp_millis(),
                    "updated_at": (now + Duration::seconds(index as i64)).timestamp_millis(),
                })
                .to_string(),
                now + Duration::seconds(index as i64),
            );
        }

        accumulate_tool_result_with_provider_metadata(
            &mut session,
            "command_run",
            json!({
                "commands": [
                    { "step": 1, "command_type": "apply_patch", "command_line": "apply_patch failing-one" },
                    { "step": 1, "command_type": "apply_patch", "command_line": "apply_patch failing-two" },
                    { "step": 1, "command_type": "apply_patch", "command_line": "apply_patch success-three" }
                ]
            }),
            json!({
                "results": [{
                    "mode": "batch",
                    "results": [
                        { "step": 1, "command_type": "apply_patch", "success": false, "output": "FINAL_BATCH_FAILURE_ONE" },
                        { "step": 1, "command_type": "apply_patch", "success": false, "output": "FINAL_BATCH_FAILURE_TWO" },
                        { "step": 1, "command_type": "apply_patch", "success": true, "output": "FINAL_BATCH_SUCCESS_THREE" }
                    ]
                }]
            }),
            false,
            Some("two commands failed".to_string()),
            Some("runtime-final-batch"),
            Some(json!({ "id": "call_final_batch" })),
        )
        .expect("final command_run batch should be logged");

        let messages = build_messages_from_session(&session);
        let function_calls = messages
            .iter()
            .filter(|message| {
                message.get("type").and_then(serde_json::Value::as_str) == Some("function_call")
            })
            .collect::<Vec<_>>();
        let joined = messages
            .iter()
            .map(serde_json::Value::to_string)
            .collect::<Vec<_>>()
            .join("\n");

        assert_eq!(
            function_calls.len(),
            1,
            "one command_run call should replay into provider context: {joined}"
        );
        let function_outputs = messages
            .iter()
            .filter(|message| {
                message.get("type").and_then(serde_json::Value::as_str)
                    == Some("function_call_output")
            })
            .collect::<Vec<_>>();
        assert_eq!(
            function_outputs.len(),
            1,
            "only final batch output should replay: {joined}"
        );
        assert_eq!(
            function_calls[0]["call_id"], function_outputs[0]["call_id"],
            "command_run call and output must replay as a matched pair: {joined}"
        );

        let output = function_outputs[0]
            .get("output")
            .and_then(serde_json::Value::as_str)
            .expect("function output");
        let output_json: serde_json::Value =
            serde_json::from_str(output).expect("structured command_run output");
        assert_eq!(output_json["results"].as_array().expect("results").len(), 3);
        assert!(output_json["results"][2].get("step").is_none());
        assert!(output_json["results"][2].get("command_type").is_none());
        assert!(output_json["results"][2].get("command_line").is_none());
        assert!(output.contains("FINAL_BATCH_FAILURE_ONE"), "{output}");
        assert!(output.contains("FINAL_BATCH_FAILURE_TWO"), "{output}");
        assert!(output.contains("FINAL_BATCH_SUCCESS_THREE"), "{output}");
        assert!(
            !joined.contains("STREAMED_PER_COMMAND_ONE")
                && !joined.contains("STREAMED_PER_COMMAND_TWO"),
            "streamed audit entries must not enter provider context: {joined}"
        );
    }

    #[test]
    fn reflection_compact_reinjects_objective_without_completion_audit_after_compact_message() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|err| err.into_inner());
        let mut session = session();
        session.reflection_enabled = true;
        session.current_objective = "STATE MACHINE OBJECTIVE".to_string();
        let now = Utc::now();
        session.push_log(
            serde_json::json!({
                "type": "task_focus",
                "task_id": "task-a",
                "content": "STALE TASK FOCUS OBJECTIVE"
            })
            .to_string(),
            now,
        );

        compact_session_context(&mut session, "compact handoff summary")
            .expect("compact should succeed");
        let messages = build_messages_from_session(&session);
        let contents = messages
            .iter()
            .filter_map(|message| message.get("content").and_then(serde_json::Value::as_str))
            .collect::<Vec<_>>();

        let snapshot = contents
            .iter()
            .position(|content| content.contains("<WORKSPACE_SNAPSHOT>"))
            .expect("workspace snapshot");
        let environment = contents
            .iter()
            .position(|content| content.contains("<environment_context>"))
            .expect("environment context");
        let compact = contents
            .iter()
            .position(|content| content.contains("Agent compact handoff:\ncompact handoff summary"))
            .expect("compact core");
        assert!(
            snapshot < environment && environment < compact,
            "compact core must be after environment context: {contents:?}"
        );
        assert!(contents.iter().any(|content| content.contains(
            "Continue working toward the active thread user goal and Operation Manual."
        )));
        assert!(contents
            .iter()
            .any(|content| content.contains("[current objective]:\nSTATE MACHINE OBJECTIVE")));
        assert!(!contents
            .iter()
            .any(|content| content.contains("perform a completion audit")));
        assert!(!contents
            .iter()
            .any(|content| content.contains("STALE TASK FOCUS OBJECTIVE")));
        assert_eq!(
            contents
                .iter()
                .filter(|content| content.contains("compact handoff summary"))
                .count(),
            1
        );
    }

    #[test]
    fn planning_enabled_without_reflection_does_not_reinject_objective_after_compact_message() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|err| err.into_inner());
        let mut session = session();
        session.planning_enabled = true;
        session.reflection_enabled = false;
        session.current_objective = "STATE MACHINE OBJECTIVE".to_string();

        compact_session_context(&mut session, "compact handoff summary")
            .expect("compact should succeed");
        let messages = build_messages_from_session(&session);
        let contents = messages
            .iter()
            .filter_map(|message| message.get("content").and_then(serde_json::Value::as_str))
            .collect::<Vec<_>>();

        assert!(contents
            .iter()
            .any(|content| content.contains("Agent compact handoff:\ncompact handoff summary")));
        assert!(!contents
            .iter()
            .any(|content| content.contains("[current objective]:\nSTATE MACHINE OBJECTIVE")));
    }

    #[test]
    fn compact_session_context_does_not_append_multi_task_management_state() {
        let mut session = session();
        session.task_plan.plan_summary = "Release plan".to_string();
        session.task_plan.detailed_tasks.push(TaskStep {
            task_id: "inspect".to_string(),
            step: 1,
            task_summary: "Inspect release blockers".to_string(),
            step_deliverable_description: "List blocking files".to_string(),
            sub_session_id: "sub-inspect".to_string(),
            poll_interval: PollInterval {
                m: 15,
                d: 0,
                h: 1,
                s: 5,
            },
            start_condition: StartCondition::ScheduledTask,
            status: PlanStatus::Question,
            ..TaskStep::default()
        });
        session.task_plan.detailed_tasks.push(TaskStep {
            task_id: "verify".to_string(),
            step: 2,
            task_summary: "Verify release checklist".to_string(),
            step_deliverable_description: "Passing regression output".to_string(),
            sub_session_id: "sub-verify".to_string(),
            poll_interval: PollInterval {
                m: 0,
                d: 1,
                h: 2,
                s: 30,
            },
            start_condition: StartCondition::PollingTask,
            status: PlanStatus::Done,
            ..TaskStep::default()
        });

        compact_session_context(&mut session, "multi task handoff")
            .expect("compact should succeed");
        let messages = build_messages_from_session(&session);
        let last = messages
            .last()
            .and_then(|message| message.get("content"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();

        assert!(!last.starts_with("TASK_MANAGEMENT_STATE:"));
        assert!(!last.contains("\"plan_summary\":\"Release plan\""));
        assert!(!last.contains("\"task_id\":\"inspect\""));
    }

    #[test]
    fn build_context_replays_dialog_entries_without_rewriting_history() {
        let mut session = session();
        for index in 0..4 {
            accumulate_message(&mut session, "user", json!(format!("user-{index}")))
                .expect("user message should be logged");
            accumulate_message(
                &mut session,
                "assistant",
                json!(format!("assistant-{index}")),
            )
            .expect("assistant message should be logged");
        }

        let output = build_context(ContextInput {
            runtime: runtime(&session),
            session,
            additional_messages: vec![],
        })
        .expect("context should build");
        let contents = output
            .messages
            .iter()
            .filter_map(|message| message.get("content"))
            .map(|content| {
                content
                    .as_str()
                    .map(str::to_string)
                    .unwrap_or_else(|| content.to_string())
            })
            .collect::<Vec<_>>();

        assert!(contents.iter().any(|content| content.contains("user-0")));
        assert!(contents
            .iter()
            .any(|content| content.contains("assistant-0")));
        assert!(contents
            .iter()
            .any(|content| content.contains("assistant-1")));
        assert!(contents.iter().any(|content| content.contains("user-3")));
        assert_eq!(
            output
                .messages
                .iter()
                .filter(|message| matches!(message["role"].as_str(), Some("user" | "assistant")))
                .count(),
            9
        );
    }

    #[test]
    fn build_context_replays_user_agent_records_as_user_context() {
        let mut session = session();
        accumulate_message(
            &mut session,
            USER_AGENT_CONTEXT_ROLE,
            json!("client runtime context"),
        )
        .expect("user-agent context should log");

        let output = build_context(ContextInput {
            runtime: runtime(&session),
            session,
            additional_messages: vec![],
        })
        .expect("context should build");
        let context = output
            .messages
            .iter()
            .find(|message| {
                message["content"]
                    .as_str()
                    .is_some_and(|content| content.contains("client runtime context"))
            })
            .expect("user-agent context should be replayed");

        assert_eq!(context["role"], "user");
    }

    #[test]
    fn build_context_replays_workspace_and_environment_user_agent_records_as_developer() {
        let mut session = session();
        accumulate_message(
            &mut session,
            USER_AGENT_CONTEXT_ROLE,
            json!("<WORKSPACE_SNAPSHOT>workspace</WORKSPACE_SNAPSHOT>"),
        )
        .expect("workspace context should log");
        accumulate_message(
            &mut session,
            USER_AGENT_CONTEXT_ROLE,
            json!("<environment_context>client context</environment_context>"),
        )
        .expect("environment context should log");

        let output = build_context(ContextInput {
            runtime: runtime(&session),
            session,
            additional_messages: vec![],
        })
        .expect("context should build");
        let developer_contexts = output
            .messages
            .iter()
            .filter(|message| message["role"] == "developer")
            .collect::<Vec<_>>();

        assert_eq!(developer_contexts.len(), 2, "{:?}", output.messages);
        assert!(developer_contexts.iter().any(|message| {
            message["content"]
                .as_str()
                .is_some_and(|content| content.contains("<WORKSPACE_SNAPSHOT>"))
        }));
        assert!(developer_contexts.iter().any(|message| {
            message["content"]
                .as_str()
                .is_some_and(|content| content.contains("<environment_context>"))
        }));
    }
}
