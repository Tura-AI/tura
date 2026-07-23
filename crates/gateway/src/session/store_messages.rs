use super::*;

impl SessionStore {
    pub(crate) fn refresh_messages_from_session_db(&self, session_id: &str) -> Result<(), String> {
        const PAGE_SIZE: u64 = 1_000;

        let client = SessionDbClient::discover().map_err(|error| {
            format!("failed to discover session_db for message refresh: {error}")
        })?;
        let mut page = 0;
        let mut refreshed = Vec::new();
        loop {
            let (page_info, records) = client
                .list_session_records(session_id.to_string(), page, PAGE_SIZE)
                .map_err(|error| {
                    format!("failed to refresh messages for session {session_id}: {error}")
                })?;
            for record in records {
                let message: Message = serde_json::from_value(record.record).map_err(|error| {
                    format!("invalid message projection for session {session_id}: {error}")
                })?;
                let role = match message.role {
                    MessageRole::User => "user",
                    MessageRole::Assistant => "assistant",
                    MessageRole::System => "system",
                };
                if record.session_id != session_id
                    || message.id != record.message_id
                    || message.session_id != session_id
                    || role != record.role
                    || message.created_at != record.created_at
                    || message.updated_at != record.updated_at
                {
                    return Err(format!(
                        "Session message projection envelope does not match its payload for session {session_id}"
                    ));
                }
                refreshed.push(message);
            }
            if (page_info.page + 1).saturating_mul(page_info.page_size) >= page_info.total {
                break;
            }
            page = page.saturating_add(1);
        }
        if let Some(info) = self.sessions.write().get_mut(session_id) {
            info.message_count = refreshed.len();
        }
        self.messages
            .write()
            .insert(session_id.to_string(), refreshed);
        Ok(())
    }

    pub fn get_messages(&self, session_id: &str) -> Vec<Message> {
        self.messages
            .read()
            .get(session_id)
            .cloned()
            .unwrap_or_default()
    }

    pub fn get_frontend_messages(&self, session_id: &str) -> Vec<Message> {
        frontend_visible_messages(self.get_messages(session_id))
    }

    pub fn finalize_runtime_live_messages(
        &self,
        session_id: &str,
        runtime_id: &str,
        final_message: Option<Message>,
    ) -> Vec<Message> {
        let mut collected = Vec::new();
        {
            let mut live_messages = self.live_messages.write();
            let mut remove_session_entry = false;
            if let Some(overlays) = live_messages.get_mut(session_id) {
                let mut kept = Vec::new();
                for overlay in overlays.drain(..) {
                    if overlay.runtime_id.as_deref() == Some(runtime_id) {
                        collected.push(overlay.message);
                    } else {
                        kept.push(overlay);
                    }
                }
                if kept.is_empty() {
                    remove_session_entry = true;
                } else {
                    *overlays = kept;
                }
            }
            if remove_session_entry {
                live_messages.remove(session_id);
            }
        }

        if let Some(message) = final_message {
            collected.push(message);
        }

        let mut merged = Vec::<Message>::new();
        for message in collected {
            if let Some(existing) = merged.iter_mut().find(|item| item.id == message.id) {
                *existing = merge_message_parts(existing.clone(), message);
            } else {
                merged.push(message);
            }
        }

        if merged.is_empty() {
            return Vec::new();
        }

        {
            let mut messages = self.messages.write();
            let session_messages = messages.entry(session_id.to_string()).or_default();
            for message in &merged {
                if let Some(existing) = session_messages
                    .iter_mut()
                    .find(|candidate| candidate.id == message.id)
                {
                    *existing = merge_message_parts(existing.clone(), message.clone());
                } else {
                    session_messages.push(message.clone());
                }
            }
            if let Some(info) = self.sessions.write().get_mut(session_id) {
                info.message_count = session_messages.len();
                if let Some(updated_at) = merged.iter().map(|message| message.updated_at).max() {
                    info.updated_at = info.updated_at.max(updated_at);
                }
            }
        }

        for message in &merged {
            self.message_updated_event(message.clone());
        }
        merged
    }

    pub fn upsert_live_message(
        &self,
        session_id: &str,
        runtime_id: Option<String>,
        message: Message,
    ) -> GlobalEvent {
        let mut live_messages = self.live_messages.write();
        let overlays = live_messages.entry(session_id.to_string()).or_default();
        if let Some(existing) = overlays
            .iter_mut()
            .find(|overlay| overlay.message.id == message.id)
        {
            existing.runtime_id = runtime_id;
            existing.message = merge_message_parts(existing.message.clone(), message.clone());
        } else {
            overlays.push(LiveMessageOverlay {
                runtime_id,
                message: message.clone(),
            });
        }
        drop(live_messages);

        self.message_updated_event(message)
    }

    pub fn append_feed_text_delta(
        &self,
        session_id: &str,
        message_id: String,
        part_id: String,
        delta: String,
        created_at: i64,
        updated_at: i64,
    ) {
        let parent_id = self.latest_user_parent_id(session_id);
        let mut messages = self.messages.write();
        let session_messages = messages.entry(session_id.to_string()).or_default();
        if let Some(message) = session_messages
            .iter_mut()
            .find(|message| message.id == message_id)
        {
            if let Some(part) = message.parts.iter_mut().find(|part| part.id == part_id) {
                part.content
                    .get_or_insert_with(String::new)
                    .push_str(&delta);
                part.text.get_or_insert_with(String::new).push_str(&delta);
            } else {
                message.parts.push(MessagePart {
                    id: part_id.clone(),
                    part_type: "text".to_string(),
                    content: Some(delta.clone()),
                    text: Some(delta.clone()),
                    metadata: None,
                    call_id: None,
                    tool: None,
                    state: None,
                });
            }
            message.created_at = message.created_at.min(created_at);
            message.updated_at = message.updated_at.max(updated_at);
        } else {
            session_messages.push(Message {
                id: message_id.clone(),
                session_id: session_id.to_string(),
                role: MessageRole::Assistant,
                parent_id,
                parts: vec![MessagePart {
                    id: part_id.clone(),
                    part_type: "text".to_string(),
                    content: Some(delta.clone()),
                    text: Some(delta.clone()),
                    metadata: None,
                    call_id: None,
                    tool: None,
                    state: None,
                }],
                created_at,
                updated_at,
            });
        }
        if let Some(info) = self.sessions.write().get_mut(session_id) {
            info.message_count = session_messages.len();
            info.updated_at = info.updated_at.max(updated_at);
        }
        drop(messages);

        self.push_event(GlobalEvent::MessagePartDelta {
            properties: crate::contracts::MessagePartDeltaProperties {
                session_id: session_id.to_string(),
                message_id,
                part_id,
                created_at,
                updated_at,
                field: "text".to_string(),
                delta,
            },
        });
    }

    pub fn upsert_feed_message(&self, session_id: &str, message: Message) -> bool {
        let mut messages = self.messages.write();
        let session_messages = messages.entry(session_id.to_string()).or_default();
        let (projected, changed) = if let Some(existing) = session_messages
            .iter_mut()
            .find(|candidate| candidate.id == message.id)
        {
            let merged = merge_message_parts(existing.clone(), message);
            let changed = *existing != merged;
            *existing = merged;
            (existing.clone(), changed)
        } else {
            session_messages.push(message.clone());
            (message, true)
        };
        if let Some(info) = self.sessions.write().get_mut(session_id) {
            info.message_count = session_messages.len();
            info.updated_at = info.updated_at.max(projected.updated_at);
            if projected.role == MessageRole::User {
                info.last_user_message_at = Some(
                    info.last_user_message_at
                        .unwrap_or(projected.updated_at)
                        .max(projected.updated_at),
                );
            }
        }
        drop(messages);

        if changed {
            self.message_updated_event(projected);
        }
        changed
    }

    pub fn remove_live_messages_for_runtime(&self, session_id: &str, runtime_id: &str) {
        let mut live_messages = self.live_messages.write();
        if let Some(overlays) = live_messages.get_mut(session_id) {
            overlays.retain(|overlay| overlay.runtime_id.as_deref() != Some(runtime_id));
            if overlays.is_empty() {
                live_messages.remove(session_id);
            }
        }
    }

    pub fn get_todos(&self, session_id: &str) -> Vec<serde_json::Value> {
        self.todos
            .read()
            .get(session_id)
            .cloned()
            .unwrap_or_default()
    }

    #[cfg(any(feature = "business-tests", feature = "os-tests"))]
    pub fn todo_cursor_for_business_test(&self, session_id: &str) -> Option<u64> {
        self.todo_cursors.read().get(session_id).copied()
    }

    pub fn persist_todos(
        &self,
        session_id: &str,
        todos: Vec<serde_json::Value>,
    ) -> Result<Vec<serde_json::Value>, String> {
        let (todos, cursor) = SessionDbClient::discover()
            .and_then(|client| {
                client.update_session_todos(session_log_contract::UpdateSessionTodosRequest {
                    command_id: uuid::Uuid::new_v4().to_string(),
                    session_id: session_id.to_string(),
                    todos,
                    updated_at: Utc::now().timestamp_millis(),
                })
            })
            .map_err(|error| format!("failed to persist session todos: {error}"))?;
        Ok(self.apply_todos_projection(session_id, cursor, todos))
    }

    pub(crate) fn apply_todos_projection(
        &self,
        session_id: &str,
        cursor: u64,
        todos: Vec<serde_json::Value>,
    ) -> Vec<serde_json::Value> {
        self.apply_todos_projection_at_cursor(session_id, cursor, todos, true)
    }

    pub(crate) fn apply_todos_projection_at_cursor(
        &self,
        session_id: &str,
        cursor: u64,
        todos: Vec<serde_json::Value>,
        publish: bool,
    ) -> Vec<serde_json::Value> {
        let mut cursors = self.todo_cursors.write();
        if cursors
            .get(session_id)
            .is_some_and(|current| *current >= cursor)
        {
            return self.get_todos(session_id);
        }
        let changed = self
            .todos
            .write()
            .insert(session_id.to_string(), todos.clone())
            .as_ref()
            != Some(&todos);
        cursors.insert(session_id.to_string(), cursor);
        drop(cursors);
        if publish && changed {
            self.push_event(GlobalEvent::TodoUpdated {
                properties: serde_json::json!({
                    "sessionID": session_id,
                    "todos": todos,
                }),
            });
        }
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

        if let Err(error) = self.persist_todos(session_id, todos) {
            tracing::warn!(session_id, error, "failed to persist terminal todo state");
        }
    }

    fn message_updated_event(&self, message: Message) -> GlobalEvent {
        let session_id = message.session_id.clone();
        let event_message = message.clone();
        let event_parts = message.parts.clone();
        let event = GlobalEvent::MessageUpdated {
            properties: crate::contracts::MessageUpdatedProperties {
                session_id: session_id.clone(),
                info: crate::contracts::Message {
                    id: event_message.id.clone(),
                    session_id: event_message.session_id.clone(),
                    role: match event_message.role {
                        MessageRole::User => crate::contracts::MessageRole::User,
                        MessageRole::Assistant => crate::contracts::MessageRole::Assistant,
                        MessageRole::System => crate::contracts::MessageRole::System,
                    },
                    parts: event_message
                        .parts
                        .iter()
                        .map(|part| crate::contracts::MessagePart {
                            id: part.id.clone(),
                            session_id: event_message.session_id.clone(),
                            message_id: event_message.id.clone(),
                            part_type: part.part_type.clone(),
                            content: part.content.clone(),
                            text: part.text.clone(),
                            metadata: frontend_safe_part_value(part, part.metadata.clone()),
                            call_id: part.call_id.clone(),
                            tool: part.tool.clone(),
                            state: frontend_safe_part_state(part, part.state.clone()),
                        })
                        .collect(),
                    created_at: event_message.created_at,
                    updated_at: event_message.updated_at,
                    parent_id: event_message.parent_id.clone(),
                },
            },
        };
        self.push_event(event.clone());
        for part in event_parts {
            self.push_event(GlobalEvent::MessagePartUpdated {
                properties: crate::contracts::MessagePartUpdatedProperties {
                    session_id: session_id.clone(),
                    created_at: event_message.created_at,
                    updated_at: event_message.updated_at,
                    part: serde_json::json!({
                        "id": part.id.clone(),
                        "sessionID": session_id,
                        "messageID": message.id,
                        "type": part.part_type.clone(),
                        "text": part.text.clone().or(part.content.clone()).unwrap_or_default(),
                        "metadata": frontend_safe_part_value(&part, part.metadata.clone()),
                        "callID": part.call_id.clone(),
                        "tool": part.tool.clone(),
                        "state": frontend_safe_part_state(&part, part.state.clone()),
                    }),
                },
            });
        }
        event
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

    pub fn add_message_with_parts(
        &self,
        session_id: &str,
        role: MessageRole,
        parts: Vec<MessagePart>,
        message_id: Option<String>,
        metadata: Option<serde_json::Value>,
    ) -> Option<Message> {
        self.add_message_parts_internal(session_id, role, parts, metadata, message_id)
    }

    pub fn build_message_with_parts(
        &self,
        session_id: &str,
        role: MessageRole,
        parts: Vec<MessagePart>,
        message_id: Option<String>,
        metadata: Option<serde_json::Value>,
    ) -> Message {
        let now = Utc::now().timestamp_millis();
        Message {
            id: message_id.unwrap_or_else(|| new_message_id(now)),
            session_id: session_id.to_string(),
            role,
            parent_id: if role == MessageRole::Assistant {
                self.latest_user_parent_id(session_id)
            } else {
                None
            },
            parts: normalize_message_parts(parts, metadata),
            created_at: now,
            updated_at: now,
        }
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "runtime live overlay preserves ids and timestamps from one callback payload"
    )]
    pub fn build_text_message_with_ids_and_times(
        &self,
        session_id: &str,
        role: MessageRole,
        content: String,
        message_id: Option<String>,
        part_id: Option<String>,
        metadata: Option<serde_json::Value>,
        created_at: i64,
        updated_at: i64,
    ) -> Message {
        Message {
            id: message_id.unwrap_or_else(|| new_message_id(created_at)),
            session_id: session_id.to_string(),
            role,
            parent_id: if role == MessageRole::Assistant {
                self.latest_user_parent_id(session_id)
            } else {
                None
            },
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
            created_at,
            updated_at,
        }
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

    pub fn merge_message_metadata(
        &self,
        session_id: &str,
        message_id: &str,
        metadata: serde_json::Value,
    ) -> Option<Message> {
        let mut messages = self.messages.write();
        let message = messages
            .get_mut(session_id)?
            .iter_mut()
            .find(|message| message.id == message_id)?;
        for part in &mut message.parts {
            part.metadata = merge_part_metadata(part.metadata.take(), Some(metadata.clone()));
        }
        message.updated_at = Utc::now().timestamp_millis();
        Some(message.clone())
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
        let part = MessagePart {
            id: part_id.unwrap_or_else(|| Uuid::new_v4().to_string()),
            part_type: "text".to_string(),
            content: Some(content.clone()),
            text: Some(content),
            metadata: None,
            call_id: None,
            tool: None,
            state: None,
        };
        self.add_message_parts_internal(session_id, role, vec![part], metadata, message_id)
    }

    fn add_message_parts_internal(
        &self,
        session_id: &str,
        role: MessageRole,
        parts: Vec<MessagePart>,
        metadata: Option<serde_json::Value>,
        message_id: Option<String>,
    ) -> Option<Message> {
        let message = self.build_message_with_parts(session_id, role, parts, message_id, metadata);
        let now = message.updated_at;

        let mut messages = self.messages.write();
        let session_messages = messages.entry(session_id.to_string()).or_default();
        session_messages.push(message.clone());

        if let Some(info) = self.sessions.write().get_mut(session_id) {
            info.message_count = session_messages.len();
            info.updated_at = now;
            if role == MessageRole::User {
                info.last_user_message_at = Some(now);
            }
        }
        drop(messages);

        let event_message = message.clone();
        let event_parts = event_message.parts.clone();
        self.push_event(GlobalEvent::MessageUpdated {
            properties: crate::contracts::MessageUpdatedProperties {
                session_id: session_id.to_string(),
                info: crate::contracts::Message {
                    id: event_message.id.clone(),
                    session_id: event_message.session_id.clone(),
                    role: match event_message.role {
                        MessageRole::User => crate::contracts::MessageRole::User,
                        MessageRole::Assistant => crate::contracts::MessageRole::Assistant,
                        MessageRole::System => crate::contracts::MessageRole::System,
                    },
                    parts: event_message
                        .parts
                        .into_iter()
                        .map(|part| crate::contracts::MessagePart {
                            id: part.id.clone(),
                            session_id: event_message.session_id.clone(),
                            message_id: event_message.id.clone(),
                            part_type: part.part_type.clone(),
                            content: part.content.clone(),
                            text: part.text.clone(),
                            metadata: frontend_safe_part_value(&part, part.metadata.clone()),
                            call_id: part.call_id.clone(),
                            tool: part.tool.clone(),
                            state: frontend_safe_part_state(&part, part.state.clone()),
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
                properties: crate::contracts::MessagePartUpdatedProperties {
                    session_id: session_id.to_string(),
                    created_at: event_message.created_at,
                    updated_at: event_message.updated_at,
                    part: serde_json::json!({
                        "id": part.id.clone(),
                        "sessionID": session_id,
                        "messageID": message.id,
                        "type": part.part_type.clone(),
                        "text": part.text.clone().or(part.content.clone()).unwrap_or_default(),
                        "metadata": frontend_safe_part_value(&part, part.metadata.clone()),
                        "callID": part.call_id.clone(),
                        "tool": part.tool.clone(),
                        "state": frontend_safe_part_state(&part, part.state.clone()),
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
        self.add_tool_message_with_message_id(session_id, tool_name, call_id, state, metadata, None)
    }

    pub fn add_tool_message_with_message_id(
        &self,
        session_id: &str,
        tool_name: String,
        call_id: String,
        state: serde_json::Value,
        metadata: Option<serde_json::Value>,
        preferred_message_id: Option<String>,
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
                    self.push_event(GlobalEvent::MessagePartUpdated {
                        properties: crate::contracts::MessagePartUpdatedProperties {
                            session_id: session_id.to_string(),
                            created_at: message.created_at,
                            updated_at: message.updated_at,
                            part: serde_json::json!({
                                "id": &part.id,
                                "sessionID": session_id,
                                "messageID": message_id,
                                "type": &part.part_type,
                                "callID": &part.call_id,
                                "tool": &part.tool,
                                "state": frontend_safe_part_state(&part, part.state.clone()),
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

        if let Some(preferred_message_id) = preferred_message_id.as_deref() {
            let mut messages = self.messages.write();
            let session_messages = messages.entry(session_id.to_string()).or_default();
            if let Some(message) = session_messages
                .iter_mut()
                .find(|message| message.id == preferred_message_id)
            {
                message.updated_at = now;
                message.parts.push(part.clone());
                let message = message.clone();
                if let Some(info) = self.sessions.write().get_mut(session_id) {
                    info.updated_at = now;
                }
                drop(messages);
                self.push_event(GlobalEvent::MessageUpdated {
                    properties: crate::contracts::MessageUpdatedProperties {
                        session_id: session_id.to_string(),
                        info: crate::contracts::Message {
                            id: message.id.clone(),
                            session_id: message.session_id.clone(),
                            role: crate::contracts::MessageRole::Assistant,
                            parts: message
                                .parts
                                .iter()
                                .map(|part| crate::contracts::MessagePart {
                                    id: part.id.clone(),
                                    session_id: message.session_id.clone(),
                                    message_id: message.id.clone(),
                                    part_type: part.part_type.clone(),
                                    content: part.content.clone(),
                                    text: part.text.clone(),
                                    metadata: frontend_safe_part_value(part, part.metadata.clone()),
                                    call_id: part.call_id.clone(),
                                    tool: part.tool.clone(),
                                    state: frontend_safe_part_state(part, part.state.clone()),
                                })
                                .collect(),
                            created_at: message.created_at,
                            updated_at: message.updated_at,
                            parent_id: message.parent_id.clone(),
                        },
                    },
                });
                self.push_event(GlobalEvent::MessagePartUpdated {
                    properties: crate::contracts::MessagePartUpdatedProperties {
                        session_id: session_id.to_string(),
                        created_at: message.created_at,
                        updated_at: message.updated_at,
                        part: serde_json::json!({
                            "id": &part.id,
                            "sessionID": session_id,
                            "messageID": &message.id,
                            "type": &part.part_type,
                            "callID": &part.call_id,
                            "tool": &part.tool,
                            "state": frontend_safe_part_state(&part, part.state.clone()),
                            "metadata": frontend_safe_part_value(&part, part.metadata.clone()),
                        }),
                    },
                });
                return Some(message);
            }
        }

        let message = Message {
            id: preferred_message_id.unwrap_or_else(|| new_message_id(now)),
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

        self.push_event(GlobalEvent::MessageUpdated {
            properties: crate::contracts::MessageUpdatedProperties {
                session_id: session_id.to_string(),
                info: crate::contracts::Message {
                    id: message.id.clone(),
                    session_id: message.session_id.clone(),
                    role: crate::contracts::MessageRole::Assistant,
                    parts: vec![crate::contracts::MessagePart {
                        id: part.id.clone(),
                        session_id: message.session_id.clone(),
                        message_id: message.id.clone(),
                        part_type: part.part_type.clone(),
                        content: part.content.clone(),
                        text: part.text.clone(),
                        metadata: frontend_safe_part_value(&part, part.metadata.clone()),
                        call_id: part.call_id.clone(),
                        tool: part.tool.clone(),
                        state: frontend_safe_part_state(&part, part.state.clone()),
                    }],
                    created_at: message.created_at,
                    updated_at: message.updated_at,
                    parent_id: message.parent_id.clone(),
                },
            },
        });

        self.push_event(GlobalEvent::MessagePartUpdated {
            properties: crate::contracts::MessagePartUpdatedProperties {
                session_id: session_id.to_string(),
                created_at: message.created_at,
                updated_at: message.updated_at,
                part: serde_json::json!({
                    "id": &part.id,
                    "sessionID": session_id,
                    "messageID": &message.id,
                    "type": &part.part_type,
                    "callID": &part.call_id,
                    "tool": &part.tool,
                    "state": frontend_safe_part_state(&part, part.state.clone()),
                    "metadata": frontend_safe_part_value(&part, part.metadata.clone()),
                }),
            },
        });

        Some(message)
    }

    fn latest_user_parent_id(&self, session_id: &str) -> Option<String> {
        let messages = self
            .messages
            .read()
            .get(session_id)
            .cloned()
            .unwrap_or_default();
        messages
            .iter()
            .rev()
            .find(|message| message.role == MessageRole::User)
            .map(|message| message.id.clone())
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "legacy transient callbacks preserve explicit message and part ids"
    )]
    pub fn emit_transient_tool_message_with_ids(
        &self,
        session_id: &str,
        tool_name: String,
        call_id: String,
        state: serde_json::Value,
        metadata: Option<serde_json::Value>,
        message_id: String,
        part_id: String,
    ) -> Message {
        let now = Utc::now().timestamp_millis();
        let message = self.build_transient_tool_message_with_ids_and_times(
            session_id, tool_name, call_id, state, metadata, message_id, part_id, now, now,
        );
        self.message_updated_event(message.clone());
        message
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "runtime live overlay preserves ids and timestamps from one callback payload"
    )]
    pub fn build_transient_tool_message_with_ids_and_times(
        &self,
        session_id: &str,
        tool_name: String,
        call_id: String,
        state: serde_json::Value,
        metadata: Option<serde_json::Value>,
        message_id: String,
        part_id: String,
        created_at: i64,
        updated_at: i64,
    ) -> Message {
        let (state, metadata) = normalize_tool_message_state(&tool_name, state, metadata);
        let parent_id = self.latest_user_parent_id(session_id);
        let part = MessagePart {
            id: part_id,
            part_type: "tool".to_string(),
            content: None,
            text: None,
            metadata,
            call_id: Some(call_id),
            tool: Some(tool_name),
            state: Some(state),
        };
        Message {
            id: message_id,
            session_id: session_id.to_string(),
            role: MessageRole::Assistant,
            parent_id,
            parts: vec![part],
            created_at,
            updated_at,
        }
    }
}

fn frontend_visible_messages(messages: Vec<Message>) -> Vec<Message> {
    messages
        .into_iter()
        .filter(|message| message.role != MessageRole::System)
        .collect()
}

fn merge_message_parts(mut existing: Message, incoming: Message) -> Message {
    existing.session_id = incoming.session_id;
    existing.role = incoming.role;
    if incoming.parent_id.is_some() {
        existing.parent_id = incoming.parent_id;
    }
    existing.created_at = existing.created_at.min(incoming.created_at);
    existing.updated_at = existing.updated_at.max(incoming.updated_at);

    for part in incoming.parts {
        if let Some(existing_part) = existing
            .parts
            .iter_mut()
            .find(|candidate| same_message_part(candidate, &part))
        {
            *existing_part = part;
        } else {
            existing.parts.push(part);
        }
    }
    existing
}

fn same_message_part(left: &MessagePart, right: &MessagePart) -> bool {
    left.id == right.id
        || (left.part_type == "tool"
            && right.part_type == "tool"
            && left.tool == right.tool
            && left.call_id == right.call_id
            && left.call_id.is_some())
}

fn normalize_message_parts(
    parts: Vec<MessagePart>,
    metadata: Option<serde_json::Value>,
) -> Vec<MessagePart> {
    let normalized = parts
        .into_iter()
        .filter_map(|mut part| {
            let part_type = part.part_type.trim();
            if part_type.is_empty() {
                part.part_type = "text".to_string();
            } else if part_type != part.part_type {
                part.part_type = part_type.to_string();
            }
            if part.id.trim().is_empty() {
                part.id = Uuid::new_v4().to_string();
            }
            part.metadata = merge_part_metadata(part.metadata, metadata.clone());
            if part.text.is_none() && part.part_type == "text" {
                part.text = part.content.clone();
            }
            if part.content.is_none() && part.part_type == "text" {
                part.content = part.text.clone();
            }
            let has_payload = part.text.as_deref().is_some_and(|text| !text.is_empty())
                || part.content.as_deref().is_some_and(|text| !text.is_empty())
                || part.metadata.is_some()
                || part.state.is_some()
                || part.call_id.is_some()
                || part.tool.is_some();
            has_payload.then_some(part)
        })
        .collect::<Vec<_>>();
    if normalized.is_empty() {
        return vec![MessagePart {
            id: Uuid::new_v4().to_string(),
            part_type: "text".to_string(),
            content: Some("Prompt submitted".to_string()),
            text: Some("Prompt submitted".to_string()),
            metadata,
            call_id: None,
            tool: None,
            state: None,
        }];
    }
    normalized
}

fn merge_part_metadata(
    part_metadata: Option<serde_json::Value>,
    message_metadata: Option<serde_json::Value>,
) -> Option<serde_json::Value> {
    match (part_metadata, message_metadata) {
        (None, metadata) => metadata,
        (metadata, None) => metadata,
        (Some(serde_json::Value::Object(mut part)), Some(serde_json::Value::Object(message))) => {
            for (key, value) in message {
                part.entry(key).or_insert(value);
            }
            Some(serde_json::Value::Object(part))
        }
        (part, _) => part,
    }
}
