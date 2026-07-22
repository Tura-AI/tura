use super::*;
use lifecycle::{SessionTaskPatch, SessionTaskPlanPatch};

pub(super) fn parse_task_management_patch(
    patch: serde_json::Value,
    now: DateTime<Utc>,
) -> Result<SessionTaskPlanPatch, String> {
    if let Some(tasks) = patch.as_array() {
        let tasks = parse_task_patch_list(tasks)?;
        return Ok(SessionTaskPlanPatch {
            plan_summary: None,
            generated_task_ids: (0..tasks.len()).map(|_| random_task_id()).collect(),
            tasks: Some(tasks),
            task: None,
            generated_task_id: random_task_id(),
            now,
        });
    }
    let object = patch
        .as_object()
        .ok_or_else(|| "task_management must be an object or array".to_string())?;
    let tasks = object
        .get("tasks")
        .and_then(serde_json::Value::as_array)
        .map(|tasks| parse_task_patch_list(tasks))
        .transpose()?;
    let generated_task_ids = tasks
        .as_ref()
        .map(|tasks| (0..tasks.len()).map(|_| random_task_id()).collect())
        .unwrap_or_default();
    let task = object_has_any_field(object, TASK_MANAGEMENT_TASK_PATCH_FIELDS)
        .then(|| parse_task_patch(object))
        .transpose()?;
    Ok(SessionTaskPlanPatch {
        plan_summary: string_field(object, &["plan_summary"]),
        tasks,
        task,
        generated_task_ids,
        generated_task_id: random_task_id(),
        now,
    })
}

fn parse_task_patch_list(tasks: &[serde_json::Value]) -> Result<Vec<SessionTaskPatch>, String> {
    tasks
        .iter()
        .map(|value| {
            value
                .as_object()
                .ok_or_else(|| "tasks entries must be objects".to_string())
                .and_then(parse_task_patch)
        })
        .collect()
}

fn parse_task_patch(
    object: &serde_json::Map<String, serde_json::Value>,
) -> Result<SessionTaskPatch, String> {
    Ok(SessionTaskPatch {
        task_id: string_field(object, &["task_id"]),
        step: number_field(object, &["step"]),
        task_summary: string_field(object, &["task_summary"]),
        deliverable: string_field(object, &["deliverable"]),
        sub_session_id: string_field(object, &["sub_session_id"]),
        start_condition: first_field(object, &["start_condition"])
            .map(|value| {
                serde_json::from_value(value.clone())
                    .map_err(|error| format!("invalid start_condition: {error}"))
            })
            .transpose()?,
        start_at: first_field(object, &["start_at"])
            .map(parse_start_at)
            .transpose()?,
        poll_interval: first_field(object, &["poll_interval"])
            .map(|value| {
                serde_json::from_value(value.clone())
                    .map_err(|error| format!("invalid poll interval: {error}"))
            })
            .transpose()?,
        status: first_field(object, &["status"])
            .map(|value| {
                serde_json::from_value(value.clone())
                    .map_err(|error| format!("invalid status: {error}"))
            })
            .transpose()?,
    })
}

fn first_field<'a>(
    object: &'a serde_json::Map<String, serde_json::Value>,
    names: &[&str],
) -> Option<&'a serde_json::Value> {
    names.iter().find_map(|name| object.get(*name))
}

fn object_has_any_field(
    object: &serde_json::Map<String, serde_json::Value>,
    names: &[&str],
) -> bool {
    names.iter().any(|name| object.contains_key(*name))
}

fn string_field(
    object: &serde_json::Map<String, serde_json::Value>,
    names: &[&str],
) -> Option<String> {
    first_field(object, names)
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn number_field(
    object: &serde_json::Map<String, serde_json::Value>,
    names: &[&str],
) -> Option<u64> {
    first_field(object, names).and_then(serde_json::Value::as_u64)
}

fn parse_start_at(value: &serde_json::Value) -> Result<DateTime<Utc>, String> {
    if let Some(text) = value.as_str() {
        return DateTime::parse_from_rfc3339(text)
            .map(|datetime| datetime.with_timezone(&Utc))
            .map_err(|err| format!("invalid start_at: {err}"));
    }
    if let Some(millis) = value.as_i64() {
        return DateTime::<Utc>::from_timestamp_millis(millis)
            .ok_or_else(|| "invalid start_at milliseconds".to_string());
    }
    Err("start_at must be RFC3339 or epoch milliseconds".to_string())
}
