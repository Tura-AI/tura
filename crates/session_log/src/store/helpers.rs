use anyhow::{Context, Result};
use lifecycle::{
    PlanStatus, SessionAggregate, SessionProjection, SessionState, TaskPlan, TaskStep,
};
use serde::de::DeserializeOwned;
use serde::Deserialize;
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

pub(super) fn task_management_value(task_plan: &TaskPlan) -> Value {
    serde_json::json!({
        "plan_summary": task_plan.plan_summary,
        "tasks": task_plan.detailed_tasks,
    })
}

pub(super) fn legacy_session_aggregate(
    session_id: &str,
    state: SessionState,
    parent_id: Option<String>,
    management: &Value,
    task_management: Option<&Value>,
) -> Result<SessionAggregate> {
    let task_plan = management
        .get("task_plan")
        .cloned()
        .or_else(|| task_management.map(task_plan_from_projection))
        .map(parse_legacy_task_plan)
        .transpose()
        .with_context(|| format!("invalid task plan for session {session_id}"))?
        .unwrap_or_default();
    let mut aggregate = SessionAggregate::new(session_id.to_string());
    aggregate.state = state;
    aggregate.parent_id = parent_id;
    aggregate.task_plan = task_plan;
    aggregate.cancelled = state == SessionState::Cancelled;
    Ok(aggregate)
}

fn task_plan_from_projection(task_management: &Value) -> Value {
    serde_json::json!({
        "plan_summary": task_management
            .get("plan_summary")
            .cloned()
            .unwrap_or_else(|| Value::String(String::new())),
        "detailed_tasks": task_management
            .get("tasks")
            .cloned()
            .unwrap_or_else(|| Value::Array(Vec::new())),
    })
}

fn parse_legacy_task_plan(value: Value) -> Result<TaskPlan, serde_json::Error> {
    match serde_json::from_value::<TaskPlan>(value.clone()) {
        Ok(task_plan) => Ok(task_plan),
        Err(canonical_error) => serde_json::from_value::<LegacyTaskPlan>(value)
            .map(TaskPlan::from)
            .map_err(|_| canonical_error),
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacyTaskPlan {
    #[serde(default)]
    plan_summary: String,
    #[serde(default)]
    detailed_tasks: Vec<LegacyTaskStep>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacyTaskStep {
    #[serde(default)]
    id: String,
    #[serde(default)]
    step: String,
    #[serde(default)]
    status: PlanStatus,
    #[serde(default)]
    deliverables: Vec<String>,
}

impl From<LegacyTaskPlan> for TaskPlan {
    fn from(legacy: LegacyTaskPlan) -> Self {
        Self {
            plan_summary: legacy.plan_summary,
            detailed_tasks: legacy
                .detailed_tasks
                .into_iter()
                .enumerate()
                .map(|(index, task)| TaskStep {
                    task_id: task.id,
                    step: index as u64 + 1,
                    task_summary: task.step.clone(),
                    step_task: task.step,
                    status: task.status,
                    step_deliverable_description: task.deliverables.join("\n"),
                    ..TaskStep::default()
                })
                .collect(),
        }
    }
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
