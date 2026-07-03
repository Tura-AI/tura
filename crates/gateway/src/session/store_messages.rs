use super::*;

impl SessionStore {
    pub fn get_messages(&self, session_id: &str) -> Vec<Message> {
        self.messages
            .read()
            .get(session_id)
            .cloned()
            .unwrap_or_default()
    }

    pub fn get_frontend_messages(&self, session_id: &str) -> Vec<Message> {
        frontend_visible_messages(self.get_session_db_messages(session_id))
    }

    pub fn get_session_db_messages(&self, session_id: &str) -> Vec<Message> {
        let should_refresh = {
            let loaded = self.session_db_loaded.read().contains(session_id);
            let refresh_needed = self.session_db_refresh_needed.read().contains(session_id);
            !loaded || refresh_needed
        };
        if should_refresh {
            if let Err(error) = self.refresh_session_db_cache(session_id) {
                tracing::warn!(
                    session_id,
                    error = %error,
                    "failed to refresh session DB cache for frontend messages"
                );
            }
        }

        let db_messages = self
            .session_db_messages
            .read()
            .get(session_id)
            .cloned()
            .unwrap_or_default();
        let projection_messages = self
            .messages
            .read()
            .get(session_id)
            .cloned()
            .unwrap_or_default();

        if db_messages.is_empty() {
            return projection_messages;
        }
        if projection_messages.is_empty() {
            return db_messages;
        }

        let mut messages = db_messages;
        for message in projection_messages {
            if messages.iter().any(|candidate| candidate.id == message.id) {
                continue;
            }
            messages.push(message);
        }
        messages.sort_by_key(|message| message.created_at);
        messages
    }

    pub fn apply_runtime_sync_status(
        &self,
        session_id: &str,
        status: &RuntimeSessionSyncStatus,
        message_id: Option<&str>,
    ) -> Option<Message> {
        if status.live_overlay_active() {
            return None;
        }

        self.remove_live_messages_for_runtime(session_id, &status.runtime_id);
        self.session_db_refresh_needed
            .write()
            .insert(session_id.to_string());
        match self.refresh_session_db_cache(session_id) {
            Ok(messages) => {
                message_id.and_then(|id| messages.into_iter().find(|message| message.id == id))
            }
            Err(error) => {
                tracing::warn!(
                    session_id,
                    runtime_id = %status.runtime_id,
                    error = %error,
                    "failed to refresh session DB cache after non-live runtime status"
                );
                None
            }
        }
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
            self.session_db_refresh_needed
                .write()
                .insert(session_id.to_string());
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
            session_messages.sort_by_key(|message| message.created_at);
            if let Some(info) = self.sessions.write().get_mut(session_id) {
                info.message_count = session_messages.len();
                if let Some(updated_at) = merged.iter().map(|message| message.updated_at).max() {
                    info.updated_at = info.updated_at.max(updated_at);
                }
            }
        }

        self.session_db_refresh_needed
            .write()
            .insert(session_id.to_string());
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

    pub fn set_todos(
        &self,
        session_id: &str,
        todos: Vec<serde_json::Value>,
    ) -> Vec<serde_json::Value> {
        self.todos
            .write()
            .insert(session_id.to_string(), todos.clone());
        self.push_event(GlobalEvent::TodoUpdated {
            properties: serde_json::json!({
                "sessionID": session_id,
                "todos": todos,
            }),
        });
        todos
    }

    pub fn copy_session_context(&self, source_session_id: &str, target_session_id: &str) -> bool {
        if !self.sessions.read().contains_key(source_session_id)
            || !self.sessions.read().contains_key(target_session_id)
        {
            return false;
        }

        let source_messages = self.get_frontend_messages(source_session_id);
        let mut id_map = HashMap::new();
        let now = Utc::now().timestamp_millis();
        let copied_messages = source_messages
            .iter()
            .enumerate()
            .map(|(index, message)| {
                let id = new_message_id(now + index as i64);
                id_map.insert(message.id.clone(), id.clone());
                Message {
                    id,
                    session_id: target_session_id.to_string(),
                    role: message.role,
                    parent_id: None,
                    parts: Vec::new(),
                    created_at: message.created_at,
                    updated_at: message.updated_at,
                }
            })
            .collect::<Vec<_>>();

        let copied_messages = source_messages
            .into_iter()
            .zip(copied_messages)
            .map(|(source, mut copied)| {
                copied.parent_id = source
                    .parent_id
                    .as_ref()
                    .and_then(|parent_id| id_map.get(parent_id).cloned());
                copied.parts = source
                    .parts
                    .into_iter()
                    .map(|part| MessagePart {
                        id: Uuid::new_v4().to_string(),
                        part_type: part.part_type,
                        content: part.content,
                        text: part.text,
                        metadata: part.metadata,
                        call_id: part.call_id,
                        tool: part.tool,
                        state: part.state,
                    })
                    .collect();
                copied
            })
            .collect::<Vec<_>>();

        let copied_todos = self.get_todos(source_session_id);
        self.messages
            .write()
            .insert(target_session_id.to_string(), copied_messages.clone());
        self.todos
            .write()
            .insert(target_session_id.to_string(), copied_todos);

        {
            let mut children = self.children.write();
            let entry = children.entry(source_session_id.to_string()).or_default();
            if !entry.iter().any(|id| id == target_session_id) {
                entry.push(target_session_id.to_string());
            }
        }

        if let Some(info) = self.sessions.write().get_mut(target_session_id) {
            info.message_count = copied_messages.len();
            info.updated_at = now;
            info.last_user_message_at = last_user_message_at_in_messages(&copied_messages);
        }
        true
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
        let now = Utc::now().timestamp_millis();

        let parent_id = if role == MessageRole::Assistant {
            self.latest_user_parent_id(session_id)
        } else {
            None
        };

        let parts = normalize_message_parts(parts, metadata);

        let message = Message {
            id: message_id.unwrap_or_else(|| new_message_id(now)),
            session_id: session_id.to_string(),
            role,
            parent_id,
            parts,
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
                info.last_user_message_at = Some(now);
                if let Some(timestamp) = chrono::DateTime::<Utc>::from_timestamp_millis(now) {
                    info.management.record_user_message_at(timestamp);
                }
                if let Some(text) = message.parts.iter().find_map(|part| part.text.clone()) {
                    if info.management.input.user_input.trim().is_empty() {
                        info.management.input.user_input = text;
                    }
                    if let Ok(entry) = serde_json::to_string(&message) {
                        info.management.session_log.push(entry);
                    }
                }
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
            .session_db_messages
            .read()
            .get(session_id)
            .cloned()
            .or_else(|| self.messages.read().get(session_id).cloned())
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
