//! Command-run result helpers for tool-flow code.

use chrono::Utc;

use crate::state_machine::runtime_management::RuntimeManagement;
use crate::state_machine::session_management::{PlanStatus, SessionManagement, StartCondition};

pub(crate) fn apply_task_attribution_to_streamed_result(
    session: &SessionManagement,
    streamed_result: &mut serde_json::Value,
) {
    let Some(attribution) = current_task_attribution(session) else {
        return;
    };
    if let Some(object) = streamed_result.as_object_mut() {
        object.insert("task_attribution".to_string(), attribution.clone());
        if let Some(events) = object
            .get_mut("command_events")
            .and_then(serde_json::Value::as_array_mut)
        {
            for event in events {
                if let Some(event_object) = event.as_object_mut() {
                    event_object
                        .entry("task_attribution".to_string())
                        .or_insert_with(|| attribution.clone());
                }
            }
        }
    }
}

fn current_task_attribution(session: &SessionManagement) -> Option<serde_json::Value> {
    session
        .task_plan
        .detailed_tasks
        .iter()
        .find(|task| {
            matches!(task.status, PlanStatus::Doing)
                || (task.status == PlanStatus::Todo
                    && task.start_condition == StartCondition::UserAction)
        })
        .map(|task| {
            serde_json::json!({
                "task_id": task.task_id,
                "step": task.step,
                "task_summary": task.task_summary,
                "deliverable": task.step_deliverable_description,
                "status": task.status,
            })
        })
}

pub(crate) fn record_streamed_command_events(
    session: &mut SessionManagement,
    runtime: &RuntimeManagement,
    streamed_result: &serde_json::Value,
) {
    let Some(events) = streamed_result
        .get("command_events")
        .and_then(serde_json::Value::as_array)
    else {
        return;
    };
    let now = Utc::now();
    for (index, event) in events.iter().enumerate() {
        let mut event = event.clone();
        if !event.is_object() {
            event = serde_json::json!({ "value": event });
        }
        let Some(object) = event.as_object_mut() else {
            tracing::warn!(
                session_id = %session.session_id,
                index,
                "streamed command event normalization produced a non-object value"
            );
            continue;
        };
        object.insert(
            "type".to_string(),
            serde_json::Value::String("streamed_command_event".to_string()),
        );
        object
            .entry("runtime_id".to_string())
            .or_insert_with(|| serde_json::Value::String(runtime.runtime_id.clone()));
        object
            .entry("session_id".to_string())
            .or_insert_with(|| serde_json::Value::String(session.session_id.clone()));
        object
            .entry("event_index".to_string())
            .or_insert_with(|| serde_json::Value::Number(index.into()));
        object
            .entry("timestamp".to_string())
            .or_insert_with(|| serde_json::Value::String(now.to_rfc3339()));
        session.push_log(event.to_string(), now);
    }
}

#[cfg(test)]
mod tests {
    use super::{apply_task_attribution_to_streamed_result, record_streamed_command_events};
    use crate::state_machine::agent_management::{ProviderConfig, ToolChoice};
    use crate::state_machine::runtime_management::{RuntimeManagement, RuntimeProviderConfig};
    use crate::state_machine::session_management::{
        PlanStatus, SessionInput, SessionManagement, TaskStep,
    };
    use chrono::Utc;
    use serde_json::json;
    use std::path::PathBuf;

    fn session() -> SessionManagement {
        let now = Utc::now();
        SessionManagement::new(
            "sess-task-status".to_string(),
            "task status".to_string(),
            PathBuf::from("C:/workspace"),
            false,
            "coding".to_string(),
            SessionInput {
                user_input: "fix the task".to_string(),
                file_input: vec![],
                agent: None,
                runtime_context: None,
                planning_mode_override: None,
            },
            "fix the task".to_string(),
            now,
        )
    }

    #[test]
    fn streamed_command_events_are_audited_with_active_task_attribution() {
        let mut session = session();
        session.task_plan.detailed_tasks.push(TaskStep {
            task_id: "task-aa".to_string(),
            step: 7,
            task_summary: "Inspect behavior".to_string(),
            step_deliverable_description: "Read source and fixtures.".to_string(),
            status: PlanStatus::Doing,
            ..TaskStep::default()
        });
        let mut streamed_result = json!({
            "commands": [{
                "step": 1,
                "command_type": "shell_command",
                "command_line": "rg zip_utils rust-reference/src"
            }],
            "command_events": [
                {
                    "status": "ready",
                    "provider_tool_call_id": "call_provider_1",
                    "command_index": 0,
                    "step": 1,
                    "command_type": "shell_command"
                },
                {
                    "status": "completed",
                    "result_index": 0,
                    "step": 1,
                    "command_type": "shell_command",
                    "success": true
                }
            ],
            "results": [{
                "step": 1,
                "command_type": "shell_command",
                "success": true,
                "output": "ok"
            }]
        });
        let now = Utc::now();
        let runtime = RuntimeManagement::new(
            "runtime-streamed".to_string(),
            session.session_id.clone(),
            session.session_id.clone(),
            RuntimeProviderConfig {
                base: ProviderConfig {
                    tura_llm_name: "provider".to_string(),
                    default_model_tier: None,
                    current_model: None,
                    stream: true,
                    temperature: 0.0,
                    max_tokens: 0,
                    tool_choice: ToolChoice::Auto,
                    time_out_ms: 120_000,
                },
                thinking: false,
                provider_name: "provider".to_string(),
                model_name: "model".to_string(),
                provider_url_name: "provider".to_string(),
                llm_provider_name: "provider".to_string(),
            },
            now,
        );

        apply_task_attribution_to_streamed_result(&session, &mut streamed_result);
        record_streamed_command_events(&mut session, &runtime, &streamed_result);

        assert_eq!(
            streamed_result["task_attribution"]["task_id"],
            json!("task-aa")
        );
        let events = session
            .session_log
            .iter()
            .filter_map(|entry| serde_json::from_str::<serde_json::Value>(entry).ok())
            .filter(|value| {
                value.get("type").and_then(serde_json::Value::as_str)
                    == Some("streamed_command_event")
            })
            .collect::<Vec<_>>();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0]["provider_tool_call_id"], "call_provider_1");
        assert_eq!(events[0]["task_attribution"]["task_id"], "task-aa");
        assert_eq!(events[1]["task_attribution"]["step"], 7);
    }

    #[test]
    fn streamed_command_events_wrap_scalar_values_without_panicking() {
        let mut session = session();
        let runtime = RuntimeManagement::new(
            "runtime-streamed".to_string(),
            session.session_id.clone(),
            session.session_id.clone(),
            RuntimeProviderConfig {
                base: ProviderConfig {
                    tura_llm_name: "provider".to_string(),
                    default_model_tier: None,
                    current_model: None,
                    stream: true,
                    temperature: 0.0,
                    max_tokens: 0,
                    tool_choice: ToolChoice::Auto,
                    time_out_ms: 120_000,
                },
                thinking: false,
                provider_name: "provider".to_string(),
                model_name: "model".to_string(),
                provider_url_name: "provider".to_string(),
                llm_provider_name: "provider".to_string(),
            },
            Utc::now(),
        );
        let streamed_result = json!({
            "command_events": ["ready"]
        });

        record_streamed_command_events(&mut session, &runtime, &streamed_result);

        let event = session
            .session_log
            .iter()
            .filter_map(|entry| serde_json::from_str::<serde_json::Value>(entry).ok())
            .find(|value| {
                value.get("type").and_then(serde_json::Value::as_str)
                    == Some("streamed_command_event")
            })
            .expect("streamed scalar event should be recorded");
        assert_eq!(event["value"], "ready");
        assert_eq!(event["runtime_id"], "runtime-streamed");
        assert_eq!(event["event_index"], 0);
    }
}
