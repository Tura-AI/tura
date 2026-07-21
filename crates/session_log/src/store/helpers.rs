use anyhow::{Context, Result};
use lifecycle::{SessionAggregate, SessionEvent, SessionProjection, SessionState, TaskPlan};
use rusqlite::{params, Connection};
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::path::{Path, PathBuf};

pub(super) fn bounded_page(
    requested: u64,
    page_size: u64,
    total: u64,
    zero_means_last: bool,
) -> u64 {
    if total == 0 {
        return 0;
    }
    let last = (total - 1) / page_size;
    if zero_means_last && requested == 0 {
        return last;
    }
    requested.min(last)
}

pub(super) fn string_at(value: &Value, path: &[&str]) -> Option<String> {
    path.iter()
        .try_fold(value, |current, key| current.get(*key))
        .and_then(Value::as_str)
        .map(ToString::to_string)
}

pub(super) fn i64_at(value: &Value, path: &[&str]) -> Option<i64> {
    path.iter()
        .try_fold(value, |current, key| current.get(*key))
        .and_then(Value::as_i64)
}

pub(super) fn task_management_value(task_plan: &TaskPlan) -> Value {
    serde_json::json!({
        "plan_summary": task_plan.plan_summary,
        "tasks": task_plan.detailed_tasks,
    })
}

pub(super) fn apply_lifecycle_projection(
    management: &mut Value,
    session: &mut Value,
    projection: &SessionProjection,
) -> Result<Value> {
    let task_management = task_management_value(&projection.task_plan);
    set_object_value(management, "state", serde_json::to_value(projection.state)?);
    set_object_value(
        management,
        "task_plan",
        serde_json::to_value(&projection.task_plan)?,
    );
    set_object_value(
        management,
        "is_child_session",
        Value::Bool(projection.parent_id.is_some()),
    );
    set_object_string(session, "status", projection.state.ui_status());
    set_object_value(session, "task_management", task_management.clone());
    match projection.parent_id.as_deref() {
        Some(parent_id) => {
            set_object_value(session, "parent_id", Value::String(parent_id.to_string()))
        }
        None => remove_object_field(session, "parent_id"),
    }
    set_object_value(session, "management", management.clone());
    Ok(task_management)
}

pub(super) fn replay_session_events(
    conn: &Connection,
    session_id: &str,
) -> Result<SessionAggregate> {
    let mut statement = conn.prepare(
        "SELECT event_seq, event_json FROM session_events
         WHERE session_id = ?1 ORDER BY event_seq",
    )?;
    let rows = statement.query_map(params![session_id], |row| {
        Ok((row.get::<_, u64>(0)?, row.get::<_, String>(1)?))
    })?;
    let mut events = Vec::new();
    for (expected, row) in rows.enumerate() {
        let (event_seq, event_json) = row?;
        if event_seq != expected as u64 {
            anyhow::bail!(
                "session {session_id} event sequence is not contiguous: expected {expected}, found {event_seq}"
            );
        }
        events.push(
            serde_json::from_str::<SessionEvent>(&event_json)
                .with_context(|| format!("invalid session event {event_seq} for {session_id}"))?,
        );
    }
    if events.is_empty() {
        anyhow::bail!("session {session_id} has no canonical lifecycle events");
    }
    SessionAggregate::replay(session_id.to_string(), events)
        .map_err(anyhow::Error::msg)
        .with_context(|| format!("invalid canonical lifecycle history for session {session_id}"))
}

pub(super) fn append_session_event(
    conn: &Connection,
    session_id: &str,
    event: &SessionEvent,
) -> Result<u64> {
    let event_seq = conn.query_row(
        "SELECT COALESCE(MAX(event_seq) + 1, 0) FROM session_events WHERE session_id = ?1",
        params![session_id],
        |row| row.get::<_, u64>(0),
    )?;
    conn.execute(
        "INSERT INTO session_events(session_id, event_seq, event_json) VALUES (?1, ?2, ?3)",
        params![session_id, event_seq, serde_json::to_string(event)?],
    )?;
    Ok(event_seq + 1)
}

pub(super) fn session_state_text(state: SessionState) -> Result<String> {
    match serde_json::to_value(state)? {
        Value::String(value) => Ok(value),
        _ => anyhow::bail!("session state did not serialize to a string"),
    }
}

pub(super) fn set_object_string(value: &mut Value, key: &str, next: &str) {
    if !value.is_object() {
        *value = serde_json::json!({});
    }
    if let Some(object) = value.as_object_mut() {
        object.insert(key.to_string(), Value::String(next.to_string()));
    }
}

pub(super) fn set_object_value(value: &mut Value, key: &str, next: Value) {
    if !value.is_object() {
        *value = serde_json::json!({});
    }
    if let Some(object) = value.as_object_mut() {
        object.insert(key.to_string(), next);
    }
}

pub(super) fn remove_object_field(value: &mut Value, key: &str) {
    if let Some(object) = value.as_object_mut() {
        object.remove(key);
    }
}

pub(super) fn millis_to_rfc3339(millis: i64) -> Result<String> {
    let timestamp = chrono::DateTime::<chrono::Utc>::from_timestamp_millis(millis)
        .context("invalid session timestamp millis")?;
    Ok(timestamp.to_rfc3339_opts(chrono::SecondsFormat::Millis, true))
}

pub(super) fn path_text(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

pub(super) fn parse_json_field<T: DeserializeOwned>(
    text: &str,
    field: &str,
    session_id: Option<&str>,
) -> Result<T> {
    serde_json::from_str(text).with_context(|| match session_id {
        Some(session_id) => format!("failed to parse {field} for session {session_id}"),
        None => format!("failed to parse {field}"),
    })
}

pub(super) fn remove_sqlite_files(path: &Path) -> Result<()> {
    for suffix in ["", "-wal", "-shm"] {
        let target = PathBuf::from(format!("{}{}", path.display(), suffix));
        if target.exists() {
            std::fs::remove_file(&target)
                .with_context(|| format!("failed to remove {}", target.display()))?;
        }
    }
    Ok(())
}
