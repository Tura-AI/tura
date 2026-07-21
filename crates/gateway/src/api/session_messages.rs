use super::*;
use lifecycle::SessionCommand;

pub async fn list_messages(
    Path(session_id): Path<String>,
    Query(params): Query<MessageListParams>,
) -> Json<Vec<Message>> {
    Json(list_messages_value(&session_id, &params))
}

pub fn list_messages_value(session_id: &str, params: &MessageListParams) -> Vec<Message> {
    page_messages(session_store().get_frontend_messages(session_id), params)
        .into_iter()
        .map(message_with_parts_from_store)
        .collect()
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
) -> impl IntoResponse {
    let user_message = session_store().build_message_with_parts(
        &session_id,
        SessionMessageRole::User,
        vec![crate::session::MessagePart {
            id: uuid::Uuid::new_v4().to_string(),
            part_type: "text".to_string(),
            content: Some(payload.content.clone()),
            text: Some(payload.content.clone()),
            metadata: None,
            call_id: None,
            tool: None,
            state: None,
        }],
        None,
        None,
    );
    if let Err(error) = session_store().execute_canonical_session_command_with_message(
        &session_id,
        SessionCommand::StartUserTurn,
        user_message,
    ) {
        return session_mutation_error(error);
    }
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

    if let Some(message) = final_agent_message(&session_id, before_count) {
        return Json(api_message_from_store(message)).into_response();
    }

    Json(Message {
        id: "error".to_string(),
        session_id,
        role: MessageRole::Assistant,
        parts: Vec::new(),
        created_at: 0,
        updated_at: 0,
        parent_id: None,
    })
    .into_response()
}

pub async fn get_message(Path((session_id, message_id)): Path<(String, String)>) -> Json<Message> {
    let message = session_store()
        .get_frontend_messages(&session_id)
        .into_iter()
        .find(|message| message.id == message_id)
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
    let message = session_store()
        .get_frontend_messages(&session_id)
        .into_iter()
        .find(|message| message.id == message_id);
    let part = message
        .and_then(|message| message.parts.into_iter().find(|part| part.id == part_id))
        .map(|part| part_json(&session_id, &message_id, part))
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

pub async fn get_todos(Path(session_id): Path<String>) -> Json<Vec<serde_json::Value>> {
    Json(session_store().get_todos(&session_id))
}
