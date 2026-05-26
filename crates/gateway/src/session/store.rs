//! Session store - manages session persistence using mano state machine
//!
//! This module provides session storage functionality using the SessionInfo
//! structure that wraps SessionManagement from mano.

use crate::api::types::{GlobalEvent, Session as ApiSession, SessionStatus as ApiSessionStatus};
use crate::session::config::{load_config, merge_config, sessions_dir, TuraSessionConfig};
use crate::session::manager::{
    agent_for_session_type, default_use_last_tool_call_response_for_session,
    normalize_session_type, runtime_provider_for_session, LspSessionConfig, SessionInfo,
    SessionManager, SessionStatus as SessionStatusMano,
};
use chrono::{DateTime, Utc};
use code_tools_suite::state_machine::session_management::{
    PlanStatus, PollInterval, SessionState, StartCondition, TaskStep,
};
use parking_lot::RwLock;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Message {
    pub id: String,
    pub session_id: String,
    pub role: MessageRole,
    pub parent_id: Option<String>,
    pub parts: Vec<MessagePart>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    User,
    Assistant,
    System,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MessagePart {
    pub id: String,
    #[serde(rename = "type")]
    pub part_type: String,
    pub content: Option<String>,
    pub text: Option<String>,
    pub metadata: Option<serde_json::Value>,
    pub call_id: Option<String>,
    pub tool: Option<String>,
    pub state: Option<serde_json::Value>,
}

pub struct SessionStore {
    sessions: Arc<RwLock<HashMap<String, SessionInfo>>>,
    messages: Arc<RwLock<HashMap<String, Vec<Message>>>>,
    todos: Arc<RwLock<HashMap<String, Vec<serde_json::Value>>>>,
    children: Arc<RwLock<HashMap<String, Vec<String>>>>,
    user_commands: Arc<RwLock<HashMap<String, Vec<String>>>>,
    cancelled: Arc<RwLock<HashSet<String>>>,
    current_session_id: Arc<RwLock<Option<String>>>,
    events: Arc<RwLock<Vec<GlobalEvent>>>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct PersistedSessionRecord {
    info: SessionInfo,
    #[serde(default)]
    parent_id: Option<String>,
    messages: Vec<Message>,
    todos: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScheduledTaskRun {
    pub session_id: String,
    pub task_summary: String,
    pub start_condition: StartCondition,
}

impl Default for SessionStore {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionStore {
    pub fn new() -> Self {
        let store = Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            messages: Arc::new(RwLock::new(HashMap::new())),
            todos: Arc::new(RwLock::new(HashMap::new())),
            children: Arc::new(RwLock::new(HashMap::new())),
            user_commands: Arc::new(RwLock::new(HashMap::new())),
            cancelled: Arc::new(RwLock::new(HashSet::new())),
            current_session_id: Arc::new(RwLock::new(None)),
            events: Arc::new(RwLock::new(Vec::new())),
        };
        store.init_default_session();
        store
    }

    fn init_default_session(&self) {
        let info = SessionManager::create_session(None, None, None, Some("coding".to_string()));
        let session_id = info.id.clone();
        self.sessions.write().insert(session_id.clone(), info);
        self.current_session_id.write().replace(session_id.clone());

        let now = Utc::now().timestamp_millis();
        let welcome_message = Message {
            id: new_message_id(now),
            session_id: session_id.clone(),
            role: MessageRole::Assistant,
            parent_id: None,
            parts: vec![MessagePart {
                id: Uuid::new_v4().to_string(),
                part_type: "text".to_string(),
                content: Some(
                    "Hello! I'm ready to help you. How can I assist you today?".to_string(),
                ),
                text: Some("Hello! I'm ready to help you. How can I assist you today?".to_string()),
                metadata: None,
                call_id: None,
                tool: None,
                state: None,
            }],
            created_at: now,
            updated_at: now,
        };
        self.messages
            .write()
            .insert(session_id, vec![welcome_message]);
    }

    pub fn hydrate_directory(&self, directory: Option<String>) {
        let Some(directory) = directory else {
            return;
        };
        let directory = PathBuf::from(directory);
        let dir = sessions_dir(&directory);
        let Ok(entries) = std::fs::read_dir(&dir) else {
            return;
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|value| value.to_str()) != Some("json") {
                continue;
            }
            if let Err(err) = self.load_persisted_session(&path) {
                tracing::warn!(path = %path.display(), error = %err, "failed to load persisted session");
            }
        }
    }

    fn load_persisted_session(&self, path: &Path) -> Result<(), String> {
        let content = std::fs::read_to_string(path).map_err(|err| err.to_string())?;
        let mut record: PersistedSessionRecord =
            serde_json::from_str(&content).map_err(|err| err.to_string())?;
        record.info.message_count = record.messages.len();
        record.info.use_last_tool_call_response = default_use_last_tool_call_response_for_session(
            record.info.session_type.as_deref().unwrap_or("coding"),
            record.info.agent.as_deref(),
        );
        record.info.management.use_last_tool_call_response =
            record.info.use_last_tool_call_response;
        let session_id = record.info.id.clone();

        if self.sessions.read().contains_key(&session_id) {
            return Ok(());
        }

        self.sessions
            .write()
            .insert(session_id.clone(), record.info);
        self.messages
            .write()
            .insert(session_id.clone(), record.messages);
        self.todos.write().insert(session_id.clone(), record.todos);
        if let Some(parent_id) = record.parent_id.filter(|value| !value.trim().is_empty()) {
            let mut children = self.children.write();
            let entry = children.entry(parent_id).or_default();
            if !entry.iter().any(|id| id == &session_id) {
                entry.push(session_id);
            }
        }
        Ok(())
    }

    fn persist_session(&self, session_id: &str) {
        if let Err(err) = self.persist_session_result(session_id) {
            tracing::warn!(session_id, error = %err, "failed to persist session");
        }
    }

    fn persist_session_result(&self, session_id: &str) -> Result<(), String> {
        let info = self
            .sessions
            .read()
            .get(session_id)
            .cloned()
            .ok_or_else(|| "session not found".to_string())?;
        let Some(directory) = info_directory(&info) else {
            return Ok(());
        };
        let messages = self
            .messages
            .read()
            .get(session_id)
            .cloned()
            .unwrap_or_default();
        let todos = self
            .todos
            .read()
            .get(session_id)
            .cloned()
            .unwrap_or_default();
        let parent_id = self.parent_for_child(session_id);
        let record = PersistedSessionRecord {
            info,
            parent_id,
            messages,
            todos,
        };

        let dir = sessions_dir(&directory);
        std::fs::create_dir_all(&dir).map_err(|err| err.to_string())?;
        let path = dir.join(format!("{session_id}.json"));
        let content = serde_json::to_string_pretty(&record).map_err(|err| err.to_string())?;
        std::fs::write(path, content).map_err(|err| err.to_string())
    }

    fn persist_active_config(&self, session: &ApiSession) {
        let Some(directory) = session
            .directory
            .as_deref()
            .filter(|value| !value.trim().is_empty())
        else {
            return;
        };
        let mut patch = TuraSessionConfig {
            model: session.model.clone(),
            active_agent: session.agent.clone(),
            session_type: session.session_type.clone(),
            kill_processes_on_start: Some(session.kill_processes_on_start),
            validator_enabled: Some(session.validator_enabled),
            force_multiple_tasks: Some(session.force_multiple_tasks),
            model_variant: session.model_variant.clone(),
            model_acceleration_enabled: Some(session.model_acceleration_enabled),
            ..TuraSessionConfig::default()
        };
        patch.fill_model_parts();
        if let Err(err) = merge_config(directory, patch) {
            tracing::warn!(directory, error = %err, "failed to persist active session config");
        }
    }

    pub fn list_sessions(&self) -> Vec<ApiSession> {
        let parent_by_child = self.parent_by_child();
        self.sessions
            .read()
            .values()
            .map(|info| api_session_from_info(info, parent_by_child.get(&info.id).cloned()))
            .collect()
    }

    pub fn get_session(&self, session_id: &str) -> Option<ApiSession> {
        let parent_id = self.parent_for_child(session_id);
        self.sessions
            .read()
            .get(session_id)
            .map(|info| api_session_from_info(info, parent_id))
    }

    pub fn get_session_info(&self, session_id: &str) -> Option<SessionInfo> {
        self.sessions.read().get(session_id).cloned()
    }

    pub fn list_child_sessions(&self, parent_session_id: &str) -> Vec<ApiSession> {
        let child_ids = self
            .children
            .read()
            .get(parent_session_id)
            .cloned()
            .unwrap_or_default();
        let sessions = self.sessions.read();
        child_ids
            .into_iter()
            .filter_map(|child_id| {
                sessions
                    .get(&child_id)
                    .map(|info| api_session_from_info(info, Some(parent_session_id.to_string())))
            })
            .collect()
    }

    pub fn list_child_session_ids(&self, parent_session_id: &str) -> Vec<String> {
        self.children
            .read()
            .get(parent_session_id)
            .cloned()
            .unwrap_or_default()
    }

    pub fn cancellation_scope_session_ids(&self, session_id: &str) -> Vec<String> {
        let root_id = self.root_session_id(session_id);
        let children = self.children.read().clone();
        let mut ids = Vec::new();
        let mut stack = vec![root_id];
        let mut seen = HashSet::new();

        while let Some(id) = stack.pop() {
            if !seen.insert(id.clone()) {
                continue;
            }
            ids.push(id.clone());
            if let Some(child_ids) = children.get(&id) {
                for child_id in child_ids.iter().rev() {
                    stack.push(child_id.clone());
                }
            }
        }

        ids
    }

    pub fn append_user_command(&self, session_id: &str, command: impl Into<String>) -> Vec<String> {
        let command = command.into();
        let command = command.trim();
        if command.is_empty() {
            return self.user_commands_for_session(session_id);
        }
        let root_id = self.root_session_id(session_id);
        let mut commands = self.user_commands.write();
        let entry = commands.entry(root_id).or_default();
        entry.push(command.to_string());
        entry.clone()
    }

    pub fn user_commands_for_session(&self, session_id: &str) -> Vec<String> {
        let root_id = self.root_session_id(session_id);
        self.user_commands
            .read()
            .get(&root_id)
            .cloned()
            .unwrap_or_default()
    }

    pub fn register_child_session(
        &self,
        parent_session_id: &str,
        child_session_id: &str,
        directory: Option<String>,
        name: Option<String>,
        task_instruction: Option<String>,
    ) -> ApiSession {
        let now = Utc::now().timestamp_millis();
        if let Some(existing) = self.sessions.write().get_mut(child_session_id) {
            existing.status = SessionStatusMano::Busy;
            existing.updated_at = now;
            if existing.directory.is_none() {
                existing.directory = directory.clone();
            }
            if existing.name.is_none() {
                existing.name = name
                    .clone()
                    .or_else(|| Some(format!("Subtask {}", child_session_id)));
            }
            {
                let mut children = self.children.write();
                let entry = children.entry(parent_session_id.to_string()).or_default();
                if !entry.iter().any(|id| id == child_session_id) {
                    entry.push(child_session_id.to_string());
                }
            }
            return api_session_from_info(existing, Some(parent_session_id.to_string()));
        }

        let mut info = SessionManager::create_session(
            directory,
            None,
            Some("coding_agent".to_string()),
            Some("coding".to_string()),
        );
        info.id = child_session_id.to_string();
        info.name = name.or_else(|| Some(format!("Subtask {}", child_session_id)));
        info.status = SessionStatusMano::Busy;
        info.created_at = now;
        info.updated_at = now;
        info.management.session_id = child_session_id.to_string();
        if let Some(parent) = self.sessions.read().get(parent_session_id) {
            info.disable_permission_restrictions = parent.disable_permission_restrictions;
            info.management.disable_permission_restrictions =
                parent.management.disable_permission_restrictions;
        }
        if let Some(name) = info.name.clone() {
            info.management.session_name = name;
        }
        let session = api_session_from_info(&info, Some(parent_session_id.to_string()));

        self.sessions
            .write()
            .insert(child_session_id.to_string(), info);
        self.messages
            .write()
            .entry(child_session_id.to_string())
            .or_default();
        self.todos
            .write()
            .entry(child_session_id.to_string())
            .or_default();
        {
            let mut children = self.children.write();
            let entry = children.entry(parent_session_id.to_string()).or_default();
            if !entry.iter().any(|id| id == child_session_id) {
                entry.push(child_session_id.to_string());
            }
        }

        if let Some(task_instruction) = task_instruction.filter(|value| !value.trim().is_empty()) {
            let _ = self.add_message(child_session_id, MessageRole::User, task_instruction);
        }

        session
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "gateway session creation mirrors the persisted session schema"
    )]
    pub fn create_session(
        &self,
        directory: Option<String>,
        model: Option<String>,
        agent: Option<String>,
        session_type: Option<String>,
        lsp: Option<LspSessionConfig>,
        kill_processes_on_start: bool,
        validator_enabled: bool,
        force_multiple_tasks: bool,
        model_variant: Option<String>,
        model_acceleration_enabled: bool,
        disable_permission_restrictions: bool,
    ) -> ApiSession {
        self.hydrate_directory(directory.clone());
        let persisted_config = directory.as_deref().map(load_config).unwrap_or_default();
        let model = model.or(persisted_config.model.clone());
        let agent = agent.or(persisted_config.active_agent.clone());
        let session_type = session_type.or(persisted_config.session_type.clone());
        let info = SessionManager::create_session(directory, model, agent, session_type);
        let mut info = info;
        if let Some(lsp) = lsp {
            info.lsp = lsp;
        }
        info.kill_processes_on_start = kill_processes_on_start;
        info.validator_enabled = validator_enabled;
        info.force_multiple_tasks = force_multiple_tasks;
        info.model_variant = model_variant.or(persisted_config.model_variant.clone());
        info.model_acceleration_enabled = model_acceleration_enabled;
        info.disable_permission_restrictions = disable_permission_restrictions;
        info.management.disable_permission_restrictions = disable_permission_restrictions;
        info.use_last_tool_call_response = default_use_last_tool_call_response_for_session(
            info.session_type.as_deref().unwrap_or("coding"),
            info.agent.as_deref(),
        );
        info.management.use_last_tool_call_response = info.use_last_tool_call_response;
        let session_id = info.id.clone();

        let session = api_session_from_info(&info, None);

        self.sessions.write().insert(session_id.clone(), info);
        self.messages.write().insert(session_id, Vec::new());
        self.todos.write().insert(session.id.clone(), Vec::new());
        self.persist_active_config(&session);
        self.persist_session(&session.id);

        session
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "gateway session update applies independent optional patch fields"
    )]
    pub fn update_session(
        &self,
        session_id: &str,
        title: Option<String>,
        model: Option<String>,
        agent: Option<String>,
        session_type: Option<String>,
        lsp: Option<LspSessionConfig>,
        kill_processes_on_start: Option<bool>,
        validator_enabled: Option<bool>,
        force_multiple_tasks: Option<bool>,
        disable_permission_restrictions: Option<bool>,
        task_management: Option<serde_json::Value>,
    ) -> Option<ApiSession> {
        let parent_id = self.parent_for_child(session_id);
        let mut sessions = self.sessions.write();
        let info = sessions.get_mut(session_id)?;
        let has_model_override = model.is_some();

        if let Some(title) = title
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
        {
            info.name = Some(title.clone());
            info.management.session_name = title;
        }

        if let Some(model) = model {
            info.model = Some(model);
        }

        if agent.is_some() || session_type.is_some() {
            let next_type =
                normalize_session_type(session_type, agent.as_deref().or(info.agent.as_deref()));
            info.session_type = Some(next_type.clone());
            info.agent = agent.or_else(|| agent_for_session_type(&next_type));
            info.use_last_tool_call_response =
                default_use_last_tool_call_response_for_session(&next_type, info.agent.as_deref());
            info.management.use_last_tool_call_response = info.use_last_tool_call_response;
        }
        if !has_model_override {
            if let Some(provider) = runtime_provider_for_session(
                info.session_type.as_deref().unwrap_or("coding"),
                info.agent.as_deref(),
            ) {
                info.model = Some(provider);
            }
        }
        if let Some(lsp) = lsp {
            info.lsp = lsp;
        }
        if let Some(kill_processes_on_start) = kill_processes_on_start {
            info.kill_processes_on_start = kill_processes_on_start;
        }
        if let Some(validator_enabled) = validator_enabled {
            info.validator_enabled = validator_enabled;
        }
        if let Some(force_multiple_tasks) = force_multiple_tasks {
            info.force_multiple_tasks = force_multiple_tasks;
        }
        if let Some(disable_permission_restrictions) = disable_permission_restrictions {
            info.disable_permission_restrictions = disable_permission_restrictions;
            info.management.disable_permission_restrictions = disable_permission_restrictions;
        }
        if let Some(task_management) = task_management {
            let mut patched = info.clone();
            match apply_task_management_patch(&mut patched, task_management) {
                Ok(()) => {
                    info.name = patched.name;
                    info.management.session_name = patched.management.session_name;
                    info.management.task_plan = patched.management.task_plan;
                }
                Err(err) => {
                    tracing::warn!(session_id, error = %err, "invalid task management patch ignored");
                }
            }
        }

        info.updated_at = Utc::now().timestamp_millis();

        let session = api_session_from_info(info, parent_id);
        drop(sessions);
        self.persist_active_config(&session);
        self.persist_session(session_id);
        Some(session)
    }

    pub fn delete_session(&self, session_id: &str) -> bool {
        if self.sessions.write().remove(session_id).is_some() {
            self.messages.write().remove(session_id);
            self.todos.write().remove(session_id);
            self.children.write().remove(session_id);
            self.cancelled.write().remove(session_id);
            for child_ids in self.children.write().values_mut() {
                child_ids.retain(|child_id| child_id != session_id);
            }

            let mut current = self.current_session_id.write();
            if *current == Some(session_id.to_string()) {
                let sessions = self.sessions.read();
                *current = sessions.keys().next().cloned();
            }
            true
        } else {
            false
        }
    }

    pub fn get_current_session(&self) -> Option<ApiSession> {
        let current_id = self.current_session_id.read().clone();
        current_id.and_then(|id| self.get_session(&id))
    }

    pub fn set_current_session(&self, session_id: &str) -> bool {
        if self.sessions.read().contains_key(session_id) {
            *self.current_session_id.write() = Some(session_id.to_string());
            true
        } else {
            false
        }
    }

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
        self.persist_session(session_id);
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
        self.persist_session(session_id);

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
                                "id": part.id.clone(),
                                "sessionID": session_id,
                                "messageID": message_id,
                                "type": part.part_type.clone(),
                                "callID": part.call_id.clone(),
                                "tool": part.tool.clone(),
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
                    "id": part.id.clone(),
                    "sessionID": session_id,
                    "messageID": message.id.clone(),
                    "type": part.part_type.clone(),
                    "callID": part.call_id.clone(),
                    "tool": part.tool.clone(),
                    "state": frontend_safe_part_value(&part, part.state.clone()),
                    "metadata": frontend_safe_part_value(&part, part.metadata.clone()),
                }),
            },
        });

        Some(message)
    }

    pub fn update_session_status(&self, session_id: &str, status: SessionStatusMano) {
        if let Some(info) = self.sessions.write().get_mut(session_id) {
            let now = Utc::now();
            let target_state = match status {
                SessionStatusMano::Idle => SessionState::Created,
                SessionStatusMano::Busy => SessionState::Running,
                SessionStatusMano::Error => SessionState::Failed,
            };
            if info.transition(target_state).is_err() && matches!(status, SessionStatusMano::Idle) {
                info.management.state = SessionState::Created;
                info.management.session_last_update_at = now;
            }
            info.status = status;
            info.updated_at = now.timestamp_millis();
        }
        self.persist_session(session_id);
        self.push_event(GlobalEvent::SessionStatus {
            properties: crate::api::types::SessionStatusProperties {
                session_id: session_id.to_string(),
                status: match status {
                    SessionStatusMano::Idle => serde_json::json!({ "type": "idle" }),
                    SessionStatusMano::Busy => serde_json::json!({ "type": "busy" }),
                    SessionStatusMano::Error => serde_json::json!({ "type": "error" }),
                },
            },
        });
    }

    pub fn claim_due_task_runs(&self, now: DateTime<Utc>) -> Vec<ScheduledTaskRun> {
        let mut claimed = Vec::new();
        let mut persist_ids = Vec::new();
        {
            let mut sessions = self.sessions.write();
            for info in sessions.values_mut() {
                if !matches!(info.status, SessionStatusMano::Idle) {
                    continue;
                }

                let Some(task_index) = info
                    .management
                    .task_plan
                    .detailed_tasks
                    .iter()
                    .position(|task| task_is_scheduler_eligible(task, now))
                else {
                    continue;
                };

                let plan_summary = info.management.task_plan.plan_summary.clone();
                let task = &mut info.management.task_plan.detailed_tasks[task_index];
                let start_condition = task.start_condition;
                let task_summary = task_display_summary(task, &plan_summary);
                task.status = PlanStatus::Doing;
                if matches!(start_condition, StartCondition::PollingTask) {
                    task.start_at = next_polling_start(task.start_at, task.poll_interval, now);
                }

                info.status = SessionStatusMano::Busy;
                info.updated_at = now.timestamp_millis();
                info.management.state = SessionState::Running;
                info.management.session_last_update_at = now;
                claimed.push(ScheduledTaskRun {
                    session_id: info.id.clone(),
                    task_summary,
                    start_condition,
                });
                persist_ids.push(info.id.clone());
            }
        }

        for session_id in persist_ids {
            self.persist_session(&session_id);
            if let Some(session) = self.get_session(&session_id) {
                self.push_event(GlobalEvent::SessionUpdated {
                    properties: crate::api::types::SessionUpdatedProperties {
                        session_id: session_id.clone(),
                        info: session,
                    },
                });
            }
            self.push_event(GlobalEvent::SessionStatus {
                properties: crate::api::types::SessionStatusProperties {
                    session_id,
                    status: serde_json::json!({ "type": "busy" }),
                },
            });
        }

        claimed
    }

    pub fn replace_management(
        &self,
        session_id: &str,
        management: code_tools_suite::state_machine::session_management::SessionManagement,
    ) {
        if let Some(info) = self.sessions.write().get_mut(session_id) {
            info.management = management;
            info.updated_at = Utc::now().timestamp_millis();
            info.status = SessionStatusMano::from_state(info.management.state);
        }
        self.persist_session(session_id);
    }

    pub fn session_count(&self) -> usize {
        self.sessions.read().len()
    }

    pub fn push_event(&self, event: GlobalEvent) {
        self.events.write().push(event);
    }

    pub fn pop_event(&self) -> Option<GlobalEvent> {
        let mut events = self.events.write();
        if events.is_empty() {
            return None;
        }
        Some(events.remove(0))
    }

    pub fn mark_cancelled(&self, session_id: &str) {
        self.cancelled.write().insert(session_id.to_string());
    }

    pub fn clear_cancelled(&self, session_id: &str) {
        self.cancelled.write().remove(session_id);
    }

    pub fn is_cancelled(&self, session_id: &str) -> bool {
        self.cancelled.read().contains(session_id)
    }

    fn parent_by_child(&self) -> HashMap<String, String> {
        self.children
            .read()
            .iter()
            .flat_map(|(parent_id, child_ids)| {
                child_ids
                    .iter()
                    .map(|child_id| (child_id.clone(), parent_id.clone()))
            })
            .collect()
    }

    fn parent_for_child(&self, session_id: &str) -> Option<String> {
        self.children
            .read()
            .iter()
            .find_map(|(parent_id, child_ids)| {
                child_ids
                    .iter()
                    .any(|child_id| child_id == session_id)
                    .then(|| parent_id.clone())
            })
    }

    pub fn root_session_id(&self, session_id: &str) -> String {
        let parents = self.parent_by_child();
        let mut current = session_id.to_string();
        let mut seen = HashSet::new();
        while let Some(parent) = parents.get(&current) {
            if !seen.insert(current.clone()) {
                break;
            }
            current = parent.clone();
        }
        current
    }
}

fn api_session_from_info(info: &SessionInfo, parent_id: Option<String>) -> ApiSession {
    let plan_summary = info.management.task_plan.plan_summary.trim().to_string();
    let plan_summary = (!plan_summary.is_empty()).then_some(plan_summary);
    let first_task_summary = info
        .management
        .task_plan
        .detailed_tasks
        .first()
        .map(|task| task.task_summary.trim().to_string())
        .filter(|value| !value.is_empty());
    let session_display_name = plan_summary
        .clone()
        .or(first_task_summary)
        .or_else(|| info.name.clone().filter(|value| !value.trim().is_empty()))
        .or_else(|| Some("New Session".to_string()));
    ApiSession {
        id: info.id.clone(),
        name: info.name.clone(),
        parent_id,
        created_at: info.created_at,
        updated_at: info.updated_at,
        directory: info.directory.clone(),
        model: info.model.clone(),
        agent: info.agent.clone(),
        session_type: info.session_type.clone(),
        lsp: Some(serde_json::to_value(&info.lsp).unwrap_or_else(|_| serde_json::json!({}))),
        kill_processes_on_start: info.kill_processes_on_start,
        validator_enabled: info.validator_enabled,
        force_multiple_tasks: info.force_multiple_tasks,
        disable_permission_restrictions: info.disable_permission_restrictions,
        model_variant: info.model_variant.clone(),
        model_acceleration_enabled: info.model_acceleration_enabled,
        status: match info.status {
            SessionStatusMano::Idle => ApiSessionStatus::Idle,
            SessionStatusMano::Busy => ApiSessionStatus::Busy,
            SessionStatusMano::Error => ApiSessionStatus::Error,
        },
        message_count: info.message_count,
        task_management: info.management.task_management_json(),
        plan_summary,
        session_display_name,
    }
}

fn ensure_first_task(info: &mut SessionInfo) -> &mut TaskStep {
    if info.management.task_plan.detailed_tasks.is_empty() {
        let summary = info
            .management
            .task_plan
            .plan_summary
            .clone()
            .if_empty(|| info.management.session_name.clone());
        let nonce_id = format!("{}:0", info.management.session_id);
        info.management.task_plan.detailed_tasks.push(TaskStep {
            nonce_id,
            step: 0,
            sub_session_id: String::new(),
            start_at: Utc::now(),
            poll_interval: PollInterval::default(),
            start_condition: StartCondition::UserAction,
            task_name: summary.clone(),
            status: PlanStatus::Todo,
            task_summary: summary.clone(),
            step_task: summary,
            ..TaskStep::default()
        });
    }
    info.management
        .task_plan
        .detailed_tasks
        .first_mut()
        .expect("first task should exist")
}

trait EmptyStringExt {
    fn if_empty(self, fallback: impl FnOnce() -> String) -> String;
}

impl EmptyStringExt for String {
    fn if_empty(self, fallback: impl FnOnce() -> String) -> String {
        if self.trim().is_empty() {
            fallback()
        } else {
            self
        }
    }
}

fn apply_task_management_patch(
    info: &mut SessionInfo,
    patch: serde_json::Value,
) -> Result<(), String> {
    if let Some(tasks) = patch.as_array() {
        return apply_task_list_patch(info, tasks);
    }
    let Some(object) = patch.as_object() else {
        return Err("task_management must be an object or array".to_string());
    };
    if let Some(tasks) = object.get("tasks").and_then(serde_json::Value::as_array) {
        apply_task_list_patch(info, tasks)?;
    }

    if let Some(summary) = string_field(object, &["plan_summary"]) {
        info.management.task_plan.plan_summary = summary.clone();
        if info.name.as_deref().is_none_or(str::is_empty) {
            info.name = Some(summary.clone());
        }
        if info.management.session_name.trim().is_empty()
            || info.management.session_name == "New Session"
        {
            info.management.session_name = summary;
        }
    }

    if !object_has_any_field(object, TASK_MANAGEMENT_TASK_PATCH_FIELDS) {
        return Ok(());
    }

    let task_summary = {
        let task = ensure_first_task(info);
        apply_single_task_patch(task, object)?;
        task.task_summary.clone()
    };
    if info.management.task_plan.plan_summary.trim().is_empty() {
        info.management.task_plan.plan_summary = task_summary;
    }
    Ok(())
}

const TASK_MANAGEMENT_TASK_PATCH_FIELDS: &[&str] = &[
    "nonce_id",
    "step",
    "task_summary",
    "delivery",
    "sub_session_id",
    "start_at",
    "poll_interval",
    "status",
];

fn apply_task_list_patch(
    info: &mut SessionInfo,
    tasks: &[serde_json::Value],
) -> Result<(), String> {
    for value in tasks {
        let Some(object) = value.as_object() else {
            return Err("tasks entries must be objects".to_string());
        };
        let nonce_id = string_field(object, &["nonce_id"]).unwrap_or_else(|| {
            format!(
                "{}:{}",
                info.management.session_id,
                info.management.task_plan.detailed_tasks.len()
            )
        });
        let position = info
            .management
            .task_plan
            .detailed_tasks
            .iter()
            .position(|task| task.nonce_id == nonce_id);
        let index = match position {
            Some(index) => index,
            None => {
                let step = number_field(object, &["step"])
                    .unwrap_or(info.management.task_plan.detailed_tasks.len() as u64);
                info.management.task_plan.detailed_tasks.push(TaskStep {
                    nonce_id: nonce_id.clone(),
                    step,
                    start_at: Utc::now(),
                    start_condition: StartCondition::UserAction,
                    ..TaskStep::default()
                });
                info.management.task_plan.detailed_tasks.len() - 1
            }
        };
        apply_single_task_patch(&mut info.management.task_plan.detailed_tasks[index], object)?;
        if info.management.task_plan.detailed_tasks[index]
            .nonce_id
            .trim()
            .is_empty()
        {
            info.management.task_plan.detailed_tasks[index].nonce_id = nonce_id;
        }
    }
    Ok(())
}

fn apply_single_task_patch(
    task: &mut TaskStep,
    object: &serde_json::Map<String, serde_json::Value>,
) -> Result<(), String> {
    if let Some(nonce_id) = string_field(object, &["nonce_id"]) {
        task.nonce_id = nonce_id;
    }
    if let Some(step) = number_field(object, &["step"]) {
        task.step = step;
    }
    if let Some(summary) = string_field(object, &["task_summary"]) {
        task.task_summary = summary.clone();
        task.task_name = summary.clone();
        if task.step_task.trim().is_empty() {
            task.step_task = summary;
        }
    }
    if let Some(delivery) = string_field(object, &["delivery"]) {
        task.step_deliverable_description = delivery;
    }
    if let Some(sub_session_id) = string_field(object, &["sub_session_id"]) {
        task.sub_session_id = sub_session_id;
    }
    if let Some(value) = first_field(object, &["status"]) {
        apply_unified_status(task, value)?;
    }
    if let Some(value) = first_field(object, &["poll_interval"]) {
        task.poll_interval = serde_json::from_value(value.clone())
            .map_err(|err| format!("invalid poll interval: {err}"))?;
        if task.poll_interval.m != 0
            || task.poll_interval.d != 0
            || task.poll_interval.h != 0
            || task.poll_interval.s != 0
        {
            task.start_condition = StartCondition::PollingTask;
        }
    }
    if let Some(value) = first_field(object, &["start_at"]) {
        task.start_at = parse_start_at(value)?;
        if !matches!(task.start_condition, StartCondition::PollingTask) {
            task.start_condition = StartCondition::ScheduledTask;
        }
    }
    Ok(())
}

fn apply_unified_status(task: &mut TaskStep, value: &serde_json::Value) -> Result<(), String> {
    match value.as_str() {
        Some("session_idle") => {
            task.status = PlanStatus::Todo;
            task.start_condition = StartCondition::SessionIdle;
            Ok(())
        }
        Some("user_action") => {
            task.status = PlanStatus::Todo;
            task.start_condition = StartCondition::UserAction;
            Ok(())
        }
        Some("scheduled_task") => {
            task.status = PlanStatus::Todo;
            task.start_condition = StartCondition::ScheduledTask;
            Ok(())
        }
        Some("polling_task") => {
            task.status = PlanStatus::Todo;
            task.start_condition = StartCondition::PollingTask;
            Ok(())
        }
        _ => {
            task.status = serde_json::from_value(value.clone())
                .map_err(|err| format!("invalid status: {err}"))?;
            Ok(())
        }
    }
}

fn first_field<'a>(
    object: &'a serde_json::Map<String, serde_json::Value>,
    names: &[&str],
) -> Option<&'a serde_json::Value> {
    names.iter().find_map(|name| object.get(*name))
}

fn object_has_any_field(
    object: &serde_json::Map<String, serde_json::Value>,
    names: &[&str],
) -> bool {
    names.iter().any(|name| object.contains_key(*name))
}

fn string_field(
    object: &serde_json::Map<String, serde_json::Value>,
    names: &[&str],
) -> Option<String> {
    first_field(object, names)
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn number_field(
    object: &serde_json::Map<String, serde_json::Value>,
    names: &[&str],
) -> Option<u64> {
    first_field(object, names).and_then(serde_json::Value::as_u64)
}

fn parse_start_at(value: &serde_json::Value) -> Result<DateTime<Utc>, String> {
    if let Some(text) = value.as_str() {
        return DateTime::parse_from_rfc3339(text)
            .map(|datetime| datetime.with_timezone(&Utc))
            .map_err(|err| format!("invalid start_at: {err}"));
    }
    if let Some(millis) = value.as_i64() {
        return DateTime::<Utc>::from_timestamp_millis(millis)
            .ok_or_else(|| "invalid start_at milliseconds".to_string());
    }
    Err("start_at must be RFC3339 or epoch milliseconds".to_string())
}

fn task_is_scheduler_eligible(task: &TaskStep, now: DateTime<Utc>) -> bool {
    if matches!(task.status, PlanStatus::Done | PlanStatus::Archived) {
        return false;
    }
    match task.start_condition {
        StartCondition::ScheduledTask | StartCondition::PollingTask => {
            matches!(task.status, PlanStatus::Todo | PlanStatus::Question) && task.start_at <= now
        }
        StartCondition::SessionIdle => {
            matches!(task.status, PlanStatus::Todo | PlanStatus::Question)
        }
        StartCondition::UserAction => false,
    }
}

fn task_display_summary(task: &TaskStep, plan_summary: &str) -> String {
    [
        task.task_summary.as_str(),
        task.task_name.as_str(),
        task.step_task.as_str(),
        plan_summary,
    ]
    .into_iter()
    .map(str::trim)
    .find(|value| !value.is_empty())
    .unwrap_or("Continue planned task")
    .to_string()
}

fn next_polling_start(
    previous_start: DateTime<Utc>,
    interval: PollInterval,
    now: DateTime<Utc>,
) -> DateTime<Utc> {
    let seconds = interval
        .s
        .saturating_add(interval.m.saturating_mul(60))
        .saturating_add(interval.h.saturating_mul(60 * 60))
        .saturating_add(interval.d.saturating_mul(24 * 60 * 60));
    let seconds = seconds.max(1);
    let step = chrono::Duration::seconds(seconds.min(i64::MAX as u64) as i64);
    let mut next = previous_start + step;
    while next <= now {
        next += step;
    }
    next
}

fn info_directory(info: &SessionInfo) -> Option<PathBuf> {
    info.directory
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .or_else(|| Some(info.management.session_directory.clone()))
}

fn frontend_safe_value(value: Option<serde_json::Value>) -> Option<serde_json::Value> {
    value.map(sanitize_frontend_value)
}

fn frontend_safe_part_value(
    part: &MessagePart,
    value: Option<serde_json::Value>,
) -> Option<serde_json::Value> {
    if part.part_type == "tool" && part.tool.as_deref() == Some("runtime") {
        return value;
    }
    frontend_safe_value(value)
}

fn normalize_tool_message_state(
    tool_name: &str,
    mut state: serde_json::Value,
    metadata: Option<serde_json::Value>,
) -> (serde_json::Value, Option<serde_json::Value>) {
    let Some(state_object) = state.as_object_mut() else {
        return (state, metadata);
    };
    if state_object
        .get("status")
        .and_then(serde_json::Value::as_str)
        != Some("running")
    {
        return (state, metadata);
    }

    let metadata_ref = metadata.as_ref().or_else(|| state_object.get("metadata"));
    let Some(metadata_object) = metadata_ref.and_then(serde_json::Value::as_object) else {
        return (state, metadata);
    };
    if metadata_object
        .get("kind")
        .and_then(serde_json::Value::as_str)
        != Some("mano_tool_call")
    {
        return (state, metadata);
    }
    if metadata_object
        .get("streaming_partial")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false)
    {
        return (state, metadata);
    }
    let Some(output) = metadata_object.get("output") else {
        return (state, metadata);
    };

    let ok = output
        .get("ok")
        .and_then(serde_json::Value::as_bool)
        .or_else(|| {
            metadata_object
                .get("success")
                .and_then(serde_json::Value::as_bool)
        })
        .unwrap_or(true);
    let output_text = tool_output_display_text(output, metadata_object.get("error"));
    let error_value = metadata_object
        .get("error")
        .cloned()
        .unwrap_or_else(|| serde_json::json!("Tool execution failed"));
    if ok {
        state_object.insert("status".to_string(), serde_json::json!("completed"));
        state_object.insert(
            "title".to_string(),
            serde_json::json!(format!("Called `{tool_name}`")),
        );
        state_object
            .entry("output".to_string())
            .or_insert(output_text);
    } else {
        state_object.insert("status".to_string(), serde_json::json!("error"));
        state_object.insert("error".to_string(), error_value);
    }
    if let Some(time) = state_object
        .get_mut("time")
        .and_then(serde_json::Value::as_object_mut)
    {
        time.entry("end".to_string())
            .or_insert_with(|| serde_json::json!(Utc::now().timestamp_millis()));
    }

    (state, metadata)
}

fn tool_output_display_text(
    output: &serde_json::Value,
    error: Option<&serde_json::Value>,
) -> serde_json::Value {
    if let Some(error) = error.and_then(serde_json::Value::as_str) {
        return serde_json::Value::String(error.to_string());
    }
    match serde_json::to_string(output) {
        Ok(text) => serde_json::Value::String(text),
        Err(_) => serde_json::Value::String(String::new()),
    }
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

lazy_static::lazy_static! {
    pub static ref SESSION_STORE: SessionStore = SessionStore::new();
}

pub fn session_store() -> &'static SessionStore {
    &SESSION_STORE
}

fn new_message_id(now: i64) -> String {
    format!("msg-{now:013}-{}", Uuid::new_v4())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn update_session_status_updates_stored_status() {
        let store = SessionStore::new();
        let session = store.create_session(
            Some("C:/workspace".to_string()),
            None,
            None,
            Some("coding".to_string()),
            None,
            false,
            false,
            false,
            None,
            false,
            false,
        );

        store.update_session_status(&session.id, SessionStatusMano::Busy);
        let updated = store
            .get_session(&session.id)
            .expect("session should exist");
        assert_eq!(updated.status, ApiSessionStatus::Busy);

        store.update_session_status(&session.id, SessionStatusMano::Idle);
        let updated = store
            .get_session(&session.id)
            .expect("session should exist");
        assert_eq!(updated.status, ApiSessionStatus::Idle);
    }

    #[test]
    fn add_tool_message_updates_existing_call_id() {
        let store = SessionStore::new();
        let session = store.create_session(
            Some("C:/workspace".to_string()),
            None,
            None,
            Some("coding".to_string()),
            None,
            false,
            false,
            false,
            None,
            false,
            false,
        );

        let first = store
            .add_tool_message(
                &session.id,
                "grep".to_string(),
                "call-1".to_string(),
                serde_json::json!({
                    "status": "running",
                    "input": { "pattern": "foo" },
                    "time": { "start": 1 }
                }),
                None,
            )
            .expect("running tool message should be stored");

        let second = store
            .add_tool_message(
                &session.id,
                "grep".to_string(),
                "call-1".to_string(),
                serde_json::json!({
                    "status": "completed",
                    "input": { "pattern": "foo" },
                    "output": "matched",
                    "title": "Called `grep`",
                    "metadata": {},
                    "time": { "start": 1, "end": 2 }
                }),
                None,
            )
            .expect("completed tool message should update stored message");

        assert_eq!(first.id, second.id);
        let messages = store.get_messages(&session.id);
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].parts.len(), 1);
        assert_eq!(
            messages[0].parts[0]
                .state
                .as_ref()
                .and_then(|state| state.get("status"))
                .and_then(serde_json::Value::as_str),
            Some("completed")
        );
    }

    #[test]
    fn add_tool_message_normalizes_running_state_with_final_output_metadata() {
        let store = SessionStore::new();
        let session = store.create_session(
            Some("C:/workspace".to_string()),
            None,
            None,
            Some("general".to_string()),
            None,
            false,
            false,
            false,
            None,
            false,
            false,
        );

        store
            .add_tool_message(
                &session.id,
                "command_run".to_string(),
                "call-1".to_string(),
                serde_json::json!({
                    "status": "running",
                    "input": { "commands": [] },
                    "metadata": {
                        "kind": "mano_tool_call",
                        "output": {
                            "ok": false,
                            "errors": [{ "message": "bad command" }]
                        }
                    },
                    "time": { "start": 1 }
                }),
                Some(serde_json::json!({
                    "kind": "mano_tool_call",
                    "output": {
                        "ok": false,
                        "errors": [{ "message": "bad command" }]
                    },
                    "error": "bad command"
                })),
            )
            .expect("tool message should be stored");

        let messages = store.get_messages(&session.id);
        let state = messages[0].parts[0]
            .state
            .as_ref()
            .expect("part should have state");
        assert_eq!(
            state.get("status").and_then(serde_json::Value::as_str),
            Some("error")
        );
        assert_eq!(
            state.get("error").and_then(serde_json::Value::as_str),
            Some("bad command")
        );
        assert!(state
            .get("time")
            .and_then(|time| time.get("end"))
            .and_then(serde_json::Value::as_i64)
            .is_some());
    }

    #[test]
    fn user_commands_are_shared_from_parent_to_child_sessions() {
        let store = SessionStore::new();
        let child_id = format!("child-{}", Uuid::new_v4());
        let session = store.create_session(
            Some("C:/workspace".to_string()),
            None,
            None,
            Some("coding".to_string()),
            None,
            false,
            false,
            false,
            None,
            false,
            false,
        );

        store.register_child_session(
            &session.id,
            &child_id,
            Some("C:/workspace".to_string()),
            Some("Subtask".to_string()),
            Some("read files".to_string()),
        );
        store.append_user_command(&session.id, "focus on tests");

        assert_eq!(
            store.user_commands_for_session(&session.id),
            vec!["focus on tests"]
        );
        assert_eq!(
            store.user_commands_for_session(&child_id),
            vec!["focus on tests"]
        );

        store.append_user_command(&child_id, "also update docs");
        assert_eq!(
            store.user_commands_for_session(&session.id),
            vec!["focus on tests", "also update docs"]
        );
        assert_eq!(
            store.user_commands_for_session(&child_id),
            vec!["focus on tests", "also update docs"]
        );
    }

    #[test]
    fn hydrated_child_session_keeps_parent_mapping() {
        let root = std::env::temp_dir().join(format!("tura-child-session-{}", Uuid::new_v4()));
        let directory = root.to_string_lossy().to_string();
        let store = SessionStore::new();
        let parent = store.create_session(
            Some(directory.clone()),
            None,
            None,
            Some("coding".to_string()),
            None,
            false,
            false,
            false,
            None,
            false,
            false,
        );

        store.register_child_session(
            &parent.id,
            "child-1",
            Some(directory.clone()),
            Some("Subtask".to_string()),
            Some("read files".to_string()),
        );

        let hydrated = SessionStore::new();
        hydrated.hydrate_directory(Some(directory.clone()));
        let child = hydrated
            .get_session("child-1")
            .expect("child should hydrate");

        assert_eq!(child.parent_id.as_deref(), Some(parent.id.as_str()));
        assert_eq!(hydrated.list_child_session_ids(&parent.id), vec!["child-1"]);

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn child_session_derives_workspace_and_task_instruction_context() {
        let store = SessionStore::new();
        let child_id = format!("child-{}", Uuid::new_v4());
        let parent = store.create_session(
            Some("C:/workspace".to_string()),
            None,
            None,
            Some("coding".to_string()),
            None,
            false,
            false,
            true,
            None,
            false,
            true,
        );

        let child = store.register_child_session(
            &parent.id,
            &child_id,
            parent.directory.clone(),
            Some("Backend subtask".to_string()),
            Some("Read docs/backend/ACCEPTANCE.md and implement the backend module.".to_string()),
        );
        let child_info = store
            .get_session_info(&child_id)
            .expect("child session info should exist");
        let messages = store.get_messages(&child_id);

        assert_eq!(child.parent_id.as_deref(), Some(parent.id.as_str()));
        assert_eq!(child.directory.as_deref(), Some("C:/workspace"));
        assert_eq!(
            child_info.management.session_directory,
            PathBuf::from("C:/workspace")
        );
        assert!(child_info.management.disable_permission_restrictions);
        assert!(messages.iter().any(|message| {
            message.role == MessageRole::User
                && message.parts.iter().any(|part| {
                    part.text
                        .as_deref()
                        .is_some_and(|text| text.contains("docs/backend/ACCEPTANCE.md"))
                })
        }));
    }

    #[test]
    fn cancellation_scope_includes_root_and_descendants_from_child() {
        let store = SessionStore::new();
        let child_id = format!("child-{}", uuid::Uuid::new_v4());
        let grandchild_id = format!("grandchild-{}", uuid::Uuid::new_v4());
        let root = store.create_session(
            Some("C:/workspace".to_string()),
            None,
            None,
            Some("coding".to_string()),
            None,
            false,
            false,
            false,
            None,
            false,
            false,
        );

        store.register_child_session(
            &root.id,
            &child_id,
            Some("C:/workspace".to_string()),
            Some("Subtask 1".to_string()),
            Some("first".to_string()),
        );
        store.register_child_session(
            &child_id,
            &grandchild_id,
            Some("C:/workspace".to_string()),
            Some("Subtask 1.1".to_string()),
            Some("nested".to_string()),
        );

        assert_eq!(
            store.cancellation_scope_session_ids(&child_id),
            vec![root.id.clone(), child_id, grandchild_id]
        );
    }

    #[test]
    fn update_session_title_persists_to_management_name() {
        let store = SessionStore::new();
        let session = store.create_session(
            Some("C:/workspace".to_string()),
            None,
            None,
            Some("coding".to_string()),
            None,
            false,
            false,
            false,
            None,
            false,
            false,
        );

        let updated = store
            .update_session(
                &session.id,
                Some("修复登录流程".to_string()),
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
            )
            .expect("session should update");

        assert_eq!(updated.name.as_deref(), Some("修复登录流程"));
        let info = store.sessions.read();
        let stored = info.get(&session.id).expect("session should remain stored");
        assert_eq!(stored.name.as_deref(), Some("修复登录流程"));
        assert_eq!(stored.management.session_name, "修复登录流程");
    }

    #[test]
    fn update_session_task_management_persists_and_lists_status() {
        let store = SessionStore::new();
        let session = store.create_session(
            Some("C:/workspace".to_string()),
            None,
            None,
            Some("coding".to_string()),
            None,
            false,
            false,
            false,
            None,
            false,
            false,
        );

        let updated = store
            .update_session(
                &session.id,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                Some(serde_json::json!({
                    "plan_summary": "计划入口名称",
                    "task_summary": "执行状态机名称",
                    "status": "question",
                    "start_at": "2026-05-25T08:30:00Z",
                    "poll_interval": { "m": 0, "d": 1, "h": 2, "s": 3 },
                    "sub_session_id": "sub-1",
                    "step": 2
                })),
            )
            .expect("session should update");

        assert_eq!(updated.plan_summary.as_deref(), Some("计划入口名称"));
        assert_eq!(
            updated.task_management["status"],
            serde_json::json!("question")
        );
        assert!(updated.task_management.get("start_condition").is_none());
        assert_eq!(updated.task_management["step"], serde_json::json!(2));

        let listed = store
            .list_sessions()
            .into_iter()
            .find(|item| item.id == session.id)
            .expect("session should be listed");
        assert_eq!(listed.session_display_name.as_deref(), Some("计划入口名称"));
        assert_eq!(listed.task_management["sub_session_id"], "sub-1");
    }

    #[test]
    fn single_task_patch_defaults_nonce_to_session_step_zero() {
        let store = SessionStore::new();
        let session = store.create_session(
            Some("C:/workspace".to_string()),
            None,
            None,
            Some("coding".to_string()),
            None,
            false,
            false,
            false,
            None,
            false,
            false,
        );

        let updated = store
            .update_session(
                &session.id,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                Some(serde_json::json!({
                    "plan_summary": "Single task contract",
                    "task_summary": "Run one task"
                })),
            )
            .expect("session should update");

        assert_eq!(
            updated.task_management["nonce_id"],
            serde_json::json!(format!("{}:0", session.id))
        );
        assert_eq!(updated.task_management["step"], serde_json::json!(0));
    }

    #[test]
    fn multi_task_patch_matches_nonce_and_creates_defaulted_tasks() {
        let store = SessionStore::new();
        let session = store.create_session(
            Some("C:/workspace".to_string()),
            None,
            None,
            Some("coding".to_string()),
            None,
            false,
            false,
            false,
            None,
            false,
            false,
        );

        let planned = store
            .update_session(
                &session.id,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                Some(serde_json::json!({
                    "plan_summary": "Multi task contract",
                    "tasks": [
                        {
                            "nonce_id": "inspect",
                            "step": 0,
                            "task_summary": "Inspect wiring",
                            "delivery": "Find the files."
                        },
                        {
                            "nonce_id": "verify",
                            "step": 1,
                            "task_summary": "Verify wiring",
                            "delivery": "Delivery spelling.",
                            "status": "question"
                        }
                    ]
                })),
            )
            .expect("initial multi-task patch should update");

        assert_eq!(
            planned.task_management["tasks"]
                .as_array()
                .expect("task_management.tasks should be an array")
                .len(),
            2
        );

        let updated = store
            .update_session(
                &session.id,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                Some(serde_json::json!({
                    "tasks": [
                        {
                            "nonce_id": "inspect",
                            "status": "done"
                        },
                        {
                            "task_summary": "Generated follow-up"
                        }
                    ]
                })),
            )
            .expect("follow-up multi-task patch should update");

        let tasks = updated.task_management["tasks"]
            .as_array()
            .expect("multi-task state should serialize as tasks array");
        assert_eq!(tasks.len(), 3);
        assert_eq!(tasks[0]["nonce_id"], "inspect");
        assert_eq!(tasks[0]["status"], "done");
        assert_eq!(tasks[1]["nonce_id"], "verify");
        assert_eq!(tasks[1]["status"], "question");
        assert_eq!(tasks[1]["delivery"], "Delivery spelling.");
        assert_eq!(tasks[2]["nonce_id"], format!("{}:2", session.id));
        assert_eq!(tasks[2]["step"], 2);
        assert_eq!(tasks[2]["task_summary"], "Generated follow-up");
        assert!(tasks[2].get("status").is_none());
        assert!(tasks[2].get("start_condition").is_none());
    }

    #[test]
    fn task_management_patch_accepts_all_contract_enums() {
        let store = SessionStore::new();
        let session = store.create_session(
            Some("C:/workspace".to_string()),
            None,
            None,
            Some("coding".to_string()),
            None,
            false,
            false,
            false,
            None,
            false,
            false,
        );

        for status in ["todo", "doing", "question", "done", "archived"] {
            let updated = store
                .update_session(
                    &session.id,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    Some(serde_json::json!({ "status": status })),
                )
                .expect("session should update");
            if status == "todo" {
                assert!(updated.task_management.get("status").is_none());
            } else {
                assert_eq!(updated.task_management["status"], status);
            }
        }

        for start_condition in [
            "session_idle",
            "user_action",
            "scheduled_task",
            "polling_task",
        ] {
            let updated = store
                .update_session(
                    &session.id,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    Some(serde_json::json!({ "status": start_condition })),
                )
                .expect("session should update");
            assert!(updated.task_management.get("start_condition").is_none());
            assert!(updated.task_management.get("status").is_none());
        }
    }

    #[test]
    fn invalid_task_management_patch_keeps_previous_state() {
        let root = std::env::temp_dir().join(format!("tura-invalid-task-{}", Uuid::new_v4()));
        let directory = root.to_string_lossy().to_string();
        let store = SessionStore::new();
        let session = store.create_session(
            Some(directory.clone()),
            None,
            None,
            Some("coding".to_string()),
            None,
            false,
            false,
            false,
            None,
            false,
            false,
        );
        let valid = store
            .update_session(
                &session.id,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                Some(serde_json::json!({
                    "plan_summary": "Stable plan",
                    "task_summary": "Stable task",
                    "status": "todo",
                    "start_condition": "user_action",
                    "start_at": "2026-05-25T08:30:00Z",
                    "poll_interval": { "m": 0, "d": 0, "h": 1, "s": 0 }
                })),
            )
            .expect("valid patch should update");
        let previous_task_management = valid.task_management.clone();
        let previous_plan_summary = valid.plan_summary.clone();

        let invalid_status = store
            .update_session(
                &session.id,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                Some(serde_json::json!({
                    "plan_summary": "Should not leak",
                    "task_summary": "Should not leak",
                    "status": "blocked"
                })),
            )
            .expect("invalid patch remains non-fatal");
        assert_eq!(invalid_status.task_management, previous_task_management);
        assert_eq!(invalid_status.plan_summary, previous_plan_summary);

        let invalid_date = store
            .update_session(
                &session.id,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                Some(serde_json::json!({
                    "status": "done",
                    "start_at": "not-a-date"
                })),
            )
            .expect("invalid date remains non-fatal");
        assert_eq!(invalid_date.task_management, previous_task_management);

        let hydrated = SessionStore::new();
        hydrated.hydrate_directory(Some(directory));
        let persisted = hydrated
            .get_session(&session.id)
            .expect("persisted session should hydrate");
        assert_eq!(persisted.task_management, previous_task_management);
        assert_eq!(persisted.plan_summary, previous_plan_summary);

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn session_display_name_falls_back_to_new_session() {
        let mut info = SessionManager::create_session(
            Some("C:/workspace".to_string()),
            None,
            None,
            Some("coding".to_string()),
        );
        info.name = None;
        info.management.session_name.clear();
        info.management.task_plan.plan_summary.clear();

        let session = api_session_from_info(&info, None);

        assert_eq!(session.session_display_name.as_deref(), Some("New Session"));
    }

    #[test]
    fn user_messages_are_recorded_in_session_management_log() {
        let store = SessionStore::new();
        let session = store.create_session(
            Some("C:/workspace".to_string()),
            None,
            None,
            Some("coding".to_string()),
            None,
            false,
            false,
            false,
            None,
            false,
            false,
        );

        store
            .add_message(&session.id, MessageRole::User, "补充新的约束".to_string())
            .expect("message should be stored");
        let info = store
            .get_session_info(&session.id)
            .expect("session info should exist");
        assert!(info
            .management
            .session_log
            .iter()
            .any(|entry| entry.contains("补充新的约束")));
    }

    #[test]
    fn user_messages_preserve_and_hydrate_pending_task_management_state() {
        let root = std::env::temp_dir().join(format!("tura-message-task-{}", Uuid::new_v4()));
        let directory = root.to_string_lossy().to_string();
        let store = SessionStore::new();
        let session = store.create_session(
            Some(directory.clone()),
            None,
            None,
            Some("coding".to_string()),
            None,
            false,
            false,
            false,
            None,
            false,
            false,
        );
        let start_at = (Utc::now() + chrono::Duration::hours(2)).to_rfc3339();
        let scheduled = store
            .update_session(
                &session.id,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                Some(serde_json::json!({
                    "plan_summary": "Pending scheduled plan",
                    "task_summary": "Ask before continuing",
                    "status": "question",
                    "start_condition": "scheduled_task",
                    "start_at": start_at,
                    "poll_interval": { "m": 5, "d": 0, "h": 1, "s": 30 }
                })),
            )
            .expect("scheduled task state should update");
        let previous_task_management = scheduled.task_management.clone();

        store
            .add_message(
                &session.id,
                MessageRole::User,
                "用户补充：保持计划等待，不要自动改状态".to_string(),
            )
            .expect("message should be stored");

        let after_message = store
            .get_session(&session.id)
            .expect("session should remain available");
        assert_eq!(after_message.task_management, previous_task_management);

        let hydrated = SessionStore::new();
        hydrated.hydrate_directory(Some(directory));
        let persisted = hydrated
            .get_session(&session.id)
            .expect("hydrated session should exist");
        assert_eq!(persisted.task_management, previous_task_management);
        let info = hydrated
            .get_session_info(&session.id)
            .expect("hydrated session info should exist");
        assert!(info
            .management
            .session_log
            .iter()
            .any(|entry| entry.contains("保持计划等待")));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn scheduler_claims_due_idle_tasks_and_skips_ineligible_tasks() {
        let root = std::env::temp_dir().join(format!("tura-scheduled-task-{}", Uuid::new_v4()));
        let directory = root.to_string_lossy().to_string();
        let store = SessionStore::new();
        let now = Utc::now();
        let due = (now - chrono::Duration::minutes(5)).to_rfc3339();
        let future = (now + chrono::Duration::minutes(5)).to_rfc3339();
        let scheduled = store.create_session(
            Some(directory.clone()),
            None,
            None,
            Some("coding".to_string()),
            None,
            false,
            false,
            false,
            None,
            false,
            false,
        );
        let busy = store.create_session(
            Some(directory.clone()),
            None,
            None,
            Some("coding".to_string()),
            None,
            false,
            false,
            false,
            None,
            false,
            false,
        );
        let done = store.create_session(
            Some(directory.clone()),
            None,
            None,
            Some("coding".to_string()),
            None,
            false,
            false,
            false,
            None,
            false,
            false,
        );
        let user_action = store.create_session(
            Some(directory.clone()),
            None,
            None,
            Some("coding".to_string()),
            None,
            false,
            false,
            false,
            None,
            false,
            false,
        );
        let future_scheduled = store.create_session(
            Some(directory.clone()),
            None,
            None,
            Some("coding".to_string()),
            None,
            false,
            false,
            false,
            None,
            false,
            false,
        );
        let idle = store.create_session(
            Some(directory.clone()),
            None,
            None,
            Some("coding".to_string()),
            None,
            false,
            false,
            false,
            None,
            false,
            false,
        );

        store.update_session(
            &scheduled.id,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            Some(serde_json::json!({
                "task_summary": "due scheduled",
                "status": "todo",
                "start_at": due
            })),
        );
        store.update_session(
            &busy.id,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            Some(serde_json::json!({
                "task_summary": "busy scheduled",
                "status": "todo",
                "start_at": due
            })),
        );
        store.update_session_status(&busy.id, SessionStatusMano::Busy);
        store.update_session(
            &done.id,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            Some(serde_json::json!({
                "task_summary": "done scheduled",
                "status": "done",
                "start_at": due
            })),
        );
        store.update_session(
            &user_action.id,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            Some(serde_json::json!({
                "task_summary": "manual only",
                "status": "todo"
            })),
        );
        store.update_session(
            &future_scheduled.id,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            Some(serde_json::json!({
                "task_summary": "future scheduled",
                "status": "todo",
                "start_at": future
            })),
        );
        store.update_session(
            &idle.id,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            Some(serde_json::json!({
                "task_summary": "idle pending",
                "status": "session_idle"
            })),
        );

        let claimed = store.claim_due_task_runs(now);
        let mut claimed_ids = claimed
            .iter()
            .map(|run| run.session_id.as_str())
            .collect::<Vec<_>>();
        claimed_ids.sort_unstable();
        let mut expected_ids = vec![scheduled.id.as_str(), idle.id.as_str()];
        expected_ids.sort_unstable();

        assert_eq!(claimed_ids, expected_ids);
        assert_eq!(
            store
                .get_session(&scheduled.id)
                .expect("scheduled should exist")
                .task_management["status"],
            "doing"
        );
        store.update_session_status(&scheduled.id, SessionStatusMano::Idle);
        assert!(
            store
                .claim_due_task_runs(now + chrono::Duration::minutes(1))
                .iter()
                .all(|run| run.session_id != scheduled.id),
            "scheduled task should not be claimed again after it is already doing"
        );
        assert_eq!(
            store
                .get_session(&idle.id)
                .expect("idle should exist")
                .status,
            ApiSessionStatus::Busy
        );
        assert_eq!(
            store
                .get_session(&done.id)
                .expect("done should exist")
                .task_management["status"],
            "done"
        );
        assert_eq!(
            store
                .get_session(&future_scheduled.id)
                .expect("future should exist")
                .task_management
                .get("status"),
            None
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn scheduler_claim_persists_next_polling_start() {
        let root = std::env::temp_dir().join(format!("tura-polling-task-{}", Uuid::new_v4()));
        let directory = root.to_string_lossy().to_string();
        let store = SessionStore::new();
        let now = Utc::now();
        let due = now - chrono::Duration::minutes(30);
        let session = store.create_session(
            Some(directory.clone()),
            None,
            None,
            Some("coding".to_string()),
            None,
            false,
            false,
            false,
            None,
            false,
            false,
        );
        store.update_session(
            &session.id,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            Some(serde_json::json!({
                "task_summary": "poll repo",
                "status": "todo",
                "start_condition": "polling_task",
                "start_at": due.to_rfc3339(),
                "poll_interval": { "m": 0, "d": 0, "h": 1, "s": 0 }
            })),
        );

        let claimed = store.claim_due_task_runs(now);

        assert_eq!(claimed.len(), 1);
        let updated = store
            .get_session(&session.id)
            .expect("session should exist after claim");
        let next_start = DateTime::parse_from_rfc3339(
            updated
                .task_management
                .get("start_at")
                .and_then(serde_json::Value::as_str)
                .expect("start_at should serialize"),
        )
        .expect("start_at should parse")
        .with_timezone(&Utc);
        assert!(next_start > now);

        let hydrated = SessionStore::new();
        hydrated.hydrate_directory(Some(directory));
        let persisted = hydrated
            .get_session(&session.id)
            .expect("persisted polling session should hydrate");
        assert_eq!(
            persisted.task_management["start_at"],
            updated.task_management["start_at"]
        );
        store.update_session_status(&session.id, SessionStatusMano::Idle);
        assert!(
            store.claim_due_task_runs(now).is_empty(),
            "polling task should not be reclaimed until its next start_at is due"
        );

        let _ = std::fs::remove_dir_all(root);
    }
}
