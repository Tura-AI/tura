//! Incremental Session context and projection checkpoints.

use crate::session_log_client::SessionLogClient;
use crate::tool_callback_sanitizer::sanitize_tool_callback_output;
use chrono::{DateTime, Utc};
use lifecycle::{SessionLogEntry, SessionManagement};
use serde_json::Value;
use session_log_contract::{
    PersistSessionDeltaRequest, SessionContextRecord, SessionDeltaEntry, SessionRecordProjection,
};

#[derive(Debug)]
pub(crate) struct SessionDeltaWriter {
    client: SessionLogClient,
    next_sequence: u64,
    next_management_sequence: u64,
    retained_from_sequence: u64,
    local_retained_from_sequence: u64,
    last_management: Option<SessionManagement>,
}

impl SessionDeltaWriter {
    pub(crate) fn new(session: &SessionManagement) -> Result<Self, String> {
        let client = SessionLogClient::discover()
            .map_err(|error| format!("failed to discover session service: {error}"))?;
        let context = client.read_context_slice(
            session.session_id.clone(),
            session.context_tokens.limit.max(1),
        )?;
        let local_end = session
            .session_log_retention
            .omitted_entries
            .saturating_add(session.session_log.len() as u64);
        if context.next_sequence < session.session_log_retention.omitted_entries
            || context.next_sequence > local_end
        {
            return Err(format!(
                "session {} context cursor {} is outside local retained range {}..={}",
                session.session_id,
                context.next_sequence,
                session.session_log_retention.omitted_entries,
                local_end
            ));
        }
        let last_management = if context.next_management_sequence > 0 {
            let snapshot = client
                .get_session(session.session_id.clone())
                .map_err(|error| {
                    format!(
                        "failed to load persisted management for session {}: {error}",
                        session.session_id
                    )
                })?
                .ok_or_else(|| format!("session {} not found", session.session_id))?;
            let management = snapshot.into_management().map_err(|error| {
                format!(
                    "invalid persisted session snapshot for {}: {error}",
                    session.session_id
                )
            })?;
            Some(persisted_management_baseline(
                &session.session_id,
                management,
                context.retained_from_sequence,
            )?)
        } else {
            None
        };
        Ok(Self {
            client,
            next_sequence: context.next_sequence,
            next_management_sequence: context.next_management_sequence,
            retained_from_sequence: context.retained_from_sequence,
            local_retained_from_sequence: session.session_log_retention.omitted_entries,
            last_management,
        })
    }

    pub(crate) fn checkpoint(
        &mut self,
        session: &SessionManagement,
        stage: &str,
    ) -> Result<(), String> {
        let local_retained_from_sequence = session.session_log_retention.omitted_entries;
        if local_retained_from_sequence < self.local_retained_from_sequence {
            return Err(format!(
                "session {} checkpoint {stage} moved local retention backward from {} to {}",
                session.session_id, self.local_retained_from_sequence, local_retained_from_sequence
            ));
        }
        let retained_from_sequence =
            if local_retained_from_sequence > self.local_retained_from_sequence {
                local_retained_from_sequence
            } else {
                self.retained_from_sequence
            };
        if self.next_sequence < local_retained_from_sequence {
            return Err(format!(
                "session {} checkpoint {stage} dropped unpersisted context before sequence {}",
                session.session_id, self.next_sequence
            ));
        }
        let local_start = usize::try_from(self.next_sequence - local_retained_from_sequence)
            .map_err(|_| "session context cursor does not fit memory index".to_string())?;
        if local_start > session.session_log.len() {
            return Err(format!(
                "session {} checkpoint {stage} cursor {} exceeds local context end {}",
                session.session_id,
                self.next_sequence,
                local_retained_from_sequence.saturating_add(session.session_log.len() as u64)
            ));
        }
        let entries = session.session_log[local_start..]
            .iter()
            .enumerate()
            .map(|(offset, raw_record)| {
                let sequence = self.next_sequence.saturating_add(offset as u64);
                session_delta_entry(session, sequence, raw_record)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let expected_next = self.next_sequence.saturating_add(entries.len() as u64);
        let management = session.persistence_view(retained_from_sequence);
        let management_delta =
            SessionManagement::persistence_delta(self.last_management.as_ref(), &management);
        let expected_next_management = self.next_management_sequence.saturating_add(1);
        let (next_sequence, next_management_sequence) =
            self.client
                .persist_session_delta(PersistSessionDeltaRequest {
                    session_id: session.session_id.clone(),
                    management_sequence: self.next_management_sequence,
                    management_delta,
                    retained_from_sequence,
                    entries,
                })?;
        if next_sequence != expected_next {
            return Err(format!(
                "session {} checkpoint {stage} acknowledged context cursor {}, expected {}",
                session.session_id, next_sequence, expected_next
            ));
        }
        if next_management_sequence != expected_next_management {
            return Err(format!(
                "session {} checkpoint {stage} acknowledged management cursor {}, expected {}",
                session.session_id, next_management_sequence, expected_next_management
            ));
        }
        self.next_sequence = next_sequence;
        self.next_management_sequence = next_management_sequence;
        self.retained_from_sequence = retained_from_sequence;
        self.local_retained_from_sequence = local_retained_from_sequence;
        self.last_management = Some(management);
        Ok(())
    }
}

fn persisted_management_baseline(
    session_id: &str,
    mut management: SessionManagement,
    retained_from_sequence: u64,
) -> Result<SessionManagement, String> {
    if management.session_id != session_id {
        return Err(format!(
            "persisted management session id {} does not match {session_id}",
            management.session_id
        ));
    }
    management.clear_session_log();
    management.session_log_retention.omitted_entries = retained_from_sequence;
    Ok(management)
}

pub(crate) fn persist_session_checkpoint(
    writer: &mut Option<SessionDeltaWriter>,
    session: &SessionManagement,
    stage: &str,
) -> Result<(), String> {
    if let Some(writer) = writer {
        writer.checkpoint(session, stage)?;
    }
    crate::gateway_events::emit_cli_live_session_checkpoint(session, stage);
    Ok(())
}

fn session_delta_entry(
    session: &SessionManagement,
    sequence: u64,
    entry: &SessionLogEntry,
) -> Result<SessionDeltaEntry, String> {
    let index = usize::try_from(sequence)
        .map_err(|_| format!("session context sequence {sequence} does not fit platform index"))?;
    let base_time = session.session_created_at.timestamp_millis();
    let record = if let Some((runtime_id, tool_part, created_at, updated_at)) =
        runtime_tool_part_from_value(index, entry.value(), base_time)
    {
        serde_json::json!({
            "id": crate::gateway_events::runtime_message_id(&runtime_id),
            "session_id": session.session_id,
            "role": "assistant",
            "parent_id": null,
            "parts": [tool_part],
            "created_at": created_at,
            "updated_at": updated_at,
        })
    } else {
        persisted_message_from_value(session, index, entry.value(), entry.raw(), base_time)
    };
    let projection = session_message_projection(&session.session_id, record);
    Ok(SessionDeltaEntry {
        context: SessionContextRecord {
            sequence,
            raw_record: entry.raw().to_string(),
        },
        projection,
    })
}

fn session_message_projection(session_id: &str, record: Value) -> Option<SessionRecordProjection> {
    let role = record.get("role").and_then(Value::as_str)?;
    if !matches!(role, "user" | "assistant")
        || record.get("parts").and_then(Value::as_array).is_none()
    {
        return None;
    }
    let message_id = record
        .get("id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())?
        .to_string();
    let created_at = record.get("created_at").and_then(Value::as_i64)?;
    let updated_at = record.get("updated_at").and_then(Value::as_i64)?;
    Some(SessionRecordProjection {
        session_id: session_id.to_string(),
        message_id,
        role: role.to_string(),
        created_at,
        updated_at,
        record,
    })
}

fn persisted_message_from_value(
    session: &SessionManagement,
    index: usize,
    parsed: &Value,
    raw: &str,
    base_time: i64,
) -> Value {
    let mut value = parsed.clone();
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
            "content": raw,
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

#[cfg(test)]
fn persisted_message(
    session: &SessionManagement,
    index: usize,
    raw: &str,
    base_time: i64,
) -> Value {
    let entry = SessionLogEntry::new(raw);
    persisted_message_from_value(session, index, entry.value(), entry.raw(), base_time)
}

fn runtime_tool_part_from_value(
    index: usize,
    value: &Value,
    base_time: i64,
) -> Option<(String, Value, i64, i64)> {
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
    let output = sanitize_tool_callback_output(object.get("output").unwrap_or(&Value::Null));
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

#[cfg(test)]
mod tests {
    use super::{persisted_management_baseline, persisted_message, session_message_projection};
    use chrono::Utc;
    use lifecycle::{SessionInput, SessionManagement};
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
    fn persisted_management_is_the_delta_baseline_after_in_memory_changes() {
        let mut persisted = session();
        persisted.replace_session_log(["persisted context".to_string()]);
        let mut current = persisted.clone();
        current.planning_enabled = true;
        current.reflection_enabled = true;

        let baseline = persisted_management_baseline(&current.session_id, persisted, 4)
            .expect("persisted management baseline");
        let delta = SessionManagement::persistence_delta(Some(&baseline), &current);

        assert_eq!(delta.planning_enabled, Some(true));
        assert_eq!(delta.reflection_enabled, Some(true));
        assert!(baseline.session_log.is_empty());
        assert_eq!(baseline.session_log_retention.omitted_entries, 4);
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
    fn system_context_records_do_not_create_frontend_message_projections() {
        let system = serde_json::json!({
            "id": "snapshot-session:log:2",
            "session_id": "snapshot-session",
            "role": "system",
            "parts": [{
                "id": "snapshot-session:log:2:part",
                "type": "text",
                "content": "internal operation manual",
                "text": "internal operation manual"
            }],
            "created_at": 1_000,
            "updated_at": 1_000
        });
        let assistant = serde_json::json!({
            "id": "snapshot-session:assistant",
            "session_id": "snapshot-session",
            "role": "assistant",
            "parts": [{
                "id": "snapshot-session:assistant:part",
                "type": "text",
                "content": "visible reply",
                "text": "visible reply"
            }],
            "created_at": 1_001,
            "updated_at": 1_001
        });

        assert!(session_message_projection("snapshot-session", system).is_none());
        assert!(session_message_projection("snapshot-session", assistant).is_some());
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
