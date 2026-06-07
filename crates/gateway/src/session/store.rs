//! Session store - manages session persistence using mano state machine
//!
//! This module provides session storage functionality using the SessionInfo
//! structure that wraps SessionManagement from mano.

use crate::api::types::{GlobalEvent, Session as ApiSession, SessionStatus as ApiSessionStatus};
use crate::session::config::{load_config, merge_config, TuraSessionConfig};
use crate::session::manager::{
    agent_for_session_type, default_use_last_tool_call_response_for_session,
    normalize_session_type, runtime_provider_for_session, SessionInfo, SessionManager,
    SessionStatus as SessionStatusMano, CODING_AGENT_NAME,
};
use crate::session_db_client::SessionDbClient;
use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use runtime::state_machine::session_management::{
    PlanStatus, PollInterval, SessionState, StartCondition, TaskStep,
};
use session_log::{SessionRecord, SessionSnapshot};
use std::collections::{HashMap, HashSet};
#[cfg(test)]
use std::path::PathBuf;
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

#[derive(Debug, Clone)]
struct PersistedSessionRecord {
    info: SessionInfo,
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

#[path = "store_task_management.rs"]
mod store_task_management;
use store_task_management::{
    apply_task_management_patch, next_polling_start, task_display_summary,
    task_is_scheduler_eligible,
};

#[path = "store_frontend.rs"]
mod store_frontend;
use store_frontend::{frontend_safe_part_value, normalize_tool_message_state};

#[path = "store_messages.rs"]
mod store_messages;

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
        let client = match SessionDbClient::discover() {
            Ok(client) => client,
            Err(err) => {
                tracing::warn!(directory, error = %err, "failed to discover session_log client");
                return;
            }
        };
        let mut page = 0;
        const PAGE_SIZE: u64 = 500;
        loop {
            let (page_info, sessions) = match client.list_sessions(
                directory.clone(),
                page,
                PAGE_SIZE,
            ) {
                Ok(result) => result,
                Err(err) => {
                    tracing::warn!(directory, error = %err, "failed to hydrate sessions from session_log");
                    return;
                }
            };
            for snapshot in sessions {
                let records = match client.list_session_records(
                    snapshot.session_id.clone(),
                    0,
                    10_000,
                ) {
                    Ok((_, records)) => records,
                    Err(err) => {
                        tracing::warn!(session_id = %snapshot.session_id, error = %err, "failed to hydrate session records from session_log");
                        Vec::new()
                    }
                };
                if let Err(err) = self.load_persisted_session(snapshot, records) {
                    tracing::warn!(error = %err, "failed to load persisted session");
                }
            }
            if (page_info.page + 1) * page_info.page_size >= page_info.total {
                break;
            }
            page += 1;
        }
    }

    fn hydrate_directory_background(&self, directory: Option<String>) {
        let Some(directory) = directory else {
            return;
        };
        std::thread::spawn(move || {
            session_store().hydrate_directory(Some(directory));
        });
    }

    fn load_persisted_session(
        &self,
        snapshot: SessionSnapshot,
        records: Vec<SessionRecord>,
    ) -> Result<(), String> {
        let mut record = persisted_record_from_session_log(snapshot, records)?;
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

    fn persist_session_background(&self, session_id: &str) {
        let session_id = session_id.to_string();
        std::thread::spawn(move || {
            if let Err(err) = session_store().persist_session_result(&session_id) {
                tracing::warn!(session_id, error = %err, "failed to persist session");
            }
        });
    }

    pub fn persist_session_ack(&self, session_id: &str) -> Result<(), String> {
        self.persist_session_result(session_id)
    }

    fn persist_session_result(&self, session_id: &str) -> Result<(), String> {
        let info = self
            .sessions
            .read()
            .get(session_id)
            .cloned()
            .ok_or_else(|| "session not found".to_string())?;
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

        SessionDbClient::discover()
            .map_err(|err| err.to_string())?
            .upsert_session(
                serde_json::to_value(&record.info).map_err(|err| err.to_string())?,
                record.parent_id,
                record
                    .messages
                    .into_iter()
                    .map(serde_json::to_value)
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|err| err.to_string())?,
                record.todos,
            )
            .map_err(|err| err.to_string())?;
        Ok(())
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
            force_planning: Some(session.force_planning),
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

    pub fn take_user_commands_for_session(&self, session_id: &str) -> Vec<String> {
        let root_id = self.root_session_id(session_id);
        self.user_commands
            .write()
            .remove(&root_id)
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
                existing.directory = directory;
            }
            if existing.management.session_name.trim().is_empty() {
                existing.management.session_name =
                    name.unwrap_or_else(|| format!("Subtask {}", child_session_id));
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
            Some(CODING_AGENT_NAME.to_string()),
            Some("coding".to_string()),
        );
        info.id = child_session_id.to_string();
        info.management.session_name =
            name.unwrap_or_else(|| format!("Subtask {}", child_session_id));
        info.status = SessionStatusMano::Busy;
        info.created_at = now;
        info.updated_at = now;
        info.management.session_id = child_session_id.to_string();
        if let Some(parent) = self.sessions.read().get(parent_session_id) {
            info.disable_permission_restrictions = parent.disable_permission_restrictions;
            info.management.disable_permission_restrictions =
                parent.management.disable_permission_restrictions;
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
        kill_processes_on_start: bool,
        validator_enabled: bool,
        force_planning: bool,
        model_variant: Option<String>,
        model_acceleration_enabled: bool,
        disable_permission_restrictions: bool,
    ) -> ApiSession {
        self.hydrate_directory_background(directory.clone());
        let persisted_config = directory.as_deref().map(load_config).unwrap_or_default();
        let model = model.or(persisted_config.model.clone());
        let agent = agent.or(persisted_config.active_agent.clone());
        let session_type = session_type.or(persisted_config.session_type.clone());
        let info = SessionManager::create_session(directory, model, agent, session_type);
        let mut info = info;
        info.kill_processes_on_start = kill_processes_on_start;
        info.validator_enabled = validator_enabled;
        info.force_planning = force_planning;
        info.model_variant = model_variant.or(persisted_config.model_variant);
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
        self.persist_session_background(&session.id);

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
        kill_processes_on_start: Option<bool>,
        validator_enabled: Option<bool>,
        force_planning: Option<bool>,
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
        if let Some(kill_processes_on_start) = kill_processes_on_start {
            info.kill_processes_on_start = kill_processes_on_start;
        }
        if let Some(validator_enabled) = validator_enabled {
            info.validator_enabled = validator_enabled;
        }
        if let Some(force_planning) = force_planning {
            info.force_planning = force_planning;
        }
        if let Some(disable_permission_restrictions) = disable_permission_restrictions {
            info.disable_permission_restrictions = disable_permission_restrictions;
            info.management.disable_permission_restrictions = disable_permission_restrictions;
        }
        if let Some(task_management) = task_management {
            let mut patched = info.clone();
            match apply_task_management_patch(&mut patched, task_management) {
                Ok(()) => {
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

    pub fn update_session_auto_session_name(
        &self,
        session_id: &str,
        auto_session_name: bool,
    ) -> Option<ApiSession> {
        let parent_id = self.parent_for_child(session_id);
        let mut sessions = self.sessions.write();
        let info = sessions.get_mut(session_id)?;
        info.management.auto_session_name = auto_session_name;
        info.updated_at = Utc::now().timestamp_millis();
        let session = api_session_from_info(info, parent_id);
        drop(sessions);
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
        self.persist_session_background(session_id);
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
        management: runtime::state_machine::session_management::SessionManagement,
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
    let session_name = info.management.session_name.trim().to_string();
    let session_name = (!session_name.is_empty()).then_some(session_name);
    let session_display_name = plan_summary
        .clone()
        .or(first_task_summary)
        .or_else(|| session_name.clone())
        .or_else(|| Some("New Session".to_string()));
    ApiSession {
        id: info.id.clone(),
        name: session_name,
        parent_id,
        created_at: info.created_at,
        updated_at: info.updated_at,
        directory: info.directory.clone(),
        model: info.model.clone(),
        agent: info.agent.clone(),
        session_type: info.session_type.clone(),
        auto_session_name: info.management.auto_session_name,
        kill_processes_on_start: info.kill_processes_on_start,
        validator_enabled: info.validator_enabled,
        force_planning: info.force_planning,
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

fn persisted_record_from_session_log(
    snapshot: SessionSnapshot,
    records: Vec<SessionRecord>,
) -> Result<PersistedSessionRecord, String> {
    let mut info = serde_json::from_value::<SessionInfo>(snapshot.session.clone())
        .or_else(|_| {
            serde_json::from_value(snapshot.management.clone())
                .map(|management| SessionInfo::from_management(&management))
        })
        .map_err(|err| format!("invalid session_log session snapshot: {err}"))?;
    info.id = snapshot.session_id.clone();
    info.created_at = snapshot.created_at;
    info.updated_at = snapshot.updated_at;
    if !snapshot.workspace.trim().is_empty() {
        info.directory = Some(snapshot.workspace);
    }
    info.message_count = snapshot.message_count as usize;

    let messages = records
        .into_iter()
        .map(|record| {
            serde_json::from_value::<Message>(record.record)
                .map_err(|err| format!("invalid session_log message record: {err}"))
        })
        .collect::<Result<Vec<_>, _>>()?;

    Ok(PersistedSessionRecord {
        info,
        parent_id: snapshot.parent_id,
        messages,
        todos: snapshot.todos,
    })
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

fn random_task_id() -> String {
    Uuid::new_v4().simple().to_string()[..8].to_string()
}

#[cfg(test)]
#[path = "store_tests.rs"]
mod tests;
