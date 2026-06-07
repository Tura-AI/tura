use super::*;

impl SessionStore {
    pub fn get_messages(&self, session_id: &str) -> Vec<Message> {
        self.messages
            .read()
            .get(session_id)
            .cloned()
            .unwrap_or_default()
    }

    pub fn get_todos(&self, session_id: &str) -> Vec<serde_json::Value> {
        self.todos
            .read()
            .get(session_id)
            .cloned()
            .unwrap_or_default()
    }

    pub fn set_todos(
        &self,
        session_id: &str,
        todos: Vec<serde_json::Value>,
    ) -> Vec<serde_json::Value> {
        self.todos
            .write()
            .insert(session_id.to_string(), todos.clone());
        self.persist_session_background(session_id);
        self.push_event(GlobalEvent::TodoUpdated {
            properties: serde_json::json!({
                "sessionID": session_id,
                "todos": todos,
            }),
        });
        todos
    }

    pub fn finish_todos(&self, session_id: &str, success: bool) {
        let mut todos = self.get_todos(session_id);
        if todos.is_empty() {
            return;
        }

        for todo in &mut todos {
            let current = todo
                .get("status")
                .and_then(|value| value.as_str())
                .unwrap_or("pending");
            if matches!(current, "completed" | "cancelled") {
                continue;
            }
            let status = if success { "completed" } else { "cancelled" };
            if let Some(object) = todo.as_object_mut() {
                object.insert("status".to_string(), serde_json::json!(status));
            }
        }

        self.set_todos(session_id, todos);
    }

    pub fn add_message(
        &self,
        session_id: &str,
        role: MessageRole,
        content: String,
    ) -> Option<Message> {
        self.add_message_with_metadata(session_id, role, content, None)
    }

    pub fn add_message_with_ids(
        &self,
        session_id: &str,
        role: MessageRole,
        content: String,
        message_id: Option<String>,
        part_id: Option<String>,
        metadata: Option<serde_json::Value>,
    ) -> Option<Message> {
        self.add_message_internal(session_id, role, content, metadata, message_id, part_id)
    }

    pub fn add_message_with_metadata(
        &self,
        session_id: &str,
        role: MessageRole,
        content: String,
        metadata: Option<serde_json::Value>,
    ) -> Option<Message> {
        self.add_message_internal(session_id, role, content, metadata, None, None)
    }

    fn add_message_internal(
        &self,
        session_id: &str,
        role: MessageRole,
        content: String,
        metadata: Option<serde_json::Value>,
        message_id: Option<String>,
        part_id: Option<String>,
    ) -> Option<Message> {
        let now = Utc::now().timestamp_millis();

        let parent_id = if role == MessageRole::Assistant {
            self.messages.read().get(session_id).and_then(|messages| {
                messages
                    .iter()
                    .rev()
                    .find(|message| message.role == MessageRole::User)
                    .map(|message| message.id.clone())
            })
        } else {
            None
        };

        let message = Message {
            id: message_id.unwrap_or_else(|| new_message_id(now)),
            session_id: session_id.to_string(),
            role,
            parent_id,
            parts: vec![MessagePart {
                id: part_id.unwrap_or_else(|| Uuid::new_v4().to_string()),
                part_type: "text".to_string(),
                content: Some(content.clone()),
                text: Some(content),
                metadata,
                call_id: None,
                tool: None,
                state: None,
            }],
            created_at: now,
            updated_at: now,
        };

        let mut messages = self.messages.write();
        let session_messages = messages.entry(session_id.to_string()).or_default();
        session_messages.push(message.clone());

        if let Some(info) = self.sessions.write().get_mut(session_id) {
            info.message_count = session_messages.len();
            info.updated_at = now;
            if role == MessageRole::User {
                if let Some(text) = message.parts.first().and_then(|part| part.text.clone()) {
                    if info.management.input.user_input.trim().is_empty() {
                        info.management.input.user_input = text.clone();
                    }
                    info.management
                        .session_log
                        .push(format!("user_input: {text}"));
                }
            }
        }
        drop(messages);
        self.persist_session_background(session_id);

        let event_message = message.clone();
        let event_parts = event_message.parts.clone();
        self.push_event(GlobalEvent::MessageUpdated {
            properties: crate::api::types::MessageUpdatedProperties {
                session_id: session_id.to_string(),
                info: crate::api::types::Message {
                    id: event_message.id,
                    session_id: event_message.session_id,
                    role: match event_message.role {
                        MessageRole::User => crate::api::types::MessageRole::User,
                        MessageRole::Assistant => crate::api::types::MessageRole::Assistant,
                        MessageRole::System => crate::api::types::MessageRole::System,
                    },
                    parts: event_message
                        .parts
                        .into_iter()
                        .map(|part| crate::api::types::MessagePart {
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
                    created_at: event_message.created_at,
                    updated_at: event_message.updated_at,
                    parent_id: event_message.parent_id,
                },
            },
        });
        for part in event_parts {
            self.push_event(GlobalEvent::MessagePartUpdated {
                properties: crate::api::types::MessagePartUpdatedProperties {
                    session_id: session_id.to_string(),
                    part: serde_json::json!({
                        "id": part.id.clone(),
                        "sessionID": session_id,
                        "messageID": message.id,
                        "type": part.part_type.clone(),
                        "text": part.text.clone().or(part.content.clone()).unwrap_or_default(),
                        "metadata": frontend_safe_part_value(&part, part.metadata.clone()),
                        "callID": part.call_id.clone(),
                        "tool": part.tool.clone(),
                        "state": frontend_safe_part_value(&part, part.state.clone()),
                    }),
                },
            });
        }

        Some(message)
    }

    pub fn add_tool_message(
        &self,
        session_id: &str,
        tool_name: String,
        call_id: String,
        state: serde_json::Value,
        metadata: Option<serde_json::Value>,
    ) -> Option<Message> {
        let now = Utc::now().timestamp_millis();
        let (state, metadata) = normalize_tool_message_state(&tool_name, state, metadata);

        let parent_id = self.messages.read().get(session_id).and_then(|messages| {
            messages
                .iter()
                .rev()
                .find(|message| message.role == MessageRole::User)
                .map(|message| message.id.clone())
        });

        {
            let mut messages = self.messages.write();
            let session_messages = messages.entry(session_id.to_string()).or_default();
            if let Some(message) = session_messages.iter_mut().find(|message| {
                message.parts.iter().any(|part| {
                    part.part_type == "tool"
                        && part.call_id.as_deref() == Some(call_id.as_str())
                        && part.tool.as_deref() == Some(tool_name.as_str())
                })
            }) {
                message.updated_at = now;
                if let Some(part) = message.parts.iter_mut().find(|part| {
                    part.part_type == "tool"
                        && part.call_id.as_deref() == Some(call_id.as_str())
                        && part.tool.as_deref() == Some(tool_name.as_str())
                }) {
                    part.state = Some(state);
                    part.metadata = metadata;
                    let part = part.clone();
                    let message_id = message.id.clone();
                    let message = message.clone();
                    if let Some(info) = self.sessions.write().get_mut(session_id) {
                        info.updated_at = now;
                    }
                    drop(messages);
                    self.persist_session(session_id);
                    self.push_event(GlobalEvent::MessagePartUpdated {
                        properties: crate::api::types::MessagePartUpdatedProperties {
                            session_id: session_id.to_string(),
                            part: serde_json::json!({
                                "id": &part.id,
                                "sessionID": session_id,
                                "messageID": message_id,
                                "type": &part.part_type,
                                "callID": &part.call_id,
                                "tool": &part.tool,
                                "state": frontend_safe_part_value(&part, part.state.clone()),
                                "metadata": frontend_safe_part_value(&part, part.metadata.clone()),
                            }),
                        },
                    });
                    return Some(message);
                }
            }
        }

        let part = MessagePart {
            id: Uuid::new_v4().to_string(),
            part_type: "tool".to_string(),
            content: None,
            text: None,
            metadata,
            call_id: Some(call_id),
            tool: Some(tool_name),
            state: Some(state),
        };

        let message = Message {
            id: new_message_id(now),
            session_id: session_id.to_string(),
            role: MessageRole::Assistant,
            parent_id,
            parts: vec![part.clone()],
            created_at: now,
            updated_at: now,
        };

        let mut messages = self.messages.write();
        let session_messages = messages.entry(session_id.to_string()).or_default();
        session_messages.push(message.clone());

        if let Some(info) = self.sessions.write().get_mut(session_id) {
            info.message_count = session_messages.len();
            info.updated_at = now;
        }
        drop(messages);
        self.persist_session(session_id);

        self.push_event(GlobalEvent::MessageUpdated {
            properties: crate::api::types::MessageUpdatedProperties {
                session_id: session_id.to_string(),
                info: crate::api::types::Message {
                    id: message.id.clone(),
                    session_id: message.session_id.clone(),
                    role: crate::api::types::MessageRole::Assistant,
                    parts: vec![crate::api::types::MessagePart {
                        id: part.id.clone(),
                        part_type: part.part_type.clone(),
                        content: part.content.clone(),
                        text: part.text.clone(),
                        metadata: frontend_safe_part_value(&part, part.metadata.clone()),
                        call_id: part.call_id.clone(),
                        tool: part.tool.clone(),
                        state: frontend_safe_part_value(&part, part.state.clone()),
                    }],
                    created_at: message.created_at,
                    updated_at: message.updated_at,
                    parent_id: message.parent_id.clone(),
                },
            },
        });

        self.push_event(GlobalEvent::MessagePartUpdated {
            properties: crate::api::types::MessagePartUpdatedProperties {
                session_id: session_id.to_string(),
                part: serde_json::json!({
                    "id": &part.id,
                    "sessionID": session_id,
                    "messageID": &message.id,
                    "type": &part.part_type,
                    "callID": &part.call_id,
                    "tool": &part.tool,
                    "state": frontend_safe_part_value(&part, part.state.clone()),
                    "metadata": frontend_safe_part_value(&part, part.metadata.clone()),
                }),
            },
        });

        Some(message)
    }
}
