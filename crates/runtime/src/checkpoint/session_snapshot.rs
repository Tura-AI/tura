//! Whole-session snapshot compatibility checkpoints.

use crate::profile_timings;
use crate::session_log_client::SessionLogClient;
use crate::state_machine::session_management::SessionManagement;
use chrono::{DateTime, Utc};
use serde_json::Value;
use std::collections::HashMap;
use std::time::Instant;

pub(crate) fn persist_session_snapshot(session: &SessionManagement) -> Result<(), String> {
    persist_session_snapshot_for_stage(session, None)
}

fn persist_session_snapshot_for_stage(
    session: &SessionManagement,
    stage: Option<&str>,
) -> Result<(), String> {
    let total_start = Instant::now();
    let record_start = Instant::now();
    let record = persisted_record(session);
    let record_elapsed = record_start.elapsed();
    let profiling = profile_timings::enabled();
    let record_bytes = if profiling {
        profile_timings::json_bytes(&record)
    } else {
        0
    };
    profile_timings::log_duration(
        "persist_session_snapshot.persisted_record",
        record_elapsed,
        serde_json::json!({
            "session_id": session.session_id,
            "stage": stage,
            "session_log_entries": session.session_log.len(),
            "record_bytes": record_bytes,
        }),
    );
    let session_info = record
        .get("info")
        .cloned()
        .ok_or_else(|| "persisted session info missing".to_string())?;
    let messages_start = Instant::now();
    let messages = persisted_messages(session);
    let messages_elapsed = messages_start.elapsed();
    let messages_bytes = if profiling {
        profile_timings::json_vec_bytes(&messages)
    } else {
        0
    };
    profile_timings::log_duration(
        "persist_session_snapshot.persisted_messages_for_upsert",
        messages_elapsed,
        serde_json::json!({
            "session_id": session.session_id,
            "stage": stage,
            "session_log_entries": session.session_log.len(),
            "message_count": messages.len(),
            "messages_bytes": messages_bytes,
        }),
    );
    let discover_start = Instant::now();
    let client = SessionLogClient::discover().map_err(|err| {
        format!(
            "failed to discover session_log client for session snapshot {}: {err}",
            session.session_id
        )
    })?;
    profile_timings::log_elapsed(
        "persist_session_snapshot.discover_client",
        discover_start,
        serde_json::json!({
            "session_id": session.session_id,
            "stage": stage,
        }),
    );
    let upsert_start = Instant::now();
    let upsert_result = client.upsert_session(session_info, None, messages, Vec::new());
    profile_timings::log_elapsed(
        "persist_session_snapshot.upsert_session",
        upsert_start,
        serde_json::json!({
            "session_id": session.session_id,
            "stage": stage,
            "success": upsert_result.is_ok(),
        }),
    );
    let result = upsert_result.map_err(|err| {
        format!(
            "failed to persist session snapshot {}: {err}",
            session.session_id
        )
    });
    profile_timings::log_elapsed(
        "persist_session_snapshot.total",
        total_start,
        serde_json::json!({
            "session_id": session.session_id,
            "stage": stage,
            "success": result.is_ok(),
        }),
    );
    result
}

pub(crate) fn persist_session_checkpoint(session: &SessionManagement, stage: &str) {
    if let Err(err) = persist_session_snapshot_for_stage(session, Some(stage)) {
        tracing::warn!(
            session_id = %session.session_id,
            stage,
            error = %err,
            "failed to persist gateway session checkpoint"
        );
    }
    crate::gateway_events::emit_cli_live_session_checkpoint(session, stage);
}

fn persisted_record(session: &SessionManagement) -> serde_json::Value {
    let messages_start = Instant::now();
    let messages = persisted_messages(session);
    let messages_elapsed = messages_start.elapsed();
    let messages_bytes = if profile_timings::enabled() {
        profile_timings::json_vec_bytes(&messages)
    } else {
        0
    };
    profile_timings::log_duration(
        "persisted_record.messages",
        messages_elapsed,
        serde_json::json!({
            "session_id": session.session_id,
            "session_log_entries": session.session_log.len(),
            "message_count": messages.len(),
            "messages_bytes": messages_bytes,
        }),
    );
    serde_json::json!({
        "info": {
            "id": session.session_id,
            "name": session.session_name,
            "created_at": session.session_created_at.timestamp_millis(),
            "updated_at": session.session_last_update_at.timestamp_millis(),
            "directory": session.session_directory.to_string_lossy(),
            "model": null,
            "agent": session.input.agent,
            "session_type": session.session_topic,
            "kill_processes_on_start": false,
            "validator_enabled": false,
            "force_planning": false,
            "model_variant": null,
            "model_acceleration_enabled": false,
            "disable_permission_restrictions": session.disable_permission_restrictions,
            "use_last_tool_call_response": session.use_last_tool_call_response,
            "status": session_status(session),
            "message_count": session.session_current_turn,
            "management": session,
        },
        "parent_id": null,
        "messages": messages,
        "todos": [],
    })
}

fn persisted_messages(session: &SessionManagement) -> Vec<Value> {
    let base_time = session.session_created_at.timestamp_millis();
    let mut messages = Vec::new();
    let mut runtime_message_indexes = HashMap::new();

    for (index, entry) in session.session_log.iter().enumerate() {
        if let Some((runtime_id, tool_part, created_at, updated_at)) =
            runtime_tool_part_from_log_entry(index, entry, base_time)
        {
            merge_runtime_tool_part(
                session,
                &mut messages,
                &mut runtime_message_indexes,
                &runtime_id,
                tool_part,
                created_at,
                updated_at,
            );
            continue;
        }

        let message = persisted_message(session, index, entry, base_time);
        if let Some(runtime_id) = assistant_runtime_id(&message) {
            if let Some(existing_index) = runtime_message_indexes.get(&runtime_id).copied() {
                merge_runtime_message(&mut messages[existing_index], message);
            } else {
                runtime_message_indexes.insert(runtime_id, messages.len());
                messages.push(message);
            }
        } else {
            messages.push(message);
        }
    }

    messages
}

fn persisted_message(
    session: &SessionManagement,
    index: usize,
    entry: &str,
    base_time: i64,
) -> Value {
    let mut value = serde_json::from_str::<Value>(entry).unwrap_or_else(|_| {
        serde_json::json!({
            "type": "log",
            "content": entry,
        })
    });
    if !value.is_object() {
        value = serde_json::json!({
            "type": "log",
            "content": value,
        });
    }

    let Some(object) = value.as_object_mut() else {
        tracing::warn!(
            session_id = %session.session_id,
            index,
            "persisted message normalization produced a non-object value"
        );
        return serde_json::json!({
            "id": format!("{}:log:{index}", session.session_id),
            "role": "event",
            "type": "log",
            "content": entry,
            "created_at": base_time.saturating_add(index as i64),
            "updated_at": base_time.saturating_add(index as i64),
            "session_id": session.session_id.clone(),
        });
    };
    let fallback_time = base_time.saturating_add(index as i64);
    let created_at = object
        .get("created_at")
        .and_then(valid_timestamp_millis)
        .or_else(|| object.get("timestamp").and_then(timestamp_millis))
        .unwrap_or(fallback_time);
    let updated_at = object
        .get("updated_at")
        .and_then(valid_timestamp_millis)
        .or_else(|| object.get("timestamp").and_then(timestamp_millis))
        .unwrap_or(created_at);

    if is_user_context_without_frontend_id(object) {
        normalize_user_context_record(session, index, object, created_at, updated_at);
        return value;
    }

    if let Some(message) =
        conversation_message_record(session, index, object, created_at, updated_at)
    {
        return message;
    }

    object
        .entry("id".to_string())
        .or_insert_with(|| Value::String(format!("{}:log:{index}", session.session_id)));
    let role = record_role(object);
    object
        .entry("role".to_string())
        .or_insert_with(|| Value::String(role));
    object
        .entry("created_at".to_string())
        .and_modify(|value| set_timestamp_if_invalid(value, created_at))
        .or_insert_with(|| Value::Number(created_at.into()));
    object
        .entry("updated_at".to_string())
        .and_modify(|value| set_timestamp_if_invalid(value, updated_at))
        .or_insert_with(|| Value::Number(updated_at.into()));
    object
        .entry("session_id".to_string())
        .or_insert_with(|| Value::String(session.session_id.clone()));
    value
}

fn runtime_tool_part_from_log_entry(
    index: usize,
    entry: &str,
    base_time: i64,
) -> Option<(String, Value, i64, i64)> {
    let value = serde_json::from_str::<Value>(entry).ok()?;
    let object = value.as_object()?;
    if object.get("type").and_then(Value::as_str) != Some("tool_result") {
        return None;
    }
    let runtime_id = object
        .get("runtime_id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|runtime_id| !runtime_id.is_empty())?
        .to_string();
    let tool_name = object
        .get("tool_name")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|tool_name| !tool_name.is_empty())?;
    let fallback_time = base_time.saturating_add(index as i64);
    let created_at = object
        .get("created_at")
        .and_then(valid_timestamp_millis)
        .or_else(|| object.get("timestamp").and_then(timestamp_millis))
        .unwrap_or(fallback_time);
    let updated_at = object
        .get("updated_at")
        .and_then(valid_timestamp_millis)
        .or_else(|| object.get("timestamp").and_then(timestamp_millis))
        .unwrap_or(created_at);
    Some((
        runtime_id.clone(),
        runtime_tool_part(&runtime_id, tool_name, object, created_at, updated_at),
        created_at,
        updated_at,
    ))
}

fn runtime_tool_part(
    runtime_id: &str,
    tool_name: &str,
    object: &serde_json::Map<String, Value>,
    created_at: i64,
    updated_at: i64,
) -> Value {
    let input = object.get("input").cloned().unwrap_or(Value::Null);
    let output = object.get("output").cloned().unwrap_or(Value::Null);
    let success = object.get("success").and_then(Value::as_bool);
    let error = object.get("error").cloned().unwrap_or(Value::Null);
    let status = match success {
        Some(false) => "failed",
        _ => "completed",
    };
    let part_id = crate::gateway_events::runtime_tool_part_id(runtime_id, tool_name);
    let metadata = serde_json::json!({
        "kind": "mano_tool_call",
        "tool": tool_name,
        "runtime_id": runtime_id,
        "input": input,
        "output": output,
        "success": success,
        "error": error,
        "transient": true,
        "streaming_partial": false,
    });
    serde_json::json!({
        "id": part_id,
        "type": "tool",
        "content": null,
        "text": null,
        "metadata": metadata,
        "call_id": part_id,
        "tool": tool_name,
        "state": {
            "status": status,
            "input": input,
            "output": output,
            "metadata": metadata,
            "time": {
                "start": created_at,
                "end": updated_at,
            }
        }
    })
}

fn merge_runtime_tool_part(
    session: &SessionManagement,
    messages: &mut Vec<Value>,
    runtime_message_indexes: &mut HashMap<String, usize>,
    runtime_id: &str,
    tool_part: Value,
    created_at: i64,
    updated_at: i64,
) {
    let message_index = runtime_message_indexes
        .get(runtime_id)
        .copied()
        .unwrap_or_else(|| {
            let message = serde_json::json!({
                "id": crate::gateway_events::runtime_message_id(runtime_id),
                "session_id": session.session_id,
                "role": "assistant",
                "parent_id": null,
                "parts": [],
                "created_at": created_at,
                "updated_at": updated_at,
            });
            messages.push(message);
            let message_index = messages.len() - 1;
            runtime_message_indexes.insert(runtime_id.to_string(), message_index);
            message_index
        });
    merge_part_into_message(&mut messages[message_index], tool_part);
    merge_message_times(&mut messages[message_index], created_at, updated_at);
}

fn merge_runtime_message(existing: &mut Value, incoming: Value) {
    let incoming_created_at = incoming.get("created_at").and_then(valid_timestamp_millis);
    let incoming_updated_at = incoming.get("updated_at").and_then(valid_timestamp_millis);
    if let Some(parts) = incoming.get("parts").and_then(Value::as_array) {
        for part in parts {
            merge_part_into_message(existing, part.clone());
        }
    }
    if let Some(created_at) = incoming_created_at {
        merge_message_created_at(existing, created_at);
    }
    if let Some(updated_at) = incoming_updated_at {
        merge_message_updated_at(existing, updated_at);
    }
}

fn merge_part_into_message(message: &mut Value, part: Value) {
    let Some(parts) = message.get_mut("parts").and_then(Value::as_array_mut) else {
        return;
    };
    let part_id = part
        .get("id")
        .and_then(Value::as_str)
        .map(ToString::to_string);
    if let Some(part_id) = part_id {
        if let Some(existing) = parts
            .iter_mut()
            .find(|existing| existing.get("id").and_then(Value::as_str) == Some(part_id.as_str()))
        {
            *existing = part;
            return;
        }
    }
    parts.push(part);
    parts.sort_by_key(runtime_part_order);
}

fn runtime_part_order(part: &Value) -> u8 {
    match part.get("type").and_then(Value::as_str) {
        Some("text") => 0,
        Some("tool") => 1,
        _ => 2,
    }
}

fn merge_message_times(message: &mut Value, created_at: i64, updated_at: i64) {
    merge_message_created_at(message, created_at);
    merge_message_updated_at(message, updated_at);
}

fn merge_message_created_at(message: &mut Value, created_at: i64) {
    let existing = message
        .get("created_at")
        .and_then(valid_timestamp_millis)
        .unwrap_or(created_at);
    if let Some(object) = message.as_object_mut() {
        object.insert(
            "created_at".to_string(),
            Value::Number(existing.min(created_at).into()),
        );
    }
}

fn merge_message_updated_at(message: &mut Value, updated_at: i64) {
    let existing = message
        .get("updated_at")
        .and_then(valid_timestamp_millis)
        .unwrap_or(updated_at);
    if let Some(object) = message.as_object_mut() {
        object.insert(
            "updated_at".to_string(),
            Value::Number(existing.max(updated_at).into()),
        );
    }
}

fn assistant_runtime_id(message: &Value) -> Option<String> {
    if message.get("role").and_then(Value::as_str) != Some("assistant") {
        return None;
    }
    message
        .get("id")
        .and_then(Value::as_str)
        .and_then(|id| id.strip_suffix(".message"))
        .map(str::to_string)
}

fn conversation_message_record(
    session: &SessionManagement,
    index: usize,
    object: &serde_json::Map<String, Value>,
    created_at: i64,
    updated_at: i64,
) -> Option<Value> {
    if object.get("parts").and_then(Value::as_array).is_some() {
        let role = object.get("role").and_then(Value::as_str);
        if !matches!(role, Some("user" | "assistant" | "system")) {
            return None;
        }
        if matches!(role, Some("user")) && !has_frontend_message_id(object) {
            return None;
        }
        let mut message = Value::Object(object.clone());
        if let Some(message_object) = message.as_object_mut() {
            message_object
                .entry("id".to_string())
                .or_insert_with(|| Value::String(format!("{}:log:{index}", session.session_id)));
            message_object
                .entry("session_id".to_string())
                .or_insert_with(|| Value::String(session.session_id.clone()));
            message_object
                .entry("created_at".to_string())
                .and_modify(|value| set_timestamp_if_invalid(value, created_at))
                .or_insert_with(|| Value::Number(created_at.into()));
            message_object
                .entry("updated_at".to_string())
                .and_modify(|value| set_timestamp_if_invalid(value, updated_at))
                .or_insert_with(|| Value::Number(updated_at.into()));
        }
        return Some(message);
    }

    let role = object.get("role").and_then(Value::as_str)?;
    if !matches!(role, "user" | "assistant" | "system") {
        return None;
    }
    if role == "user" && !has_frontend_message_id(object) {
        return None;
    }
    let content = conversation_content_text(object.get("content")?)?;
    let message_id = object
        .get("id")
        .and_then(Value::as_str)
        .map(ToString::to_string)
        .unwrap_or_else(|| format!("{}:log:{index}", session.session_id));
    let part_id = object
        .get("part_id")
        .and_then(Value::as_str)
        .map(ToString::to_string)
        .unwrap_or_else(|| format!("{message_id}:part"));
    let metadata = object.get("metadata").cloned();
    let has_metadata = metadata.is_some();

    let mut part = serde_json::json!({
        "id": part_id,
        "type": "text",
        "content": content,
        "text": content,
        "metadata": metadata,
        "call_id": null,
        "tool": null,
        "state": null,
    });
    if !has_metadata {
        if let Some(part_object) = part.as_object_mut() {
            part_object.remove("metadata");
        }
    }

    Some(serde_json::json!({
        "id": message_id,
        "session_id": session.session_id,
        "role": role,
        "parent_id": object.get("parent_id").cloned().unwrap_or(Value::Null),
        "parts": [part],
        "created_at": created_at,
        "updated_at": updated_at,
    }))
}

fn is_user_context_without_frontend_id(object: &serde_json::Map<String, Value>) -> bool {
    matches!(object.get("role").and_then(Value::as_str), Some("user"))
        && !has_frontend_message_id(object)
}

fn has_frontend_message_id(object: &serde_json::Map<String, Value>) -> bool {
    object
        .get("id")
        .and_then(Value::as_str)
        .map(str::trim)
        .is_some_and(|id| !id.is_empty() && !id.contains(":log:"))
}

fn normalize_user_context_record(
    session: &SessionManagement,
    index: usize,
    object: &mut serde_json::Map<String, Value>,
    created_at: i64,
    updated_at: i64,
) {
    object
        .entry("id".to_string())
        .or_insert_with(|| Value::String(format!("{}:context:{index}", session.session_id)));
    object
        .entry("type".to_string())
        .or_insert_with(|| Value::String("user_context".to_string()));
    object
        .entry("source_role".to_string())
        .or_insert_with(|| Value::String("user".to_string()));
    object.insert("role".to_string(), Value::String("log".to_string()));
    object
        .entry("created_at".to_string())
        .and_modify(|value| set_timestamp_if_invalid(value, created_at))
        .or_insert_with(|| Value::Number(created_at.into()));
    object
        .entry("updated_at".to_string())
        .and_modify(|value| set_timestamp_if_invalid(value, updated_at))
        .or_insert_with(|| Value::Number(updated_at.into()));
    object
        .entry("session_id".to_string())
        .or_insert_with(|| Value::String(session.session_id.clone()));
}

fn conversation_content_text(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => {
            let text = text.trim();
            (!text.is_empty()).then(|| text.to_string())
        }
        Value::Array(items) => {
            let text = items
                .iter()
                .filter_map(|item| {
                    item.as_str().map(ToString::to_string).or_else(|| {
                        item.get("text")
                            .and_then(Value::as_str)
                            .map(ToString::to_string)
                    })
                })
                .collect::<Vec<_>>()
                .join("");
            (!text.trim().is_empty()).then(|| text.trim().to_string())
        }
        Value::Object(object) => object
            .get("text")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|text| !text.is_empty())
            .map(ToString::to_string),
        _ => None,
    }
}

fn timestamp_millis(value: &Value) -> Option<i64> {
    let text = value.as_str()?;
    DateTime::parse_from_rfc3339(text)
        .ok()
        .map(|timestamp| timestamp.with_timezone(&Utc).timestamp_millis())
        .filter(|timestamp| *timestamp > 0)
}

fn valid_timestamp_millis(value: &Value) -> Option<i64> {
    value.as_i64().filter(|timestamp| *timestamp > 0)
}

fn set_timestamp_if_invalid(value: &mut Value, fallback: i64) {
    if valid_timestamp_millis(value).is_none() {
        *value = Value::Number(fallback.into());
    }
}

fn record_role(object: &serde_json::Map<String, Value>) -> String {
    match object.get("type").and_then(Value::as_str) {
        Some("tool_result") => "tool".to_string(),
        Some("runtime_usage") => "runtime".to_string(),
        Some("context_compaction") => "system".to_string(),
        Some(kind) if !kind.trim().is_empty() => kind.to_string(),
        _ => "event".to_string(),
    }
}

fn session_status(session: &SessionManagement) -> &'static str {
    use crate::state_machine::session_management::SessionState;

    match session.state {
        SessionState::Created | SessionState::Completed => "idle",
        SessionState::Running | SessionState::Paused => "busy",
        SessionState::Failed | SessionState::Cancelled | SessionState::Interrupted => "error",
    }
}

#[cfg(test)]
mod tests {
    use super::{persisted_message, persisted_messages};
    use crate::state_machine::session_management::{SessionInput, SessionManagement};
    use chrono::Utc;
    use std::path::PathBuf;

    fn session() -> SessionManagement {
        let now = Utc::now();
        SessionManagement::new(
            "snapshot-session".to_string(),
            "snapshot".to_string(),
            PathBuf::from("C:/workspace"),
            false,
            "coding".to_string(),
            SessionInput {
                user_input: "persist".to_string(),
                file_input: vec![],
                agent: None,
                runtime_context: None,
                planning_mode_override: None,
            },
            "persist".to_string(),
            now,
        )
    }

    #[test]
    fn persisted_message_wraps_non_object_json_without_panicking() {
        let session = session();
        let value = persisted_message(&session, 2, "\"plain text\"", 1_000);

        assert_eq!(value["type"], "log");
        assert_eq!(value["content"], "plain text");
        assert_eq!(value["role"], "log");
        assert_eq!(value["created_at"], 1_002);
        assert_eq!(value["session_id"], "snapshot-session");
    }

    #[test]
    fn persisted_message_turns_assistant_content_into_hydratable_message() {
        let session = session();
        let value = persisted_message(
            &session,
            7,
            &serde_json::json!({
                "id": "runtime-1.message",
                "part_id": "runtime-1.message",
                "role": "assistant",
                "content": "final visible reply",
                "created_at": 42,
                "updated_at": 43,
                "runtime_id": "runtime-1"
            })
            .to_string(),
            1_000,
        );

        assert_eq!(value["id"], "runtime-1.message");
        assert_eq!(value["session_id"], "snapshot-session");
        assert_eq!(value["role"], "assistant");
        assert_eq!(value["created_at"], 42);
        assert_eq!(value["updated_at"], 43);
        assert_eq!(value["parts"][0]["id"], "runtime-1.message");
        assert_eq!(value["parts"][0]["type"], "text");
        assert_eq!(value["parts"][0]["text"], "final visible reply");
        assert_eq!(value["parts"][0]["content"], "final visible reply");
    }

    #[test]
    fn persisted_messages_merges_runtime_tool_result_into_runtime_assistant_message() {
        let mut session = session();
        let assistant_created_at = chrono::DateTime::parse_from_rfc3339("2026-06-14T08:09:11Z")
            .expect("timestamp")
            .timestamp_millis();
        let assistant_updated_at = assistant_created_at + 100;
        let tool_finished_at = "2026-06-14T08:09:14Z";
        session.push_log(
            serde_json::json!({
                "id": "runtime-merge-1.message",
                "part_id": "runtime-merge-1.message",
                "role": "assistant",
                "content": "I checked the workspace.",
                "created_at": assistant_created_at,
                "updated_at": assistant_updated_at,
                "runtime_id": "runtime-merge-1"
            })
            .to_string(),
            Utc::now(),
        );
        session.push_log(
            serde_json::json!({
                "type": "tool_result",
                "runtime_id": "runtime-merge-1",
                "tool_name": "command_run",
                "input": {
                    "commands": [{
                        "step": 1,
                        "command_type": "shell_command",
                        "command_line": "Get-ChildItem"
                    }]
                },
                "output": {
                    "streamed_command_run_result": {
                        "results": [{
                            "step": 1,
                            "command_type": "shell_command",
                            "command_line": "Get-ChildItem",
                            "success": true
                        }]
                    }
                },
                "success": true,
                "error": null,
                "timestamp": tool_finished_at
            })
            .to_string(),
            Utc::now(),
        );

        let messages = persisted_messages(&session);

        assert_eq!(messages.len(), 1, "{messages:#?}");
        let message = &messages[0];
        assert_eq!(message["id"], "runtime-merge-1.message");
        assert_eq!(message["role"], "assistant");
        assert_eq!(message["created_at"], assistant_created_at);
        assert_eq!(
            message["updated_at"],
            chrono::DateTime::parse_from_rfc3339(tool_finished_at)
                .expect("timestamp")
                .timestamp_millis()
        );
        assert_eq!(message["parts"].as_array().expect("parts").len(), 2);
        assert_eq!(message["parts"][0]["id"], "runtime-merge-1.message");
        assert_eq!(message["parts"][0]["text"], "I checked the workspace.");
        assert_eq!(
            message["parts"][1]["id"],
            "runtime-merge-1.tool.command_run"
        );
        assert_eq!(message["parts"][1]["tool"], "command_run");
        assert_eq!(
            message["parts"][1]["call_id"],
            "runtime-merge-1.tool.command_run"
        );
        assert_eq!(message["parts"][1]["state"]["status"], "completed");
        assert_eq!(
            message["parts"][1]["state"]["input"]["commands"][0]["command_line"],
            "Get-ChildItem"
        );
        assert_eq!(
            message["parts"][1]["state"]["output"]["streamed_command_run_result"]["results"][0]
                ["success"],
            true
        );
    }

    #[test]
    fn persisted_message_keeps_user_context_without_frontend_id_auxiliary() {
        let session = session();
        let user = persisted_message(
            &session,
            1,
            &serde_json::json!({
                "role": "user",
                "content": [{"type": "input_text", "text": "hello"}, {"text": " world"}]
            })
            .to_string(),
            1_000,
        );
        let user_with_parts = persisted_message(
            &session,
            2,
            &serde_json::json!({
                "role": "user",
                "parts": [{
                    "id": "synthetic-part",
                    "type": "text",
                    "text": "hello from context",
                    "content": "hello from context"
                }]
            })
            .to_string(),
            1_000,
        );

        assert_eq!(user["id"], "snapshot-session:context:1");
        assert_eq!(user["role"], "log");
        assert_eq!(user["source_role"], "user");
        assert_eq!(user["type"], "user_context");
        assert_eq!(user["content"][0]["text"], "hello");
        assert!(user.get("parts").is_none());

        assert_eq!(user_with_parts["id"], "snapshot-session:context:2");
        assert_eq!(user_with_parts["role"], "log");
        assert_eq!(user_with_parts["source_role"], "user");
        assert_eq!(user_with_parts["type"], "user_context");
        assert_eq!(user_with_parts["parts"][0]["text"], "hello from context");
    }

    #[test]
    fn persisted_message_keeps_frontend_user_id_hydratable() {
        let session = session();
        let value = persisted_message(
            &session,
            1,
            &serde_json::json!({
                "id": "msg_tui_frontend-1",
                "part_id": "part_tui_frontend-1",
                "role": "user",
                "content": [{"type": "input_text", "text": "hello"}, {"text": " world"}]
            })
            .to_string(),
            1_000,
        );

        assert_eq!(value["id"], "msg_tui_frontend-1");
        assert_eq!(value["role"], "user");
        assert_eq!(value["parts"][0]["id"], "part_tui_frontend-1");
        assert_eq!(value["parts"][0]["text"], "hello world");
    }

    #[test]
    fn persisted_message_replaces_zero_frontend_user_timestamps_before_session_log_write() {
        let session = session();
        let value = persisted_message(
            &session,
            4,
            &serde_json::json!({
                "id": "msg_tui_frontend-zero-time",
                "role": "user",
                "created_at": 0,
                "updated_at": 0,
                "parts": [{
                    "id": "part_tui_frontend-zero-time",
                    "type": "text",
                    "text": "user prompt after replay",
                    "content": "user prompt after replay"
                }]
            })
            .to_string(),
            1_000,
        );

        assert_eq!(value["id"], "msg_tui_frontend-zero-time");
        assert_eq!(value["role"], "user");
        assert_eq!(value["created_at"], 1_004);
        assert_eq!(value["updated_at"], 1_004);
        assert_eq!(value["parts"][0]["text"], "user prompt after replay");
    }

    #[test]
    fn persisted_message_uses_explicit_frontend_user_timestamp_instead_of_log_index_fallback() {
        let session = session();
        let value = persisted_message(
            &session,
            4,
            &serde_json::json!({
                "id": "msg_tui_frontend-current-time",
                "part_id": "part_tui_frontend-current-time",
                "role": "user",
                "created_at": 123_456,
                "updated_at": 123_789,
                "timestamp": "2026-06-19T12:34:56.789Z",
                "content": [{"type": "input_text", "text": "fresh user prompt"}]
            })
            .to_string(),
            1_000,
        );

        assert_eq!(value["id"], "msg_tui_frontend-current-time");
        assert_eq!(value["role"], "user");
        assert_eq!(value["created_at"], 123_456);
        assert_eq!(value["updated_at"], 123_789);
        assert_eq!(value["parts"][0]["id"], "part_tui_frontend-current-time");
        assert_eq!(value["parts"][0]["text"], "fresh user prompt");
    }

    #[test]
    fn persisted_message_normalizes_system_content_but_keeps_runtime_usage_auxiliary() {
        let session = session();
        let system = persisted_message(
            &session,
            2,
            &serde_json::json!({
                "role": "system",
                "content": {"text": "guardrail"}
            })
            .to_string(),
            1_000,
        );
        let usage = persisted_message(
            &session,
            3,
            &serde_json::json!({
                "type": "runtime_usage",
                "runtime_id": "runtime-1",
                "usage": {"total_tokens": 9},
                "timestamp": "2026-06-14T08:09:11Z"
            })
            .to_string(),
            1_000,
        );

        assert_eq!(system["role"], "system");
        assert_eq!(system["parts"][0]["text"], "guardrail");
        assert_eq!(usage["role"], "runtime");
        assert_eq!(usage["type"], "runtime_usage");
        assert!(usage.get("parts").is_none());
    }

    #[test]
    fn persisted_message_keeps_user_agent_context_auxiliary() {
        let session = session();
        let value = persisted_message(
            &session,
            4,
            &serde_json::json!({
                "role": crate::context::USER_AGENT_CONTEXT_ROLE,
                "content": "<environment_context>internal</environment_context>"
            })
            .to_string(),
            1_000,
        );

        assert_eq!(value["role"], crate::context::USER_AGENT_CONTEXT_ROLE);
        assert_eq!(
            value["content"],
            "<environment_context>internal</environment_context>"
        );
        assert!(value.get("parts").is_none());
    }
}
