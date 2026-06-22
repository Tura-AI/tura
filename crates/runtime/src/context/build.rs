use chrono::Utc;
use std::time::Instant;
use tracing::info;

use crate::profile_timings;
use crate::state_machine::runtime_management::RuntimeManagement;
use crate::state_machine::session_management::SessionManagement;

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
    let context_message_start = Instant::now();
    tool_result_json["context_message"] = immutable_tool_result_context_message(&tool_result_json);
    let context_message_elapsed = context_message_start.elapsed();
    profile_timings::log_duration(
        "accumulate_tool_result.context_message",
        context_message_elapsed,
        serde_json::json!({
            "session_id": session.session_id,
            "tool_name": tool_name,
            "context_message_bytes": if profiling {
                profile_timings::json_bytes(&tool_result_json["context_message"])
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
    let mut saw_context_compaction = false;
    for value in session
        .session_log
        .iter()
        .filter_map(|entry| serde_json::from_str::<serde_json::Value>(entry).ok())
    {
        if value.get("type").and_then(|kind| kind.as_str()) == Some("context_compaction") {
            saw_context_compaction = true;
            messages.clear();
            messages.extend(context_compaction_messages(&value, session));
            continue;
        }
        messages.extend(immutable_context_messages_from_log_entry(value));
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
            || role == "assistant"
        {
            let provider_role = if role == USER_AGENT_CONTEXT_ROLE {
                "user"
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
        return messages
            .iter()
            .cloned()
            .map(strip_context_reporting_fields)
            .collect();
    }

    immutable_tool_result_context_messages(&value)
}

use super::tool_results::{
    immutable_tool_result_context_message, immutable_tool_result_context_messages,
    last_tool_call_response_from_session, strip_context_reporting_fields,
    strip_tool_reporting_fields, tool_result_context_cache,
};

#[cfg(test)]
mod tests {
    use super::{
        accumulate_message, accumulate_tool_result, build_context, build_messages_from_session,
        ContextInput,
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
    fn compact_session_context_replaces_prior_tool_context_but_keeps_later_results() {
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
        assert!(joined.contains("new-output"));
        assert!(!joined.contains("old-tool-secret"));
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
            joined.contains("Context rebuild timeline before this checkpoint"),
            "{joined}"
        );
        assert!(
            joined.contains(&(base + Duration::minutes(2)).to_rfc3339()),
            "{joined}"
        );
        assert!(
            joined.contains("retained_context/agent_summary: earlier agent summary"),
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
        assert!(joined.contains("Compact context handoff"), "{joined}");
        assert!(joined.contains("new compact handoff text"), "{joined}");
        assert!(
            !joined.contains("old user requirement kept from retained context"),
            "{joined}"
        );
        let summary = joined
            .find("earlier agent summary")
            .expect("summary position");
        let user = joined
            .find("current run real user request")
            .expect("user position");
        let agent = joined
            .find("current run visible agent progress")
            .expect("agent position");
        assert!(
            summary < user && user < agent,
            "timeline must be timestamp sorted: {joined}"
        );
    }

    #[test]
    fn automatic_compact_context_preserves_recent_tool_results_and_trims_older_history() {
        let root = tempfile::TempDir::new().expect("tempdir");
        let base = Utc::now() - Duration::minutes(90);
        let mut session = session();
        session.session_directory = root.path().to_path_buf();
        session.session_started_at = base;
        for index in 0..90 {
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
                "context_cache": {
                    "output": "RECENT_TOOL_RESULT_SENTINEL"
                },
                "success": true,
                "timestamp": (base + Duration::minutes(80)).to_rfc3339()
            })
            .to_string(),
            base + Duration::minutes(80),
        );

        compact_session_context_automatically(&mut session, "automatic handoff")
            .expect("automatic compact should succeed");
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

        assert!(content.len() <= 36_000 + 100);
        assert!(
            content.contains("older timeline entries omitted"),
            "{content}"
        );
        assert!(content.contains("RECENT_TOOL_RESULT_SENTINEL"), "{content}");
        assert!(!content.contains("old-history-00"), "{content}");
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
    fn planning_compact_reinjects_objective_without_completion_audit_after_compact_message() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|err| err.into_inner());
        let mut session = session();
        session.planning_enabled = true;
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

        assert!(contents.first().is_some_and(
            |content| content.contains("Compact context handoff:\ncompact handoff summary")
        ));
        assert!(contents
            .iter()
            .any(|content| content.contains("Continue working toward the active thread goal.")));
        assert!(contents
            .iter()
            .any(|content| content.contains("[current objective]:\nSTATE MACHINE OBJECTIVE")));
        assert!(!contents
            .iter()
            .any(|content| content.contains("perform a completion audit")));
        assert!(!contents
            .iter()
            .any(|content| content.contains("STALE TASK FOCUS OBJECTIVE")));
        assert!(!contents
            .iter()
            .skip(1)
            .any(|content| content.contains("compact handoff summary")));
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
            json!("<environment_context>client context</environment_context>"),
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
                    .is_some_and(|content| content.contains("client context"))
            })
            .expect("user-agent context should be replayed");

        assert_eq!(context["role"], "user");
    }
}
