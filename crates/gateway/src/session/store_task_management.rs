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

fn parse_task_patch_list(
    tasks: &[serde_json::Value],
) -> Result<Vec<SessionTaskPatch>, String> {
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

fn ensure_first_task(info: &mut SessionInfo) -> Result<&mut TaskStep, String> {
    if info.management.task_plan.detailed_tasks.is_empty() {
        let summary = info
            .management
            .task_plan
            .plan_summary
            .clone()
            .if_empty(|| info.management.session_name.clone());
        let task_id = random_task_id();
        info.management.task_plan.detailed_tasks.push(TaskStep {
            task_id,
            step: 1,
            sub_session_id: String::new(),
            start_at: Utc::now(),
            poll_interval: PollInterval::default(),
            start_condition: StartCondition::UserAction,
            status: PlanStatus::Todo,
            task_summary: summary.clone(),
            step_task: summary,
            ..TaskStep::default()
        });
    }
    info.management
        .task_plan
        .detailed_tasks
        .first_mut()
        .ok_or_else(|| "failed to create default task".to_string())
}

trait EmptyStringExt {
    fn if_empty(self, fallback: impl FnOnce() -> String) -> String;
}

impl EmptyStringExt for String {
    fn if_empty(self, fallback: impl FnOnce() -> String) -> String {
        if self.trim().is_empty() {
            fallback()
        } else {
            self
        }
    }
}

pub(super) fn apply_task_management_patch(
    info: &mut SessionInfo,
    patch: serde_json::Value,
) -> Result<(), String> {
    if let Some(tasks) = patch.as_array() {
        return apply_task_list_patch(info, tasks);
    }
    let Some(object) = patch.as_object() else {
        return Err("task_management must be an object or array".to_string());
    };
    if let Some(tasks) = object.get("tasks").and_then(serde_json::Value::as_array) {
        apply_task_list_patch(info, tasks)?;
    }

    if let Some(summary) = string_field(object, &["plan_summary"]) {
        info.management.task_plan.plan_summary = summary;
    }

    if !object_has_any_field(object, TASK_MANAGEMENT_TASK_PATCH_FIELDS) {
        return Ok(());
    }

    let task_id = string_field(object, &["task_id"]);
    if task_id.is_none() && info.management.task_plan.detailed_tasks.len() > 1 {
        return Err(
            "task_management task patch requires task_id for multi-task sessions".to_string(),
        );
    }

    let (task_summary, updated_task_summary) = {
        let task = match task_id.as_deref() {
            Some(id) => {
                let index = info
                    .management
                    .task_plan
                    .detailed_tasks
                    .iter()
                    .position(|task| task.task_id == id);
                match index {
                    Some(index) => &mut info.management.task_plan.detailed_tasks[index],
                    None => {
                        let step = number_field(object, &["step"])
                            .unwrap_or((info.management.task_plan.detailed_tasks.len() + 1) as u64);
                        info.management.task_plan.detailed_tasks.push(TaskStep {
                            task_id: id.to_string(),
                            step,
                            start_at: Utc::now(),
                            start_condition: StartCondition::UserAction,
                            ..TaskStep::default()
                        });
                        info.management
                            .task_plan
                            .detailed_tasks
                            .last_mut()
                            .ok_or_else(|| "failed to create task_management task".to_string())?
                    }
                }
            }
            None => ensure_first_task(info)?,
        };
        let updated_task_summary = apply_single_task_patch(task, object)?;
        (task.task_summary.clone(), updated_task_summary)
    };
    if info.management.auto_session_name {
        if let Some(updated_task_summary) = updated_task_summary {
            set_auto_session_name(info, &updated_task_summary);
        }
    }
    if info.management.task_plan.plan_summary.trim().is_empty() {
        info.management.task_plan.plan_summary = task_summary;
    }
    Ok(())
}

const TASK_MANAGEMENT_TASK_PATCH_FIELDS: &[&str] = &[
    "task_id",
    "step",
    "task_summary",
    "deliverable",
    "sub_session_id",
    "start_condition",
    "start_at",
    "poll_interval",
    "status",
];

fn apply_task_list_patch(
    info: &mut SessionInfo,
    tasks: &[serde_json::Value],
) -> Result<(), String> {
    let mut requested_task_order = Vec::new();
    for value in tasks {
        let Some(object) = value.as_object() else {
            return Err("tasks entries must be objects".to_string());
        };
        let task_id = string_field(object, &["task_id"]).unwrap_or_else(random_task_id);
        let position = info
            .management
            .task_plan
            .detailed_tasks
            .iter()
            .position(|task| task.task_id == task_id);
        // Only tasks the patch explicitly references for reordering should drive
        // the requested order. Newly created tasks append in iteration order via
        // their existing position, so an untouched existing task is never pushed
        // behind a freshly generated one.
        if position.is_some() && !requested_task_order.contains(&task_id) {
            requested_task_order.push(task_id.clone());
        }
        let index = match position {
            Some(index) => index,
            None => {
                let step = number_field(object, &["step"])
                    .unwrap_or((info.management.task_plan.detailed_tasks.len() + 1) as u64);
                info.management.task_plan.detailed_tasks.push(TaskStep {
                    task_id: task_id.clone(),
                    step,
                    start_at: Utc::now(),
                    start_condition: StartCondition::UserAction,
                    ..TaskStep::default()
                });
                info.management.task_plan.detailed_tasks.len() - 1
            }
        };
        if let Some(task_summary) =
            apply_single_task_patch(&mut info.management.task_plan.detailed_tasks[index], object)?
        {
            if info.management.auto_session_name {
                set_auto_session_name(info, &task_summary);
            }
        }
        if info.management.task_plan.detailed_tasks[index]
            .task_id
            .trim()
            .is_empty()
        {
            info.management.task_plan.detailed_tasks[index].task_id = task_id;
        }
    }
    reorder_tasks_by_patch_order(
        &mut info.management.task_plan.detailed_tasks,
        &requested_task_order,
    );
    renumber_task_steps(&mut info.management.task_plan.detailed_tasks);
    Ok(())
}

fn reorder_tasks_by_patch_order(tasks: &mut [TaskStep], requested_task_order: &[String]) {
    let requested_positions: HashMap<&str, usize> = requested_task_order
        .iter()
        .enumerate()
        .map(|(index, task_id)| (task_id.as_str(), index))
        .collect();
    let existing_positions: HashMap<String, usize> = tasks
        .iter()
        .enumerate()
        .map(|(index, task)| (task.task_id.clone(), index))
        .collect();
    tasks.sort_by_key(|task| {
        (
            requested_positions
                .get(task.task_id.as_str())
                .copied()
                .unwrap_or(usize::MAX),
            existing_positions
                .get(&task.task_id)
                .copied()
                .unwrap_or(usize::MAX),
        )
    });
}

fn renumber_task_steps(tasks: &mut [TaskStep]) {
    for (index, task) in tasks.iter_mut().enumerate() {
        task.step = (index + 1) as u64;
    }
}

fn apply_single_task_patch(
    task: &mut TaskStep,
    object: &serde_json::Map<String, serde_json::Value>,
) -> Result<Option<String>, String> {
    let mut updated_task_summary = None;
    if let Some(task_id) = string_field(object, &["task_id"]) {
        task.task_id = task_id;
    }
    if let Some(step) = number_field(object, &["step"]) {
        task.step = step;
    }
    if let Some(summary) = string_field(object, &["task_summary"]) {
        task.task_summary = summary.clone();
        updated_task_summary = Some(summary.clone());
        if task.step_task.trim().is_empty() {
            task.step_task = summary;
        }
    }
    if let Some(delivery) = string_field(object, &["deliverable"]) {
        task.step_deliverable_description = delivery;
    }
    if let Some(sub_session_id) = string_field(object, &["sub_session_id"]) {
        task.sub_session_id = sub_session_id;
    }
    if let Some(value) = first_field(object, &["status"]) {
        apply_status_patch(task, value)?;
    }
    if let Some(value) = first_field(object, &["poll_interval"]) {
        task.poll_interval = serde_json::from_value(value.clone())
            .map_err(|err| format!("invalid poll interval: {err}"))?;
        if task.poll_interval.m != 0
            || task.poll_interval.d != 0
            || task.poll_interval.h != 0
            || task.poll_interval.s != 0
        {
            task.start_condition = StartCondition::PollingTask;
        } else if matches!(task.start_condition, StartCondition::PollingTask) {
            task.start_condition = StartCondition::UserAction;
        }
    }
    if let Some(value) = first_field(object, &["start_at"]) {
        task.start_at = parse_start_at(value)?;
        if !matches!(task.start_condition, StartCondition::PollingTask) {
            task.start_condition = StartCondition::ScheduledTask;
        }
    }
    if let Some(value) = first_field(object, &["start_condition"]) {
        apply_start_condition_patch(task, value)?;
    }
    Ok(updated_task_summary)
}

fn set_auto_session_name(info: &mut SessionInfo, task_summary: &str) {
    let task_summary = task_summary.trim();
    if !task_summary.is_empty() {
        info.management.session_name = task_summary.to_string();
    }
}

fn apply_status_patch(task: &mut TaskStep, value: &serde_json::Value) -> Result<(), String> {
    task.status =
        serde_json::from_value(value.clone()).map_err(|err| format!("invalid status: {err}"))?;
    Ok(())
}

fn apply_start_condition_patch(
    task: &mut TaskStep,
    value: &serde_json::Value,
) -> Result<(), String> {
    task.start_condition = serde_json::from_value(value.clone())
        .map_err(|err| format!("invalid start_condition: {err}"))?;
    Ok(())
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
