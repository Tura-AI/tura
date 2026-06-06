use super::*;

pub async fn list_messages(Path(session_id): Path<String>) -> Json<Vec<serde_json::Value>> {
    let messages = session_store().get_messages(&session_id);
    let api_messages: Vec<serde_json::Value> = messages
        .into_iter()
        .map(message_with_parts_from_store)
        .collect();
    Json(api_messages)
}

pub async fn send_message(
    Path(session_id): Path<String>,
    Json(payload): Json<SendMessageRequest>,
) -> Json<Message> {
    session_store().add_message(
        &session_id,
        SessionMessageRole::User,
        payload.content.clone(),
    );
    session_store().update_session_status(&session_id, SessionStatusMano::Busy);
    let before_count = session_store().get_messages(&session_id).len();
    run_mano_for_prompt(
        session_id.clone(),
        serde_json::json!({
            "parts": [{
                "type": "text",
                "text": payload.content,
            }]
        }),
    );

    if let Some(msg) = final_agent_message(&session_id, before_count) {
        return Json(api_message_from_store(msg));
    }

    Json(Message {
        id: "error".to_string(),
        session_id,
        role: MessageRole::Assistant,
        parts: vec![],
        created_at: 0,
        updated_at: 0,
        parent_id: None,
    })
}

pub async fn send_agent_message(
    Path(session_id): Path<String>,
    Json(payload): Json<SendAgentMessageRequest>,
) -> Json<SendAgentMessageResponse> {
    let content = agent_message_content(&payload);
    let message = if content.trim().is_empty() {
        None
    } else {
        session_store().add_message_with_metadata(
            &session_id,
            SessionMessageRole::Assistant,
            content,
            agent_message_metadata(&payload),
        )
    };
    let tool_message = payload.tool_call.as_ref().and_then(|tool_call| {
        if let Some(todos) = planning_todos(tool_call) {
            session_store().set_todos(&session_id, todos);
        }
        session_store().add_tool_message(
            &session_id,
            tool_call.tool_name.clone(),
            tool_call.call_id.clone(),
            tool_call.state.clone(),
            tool_call.metadata.clone(),
        )
    });
    sync_auto_session_name_from_agent_tool_call(&session_id, payload.tool_call.as_ref());

    match message.or(tool_message) {
        Some(message) => Json(SendAgentMessageResponse {
            ok: true,
            session_id: session_id.clone(),
            message_id: Some(message.id.clone()),
            event: {
                let info = api_message_from_store(message);
                let event = GlobalEvent::MessageUpdated {
                    properties: MessageUpdatedProperties { session_id, info },
                };
                session_store().push_event(event.clone());
                Some(event)
            },
            error: None,
        }),
        None => Json(SendAgentMessageResponse {
            ok: false,
            session_id,
            message_id: None,
            event: None,
            error: Some("failed to store agent message".to_string()),
        }),
    }
}

fn sync_auto_session_name_from_agent_tool_call(
    session_id: &str,
    tool_call: Option<&SendAgentToolCall>,
) {
    let Some(summary) = tool_call.and_then(last_task_summary_from_tool_call) else {
        return;
    };
    let Some(current_session) = session_store().get_session(session_id) else {
        return;
    };
    let default_name = current_session
        .name
        .as_deref()
        .is_none_or(|name| name.trim().is_empty() || name.starts_with("Session-"));
    if !current_session.auto_session_name && !default_name {
        return;
    }
    let Some(session) = session_store().update_session(
        session_id,
        Some(summary),
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    ) else {
        return;
    };
    session_store().push_event(GlobalEvent::SessionUpdated {
        properties: SessionUpdatedProperties {
            session_id: session.id.clone(),
            info: session,
        },
    });
}

fn last_task_summary_from_tool_call(tool_call: &SendAgentToolCall) -> Option<String> {
    let mut summaries = Vec::new();
    if let Some(output) = tool_call
        .metadata
        .as_ref()
        .and_then(|metadata| metadata.get("output"))
    {
        collect_task_summaries(output, &mut summaries);
    }
    if summaries.is_empty() {
        if let Some(output) = tool_call
            .state
            .get("metadata")
            .and_then(|metadata| metadata.get("output"))
            .or_else(|| tool_call.state.get("output"))
        {
            collect_task_summaries(output, &mut summaries);
        }
    }
    if summaries.is_empty() {
        collect_task_summaries(&tool_call.state, &mut summaries);
    }
    summaries.pop()
}

fn collect_task_summaries(value: &serde_json::Value, summaries: &mut Vec<String>) {
    match value {
        serde_json::Value::Object(object) => {
            if let Some(summary) = object
                .get("task_summary")
                .and_then(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
            {
                summaries.push(summary.to_string());
            }
            for child in object.values() {
                collect_task_summaries(child, summaries);
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                collect_task_summaries(item, summaries);
            }
        }
        _ => {}
    }
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct SendAgentMessageRequest {
    pub reply_message: String,
    pub new_learning: String,
    pub step_summary: Option<String>,
    #[serde(default)]
    pub media: Vec<SendAgentMedia>,
    pub runtime_id: Option<String>,
    pub tool_call: Option<SendAgentToolCall>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct SendAgentToolCall {
    pub tool_name: String,
    pub call_id: String,
    pub state: serde_json::Value,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct SendAgentMedia {
    pub path: String,
    #[serde(rename = "type")]
    pub media_type: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SendAgentMessageResponse {
    pub ok: bool,
    pub session_id: String,
    pub message_id: Option<String>,
    pub event: Option<GlobalEvent>,
    pub error: Option<String>,
}

pub(super) fn agent_message_content(payload: &SendAgentMessageRequest) -> String {
    if payload.tool_call.is_some()
        && payload.reply_message.trim().is_empty()
        && payload.media.is_empty()
    {
        return String::new();
    }

    let mut content = frontend_safe_reply_message(&payload.reply_message);

    if !payload.media.is_empty() {
        if !content.trim().is_empty() {
            content.push_str("\n\n");
        }
        for item in &payload.media {
            content.push_str("[MEDIA:");
            content.push_str(&item.path);
            content.push_str(":MEDIA]\n");
        }
    }

    content
}

pub(super) fn agent_message_metadata(
    payload: &SendAgentMessageRequest,
) -> Option<serde_json::Value> {
    let step_summary = payload
        .step_summary
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());

    step_summary?;

    let mut metadata = serde_json::Map::new();
    if let Some(step_summary) = step_summary {
        metadata.insert(
            "step_summary".to_string(),
            serde_json::Value::String(step_summary.to_string()),
        );
    }
    Some(serde_json::Value::Object(metadata))
}

pub(super) fn planning_todos(tool_call: &SendAgentToolCall) -> Option<Vec<serde_json::Value>> {
    if tool_call.tool_name != "planning" {
        return None;
    }

    let input = tool_call
        .state
        .get("input")
        .or_else(|| tool_call.metadata.as_ref()?.get("input"))?;
    let steps = input.get("steps")?.as_array()?;
    if steps.is_empty() {
        return None;
    }

    let status = tool_call
        .state
        .get("status")
        .and_then(|value| value.as_str());
    let output_steps = planning_output_steps(tool_call);
    let running_index = if status == Some("running") {
        steps
            .iter()
            .enumerate()
            .filter(|(index, _)| {
                let number = index + 1;
                !output_steps.iter().any(|item| {
                    item.get("index").and_then(|value| value.as_u64()) == Some(number as u64)
                })
            })
            .map(|(index, _)| index)
            .next()
    } else {
        None
    };

    Some(
        steps
            .iter()
            .enumerate()
            .map(|(index, step)| {
                let number = index + 1;
                let output_step = output_steps.iter().find(|item| {
                    item.get("index").and_then(|value| value.as_u64()) == Some(number as u64)
                });
                let status = match output_step {
                    Some(item)
                        if item.get("ok").and_then(|value| value.as_bool()) == Some(true) =>
                    {
                        "completed"
                    }
                    Some(_) => "cancelled",
                    None if status == Some("running") && Some(index) == running_index => {
                        "in_progress"
                    }
                    None if status == Some("pending") => "pending",
                    None if matches!(status, Some("completed" | "error")) => "cancelled",
                    None => "pending",
                };
                serde_json::json!({
                    "id": format!("{}:{number}", tool_call.call_id),
                    "content": todo_content(step, number),
                    "status": status,
                    "priority": "medium",
                })
            })
            .collect(),
    )
}

fn planning_output_steps(tool_call: &SendAgentToolCall) -> Vec<serde_json::Value> {
    let raw = tool_call
        .metadata
        .as_ref()
        .and_then(|metadata| metadata.get("output"))
        .or_else(|| tool_call.state.get("output"));
    let Some(output) = raw.and_then(parse_json_value) else {
        return Vec::new();
    };
    let result = output
        .get("results")
        .and_then(|results| results.as_array())
        .and_then(|results| results.iter().find(|value| value.is_object()))
        .unwrap_or(&output);

    result
        .get("steps")
        .and_then(|steps| steps.as_array())
        .cloned()
        .unwrap_or_default()
}

fn parse_json_value(value: &serde_json::Value) -> Option<serde_json::Value> {
    match value {
        serde_json::Value::String(text) => serde_json::from_str(text).ok(),
        value if value.is_object() => Some(value.clone()),
        _ => None,
    }
}

fn todo_content(step: &serde_json::Value, number: usize) -> String {
    step.get("step_goal")
        .and_then(|value| value.as_str())
        .or_else(|| {
            step.get("task_instruction")
                .and_then(|value| value.as_str())
        })
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .unwrap_or_else(|| format!("Step {number}"))
}

pub async fn get_message(
    Path((session_id, message_id)): Path<(String, String)>,
) -> Json<serde_json::Value> {
    let messages = session_store().get_messages(&session_id);
    let message = messages
        .into_iter()
        .find(|m| m.id == message_id)
        .map(message_with_parts_from_store)
        .unwrap_or_else(|| {
            serde_json::json!({
                "info": {
                    "id": message_id,
                    "sessionID": session_id,
                    "role": "user",
                    "time": { "created": 0 },
                    "parts": [],
                },
                "parts": [],
            })
        });
    Json(message)
}

pub async fn get_message_part(
    Path((session_id, message_id, part_id)): Path<(String, String, String)>,
) -> Json<serde_json::Value> {
    let messages = session_store().get_messages(&session_id);
    let message = messages.into_iter().find(|m| m.id == message_id);

    let part = message
        .and_then(|m| m.parts.into_iter().find(|p| p.id == part_id))
        .map(|p| part_json(&session_id, &message_id, p))
        .unwrap_or_else(|| {
            serde_json::json!({
                "id": part_id,
                "sessionID": session_id,
                "messageID": message_id,
                "type": "text",
                "text": "",
            })
        });
    Json(part)
}

// ============================================================================
// Session Commands
// ============================================================================

pub async fn session_command(
    Path(session_id): Path<String>,
    Json(payload): Json<CommandRequest>,
) -> Json<CommandResponse> {
    let directory = session_store()
        .get_session(&session_id)
        .and_then(|session| session.directory)
        .unwrap_or_else(|| ".".to_string());
    let output = run_session_shell_command(&directory, &payload.command)
        .unwrap_or_else(|error| format!("failed to run session command: {error}"));
    Json(CommandResponse { output })
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct CommandRequest {
    pub command: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct CommandResponse {
    pub output: String,
}

// ============================================================================
// Session Todo
// ============================================================================

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[allow(dead_code)]
pub struct TodoItem {
    pub id: String,
    pub content: String,
    pub status: String,
    pub priority: String,
}

pub async fn get_todos(Path(session_id): Path<String>) -> Json<Vec<serde_json::Value>> {
    Json(session_store().get_todos(&session_id))
}

pub async fn update_todos(
    Path(session_id): Path<String>,
    Json(payload): Json<Vec<serde_json::Value>>,
) -> Json<Vec<serde_json::Value>> {
    Json(session_store().set_todos(&session_id, payload))
}

// ============================================================================
// Session Revert / Unrevert
// ============================================================================
