//! Whole-session snapshot compatibility checkpoints.

use crate::session_log_client::SessionLogClient;
use crate::state_machine::session_management::SessionManagement;
use chrono::{DateTime, Utc};
use serde_json::Value;

pub(crate) fn persist_session_snapshot(session: &SessionManagement) -> Result<(), String> {
    let record = persisted_record(session);
    let session_info = record
        .get("info")
        .cloned()
        .ok_or_else(|| "persisted session info missing".to_string())?;
    let messages = persisted_messages(session);
    SessionLogClient::discover()
        .map_err(|err| {
            format!(
                "failed to discover session_log client for session snapshot {}: {err}",
                session.session_id
            )
        })?
        .upsert_session(session_info, None, messages, Vec::new())
        .map_err(|err| {
            format!(
                "failed to persist session snapshot {}: {err}",
                session.session_id
            )
        })
}

pub(crate) fn persist_session_checkpoint(session: &SessionManagement, stage: &str) {
    if let Err(err) = persist_session_snapshot(session) {
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
        "messages": persisted_messages(session),
        "todos": [],
    })
}

fn persisted_messages(session: &SessionManagement) -> Vec<Value> {
    let base_time = session.session_created_at.timestamp_millis();
    session
        .session_log
        .iter()
        .enumerate()
        .map(|(index, entry)| persisted_message(session, index, entry, base_time))
        .collect()
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
        .and_then(Value::as_i64)
        .or_else(|| object.get("timestamp").and_then(timestamp_millis))
        .unwrap_or(fallback_time);
    let updated_at = object
        .get("updated_at")
        .and_then(Value::as_i64)
        .or_else(|| object.get("timestamp").and_then(timestamp_millis))
        .unwrap_or(created_at);

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
        .or_insert_with(|| Value::Number(created_at.into()));
    object
        .entry("updated_at".to_string())
        .or_insert_with(|| Value::Number(updated_at.into()));
    object
        .entry("session_id".to_string())
        .or_insert_with(|| Value::String(session.session_id.clone()));
    value
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
                .or_insert_with(|| Value::Number(created_at.into()));
            message_object
                .entry("updated_at".to_string())
                .or_insert_with(|| Value::Number(updated_at.into()));
        }
        return Some(message);
    }

    let role = object.get("role").and_then(Value::as_str)?;
    if !matches!(role, "user" | "assistant" | "system") {
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
    use super::persisted_message;
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
                "id": "msg-stream-runtime-1",
                "part_id": "part-stream-runtime-1",
                "role": "assistant",
                "content": "final visible reply",
                "created_at": 42,
                "updated_at": 43,
                "runtime_id": "runtime-1"
            })
            .to_string(),
            1_000,
        );

        assert_eq!(value["id"], "msg-stream-runtime-1");
        assert_eq!(value["session_id"], "snapshot-session");
        assert_eq!(value["role"], "assistant");
        assert_eq!(value["created_at"], 42);
        assert_eq!(value["updated_at"], 43);
        assert_eq!(value["parts"][0]["id"], "part-stream-runtime-1");
        assert_eq!(value["parts"][0]["type"], "text");
        assert_eq!(value["parts"][0]["text"], "final visible reply");
        assert_eq!(value["parts"][0]["content"], "final visible reply");
    }

    #[test]
    fn persisted_message_normalizes_user_and_system_content_but_keeps_runtime_usage_auxiliary() {
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

        assert_eq!(user["role"], "user");
        assert_eq!(user["parts"][0]["text"], "hello world");
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
