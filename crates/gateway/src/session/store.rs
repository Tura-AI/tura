//! Session store - manages session persistence using mano state machine
//!
//! This module provides session storage functionality using the SessionInfo
//! structure that wraps SessionManagement from mano.

use crate::contracts::{
    GlobalEvent, Session as ApiSession, SessionContextTokens, SessionStatus as ApiSessionStatus,
};
use crate::session::config::{load_config, merge_config, TuraSessionConfig};
use crate::session::manager::{
    agent_for_session_type, default_use_last_tool_call_response_for_session,
    normalize_session_type, runtime_provider_for_session, SessionInfo, SessionManager,
    SessionStatus as SessionStatusMano, CODING_AGENT_NAME,
};
use crate::session_db_client::SessionDbClient;
use chrono::{DateTime, Utc};
use lifecycle::{
    PlanStatus, PollInterval, RuntimeProjection, SessionCommand, SessionEvent, SessionProjection,
    SessionState, StartCondition, TaskStep,
};
use parking_lot::RwLock;
use session_log_contract::{
    CreateSessionRequest as CreateSessionDbRequest, SessionCommandResult, SessionRecord,
    PersistSessionPayloadRequest, SessionSnapshot,
};
use std::collections::{HashMap, HashSet, VecDeque};
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

#[derive(Clone)]
struct LiveMessageOverlay {
    runtime_id: Option<String>,
    message: Message,
}

#[derive(Clone)]
pub struct SessionStore {
    sessions: Arc<RwLock<HashMap<String, SessionInfo>>>,
    messages: Arc<RwLock<HashMap<String, Vec<Message>>>>,
    session_db_messages: Arc<RwLock<HashMap<String, Vec<Message>>>>,
    session_db_loaded: Arc<RwLock<HashSet<String>>>,
    session_db_refresh_needed: Arc<RwLock<HashSet<String>>>,
    live_messages: Arc<RwLock<HashMap<String, Vec<LiveMessageOverlay>>>>,
    todos: Arc<RwLock<HashMap<String, Vec<serde_json::Value>>>>,
    children: Arc<RwLock<HashMap<String, Vec<String>>>>,
    current_session_id: Arc<RwLock<Option<String>>>,
    events: Arc<RwLock<EventLog>>,
}

const MAX_SESSION_EVENTS: usize = 10_000;

struct EventLog {
    next_sequence: u64,
    entries: VecDeque<EventLogEntry>,
}

struct EventLogEntry {
    sequence: u64,
    event: GlobalEvent,
}

impl EventLog {
    fn new() -> Self {
        Self {
            next_sequence: 0,
            entries: VecDeque::new(),
        }
    }
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
use store_task_management::{apply_task_management_patch, parse_task_management_patch};

#[path = "store_frontend.rs"]
mod store_frontend;
#[cfg(test)]
pub(crate) use store_frontend::frontend_safe_value;
use store_frontend::normalize_tool_message_state;
pub(crate) use store_frontend::{frontend_safe_part_state, frontend_safe_part_value};

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
            session_db_messages: Arc::new(RwLock::new(HashMap::new())),
            session_db_loaded: Arc::new(RwLock::new(HashSet::new())),
            session_db_refresh_needed: Arc::new(RwLock::new(HashSet::new())),
            live_messages: Arc::new(RwLock::new(HashMap::new())),
            todos: Arc::new(RwLock::new(HashMap::new())),
            children: Arc::new(RwLock::new(HashMap::new())),
            current_session_id: Arc::new(RwLock::new(None)),
            events: Arc::new(RwLock::new(EventLog::new())),
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

    #[cfg_attr(test, allow(dead_code))]
    fn hydrate_directory_background(&self, directory: Option<String>) {
        let Some(directory) = directory else {
            return;
        };
        let store = self.clone();
        std::thread::spawn(move || {
            store.hydrate_directory(Some(directory));
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
        self.session_db_messages
            .write()
            .insert(session_id.clone(), record.messages.clone());
        self.session_db_loaded.write().insert(session_id.clone());

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

    pub fn refresh_session_db_cache(&self, session_id: &str) -> Result<Vec<Message>, String> {
        let client = SessionDbClient::discover()
            .map_err(|err| format!("failed to discover session_log client: {err}"))?;
        let Some(snapshot) = client
            .get_session(session_id.to_string())
            .map_err(|err| format!("failed to read session snapshot from session_log: {err}"))?
        else {
            self.session_db_messages.write().remove(session_id);
            self.session_db_loaded
                .write()
                .insert(session_id.to_string());
            self.session_db_refresh_needed.write().remove(session_id);
            return Ok(Vec::new());
        };
        let (_, records) = client
            .list_session_records(session_id.to_string(), 0, 10_000)
            .map_err(|err| format!("failed to read session records from session_log: {err}"))?;
        let mut record = persisted_record_from_session_log(snapshot, records)?;
        record.info.message_count = record.messages.len();
        record.info.use_last_tool_call_response = default_use_last_tool_call_response_for_session(
            record.info.session_type.as_deref().unwrap_or("coding"),
            record.info.agent.as_deref(),
        );
        record.info.management.use_last_tool_call_response =
            record.info.use_last_tool_call_response;
        let messages = record.messages.clone();

        self.sessions
            .write()
            .insert(session_id.to_string(), record.info);
        self.session_db_messages
            .write()
            .insert(session_id.to_string(), messages.clone());
        self.todos
            .write()
            .insert(session_id.to_string(), record.todos);
        self.session_db_loaded
            .write()
            .insert(session_id.to_string());
        self.session_db_refresh_needed.write().remove(session_id);
        if let Some(parent_id) = record.parent_id.filter(|value| !value.trim().is_empty()) {
            let mut children = self.children.write();
            let entry = children.entry(parent_id).or_default();
            if !entry.iter().any(|id| id == session_id) {
                entry.push(session_id.to_string());
            }
        }
        Ok(messages)
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
            active_persona: None,
            show_react_kaomoji: None,
            ..TuraSessionConfig::default()
        };
        patch.active_provider = None;
        patch.active_model = None;
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

    pub fn session_lifecycle_projection(&self, session_id: &str) -> Option<SessionProjection> {
        self.sessions
            .read()
            .get(session_id)
            .map(|info| info.management.lifecycle_projection())
    }

    pub fn insert_projection_cache(
        &self,
        mut info: SessionInfo,
        projection: SessionProjection,
        session_name: Option<String>,
    ) -> ApiSession {
        let session_id = projection.session_id.clone();
        let parent_id = projection.parent_id.clone();
        info.id.clone_from(&session_id);
        info.management.replace_lifecycle_projection(projection);
        if let Some(session_name) = session_name {
            info.management.session_name = session_name;
        }
        info.status = SessionStatusMano::from_state(info.management.state);
        let session = api_session_from_info(&info, parent_id.clone());
        self.sessions.write().insert(session_id.clone(), info);
        self.messages.write().entry(session_id.clone()).or_default();
        self.todos.write().entry(session_id.clone()).or_default();
        self.replace_parent_cache(&session_id, parent_id);
        session
    }

    pub fn create_canonical_session(
        &self,
        info: SessionInfo,
        creation_command: SessionCommand,
    ) -> Result<ApiSession, String> {
        let workspace = info.directory.clone().unwrap_or_else(|| {
            info.management
                .session_directory
                .to_string_lossy()
                .to_string()
        });
        let request = CreateSessionDbRequest {
            session_id: info.id.clone(),
            creation_command,
            workspace,
            session_directory: info
                .management
                .session_directory
                .to_string_lossy()
                .to_string(),
            name: info.management.session_name.clone(),
            created_at: info.created_at,
            model: info.model.clone(),
            agent: info.agent.clone(),
            session_type: info
                .session_type
                .clone()
                .unwrap_or_else(|| "coding".to_string()),
            kill_processes_on_start: info.kill_processes_on_start,
            validator_enabled: info.validator_enabled,
            force_planning: info.force_planning,
            model_variant: info.model_variant.clone(),
            model_acceleration_enabled: info.model_acceleration_enabled,
            disable_permission_restrictions: info.disable_permission_restrictions,
            use_last_tool_call_response: info.use_last_tool_call_response,
            auto_session_name: info.management.auto_session_name,
        };
        let result = SessionDbClient::discover()
            .and_then(|client| client.create_session(request))
            .map_err(|error| format!("failed to create canonical session: {error}"))?;
        Ok(self.insert_projection_cache(info, result.projection, result.session_name))
    }

    pub fn apply_initial_task_management(
        &self,
        info: &mut SessionInfo,
        task_management: serde_json::Value,
    ) -> Result<(), String> {
        apply_task_management_patch(info, task_management)
    }

    pub fn execute_task_management_patch(
        &self,
        session_id: &str,
        task_management: serde_json::Value,
    ) -> Result<ApiSession, String> {
        let patch = match parse_task_management_patch(task_management, Utc::now()) {
            Ok(patch) => patch,
            Err(error) => {
                tracing::warn!(session_id, error, "invalid task management patch ignored");
                return self
                    .get_session(session_id)
                    .ok_or_else(|| format!("session {session_id} not found"));
            }
        };
        self.execute_canonical_session_command(
            session_id,
            SessionCommand::ApplyTaskPlanPatch { patch },
        )?;
        self.get_session(session_id)
            .ok_or_else(|| format!("session {session_id} projection cache is missing"))
    }

    pub fn register_canonical_child_session(
        &self,
        parent_session_id: &str,
        child_session_id: &str,
        directory: Option<String>,
        name: Option<String>,
        task_instruction: Option<String>,
    ) -> Result<ApiSession, String> {
        if self.sessions.read().contains_key(child_session_id) {
            self.execute_canonical_session_command(
                child_session_id,
                SessionCommand::RegisterChildSession {
                    parent_id: parent_session_id.to_string(),
                },
            )?;
            return self
                .get_session(child_session_id)
                .ok_or_else(|| format!("session {child_session_id} projection cache is missing"));
        }

        let parent = self.sessions.read().get(parent_session_id).cloned();
        let mut info = self.build_session_info(
            directory,
            None,
            Some(CODING_AGENT_NAME.to_string()),
            Some("coding".to_string()),
            false,
            false,
            false,
            None,
            false,
            parent
                .as_ref()
                .is_some_and(|parent| parent.disable_permission_restrictions),
        );
        info.id = child_session_id.to_string();
        info.management
            .rebind_session_id(child_session_id.to_string());
        info.management.session_name =
            name.unwrap_or_else(|| format!("Subtask {child_session_id}"));
        let session = self.create_canonical_session(
            info,
            SessionCommand::RegisterChildSession {
                parent_id: parent_session_id.to_string(),
            },
        )?;
        if let Some(task_instruction) = task_instruction.filter(|value| !value.trim().is_empty()) {
            let _ = self.add_message(child_session_id, MessageRole::User, task_instruction);
        }
        Ok(session)
    }

    pub fn execute_canonical_session_command(
        &self,
        session_id: &str,
        command: SessionCommand,
    ) -> Result<SessionCommandResult, String> {
        let result = SessionDbClient::discover()
            .and_then(|client| client.execute_session_command(session_id.to_string(), command))
            .map_err(|error| format!("failed to execute canonical session command: {error}"))?;
        if matches!(&result.event, SessionEvent::SessionDeleted) {
            self.delete_session(session_id);
        } else if self
            .replace_projection_cache(result.projection.clone(), result.session_name.clone())
            .is_none()
        {
            return Err(format!(
                "session {session_id} command succeeded but projection cache is missing"
            ));
        }
        if matches!(
            &result.event,
            SessionEvent::RuntimeCompleted { .. }
                | SessionEvent::RuntimeFailed { .. }
                | SessionEvent::SessionCancelled { .. }
        ) {
            self.session_db_refresh_needed
                .write()
                .insert(session_id.to_string());
        }
        Ok(result)
    }

    pub fn execute_canonical_session_command_with_status_event(
        &self,
        session_id: &str,
        command: SessionCommand,
    ) -> Result<SessionCommandResult, String> {
        let previous = self
            .session_lifecycle_projection(session_id)
            .map(|projection| projection.state);
        let result = self.execute_canonical_session_command(session_id, command)?;
        if previous != Some(result.projection.state) {
            self.push_current_session_status_event(session_id);
        }
        Ok(result)
    }

    pub fn replace_projection_cache(
        &self,
        projection: SessionProjection,
        session_name: Option<String>,
    ) -> Option<ApiSession> {
        let session_id = projection.session_id.clone();
        let parent_id = projection.parent_id.clone();
        let mut sessions = self.sessions.write();
        let info = sessions.get_mut(&session_id)?;
        info.management.replace_lifecycle_projection(projection);
        if let Some(session_name) = session_name {
            info.management.session_name = session_name;
        }
        info.status = SessionStatusMano::from_state(info.management.state);
        info.updated_at = Utc::now().timestamp_millis();
        let session = api_session_from_info(info, parent_id.clone());
        drop(sessions);
        self.replace_parent_cache(&session_id, parent_id);
        Some(session)
    }

    fn replace_parent_cache(&self, session_id: &str, parent_id: Option<String>) {
        let mut children = self.children.write();
        for child_ids in children.values_mut() {
            child_ids.retain(|child_id| child_id != session_id);
        }
        children.retain(|_, child_ids| !child_ids.is_empty());
        if let Some(parent_id) = parent_id {
            children
                .entry(parent_id)
                .or_default()
                .push(session_id.to_string());
        }
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

    pub fn user_command_root_session_id(&self, session_id: &str) -> String {
        self.root_session_id(session_id)
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "gateway session creation mirrors the persisted session schema"
    )]
    pub fn build_session_info(
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
    ) -> SessionInfo {
        #[cfg(not(test))]
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
        info
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "test compatibility helper mirrors session creation inputs"
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
        let info = self.build_session_info(
            directory,
            model,
            agent,
            session_type,
            kill_processes_on_start,
            validator_enabled,
            force_planning,
            model_variant,
            model_acceleration_enabled,
            disable_permission_restrictions,
        );
        let session = api_session_from_info(&info, None);
        self.sessions.write().insert(info.id.clone(), info);
        self.messages.write().insert(session.id.clone(), Vec::new());
        self.todos.write().insert(session.id.clone(), Vec::new());
        self.persist_active_config(&session);
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
        if let Some(task_management) = task_management {
            if let Err(error) = self.execute_task_management_patch(session_id, task_management) {
                tracing::warn!(
                    session_id,
                    error,
                    "canonical task management patch rejected"
                );
                return None;
            }
        }
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
        info.updated_at = Utc::now().timestamp_millis();

        let session = api_session_from_info(info, parent_id);
        drop(sessions);
        self.persist_active_config(&session);
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
        Some(session)
    }

    pub fn delete_session(&self, session_id: &str) -> bool {
        if self.sessions.write().remove(session_id).is_some() {
            self.messages.write().remove(session_id);
            self.session_db_messages.write().remove(session_id);
            self.session_db_loaded.write().remove(session_id);
            self.session_db_refresh_needed.write().remove(session_id);
            self.live_messages.write().remove(session_id);
            self.todos.write().remove(session_id);
            self.children.write().remove(session_id);
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

    pub fn attach_child_session(
        &self,
        parent_session_id: &str,
        child_session_id: &str,
    ) -> Option<ApiSession> {
        if !self.sessions.read().contains_key(parent_session_id)
            || !self.sessions.read().contains_key(child_session_id)
        {
            return None;
        }
        {
            let mut children = self.children.write();
            let entry = children.entry(parent_session_id.to_string()).or_default();
            if !entry.iter().any(|id| id == child_session_id) {
                entry.push(child_session_id.to_string());
            }
        }
        self.get_session(child_session_id)
    }

    pub fn session_payload_request(
        &self,
        session_id: &str,
    ) -> Result<PersistSessionPayloadRequest, String> {
        if !self.sessions.read().contains_key(session_id) {
            return Err(format!("session {session_id} not found"));
        }
        let records = self
            .get_messages(session_id)
            .into_iter()
            .map(serde_json::to_value)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("failed to serialize messages for {session_id}: {error}"))?;
        Ok(PersistSessionPayloadRequest {
            session_id: session_id.to_string(),
            records,
            todos: self.get_todos(session_id),
        })
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

    pub fn update_session_runtime_usage(&self, session_id: &str, usage: serde_json::Value) -> bool {
        let mut sessions = self.sessions.write();
        let Some(info) = sessions.get_mut(session_id) else {
            return false;
        };
        if info.management.runtime_usage == usage {
            return false;
        }
        info.management.runtime_usage = usage;
        info.updated_at = Utc::now().timestamp_millis();
        true
    }

    pub fn update_session_context_tokens(
        &self,
        session_id: &str,
        context_tokens: crate::contracts::SessionContextTokens,
    ) -> bool {
        let mut sessions = self.sessions.write();
        let Some(info) = sessions.get_mut(session_id) else {
            return false;
        };
        if info.management.context_tokens.input == context_tokens.input
            && info.management.context_tokens.limit == context_tokens.limit
        {
            return false;
        }
        info.management.context_tokens =
            runtime::state_machine::session_management::ContextTokenStats {
                input: context_tokens.input,
                limit: context_tokens.limit,
            };
        info.updated_at = Utc::now().timestamp_millis();
        true
    }

    pub fn push_current_session_status_event(&self, session_id: &str) {
        let Some(info) = self.sessions.read().get(session_id).cloned() else {
            return;
        };
        let context_tokens = session_context_tokens(&info);
        self.push_event(GlobalEvent::SessionStatus {
            properties: crate::contracts::SessionStatusProperties {
                session_id: session_id.to_string(),
                updated_at: info.updated_at,
                status: match info.status {
                    SessionStatusMano::Idle => serde_json::json!({ "type": "idle" }),
                    SessionStatusMano::Busy => serde_json::json!({ "type": "busy" }),
                    SessionStatusMano::Error => serde_json::json!({ "type": "error" }),
                },
                context_tokens,
                usage: session_usage_from_info(&info, context_tokens),
            },
        });
    }

    pub fn claim_due_task_runs(&self, now: DateTime<Utc>) -> Vec<ScheduledTaskRun> {
        let candidate_ids = self
            .sessions
            .read()
            .values()
            .filter(|info| matches!(info.status, SessionStatusMano::Idle))
            .filter(|info| {
                info.management
                    .task_plan
                    .detailed_tasks
                    .iter()
                    .any(|task| task.scheduler_eligible(now))
            })
            .map(|info| info.id.clone())
            .collect::<Vec<_>>();
        let mut claimed = Vec::new();
        for session_id in candidate_ids {
            match self.execute_canonical_session_command(
                &session_id,
                SessionCommand::StartScheduledTask { now },
            ) {
                Ok(result) => match result.event {
                    SessionEvent::ScheduledTaskClaimed {
                        task_summary,
                        start_condition,
                        ..
                    } => claimed.push(ScheduledTaskRun {
                        session_id,
                        task_summary,
                        start_condition,
                    }),
                    event => tracing::warn!(
                        session_id,
                        event = ?event,
                        "session service returned an unexpected scheduler claim event"
                    ),
                },
                Err(error) => tracing::debug!(
                    session_id,
                    error,
                    "scheduled task candidate was not claimable"
                ),
            }
        }

        for session_id in claimed.iter().map(|run| run.session_id.clone()) {
            let (context_tokens, usage) = if let Some(session) = self.get_session(&session_id) {
                let context_tokens = session.context_tokens;
                let usage = session.usage.clone();
                self.push_event(GlobalEvent::SessionUpdated {
                    properties: crate::contracts::SessionUpdatedProperties {
                        session_id: session_id.clone(),
                        info: session,
                    },
                });
                (context_tokens, usage)
            } else {
                let context_tokens = crate::contracts::SessionContextTokens::default();
                (
                    context_tokens,
                    crate::contracts::SessionUsage::new(context_tokens, serde_json::Value::Null),
                )
            };
            self.push_event(GlobalEvent::SessionStatus {
                properties: crate::contracts::SessionStatusProperties {
                    session_id,
                    updated_at: now.timestamp_millis(),
                    status: serde_json::json!({ "type": "busy" }),
                    context_tokens,
                    usage,
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
    }

    pub fn session_count(&self) -> usize {
        self.sessions.read().len()
    }

    pub fn push_event(&self, event: GlobalEvent) {
        let mut log = self.events.write();
        let sequence = log.next_sequence;
        log.next_sequence = log.next_sequence.saturating_add(1);
        log.entries.push_back(EventLogEntry { sequence, event });
        while log.entries.len() > MAX_SESSION_EVENTS {
            log.entries.pop_front();
        }
    }

    pub fn event_cursor(&self) -> u64 {
        self.events.read().next_sequence
    }

    pub fn next_event(&self, cursor: &mut u64) -> Option<GlobalEvent> {
        let log = self.events.read();
        let first_sequence = log.entries.front()?.sequence;
        if *cursor < first_sequence {
            *cursor = first_sequence;
        }
        let index = cursor.saturating_sub(first_sequence) as usize;
        let entry = log.entries.get(index)?;
        *cursor = entry.sequence.saturating_add(1);
        Some(entry.event.clone())
    }

    pub fn pop_event(&self) -> Option<GlobalEvent> {
        self.events
            .write()
            .entries
            .pop_front()
            .map(|entry| entry.event)
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
    let session_display_name = session_name
        .clone()
        .or_else(|| plan_summary.clone())
        .or(first_task_summary)
        .or_else(|| Some("New Session".to_string()));
    ApiSession {
        id: info.id.clone(),
        name: session_name,
        parent_id,
        created_at: info.created_at,
        updated_at: info.updated_at,
        last_user_message_at: info.last_user_message_at,
        task_start_at: session_task_start_at(info),
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
        context_tokens: session_context_tokens(info),
        usage: session_usage_from_info(info, session_context_tokens(info)),
        plan_summary,
        session_display_name,
    }
}

fn last_user_message_at_in_messages(messages: &[Message]) -> Option<i64> {
    messages
        .iter()
        .filter(|message| message.role == MessageRole::User)
        .map(|message| message.updated_at.max(message.created_at))
        .max()
}

fn session_task_start_at(info: &SessionInfo) -> Option<i64> {
    info.management
        .task_plan
        .detailed_tasks
        .iter()
        .find(|task| task.status == PlanStatus::Doing)
        .or_else(|| info.management.task_plan.detailed_tasks.first())
        .map(|task| task.start_at.timestamp_millis())
        .or_else(|| Some(info.management.session_started_at.timestamp_millis()))
}

fn session_context_tokens(info: &SessionInfo) -> SessionContextTokens {
    SessionContextTokens {
        input: info.management.context_tokens.input,
        limit: info.management.context_tokens.limit,
    }
}

fn session_usage_from_info(
    info: &SessionInfo,
    context_tokens: SessionContextTokens,
) -> crate::contracts::SessionUsage {
    crate::contracts::SessionUsage::new(context_tokens, info.management.runtime_usage.clone())
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
    if let Ok(management) = serde_json::from_value(snapshot.management.clone()) {
        info.management = management;
    }
    apply_session_log_snapshot_lifecycle(&mut info, &snapshot);
    info.id = snapshot.session_id.clone();
    info.created_at = snapshot.created_at;
    info.updated_at = snapshot.updated_at;
    info.last_user_message_at = snapshot.last_user_message_at;
    if let Some(last_user_message_at) = snapshot
        .last_user_message_at
        .and_then(DateTime::<Utc>::from_timestamp_millis)
    {
        info.management.session_last_user_message_at = last_user_message_at;
    }
    if !snapshot.workspace.trim().is_empty() {
        info.directory = Some(snapshot.workspace);
    }
    info.message_count = snapshot.message_count as usize;

    // Only user/assistant/system records are conversation messages. The runtime
    // also persists auxiliary records (log / tool / runtime / event checkpoints)
    // that are not `Message`s; skip any record that does not deserialize rather
    // than failing the whole session load (a single such record must not make a
    // session invisible to the gateway).
    let messages = records
        .into_iter()
        .filter_map(|record| match serde_json::from_value::<Message>(record.record) {
            Ok(message) => Some(message),
            Err(err) => {
                tracing::debug!(error = %err, "skipping non-message session_log record during hydration");
                None
            }
        })
        .collect::<Vec<_>>();

    Ok(PersistedSessionRecord {
        info,
        parent_id: snapshot.parent_id,
        messages,
        todos: snapshot.todos,
    })
}

fn apply_session_log_snapshot_lifecycle(info: &mut SessionInfo, snapshot: &SessionSnapshot) {
    if let Some(projection) = snapshot.lifecycle_projection.clone() {
        info.status = SessionStatusMano::from_state(projection.state);
        info.management.replace_lifecycle_projection(projection);
    } else if let Some(state) = snapshot.state.as_deref().and_then(session_state_from_text) {
        info.management.restore_state(state);
        info.status = SessionStatusMano::from_state(state);
    } else if let Some(status) = snapshot
        .status
        .as_deref()
        .and_then(session_status_from_text)
    {
        info.status = status;
        info.management
            .restore_state(representative_state_for_status(
                status,
                info.management.state,
            ));
    } else {
        info.status = SessionStatusMano::from_state(info.management.state);
    }

    if let Some(created_at) = DateTime::<Utc>::from_timestamp_millis(snapshot.created_at) {
        info.management.session_created_at = created_at;
    }
    if let Some(updated_at) = DateTime::<Utc>::from_timestamp_millis(snapshot.updated_at) {
        info.management.session_last_update_at = updated_at;
    }
}

fn session_state_from_text(value: &str) -> Option<SessionState> {
    serde_json::from_value(serde_json::Value::String(value.trim().to_ascii_lowercase())).ok()
}

fn session_status_from_text(value: &str) -> Option<SessionStatusMano> {
    match value.trim().to_ascii_lowercase().as_str() {
        "idle" => Some(SessionStatusMano::Idle),
        "busy" => Some(SessionStatusMano::Busy),
        "error" => Some(SessionStatusMano::Error),
        _ => None,
    }
}

fn representative_state_for_status(
    status: SessionStatusMano,
    current: SessionState,
) -> SessionState {
    match status {
        SessionStatusMano::Idle
            if matches!(current, SessionState::Created | SessionState::Completed) =>
        {
            current
        }
        SessionStatusMano::Idle => SessionState::Created,
        SessionStatusMano::Busy
            if matches!(current, SessionState::Running | SessionState::Paused) =>
        {
            current
        }
        SessionStatusMano::Busy => SessionState::Running,
        SessionStatusMano::Error
            if matches!(
                current,
                SessionState::Failed | SessionState::Cancelled | SessionState::Interrupted
            ) =>
        {
            current
        }
        SessionStatusMano::Error => SessionState::Failed,
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

fn random_task_id() -> String {
    Uuid::new_v4().simple().to_string()[..8].to_string()
}

#[cfg(test)]
#[path = "store_tests.rs"]
mod tests;
