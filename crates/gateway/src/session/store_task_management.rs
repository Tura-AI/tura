use super::*;

fn ensure_first_task(info: &mut SessionInfo) -> &mut TaskStep {
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
        .expect("first task should exist")
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
                            .expect("new task should exist")
                    }
                }
            }
            None => ensure_first_task(info),
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
    renumber_task_steps(&mut info.management.task_plan.detailed_tasks);
    Ok(())
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
    match value.as_str() {
        Some("session_idle") => {
            task.status = PlanStatus::Todo;
            task.start_condition = StartCondition::SessionIdle;
            Ok(())
        }
        Some("user_action") => {
            task.status = PlanStatus::Todo;
            task.start_condition = StartCondition::UserAction;
            Ok(())
        }
        Some("scheduled_task") => {
            task.status = PlanStatus::Todo;
            task.start_condition = StartCondition::ScheduledTask;
            Ok(())
        }
        Some("polling_task") => {
            task.status = PlanStatus::Todo;
            task.start_condition = StartCondition::PollingTask;
            Ok(())
        }
        _ => {
            task.status = serde_json::from_value(value.clone())
                .map_err(|err| format!("invalid status: {err}"))?;
            Ok(())
        }
    }
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

pub(super) fn task_is_scheduler_eligible(task: &TaskStep, now: DateTime<Utc>) -> bool {
    if matches!(
        task.status,
        PlanStatus::WaitingUser | PlanStatus::Done | PlanStatus::Archived
    ) {
        return false;
    }
    match task.start_condition {
        StartCondition::ScheduledTask | StartCondition::PollingTask => {
            matches!(task.status, PlanStatus::Todo | PlanStatus::Question) && task.start_at <= now
        }
        StartCondition::SessionIdle => {
            matches!(task.status, PlanStatus::Todo | PlanStatus::Question)
        }
        StartCondition::UserAction => false,
    }
}

pub(super) fn task_display_summary(task: &TaskStep, plan_summary: &str) -> String {
    [
        task.task_summary.as_str(),
        task.step_task.as_str(),
        plan_summary,
    ]
    .into_iter()
    .map(str::trim)
    .find(|value| !value.is_empty())
    .unwrap_or("Continue planned task")
    .to_string()
}

pub(super) fn next_polling_start(
    previous_start: DateTime<Utc>,
    interval: PollInterval,
    now: DateTime<Utc>,
) -> DateTime<Utc> {
    let seconds = interval
        .s
        .saturating_add(interval.m.saturating_mul(60))
        .saturating_add(interval.h.saturating_mul(60 * 60))
        .saturating_add(interval.d.saturating_mul(24 * 60 * 60));
    let seconds = seconds.max(1);
    let step = chrono::Duration::seconds(seconds.min(i64::MAX as u64) as i64);
    let mut next = previous_start + step;
    while next <= now {
        next += step;
    }
    next
}
