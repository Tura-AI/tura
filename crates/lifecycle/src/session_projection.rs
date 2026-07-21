use crate::{PlanStatus, SessionManagement, TaskStep};

pub(crate) fn task_plan_summary_json(session: &SessionManagement) -> serde_json::Value {
    serde_json::json!({
        "plan_summary": session.task_plan.plan_summary,
        "tasks": session.task_plan.detailed_tasks.iter().enumerate().map(|(index, task)| {
            let mut value = serde_json::json!({
                "index": index + 1,
                "task_id": task.task_id,
                "step": task.step,
                "task_summary": task.task_summary,
                "deliverable": task.step_deliverable_description,
                "sub_session_id": task.sub_session_id,
                "start_condition": task.start_condition,
                "start_at": task.start_at,
                "poll_interval": task.poll_interval,
            });
            if let Some(task_state) = task_state_value(task) {
                value["status"] = task_state;
            }
            value
        }).collect::<Vec<_>>(),
    })
}

pub(crate) fn task_plan_detail_json(session: &SessionManagement) -> serde_json::Value {
    serde_json::to_value(&session.task_plan)
        .unwrap_or_else(|_| serde_json::json!({ "plan_summary": "", "detailed_tasks": [] }))
}

pub(crate) fn task_management_json(session: &SessionManagement) -> serde_json::Value {
    if session.task_plan.detailed_tasks.len() <= 1 {
        let task = session.task_plan.detailed_tasks.first();
        let task_summary = task
            .map(|task| task.task_summary.as_str())
            .filter(|summary| !summary.trim().is_empty())
            .unwrap_or(session.task_plan.plan_summary.as_str());
        let mut value = serde_json::json!({
            "task_id": task.map(|task| task.task_id.as_str()).unwrap_or_default(),
            "step": task.map(|task| task.step).unwrap_or_default(),
            "plan_summary": session.task_plan.plan_summary,
            "task_summary": task_summary,
            "deliverable": task.map(|task| task.step_deliverable_description.as_str()).unwrap_or_default(),
            "sub_session_id": task.map(|task| task.sub_session_id.as_str()).unwrap_or_default(),
            "start_condition": task.map(|task| task.start_condition).unwrap_or_default(),
            "start_at": task.map(|task| task.start_at).unwrap_or(session.session_started_at),
            "poll_interval": task.map(|task| task.poll_interval).unwrap_or_default(),
        });
        if let Some(task) = task {
            if let Some(task_state) = task_state_value(task) {
                value["status"] = task_state;
            }
        }
        return value;
    }

    serde_json::json!({
        "plan_summary": session.task_plan.plan_summary,
        "tasks": session.task_plan.detailed_tasks.iter().map(|task| {
            let mut value = serde_json::json!({
                "task_id": task.task_id,
                "step": task.step,
                "task_summary": task.task_summary,
                "deliverable": task.step_deliverable_description,
                "sub_session_id": task.sub_session_id,
                "start_condition": task.start_condition,
                "start_at": task.start_at,
                "poll_interval": task.poll_interval,
            });
            if let Some(task_state) = task_state_value(task) {
                value["status"] = task_state;
            }
            value
        }).collect::<Vec<_>>(),
    })
}

fn task_state_value(task: &TaskStep) -> Option<serde_json::Value> {
    if task.status != PlanStatus::default() {
        return Some(serde_json::json!(task.status));
    }
    None
}
