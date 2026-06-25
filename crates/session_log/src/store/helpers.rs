use crate::SessionState;
use anyhow::{Context, Result};
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

pub(super) fn millis_at(value: &Value, path: &[&str]) -> Option<i64> {
    string_at(value, path).and_then(|text| {
        chrono::DateTime::parse_from_rfc3339(&text)
            .ok()
            .map(|value| value.timestamp_millis())
    })
}

pub(super) fn management_task_management(management: &Value) -> Option<Value> {
    let task_plan = management.get("task_plan")?;
    let tasks = task_plan
        .get("detailed_tasks")
        .cloned()
        .unwrap_or(Value::Null);
    Some(serde_json::json!({
        "plan_summary": task_plan.get("plan_summary").cloned().unwrap_or(Value::String(String::new())),
        "tasks": tasks,
    }))
}

pub(super) fn session_state_from_management(
    management: &Value,
    session_id: &str,
) -> Result<SessionState> {
    let value = management
        .get("state")
        .cloned()
        .with_context(|| format!("session management state missing for session {session_id}"))?;
    serde_json::from_value(value)
        .with_context(|| format!("invalid canonical session state for session {session_id}"))
}

pub(super) fn session_state_text(state: SessionState) -> Result<String> {
    match serde_json::to_value(state)? {
        Value::String(value) => Ok(value),
        _ => anyhow::bail!("session state did not serialize to a string"),
    }
}

pub(super) fn transition_management_to_interrupted(
    management: &mut Value,
    session_id: &str,
    now_ms: i64,
) -> Result<bool> {
    let current = session_state_from_management(management, session_id)?;
    if !current.is_recoverable_running() {
        return Ok(false);
    }
    if !current.can_transition_to(SessionState::Interrupted) {
        anyhow::bail!(
            "invalid session state transition for {session_id}: {:?} -> {:?}",
            current,
            SessionState::Interrupted
        );
    }
    set_object_string(
        management,
        "state",
        &session_state_text(SessionState::Interrupted)?,
    );
    set_object_string(
        management,
        "session_last_update_at",
        &millis_to_rfc3339(now_ms)?,
    );
    Ok(true)
}

pub(super) fn set_object_string(value: &mut Value, key: &str, next: &str) {
    if !value.is_object() {
        *value = serde_json::json!({});
    }
    if let Some(object) = value.as_object_mut() {
        object.insert(key.to_string(), Value::String(next.to_string()));
    }
}

pub(super) fn set_object_i64(value: &mut Value, key: &str, next: i64) {
    if !value.is_object() {
        *value = serde_json::json!({});
    }
    if let Some(object) = value.as_object_mut() {
        object.insert(key.to_string(), Value::Number(next.into()));
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
