use super::*;
use runtime::state_machine::runtime_management::RuntimeSessionSyncStatus;
pub async fn list_messages(
    Path(session_id): Path<String>,
    Query(params): Query<MessageListParams>,
) -> Json<Vec<Message>> {
    let messages = page_messages(session_store().get_frontend_messages(&session_id), &params);
    let api_messages: Vec<Message> = messages
        .into_iter()
        .map(message_with_parts_from_store)
        .collect();
    Json(api_messages)
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
    Json(send_agent_message_payload(session_id, payload))
}

pub fn send_agent_message_payload(
    session_id: String,
    payload: SendAgentMessageRequest,
) -> SendAgentMessageResponse {
    let content = agent_message_content(&payload);
    if is_progress_only_agent_message(&content, &payload) {
        return SendAgentMessageResponse {
            ok: true,
            session_id,
            message_id: payload.runtime_id.as_deref().map(runtime_message_id),
            event: None,
            error: None,
        };
    }
    if let Some(response) = runtime_managed_message_response(&session_id, &payload, &content) {
        sync_auto_session_name_from_agent_tool_call(&session_id, payload.tool_call.as_ref());
        return response;
    }
    if content.trim().is_empty() {
        if let Some(response) = transient_tool_message_response(&session_id, &payload) {
            sync_auto_session_name_from_agent_tool_call(&session_id, payload.tool_call.as_ref());
            return response;
        }
    }

    let message = if content.trim().is_empty() {
        None
    } else {
        session_store().add_message_with_ids(
            &session_id,
            SessionMessageRole::Assistant,
            content,
            None,
            None,
            agent_message_metadata(&payload),
        )
    };
    let visible_tool_call = payload
        .tool_call
        .as_ref()
        .is_some_and(tool_call_visible_to_frontend);
    let persistent_tool_call = payload
        .tool_call
        .as_ref()
        .is_some_and(tool_call_persistent_to_store);
    let tool_message = payload.tool_call.as_ref().and_then(|tool_call| {
        if let Some(todos) = planning_todos(tool_call) {
            session_store().set_todos(&session_id, todos);
        }
        if !visible_tool_call || !persistent_tool_call {
            return None;
        }
        session_store().add_tool_message_with_message_id(
            &session_id,
            tool_call.tool_name.clone(),
            tool_call.call_id.clone(),
            tool_call.state.clone(),
            tool_call.metadata.clone(),
            None,
        )
    });
    sync_auto_session_name_from_agent_tool_call(&session_id, payload.tool_call.as_ref());

    match message.or(tool_message) {
        Some(message) => SendAgentMessageResponse {
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
        },
        None if payload.tool_call.is_some() && (!visible_tool_call || !persistent_tool_call) => {
            SendAgentMessageResponse {
                ok: true,
                session_id,
                message_id: None,
                event: None,
                error: None,
            }
        }
        None => SendAgentMessageResponse {
            ok: false,
            session_id,
            message_id: None,
            event: None,
            error: Some("failed to store agent message".to_string()),
        },
    }
}

fn is_progress_only_agent_message(content: &str, payload: &SendAgentMessageRequest) -> bool {
    payload.tool_call.is_none()
        && payload.media.is_empty()
        && content.trim_start().starts_with("Step summary:")
}

fn tool_call_persistent_to_store(tool_call: &SendAgentToolCall) -> bool {
    tool_call.tool_name != "command_run" && !is_transient_tool_call(tool_call)
}

fn tool_call_visible_to_frontend(tool_call: &SendAgentToolCall) -> bool {
    tool_call.tool_name != "command_run"
        || !task_status_payload(&tool_call.state)
            && !tool_call.metadata.as_ref().is_some_and(task_status_payload)
}

fn task_status_payload(value: &serde_json::Value) -> bool {
    match value {
        serde_json::Value::Object(object) => {
            object.contains_key("task_status")
                || object
                    .get("command_type")
                    .and_then(serde_json::Value::as_str)
                    .is_some_and(|value| value.eq_ignore_ascii_case("task_status"))
                || object.values().any(task_status_payload)
        }
        serde_json::Value::Array(items) => items.iter().any(task_status_payload),
        serde_json::Value::String(text) => serde_json::from_str::<serde_json::Value>(text)
            .ok()
            .is_some_and(|value| task_status_payload(&value)),
        _ => false,
    }
}

fn transient_tool_message_response(
    session_id: &str,
    payload: &SendAgentMessageRequest,
) -> Option<SendAgentMessageResponse> {
    let tool_call = payload.tool_call.as_ref()?;
    if !is_transient_tool_call(tool_call) {
        return None;
    }

    Some(SendAgentMessageResponse {
        ok: true,
        session_id: session_id.to_string(),
        message_id: None,
        event: None,
        error: None,
    })
}

fn runtime_managed_message_response(
    session_id: &str,
    payload: &SendAgentMessageRequest,
    content: &str,
) -> Option<SendAgentMessageResponse> {
    let status = payload.runtime_status.as_ref()?;
    let runtime_message_id = runtime_message_id(&status.runtime_id);
    if status.should_refresh_session_db() {
        if let Some(usage) = runtime_usage_from_payload(payload) {
            if session_store().update_session_runtime_usage(session_id, usage) {
                session_store().push_current_session_status_event(session_id);
            }
        }
        let final_message = runtime_final_message(session_id, payload, content, status);
        let messages = session_store().finalize_runtime_live_messages(
            session_id,
            &status.runtime_id,
            final_message,
        );
        let event = messages
            .last()
            .cloned()
            .map(|message| GlobalEvent::MessageUpdated {
                properties: MessageUpdatedProperties {
                    session_id: session_id.to_string(),
                    info: api_message_from_store(message),
                },
            });
        return Some(SendAgentMessageResponse {
            ok: true,
            session_id: session_id.to_string(),
            message_id: Some(runtime_message_id),
            event,
            error: None,
        });
    }

    if !status.live_overlay_active() {
        return Some(SendAgentMessageResponse {
            ok: true,
            session_id: session_id.to_string(),
            message_id: Some(runtime_message_id),
            event: None,
            error: None,
        });
    }

    if content.trim().is_empty() {
        return runtime_live_tool_message_response(session_id, payload, status);
    }

    let (created_at, updated_at) = runtime_message_times(payload);
    let message = session_store().build_text_message_with_ids_and_times(
        session_id,
        SessionMessageRole::Assistant,
        content.to_string(),
        Some(runtime_message_id),
        Some(runtime_text_part_id(&status.runtime_id)),
        agent_message_metadata(payload),
        created_at,
        updated_at,
    );
    let message_id = message.id.clone();
    let event =
        session_store().upsert_live_message(session_id, Some(status.runtime_id.clone()), message);
    Some(SendAgentMessageResponse {
        ok: true,
        session_id: session_id.to_string(),
        message_id: Some(message_id),
        event: Some(event),
        error: None,
    })
}

fn runtime_final_message(
    session_id: &str,
    payload: &SendAgentMessageRequest,
    content: &str,
    status: &RuntimeSessionSyncStatus,
) -> Option<crate::session::Message> {
    let (created_at, updated_at) = runtime_message_times(payload);
    if !content.trim().is_empty() {
        return Some(session_store().build_text_message_with_ids_and_times(
            session_id,
            SessionMessageRole::Assistant,
            content.to_string(),
            Some(runtime_message_id(&status.runtime_id)),
            Some(runtime_text_part_id(&status.runtime_id)),
            agent_message_metadata(payload),
            created_at,
            updated_at,
        ));
    }

    let tool_call = payload.tool_call.as_ref()?;
    if !tool_call_visible_to_frontend(tool_call) {
        return None;
    }
    Some(
        session_store().build_transient_tool_message_with_ids_and_times(
            session_id,
            tool_call.tool_name.clone(),
            tool_call.call_id.clone(),
            tool_call.state.clone(),
            tool_call.metadata.clone(),
            runtime_message_id(&status.runtime_id),
            runtime_tool_part_id(&status.runtime_id, &tool_call.tool_name),
            created_at,
            updated_at,
        ),
    )
}

fn runtime_live_tool_message_response(
    session_id: &str,
    payload: &SendAgentMessageRequest,
    status: &RuntimeSessionSyncStatus,
) -> Option<SendAgentMessageResponse> {
    let tool_call = payload.tool_call.as_ref()?;
    if !tool_call_visible_to_frontend(tool_call) {
        return Some(SendAgentMessageResponse {
            ok: true,
            session_id: session_id.to_string(),
            message_id: Some(runtime_message_id(&status.runtime_id)),
            event: None,
            error: None,
        });
    }

    let message_id = runtime_message_id(&status.runtime_id);
    let part_id = runtime_tool_part_id(&status.runtime_id, &tool_call.tool_name);
    let (created_at, updated_at) = runtime_message_times(payload);
    let message = session_store().build_transient_tool_message_with_ids_and_times(
        session_id,
        tool_call.tool_name.clone(),
        tool_call.call_id.clone(),
        tool_call.state.clone(),
        tool_call.metadata.clone(),
        message_id.clone(),
        part_id,
        created_at,
        updated_at,
    );
    let event =
        session_store().upsert_live_message(session_id, Some(status.runtime_id.clone()), message);
    Some(SendAgentMessageResponse {
        ok: true,
        session_id: session_id.to_string(),
        message_id: Some(message_id),
        event: Some(event),
        error: None,
    })
}

fn runtime_message_times(payload: &SendAgentMessageRequest) -> (i64, i64) {
    let now = chrono::Utc::now().timestamp_millis();
    let created_at = payload.created_at.unwrap_or(now);
    let updated_at = payload.updated_at.unwrap_or(created_at);
    (created_at, updated_at)
}

fn runtime_message_id(runtime_id: &str) -> String {
    format!("{runtime_id}.message")
}

fn runtime_text_part_id(runtime_id: &str) -> String {
    format!("{runtime_id}.message")
}

fn runtime_tool_part_id(runtime_id: &str, tool_name: &str) -> String {
    format!("{runtime_id}.tool.{tool_name}")
}

fn is_transient_tool_call(tool_call: &SendAgentToolCall) -> bool {
    tool_call.tool_name == "command_run"
        || bool_field(&tool_call.state, "transient")
        || tool_call
            .metadata
            .as_ref()
            .is_some_and(|metadata| bool_field(metadata, "transient"))
        || tool_call
            .state
            .get("metadata")
            .is_some_and(|metadata| bool_field(metadata, "transient"))
}

fn bool_field(value: &serde_json::Value, key: &str) -> bool {
    value
        .get(key)
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false)
}

pub async fn stream_agent_message(
    Path(session_id): Path<String>,
    Json(payload): Json<StreamAgentTextRequest>,
) -> Json<serde_json::Value> {
    Json(stream_agent_message_payload(session_id, payload))
}

pub fn stream_agent_message_payload(
    session_id: String,
    payload: StreamAgentTextRequest,
) -> serde_json::Value {
    if payload.delta.is_empty() {
        return serde_json::json!({ "ok": true });
    }
    let message_id = runtime_message_id(&payload.runtime_id);
    let part_id = runtime_text_part_id(&payload.runtime_id);
    // Transient streaming overlay only: emit the delta so the frontend renders
    // tokens live. The persisted message arrives later via `send_agent_message`
    // reusing the same ids, which replaces these deltas with the full reply.
    session_store().push_event(GlobalEvent::MessagePartDelta {
        properties: crate::contracts::MessagePartDeltaProperties {
            session_id: session_id.clone(),
            message_id,
            part_id,
            field: "text".to_string(),
            delta: payload.delta,
        },
    });
    serde_json::json!({ "ok": true, "session_id": session_id })
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

fn runtime_usage_from_payload(payload: &SendAgentMessageRequest) -> Option<serde_json::Value> {
    let tool_call = payload.tool_call.as_ref()?;
    if !is_runtime_output_tool_call(tool_call) {
        return None;
    }
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
        if root.get("kind").and_then(serde_json::Value::as_str) == Some("mano_runtime_usage") {
            return root.get("usage").cloned().or(Some(serde_json::Value::Null));
        }
    }
    None
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

pub async fn get_message(Path((session_id, message_id)): Path<(String, String)>) -> Json<Message> {
    let messages = session_store().get_frontend_messages(&session_id);
    let message = messages
        .into_iter()
        .find(|m| m.id == message_id)
        .map(message_with_parts_from_store)
        .unwrap_or_else(|| Message {
            id: message_id,
            session_id,
            role: MessageRole::User,
            parts: Vec::new(),
            created_at: 0,
            updated_at: 0,
            parent_id: None,
        });
    Json(message)
}

pub async fn get_message_part(
    Path((session_id, message_id, part_id)): Path<(String, String, String)>,
) -> Json<MessagePart> {
    let messages = session_store().get_frontend_messages(&session_id);
    let message = messages.into_iter().find(|m| m.id == message_id);

    let part = message
        .and_then(|m| m.parts.into_iter().find(|p| p.id == part_id))
        .map(|p| part_json(&session_id, &message_id, p))
        .unwrap_or_else(|| MessagePart {
            id: part_id,
            session_id,
            message_id,
            part_type: "text".to_string(),
            content: None,
            text: Some(String::new()),
            metadata: None,
            call_id: None,
            tool: None,
            state: None,
        });
    Json(part)
}

// ============================================================================
// Session Commands
// ============================================================================

pub async fn session_command(
    Path(session_id): Path<String>,
    Json(payload): Json<SessionCommandRequest>,
) -> Json<SessionCommandResponse> {
    let directory = session_store()
        .get_session(&session_id)
        .and_then(|session| session.directory)
        .unwrap_or_else(|| ".".to_string());
    let output = run_session_shell_command(&directory, &payload.command)
        .unwrap_or_else(|error| format!("failed to run session command: {error}"));
    Json(SessionCommandResponse { output })
}

// ============================================================================
// Session Todo
// ============================================================================

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
            runtime_status: None,
            created_at: None,
            updated_at: None,
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
    fn terminal_runtime_usage_callback_updates_session_usage_and_emits_status_event() {
        let session = session_store().create_session(
            Some("C:/workspace".to_string()),
            None,
            None,
            Some("coding".to_string()),
            false,
            false,
            false,
            None,
            false,
            false,
        );
        let mut info = session_store()
            .get_session_info(&session.id)
            .expect("session info should exist");
        info.management.context_tokens =
            runtime::state_machine::session_management::ContextTokenStats {
                input: 42_000,
                limit: 128_000,
            };
        session_store().replace_management(&session.id, info.management);
        let mut cursor = session_store().event_cursor();
        let usage = json!({
            "input_tokens": 100,
            "output_tokens": 20,
            "total_tokens": 120,
            "total_cost": 0.012,
            "currency": "USD"
        });

        let response = send_agent_message_payload(
            session.id.clone(),
            SendAgentMessageRequest {
                reply_message: String::new(),
                new_learning: String::new(),
                step_summary: None,
                media: Vec::new(),
                runtime_id: Some("runtime-usage-event".to_string()),
                tool_call: Some(SendAgentToolCall {
                    tool_name: "runtime".to_string(),
                    call_id: "runtime-usage-event".to_string(),
                    state: json!({
                        "metadata": {
                            "kind": "mano_runtime_usage",
                            "usage": usage,
                            "output": {"reply_message": "done"}
                        }
                    }),
                    metadata: Some(json!({
                        "kind": "mano_runtime_usage",
                        "usage": usage,
                        "output": {"reply_message": "done"}
                    })),
                }),
                runtime_status: Some(
                    runtime::state_machine::runtime_management::RuntimeSessionSyncStatus {
                        runtime_id: "runtime-usage-event".to_string(),
                        state: runtime::state_machine::runtime_management::RuntimeState::Finished,
                        call_result_status: runtime::state_machine::runtime_management::RuntimeCallResultStatus::Succeeded,
                        live: false,
                        session_db_refresh_required: true,
                    },
                ),
                created_at: Some(1),
                updated_at: Some(2),
            },
        );

        assert!(response.ok);
        let event = session_store()
            .next_event(&mut cursor)
            .expect("usage status event should be published");
        match event {
            GlobalEvent::SessionStatus { properties } => {
                assert_eq!(properties.session_id, session.id);
                assert_eq!(properties.usage.context_tokens.input, 42_000);
                assert_eq!(properties.usage.context_tokens.limit, 128_000);
                assert_eq!(properties.usage.tokens["total_tokens"], 120);
                assert_eq!(properties.usage.cost, Some(0.012));
                assert_eq!(properties.usage.currency.as_deref(), Some("USD"));
            }
            other => panic!("unexpected event: {other:?}"),
        }
        let updated = session_store()
            .get_session(&session.id)
            .expect("session should exist");
        assert_eq!(updated.usage.tokens["total_tokens"], 120);
        assert_eq!(updated.usage.cost, Some(0.012));
    }

    #[test]
    fn step_summary_agent_messages_are_progress_only() {
        let mut payload = request("Step summary: inspect files");
        payload.runtime_id = Some("runtime-1".to_string());

        assert!(is_progress_only_agent_message(
            &agent_message_content(&payload),
            &payload
        ));

        payload.reply_message = "Final answer".to_string();
        assert!(!is_progress_only_agent_message(
            &agent_message_content(&payload),
            &payload
        ));
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
    fn command_run_tool_calls_are_transient_and_not_persistent_without_marker() {
        let tool_call = SendAgentToolCall {
            tool_name: "command_run".to_string(),
            call_id: "call-command-run".to_string(),
            state: json!({
                "status": "completed",
                "metadata": {
                    "kind": "mano_tool_call"
                }
            }),
            metadata: Some(json!({
                "kind": "mano_tool_call",
                "tool": "command_run"
            })),
        };

        assert!(is_transient_tool_call(&tool_call));
        assert!(!tool_call_persistent_to_store(&tool_call));
        assert!(tool_call_visible_to_frontend(&tool_call));
    }

    #[test]
    fn command_run_task_status_is_neither_visible_nor_persistent() {
        let tool_call = SendAgentToolCall {
            tool_name: "command_run".to_string(),
            call_id: "call-task-status".to_string(),
            state: json!({
                "status": "running",
                "input": {
                    "commands": [{
                        "command_type": "task_status",
                        "task_status": { "status": "working" }
                    }]
                }
            }),
            metadata: None,
        };

        assert!(is_transient_tool_call(&tool_call));
        assert!(!tool_call_visible_to_frontend(&tool_call));
        assert!(!tool_call_persistent_to_store(&tool_call));
    }

    #[test]
    fn explicit_transient_tool_calls_are_not_persistent() {
        let tool_call = SendAgentToolCall {
            tool_name: "grep".to_string(),
            call_id: "call-grep".to_string(),
            state: json!({
                "status": "running",
                "transient": true
            }),
            metadata: None,
        };

        assert!(is_transient_tool_call(&tool_call));
        assert!(!tool_call_persistent_to_store(&tool_call));
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
