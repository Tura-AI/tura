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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frontend_safe_value_removes_internal_keys_recursively() {
        let value = serde_json::json!({
            "visible": true,
            "runtime_id": "runtime-secret",
            "nested": {
                "new_learning": "private",
                "keep": "public"
            },
            "items": [
                { "runtime_id": "nested-secret", "text": "shown" },
                "literal"
            ]
        });

        let sanitized = frontend_safe_value(Some(value)).expect("sanitized value");

        assert_eq!(
            sanitized,
            serde_json::json!({
                "visible": true,
                "nested": {
                    "keep": "public"
                },
                "items": [
                    { "text": "shown" },
                    "literal"
                ]
            })
        );
    }

    #[test]
    fn runtime_tool_parts_keep_runtime_metadata_and_state() {
        let part = store_part(
            "tool",
            Some("runtime"),
            Some("content fallback"),
            None,
            Some(serde_json::json!({
                "runtime_id": "runtime-1",
                "new_learning": "allowed for runtime event",
                "visible": "metadata"
            })),
            Some(serde_json::json!({
                "runtime_id": "runtime-1",
                "state": "Running"
            })),
        );

        let value = part_json("session-1", "message-1", part);

        assert_eq!(value["text"], "content fallback");
        assert_eq!(value["metadata"]["runtime_id"], "runtime-1");
        assert_eq!(
            value["metadata"]["new_learning"],
            "allowed for runtime event"
        );
        assert_eq!(value["state"]["runtime_id"], "runtime-1");
    }

    #[test]
    fn non_runtime_tool_parts_are_sanitized_and_prefer_text_over_content() {
        let part = store_part(
            "tool",
            Some("shell"),
            Some("content fallback"),
            Some("visible text"),
            Some(serde_json::json!({
                "runtime_id": "hidden",
                "new_learning": "hidden",
                "ok": true
            })),
            Some(serde_json::json!({
                "runtime_id": "hidden",
                "exit_code": 0
            })),
        );

        let value = part_json("session-1", "message-1", part);

        assert_eq!(value["sessionID"], "session-1");
        assert_eq!(value["messageID"], "message-1");
        assert_eq!(value["text"], "visible text");
        assert_eq!(value["metadata"], serde_json::json!({ "ok": true }));
        assert_eq!(value["state"], serde_json::json!({ "exit_code": 0 }));
    }

    #[test]
    fn api_message_from_store_maps_roles_parts_and_parent_id() {
        let message = crate::session::store::Message {
            id: "message-1".to_string(),
            session_id: "session-1".to_string(),
            role: crate::session::store::MessageRole::Assistant,
            parent_id: Some("parent-1".to_string()),
            parts: vec![store_part(
                "text",
                None,
                None,
                Some("hello"),
                Some(serde_json::json!({ "runtime_id": "hidden", "keep": "yes" })),
                None,
            )],
            created_at: 10,
            updated_at: 20,
        };

        let api = api_message_from_store(message);

        assert_eq!(api.id, "message-1");
        assert_eq!(api.session_id, "session-1");
        assert!(matches!(api.role, MessageRole::Assistant));
        assert_eq!(api.parent_id.as_deref(), Some("parent-1"));
        assert_eq!(api.created_at, 10);
        assert_eq!(api.updated_at, 20);
        assert_eq!(api.parts.len(), 1);
        assert_eq!(api.parts[0].text.as_deref(), Some("hello"));
        assert_eq!(
            api.parts[0].metadata,
            Some(serde_json::json!({ "keep": "yes" }))
        );
    }

    #[test]
    fn message_with_parts_keeps_info_and_flat_parts_in_sync() {
        let message = crate::session::store::Message {
            id: "message-2".to_string(),
            session_id: "session-2".to_string(),
            role: crate::session::store::MessageRole::User,
            parent_id: None,
            parts: vec![
                store_part("text", None, None, Some("first"), None, None),
                store_part("text", None, Some("second content"), None, None, None),
            ],
            created_at: 11,
            updated_at: 12,
        };

        let value = message_with_parts_from_store(message);

        assert_eq!(value["info"]["id"], "message-2");
        assert_eq!(value["info"]["sessionID"], "session-2");
        assert_eq!(value["parts"].as_array().expect("parts").len(), 2);
        assert_eq!(value["info"]["parts"], value["parts"]);
        assert_eq!(value["parts"][0]["text"], "first");
        assert_eq!(value["parts"][1]["text"], "second content");
    }

    fn store_part(
        part_type: &str,
        tool: Option<&str>,
        content: Option<&str>,
        text: Option<&str>,
        metadata: Option<serde_json::Value>,
        state: Option<serde_json::Value>,
    ) -> crate::session::store::MessagePart {
        crate::session::store::MessagePart {
            id: format!("part-{part_type}-{}", tool.unwrap_or("none")),
            part_type: part_type.to_string(),
            content: content.map(ToString::to_string),
            text: text.map(ToString::to_string),
            metadata,
            call_id: Some("call-1".to_string()),
            tool: tool.map(ToString::to_string),
            state,
        }
    }
}
