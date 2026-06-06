use super::*;

pub(crate) fn api_message_from_store(message: crate::session::store::Message) -> Message {
    Message {
        id: message.id,
        session_id: message.session_id,
        role: match message.role {
            SessionMessageRole::User => MessageRole::User,
            SessionMessageRole::Assistant => MessageRole::Assistant,
            SessionMessageRole::System => MessageRole::System,
        },
        parts: message
            .parts
            .into_iter()
            .map(|part| MessagePart {
                id: part.id.clone(),
                part_type: part.part_type.clone(),
                content: part.content.clone(),
                text: part.text.clone(),
                metadata: frontend_safe_part_value(&part, part.metadata.clone()),
                call_id: part.call_id.clone(),
                tool: part.tool.clone(),
                state: frontend_safe_part_value(&part, part.state.clone()),
            })
            .collect(),
        created_at: message.created_at,
        updated_at: message.updated_at,
        parent_id: message.parent_id,
    }
}

pub(super) fn message_with_parts_from_store(
    message: crate::session::store::Message,
) -> serde_json::Value {
    let session_id = message.session_id.clone();
    let message_id = message.id.clone();
    let parts: Vec<_> = message
        .parts
        .iter()
        .cloned()
        .map(|part| part_json(&session_id, &message_id, part))
        .collect();
    let mut info = serde_json::to_value(api_message_from_store(message))
        .unwrap_or_else(|_| serde_json::json!({}));
    if let Some(object) = info.as_object_mut() {
        object.insert("parts".to_string(), serde_json::Value::Array(parts.clone()));
    }
    serde_json::json!({
        "info": info,
        "parts": parts,
    })
}

pub(super) fn part_json(
    session_id: &str,
    message_id: &str,
    part: crate::session::store::MessagePart,
) -> serde_json::Value {
    serde_json::json!({
        "id": &part.id,
        "sessionID": session_id,
        "messageID": message_id,
        "type": &part.part_type,
        "text": part.text.clone().or(part.content.clone()).unwrap_or_default(),
        "metadata": frontend_safe_part_value(&part, part.metadata.clone()),
        "callID": &part.call_id,
        "tool": &part.tool,
        "state": frontend_safe_part_value(&part, part.state.clone()),
    })
}

pub(super) fn frontend_safe_value(value: Option<serde_json::Value>) -> Option<serde_json::Value> {
    value.map(sanitize_frontend_value)
}

fn frontend_safe_part_value(
    part: &crate::session::store::MessagePart,
    value: Option<serde_json::Value>,
) -> Option<serde_json::Value> {
    if part.part_type == "tool" && part.tool.as_deref() == Some("runtime") {
        return value;
    }
    frontend_safe_value(value)
}

fn sanitize_frontend_value(value: serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(object) => {
            let object = object
                .into_iter()
                .filter(|(key, _)| !matches!(key.as_str(), "new_learning" | "runtime_id"))
                .map(|(key, value)| (key, sanitize_frontend_value(value)))
                .collect();
            serde_json::Value::Object(object)
        }
        serde_json::Value::Array(items) => {
            serde_json::Value::Array(items.into_iter().map(sanitize_frontend_value).collect())
        }
        value => value,
    }
}
