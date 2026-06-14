use super::*;

pub async fn list_messages(
    Path(session_id): Path<String>,
    Query(params): Query<MessageListParams>,
) -> Json<Vec<serde_json::Value>> {
    let messages = page_messages(session_store().get_messages(&session_id), &params);
    let api_messages: Vec<serde_json::Value> = messages
        .into_iter()
        .map(message_with_parts_from_store)
        .collect();
    Json(api_messages)
}

#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct MessageListParams {
    pub limit: Option<usize>,
    pub before: Option<String>,
    pub after: Option<String>,
}

fn page_messages<T: Clone + MessageId>(messages: Vec<T>, params: &MessageListParams) -> Vec<T> {
    let limit = params.limit.filter(|limit| *limit > 0);
    if let Some(after) = params.after.as_deref() {
        let start = messages
            .iter()
            .position(|message| message.message_id() == after)
            .map(|index| index + 1)
            .unwrap_or(0);
        let end = limit
            .map(|limit| start.saturating_add(limit).min(messages.len()))
            .unwrap_or(messages.len());
        return messages[start..end].to_vec();
    }

    let end = params
        .before
        .as_deref()
        .and_then(|before| {
            messages
                .iter()
                .position(|message| message.message_id() == before)
        })
        .unwrap_or(messages.len());
    let start = limit.map(|limit| end.saturating_sub(limit)).unwrap_or(0);
    messages[start..end].to_vec()
}

trait MessageId {
    fn message_id(&self) -> &str;
}

impl MessageId for crate::session::Message {
    fn message_id(&self) -> &str {
        &self.id
    }
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
    } else if payload
        .message_id
        .as_deref()
        .and_then(|message_id| existing_text_message(&session_id, message_id))
        .is_some()
    {
        None
    } else {
        session_store().add_message_with_ids(
            &session_id,
            SessionMessageRole::Assistant,
            content,
            payload.message_id.clone(),
            payload.part_id.clone(),
            agent_message_metadata(&payload),
        )
    };
    let tool_message = payload.tool_call.as_ref().and_then(|tool_call| {
        if let Some(todos) = planning_todos(tool_call) {
            session_store().set_todos(&session_id, todos);
        }
        session_store().add_tool_message_with_message_id(
            &session_id,
            tool_call.tool_name.clone(),
            tool_call.call_id.clone(),
            tool_call.state.clone(),
            tool_call.metadata.clone(),
            payload.message_id.clone(),
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

fn existing_text_message(session_id: &str, message_id: &str) -> Option<crate::session::Message> {
    session_store()
        .get_messages(session_id)
        .into_iter()
        .find(|message| {
            message.id == message_id
                && message.role == SessionMessageRole::Assistant
                && message.parts.iter().any(|part| {
                    part.part_type == "text"
                        && part
                            .text
                            .as_deref()
                            .or(part.content.as_deref())
                            .is_some_and(|text| !text.trim().is_empty())
                })
        })
}

pub async fn stream_agent_message(
    Path(session_id): Path<String>,
    Json(payload): Json<StreamAgentTextRequest>,
) -> Json<serde_json::Value> {
    if payload.delta.is_empty() {
        return Json(serde_json::json!({ "ok": true }));
    }
    // Transient streaming overlay only: emit the delta so the frontend renders
    // tokens live. The persisted message arrives later via `send_agent_message`
    // reusing the same ids, which replaces these deltas with the full reply.
    session_store().push_event(GlobalEvent::MessagePartDelta {
        properties: crate::api::types::MessagePartDeltaProperties {
            session_id: session_id.clone(),
            message_id: payload.message_id,
            part_id: payload.part_id,
            field: "text".to_string(),
            delta: payload.delta,
        },
    });
    Json(serde_json::json!({ "ok": true, "session_id": session_id }))
}

fn sync_auto_session_name_from_agent_tool_call(
    session_id: &str,
    tool_call: Option<&SendAgentToolCall>,
) {
    let Some(summary) = tool_call.and_then(auto_session_name_from_tool_call) else {
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

fn auto_session_name_from_tool_call(tool_call: &SendAgentToolCall) -> Option<String> {
    if tool_call.tool_name == "planning" {
        return last_task_summary_from_planning_tool_call(tool_call);
    }
    last_task_detail_from_tool_call(tool_call)
}

fn last_task_summary_from_planning_tool_call(tool_call: &SendAgentToolCall) -> Option<String> {
    let mut summaries = Vec::new();
    if let Some(output) = tool_call
        .metadata
        .as_ref()
        .and_then(|metadata| metadata.get("output"))
    {
        collect_string_field(output, "task_summary", &mut summaries);
    }
    if summaries.is_empty() {
        if let Some(output) = tool_call
            .state
            .get("metadata")
            .and_then(|metadata| metadata.get("output"))
            .or_else(|| tool_call.state.get("output"))
        {
            collect_string_field(output, "task_summary", &mut summaries);
        }
    }
    if summaries.is_empty() {
        collect_string_field(&tool_call.state, "task_summary", &mut summaries);
    }
    summaries.pop()
}

fn last_task_detail_from_tool_call(tool_call: &SendAgentToolCall) -> Option<String> {
    let mut details = Vec::new();
    if let Some(output) = tool_call
        .metadata
        .as_ref()
        .and_then(|metadata| metadata.get("output"))
    {
        collect_string_field(output, "task_detail", &mut details);
    }
    if details.is_empty() {
        if let Some(output) = tool_call
            .state
            .get("metadata")
            .and_then(|metadata| metadata.get("output"))
            .or_else(|| tool_call.state.get("output"))
        {
            collect_string_field(output, "task_detail", &mut details);
        }
    }
    if details.is_empty() {
        collect_string_field(&tool_call.state, "task_detail", &mut details);
    }
    details.pop()
}

fn collect_string_field(value: &serde_json::Value, field: &str, values: &mut Vec<String>) {
    match value {
        serde_json::Value::Object(object) => {
            if let Some(value) = object
                .get(field)
                .and_then(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
            {
                values.push(value.to_string());
            }
            for child in object.values() {
                collect_string_field(child, field, values);
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                collect_string_field(item, field, values);
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
    /// Stable id pair from the streamed assistant text so the persisted message
    /// reuses the same ids without dropping already-visible frontend text.
    pub message_id: Option<String>,
    pub part_id: Option<String>,
}

/// One incremental assistant text token streamed from the runtime, re-emitted by
/// the gateway as a `message.part.delta` so the frontend renders tokens live.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct StreamAgentTextRequest {
    pub message_id: String,
    pub part_id: String,
    pub delta: String,
    pub runtime_id: Option<String>,
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
        && runtime_output_text_from_tool_call(payload.tool_call.as_ref()).is_none()
    {
        return String::new();
    }

    let mut content = frontend_safe_reply_message(&payload.reply_message);
    if content.trim().is_empty() {
        content =
            runtime_output_text_from_tool_call(payload.tool_call.as_ref()).unwrap_or_default();
    }

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

fn runtime_output_text_from_tool_call(tool_call: Option<&SendAgentToolCall>) -> Option<String> {
    let tool_call = tool_call?;
    if !is_runtime_output_tool_call(tool_call) {
        return None;
    }

    runtime_output_candidate_values(tool_call)
        .into_iter()
        .find_map(visible_text_from_runtime_value)
}

fn is_runtime_output_tool_call(tool_call: &SendAgentToolCall) -> bool {
    if tool_call.tool_name == "runtime" {
        return true;
    }
    [
        tool_call.metadata.as_ref(),
        tool_call.state.get("metadata"),
        tool_call
            .state
            .get("metadata")
            .and_then(|metadata| metadata.get("metadata")),
    ]
    .into_iter()
    .flatten()
    .any(|value| {
        value.get("kind").and_then(serde_json::Value::as_str) == Some("mano_runtime_usage")
    })
}

fn runtime_output_candidate_values(tool_call: &SendAgentToolCall) -> Vec<serde_json::Value> {
    let mut values = Vec::new();
    for root in [
        tool_call.metadata.as_ref(),
        Some(&tool_call.state),
        tool_call.state.get("metadata"),
        tool_call
            .state
            .get("metadata")
            .and_then(|metadata| metadata.get("metadata")),
    ]
    .into_iter()
    .flatten()
    {
        for key in ["output", "response", "result", "final", "message"] {
            if let Some(value) = root.get(key) {
                values.push(value.clone());
            }
        }
    }
    values
}

fn visible_text_from_runtime_value(value: serde_json::Value) -> Option<String> {
    if let Some(text) = value.as_str() {
        return visible_text_from_runtime_string(text);
    }
    for key in [
        "reply_message",
        "output_text",
        "final_text",
        "message",
        "text",
        "content",
        "summary",
    ] {
        if let Some(text) = value.get(key).and_then(serde_json::Value::as_str) {
            if let Some(text) = visible_text_from_runtime_string(text) {
                return Some(text);
            }
        }
    }

    let normalized = tura_llm_rust::normalize_response_content(&value);
    let text = tura_llm_rust::extract_response_text(&normalized)?;
    visible_text_from_runtime_string(&tura_llm_rust::strip_thought_blocks(&text))
}

fn visible_text_from_runtime_string(text: &str) -> Option<String> {
    let text = frontend_safe_reply_message(&tura_llm_rust::strip_thought_blocks(text));
    (!text.trim().is_empty()).then_some(text)
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn request(reply_message: &str) -> SendAgentMessageRequest {
        SendAgentMessageRequest {
            reply_message: reply_message.to_string(),
            new_learning: String::new(),
            step_summary: None,
            media: Vec::new(),
            runtime_id: None,
            tool_call: None,
            message_id: None,
            part_id: None,
        }
    }

    fn planning_tool_call(status: &str, output: Option<serde_json::Value>) -> SendAgentToolCall {
        let mut state = json!({
            "status": status,
            "input": {
                "steps": [
                    { "step_goal": "Plan" },
                    { "task_instruction": "Build" },
                    { "step_goal": "Verify" }
                ]
            }
        });
        if let Some(output) = output {
            state["output"] = output;
        }
        SendAgentToolCall {
            tool_name: "planning".to_string(),
            call_id: "plan-call".to_string(),
            state,
            metadata: None,
        }
    }

    #[test]
    fn agent_message_content_omits_empty_tool_only_messages() {
        let mut payload = request("   ");
        payload.tool_call = Some(SendAgentToolCall {
            tool_name: "command_run".to_string(),
            call_id: "call-1".to_string(),
            state: json!({}),
            metadata: None,
        });

        assert_eq!(agent_message_content(&payload), "");

        payload.media.push(SendAgentMedia {
            path: "image.png".to_string(),
            media_type: Some("image".to_string()),
        });
        assert_eq!(agent_message_content(&payload), "[MEDIA:image.png:MEDIA]\n");
    }

    #[test]
    fn agent_message_content_sanitizes_raw_tool_payload_and_appends_media_markers() {
        let mut payload = request(
            r#"{"output":{"reply_message":"Visible reply","tool_called_input":{"secret":true}}}"#,
        );
        payload.media = vec![
            SendAgentMedia {
                path: "media/a.png".to_string(),
                media_type: Some("image".to_string()),
            },
            SendAgentMedia {
                path: "docs/file.pdf".to_string(),
                media_type: Some("pdf".to_string()),
            },
        ];

        let content = agent_message_content(&payload);

        assert!(content.starts_with("Visible reply"));
        assert!(content.contains("[MEDIA:media/a.png:MEDIA]"));
        assert!(content.contains("[MEDIA:docs/file.pdf:MEDIA]"));
        assert!(!content.contains("tool_called_input"));
    }

    #[test]
    fn agent_message_metadata_is_present_only_for_nonempty_step_summary() {
        let payload = request("reply");
        assert_eq!(agent_message_metadata(&payload), None);

        let mut payload = request("reply");
        payload.step_summary = Some("  Finished setup  ".to_string());
        assert_eq!(
            agent_message_metadata(&payload),
            Some(json!({ "step_summary": "Finished setup" }))
        );

        payload.step_summary = Some("   ".to_string());
        assert_eq!(agent_message_metadata(&payload), None);
    }

    #[test]
    fn planning_todos_requires_planning_tool_and_nonempty_steps() {
        let non_planning = SendAgentToolCall {
            tool_name: "command_run".to_string(),
            call_id: "call".to_string(),
            state: json!({ "input": { "steps": [{ "step_goal": "Do" }] } }),
            metadata: None,
        };
        assert!(planning_todos(&non_planning).is_none());

        let empty_steps = SendAgentToolCall {
            tool_name: "planning".to_string(),
            call_id: "call".to_string(),
            state: json!({ "input": { "steps": [] } }),
            metadata: None,
        };
        assert!(planning_todos(&empty_steps).is_none());
    }

    #[test]
    fn planning_todos_marks_running_first_unfinished_step() {
        let tool_call = planning_tool_call(
            "running",
            Some(json!({
                "steps": [
                    { "index": 1, "ok": true }
                ]
            })),
        );

        let todos = planning_todos(&tool_call).expect("todos");

        assert_eq!(todos.len(), 3);
        assert_eq!(todos[0]["id"], "plan-call:1");
        assert_eq!(todos[0]["content"], "Plan");
        assert_eq!(todos[0]["status"], "completed");
        assert_eq!(todos[1]["content"], "Build");
        assert_eq!(todos[1]["status"], "in_progress");
        assert_eq!(todos[2]["status"], "pending");
    }

    #[test]
    fn planning_todos_marks_failed_output_step_cancelled_and_completed_turn_missing_steps_cancelled(
    ) {
        let tool_call = planning_tool_call(
            "completed",
            Some(json!({
                "results": [
                    "ignored",
                    {
                        "steps": [
                            { "index": 1, "ok": false },
                            { "index": 2, "ok": true }
                        ]
                    }
                ]
            })),
        );

        let todos = planning_todos(&tool_call).expect("todos");

        assert_eq!(todos[0]["status"], "cancelled");
        assert_eq!(todos[1]["status"], "completed");
        assert_eq!(todos[2]["status"], "cancelled");
    }

    #[test]
    fn planning_todos_reads_input_and_output_from_metadata_fallbacks() {
        let tool_call = SendAgentToolCall {
            tool_name: "planning".to_string(),
            call_id: "metadata-plan".to_string(),
            state: json!({
                "status": "pending"
            }),
            metadata: Some(json!({
                "input": {
                    "steps": [
                        { "task_instruction": "Metadata step" }
                    ]
                },
                "output": "{\"steps\":[{\"index\":1,\"ok\":true}]}"
            })),
        };

        let todos = planning_todos(&tool_call).expect("metadata todos");

        assert_eq!(todos.len(), 1);
        assert_eq!(todos[0]["id"], "metadata-plan:1");
        assert_eq!(todos[0]["content"], "Metadata step");
        assert_eq!(todos[0]["status"], "completed");
    }

    #[test]
    fn planning_output_steps_accepts_object_string_and_ignores_invalid_shapes() {
        let object_call = planning_tool_call(
            "completed",
            Some(json!({ "steps": [{ "index": 1, "ok": true }] })),
        );
        assert_eq!(planning_output_steps(&object_call).len(), 1);

        let string_call = planning_tool_call(
            "completed",
            Some(json!("{\"steps\":[{\"index\":2,\"ok\":false}]}")),
        );
        let steps = planning_output_steps(&string_call);
        assert_eq!(steps.len(), 1);
        assert_eq!(steps[0]["index"], 2);

        let invalid_call = planning_tool_call("completed", Some(json!("not-json")));
        assert!(planning_output_steps(&invalid_call).is_empty());
    }

    #[test]
    fn parse_json_value_only_accepts_objects_and_json_strings() {
        assert_eq!(
            parse_json_value(&json!("{\"ok\":true}")),
            Some(json!({ "ok": true }))
        );
        assert_eq!(
            parse_json_value(&json!({ "ok": true })),
            Some(json!({ "ok": true }))
        );
        assert_eq!(parse_json_value(&json!("not-json")), None);
        assert_eq!(parse_json_value(&json!([1, 2, 3])), None);
        assert_eq!(parse_json_value(&json!(false)), None);
    }

    #[test]
    fn todo_content_prefers_step_goal_then_task_instruction_then_number() {
        assert_eq!(todo_content(&json!({ "step_goal": "  Goal  " }), 1), "Goal");
        assert_eq!(
            todo_content(&json!({ "task_instruction": "  Do work  " }), 2),
            "Do work"
        );
        assert_eq!(todo_content(&json!({ "step_goal": " " }), 3), "Step 3");
        assert_eq!(todo_content(&json!({}), 4), "Step 4");
    }

    #[test]
    fn last_task_detail_prefers_metadata_output_then_state_metadata_then_state() {
        let call = SendAgentToolCall {
            tool_name: "command_run".to_string(),
            call_id: "call".to_string(),
            state: json!({
                "task_detail": "state detail",
                "metadata": {
                    "output": {
                        "task_detail": "state metadata detail"
                    }
                }
            }),
            metadata: Some(json!({
                "output": {
                    "items": [
                        { "task_detail": "first metadata detail" },
                        { "nested": { "task_detail": "last metadata detail" } }
                    ]
                }
            })),
        };

        assert_eq!(
            last_task_detail_from_tool_call(&call).as_deref(),
            Some("last metadata detail")
        );

        let call = SendAgentToolCall {
            metadata: None,
            ..call
        };
        assert_eq!(
            last_task_detail_from_tool_call(&call).as_deref(),
            Some("state metadata detail")
        );

        let detail_only = SendAgentToolCall {
            tool_name: "task_status".to_string(),
            call_id: "call".to_string(),
            state: json!({
                "task_summary": "must not be used",
                "metadata": {
                    "output": {
                        "task_summary": "must not be used either"
                    }
                }
            }),
            metadata: Some(json!({
                "output": {
                    "task_summary": "still not a task detail"
                }
            })),
        };
        assert_eq!(last_task_detail_from_tool_call(&detail_only), None);
    }

    #[test]
    fn planning_auto_session_name_uses_task_summary_only() {
        let call = SendAgentToolCall {
            tool_name: "planning".to_string(),
            call_id: "planning".to_string(),
            state: json!({
                "task_detail": "must not be used",
                "metadata": {
                    "output": {
                        "steps": [
                            { "task_summary": "First summary" },
                            { "task_detail": "must not be used either" },
                            { "task_summary": "Last summary" }
                        ]
                    }
                }
            }),
            metadata: None,
        };

        assert_eq!(
            auto_session_name_from_tool_call(&call).as_deref(),
            Some("Last summary")
        );
    }

    #[test]
    fn collect_string_field_walks_nested_arrays_and_objects_in_order() {
        let mut values = Vec::new();

        collect_string_field(
            &json!({
                "task_detail": "root",
                "items": [
                    { "task_detail": "child one" },
                    { "nested": { "task_detail": "child two" } },
                    { "task_detail": "", "task_summary": "must not fallback" },
                    { "task_detail": "   " }
                ]
            }),
            "task_detail",
            &mut values,
        );

        assert_eq!(values, vec!["root", "child one", "child two"]);
    }
}
