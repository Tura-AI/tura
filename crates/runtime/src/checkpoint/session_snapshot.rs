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
        .map_err(|err| err.to_string())?
        .upsert_session(session_info, None, messages, Vec::new())
        .map_err(|err| err.to_string())
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

    let object = value
        .as_object_mut()
        .expect("persisted message value normalized to object");
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
        SessionState::Failed | SessionState::Cancelled => "error",
    }
}
