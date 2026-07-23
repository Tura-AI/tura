//! Session store - caches API projections backed by the Session lifecycle service.
//!
//! `SessionInfo` contains only the typed lifecycle projection and API metadata;
//! canonical state and persistence remain owned by lifecycle and session_db.

use crate::contracts::{
    GlobalEvent, Session as ApiSession, SessionContextTokens, SessionStatus as ApiSessionStatus,
    UpdateSessionRequest as ApiUpdateSessionRequest,
};
use crate::session::config::{load_config, merge_config, TuraSessionConfig};
use crate::session::manager::{
    agent_for_session_type, default_use_last_tool_call_response_for_session,
    normalize_session_type, runtime_provider_for_session, SessionInfo, SessionManager,
    CODING_AGENT_NAME,
};
use crate::session_db_client::SessionDbClient;
use chrono::{DateTime, Utc};
use lifecycle::{PlanStatus, SessionCommand, SessionEvent, SessionProjection, StartCondition};
use parking_lot::RwLock;
use session_log_contract::{
    CreateSessionRequest as CreateSessionDbRequest, SessionCommandResult, SessionMetadataPatch,
    SessionRecordProjection, SessionSnapshot, UpdateSessionRequest as UpdateSessionDbRequest,
};
use std::collections::{HashMap, HashSet, VecDeque};
#[cfg(test)]
use std::path::PathBuf;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
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

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
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
    live_messages: Arc<RwLock<HashMap<String, Vec<LiveMessageOverlay>>>>,
    todos: Arc<RwLock<HashMap<String, Vec<serde_json::Value>>>>,
    todo_cursors: Arc<RwLock<HashMap<String, u64>>>,
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

fn session_event_ends_runtime(event: &SessionEvent) -> bool {
    matches!(
        event,
        SessionEvent::RuntimeCompleted { .. }
            | SessionEvent::RuntimeFailed { .. }
            | SessionEvent::RuntimeCancelled { .. }
            | SessionEvent::RuntimeEnded { .. }
            | SessionEvent::SessionInterrupted { .. }
            | SessionEvent::SessionCancelled { .. }
    )
}

impl EventLog {
    fn new() -> Self {
        Self {
            next_sequence: 0,
            entries: VecDeque::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScheduledTaskRun {
    pub session_id: String,
    pub task_summary: String,
    pub start_condition: StartCondition,
    pub prompt: serde_json::Value,
}

pub(crate) struct ProjectionCacheWrite {
    pub(crate) session: ApiSession,
    pub(crate) changed: bool,
    pub(crate) inserted: bool,
}

#[path = "store_task_management.rs"]
mod store_task_management;
use store_task_management::parse_task_management_patch;

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
        let store = Self::empty();
        store.init_default_session();
        store
    }

    pub(crate) fn empty() -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            messages: Arc::new(RwLock::new(HashMap::new())),
            live_messages: Arc::new(RwLock::new(HashMap::new())),
            todos: Arc::new(RwLock::new(HashMap::new())),
            todo_cursors: Arc::new(RwLock::new(HashMap::new())),
            children: Arc::new(RwLock::new(HashMap::new())),
            current_session_id: Arc::new(RwLock::new(None)),
            events: Arc::new(RwLock::new(EventLog::new())),
        }
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
        if let Err(err) = crate::session_feed::replay_directory(&client, self.clone(), &directory) {
            tracing::warn!(directory, error = %err, "failed to replay typed Session feed");
        }
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
            .map(|info| info.projection.clone())
    }

    pub fn insert_projection_cache(
        &self,
        info: SessionInfo,
        projection: SessionProjection,
        session_name: Option<String>,
        message_count: u64,
        last_user_message_at: Option<i64>,
    ) -> ApiSession {
        self.write_projection_cache(
            info,
            projection,
            session_name,
            message_count,
            last_user_message_at,
            false,
        )
        .session
    }

    fn write_projection_cache(
        &self,
        mut info: SessionInfo,
        projection: SessionProjection,
        session_name: Option<String>,
        message_count: u64,
        last_user_message_at: Option<i64>,
        only_if_absent: bool,
    ) -> ProjectionCacheWrite {
        let session_id = projection.session_id.clone();
        let parent_id = projection.parent_id.clone();
        let mut sessions = self.sessions.write();
        if only_if_absent {
            if let Some(current) = sessions.get(&session_id) {
                return ProjectionCacheWrite {
                    session: api_session_from_info(current, current.projection.parent_id.clone()),
                    changed: false,
                    inserted: false,
                };
            }
        }
        let previous = sessions
            .get(&session_id)
            .map(|current| api_session_from_info(current, current.projection.parent_id.clone()));
        info.id.clone_from(&session_id);
        info.projection = projection;
        if let Some(session_name) = session_name {
            info.name = session_name;
        }
        info.message_count = message_count as usize;
        info.last_user_message_at = last_user_message_at;
        let session = api_session_from_info(&info, parent_id.clone());
        sessions.insert(session_id.clone(), info);
        drop(sessions);
        self.messages.write().entry(session_id.clone()).or_default();
        self.todos.write().entry(session_id.clone()).or_default();
        {
            let mut current_session_id = self.current_session_id.write();
            if current_session_id.is_none() {
                current_session_id.replace(session_id.clone());
            }
        }
        self.replace_parent_cache(&session_id, parent_id);
        ProjectionCacheWrite {
            changed: previous.as_ref() != Some(&session),
            inserted: previous.is_none(),
            session,
        }
    }

    pub fn insert_snapshot_projection_cache(
        &self,
        snapshot: &SessionSnapshot,
    ) -> Result<ApiSession, String> {
        Ok(self.write_snapshot_projection_cache(snapshot)?.session)
    }

    pub(crate) fn write_snapshot_projection_cache(
        &self,
        snapshot: &SessionSnapshot,
    ) -> Result<ProjectionCacheWrite, String> {
        let info = session_info_from_snapshot(snapshot)?;
        let write = self.write_projection_cache(
            info,
            snapshot.lifecycle_projection.clone(),
            snapshot.name.clone(),
            snapshot.message_count,
            snapshot.last_user_message_at,
            false,
        );
        let mut cursors = self.todo_cursors.write();
        self.todos
            .write()
            .insert(snapshot.session_id.clone(), snapshot.todos.clone());
        cursors.remove(&snapshot.session_id);
        Ok(write)
    }

    pub(crate) fn write_feed_snapshot_projection_cache(
        &self,
        snapshot: &SessionSnapshot,
        cursor: u64,
    ) -> Result<ProjectionCacheWrite, String> {
        let write = self.write_updated_snapshot_projection_cache(snapshot)?;
        self.apply_todos_projection_at_cursor(
            &snapshot.session_id,
            cursor,
            snapshot.todos.clone(),
            false,
        );
        Ok(write)
    }

    pub(crate) fn write_updated_snapshot_projection_cache(
        &self,
        snapshot: &SessionSnapshot,
    ) -> Result<ProjectionCacheWrite, String> {
        let info = session_info_from_snapshot(snapshot)?;
        Ok(self.write_projection_cache(
            info,
            snapshot.lifecycle_projection.clone(),
            snapshot.name.clone(),
            snapshot.message_count,
            snapshot.last_user_message_at,
            false,
        ))
    }

    pub fn reduce_snapshot_projection_cache(
        &self,
        snapshot: &SessionSnapshot,
    ) -> Result<Option<ApiSession>, String> {
        let write = self.write_snapshot_projection_cache(snapshot)?;
        Ok(write.changed.then_some(write.session))
    }

    pub fn create_canonical_session(
        &self,
        info: SessionInfo,
        creation_command: SessionCommand,
    ) -> Result<ApiSession, String> {
        self.create_canonical_session_with_context(info, creation_command, false, None)
    }

    pub fn create_canonical_session_with_patch(
        &self,
        info: SessionInfo,
        creation_command: SessionCommand,
        initial_task_plan_patch: Option<lifecycle::SessionTaskPlanPatch>,
    ) -> Result<ApiSession, String> {
        self.create_canonical_session_with_context(
            info,
            creation_command,
            false,
            initial_task_plan_patch,
        )
    }

    pub fn create_canonical_fork(
        &self,
        info: SessionInfo,
        parent_id: String,
        copy_context: bool,
    ) -> Result<ApiSession, String> {
        self.create_canonical_session_with_context(
            info,
            SessionCommand::ForkSession { parent_id },
            copy_context,
            None,
        )
    }

    fn create_canonical_session_with_context(
        &self,
        info: SessionInfo,
        creation_command: SessionCommand,
        copy_context: bool,
        initial_task_plan_patch: Option<lifecycle::SessionTaskPlanPatch>,
    ) -> Result<ApiSession, String> {
        let workspace = info
            .directory
            .clone()
            .unwrap_or_else(|| info.session_directory.to_string_lossy().to_string());
        let request = CreateSessionDbRequest {
            command_id: format!("create:{}", info.id),
            session_id: info.id.clone(),
            creation_command,
            copy_context,
            workspace,
            session_directory: info.session_directory.to_string_lossy().to_string(),
            name: info.name.clone(),
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
            auto_session_name: info.auto_session_name,
            initial_task_plan_patch,
        };
        let result = SessionDbClient::discover()
            .and_then(|client| client.create_session(request))
            .map_err(|error| format!("failed to create canonical session: {error}"))?;
        let write = self.write_projection_cache(
            info,
            result.projection,
            result.session_name,
            result.message_count,
            result.last_user_message_at,
            true,
        );
        self.publish_session_created(&write);
        Ok(write.session)
    }

    pub fn delete_canonical_session(&self, session_id: &str) -> Result<Option<ApiSession>, String> {
        SessionDbClient::discover()
            .and_then(|client| client.delete_session(session_id.to_string()))
            .map_err(|error| format!("failed to delete canonical session: {error}"))?;
        Ok(self.remove_session_projection(session_id))
    }

    pub fn parse_initial_task_management(
        &self,
        task_management: serde_json::Value,
    ) -> Result<lifecycle::SessionTaskPlanPatch, String> {
        parse_task_management_patch(task_management, Utc::now())
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
        self.execute_canonical_session_update(
            session_id,
            SessionMetadataPatch::default(),
            Some(patch),
        )
    }

    fn execute_canonical_session_update(
        &self,
        session_id: &str,
        metadata: SessionMetadataPatch,
        task_plan_patch: Option<lifecycle::SessionTaskPlanPatch>,
    ) -> Result<ApiSession, String> {
        let snapshot = SessionDbClient::discover()
            .and_then(|client| {
                client.update_session(UpdateSessionDbRequest {
                    command_id: uuid::Uuid::new_v4().to_string(),
                    session_id: session_id.to_string(),
                    metadata,
                    task_plan_patch,
                })
            })
            .map_err(|error| format!("failed to update canonical session: {error}"))?;
        let write = self.write_updated_snapshot_projection_cache(&snapshot)?;
        self.persist_active_config(&write.session);
        self.publish_session_updated(&write);
        Ok(write.session)
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
        info.projection.session_id = child_session_id.to_string();
        info.name = name.unwrap_or_else(|| format!("Subtask {child_session_id}"));
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
        let refresh_error = session_event_ends_runtime(&result.event)
            .then(|| self.refresh_messages_from_session_db(session_id))
            .transpose()
            .err();
        let Some(write) = self.write_replaced_projection_cache(
            result.projection.clone(),
            result.session_name.clone(),
            None,
        ) else {
            return Err(format!(
                "session {session_id} command succeeded but projection cache is missing"
            ));
        };
        if matches!(&result.event, SessionEvent::SessionCancelled { .. }) {
            self.finish_todos(session_id, false);
        }
        self.publish_session_updated(&write);
        if let Some(error) = refresh_error {
            return Err(error);
        }
        Ok(result)
    }

    pub fn register_runtime(
        &self,
        session_id: &str,
        runtime_id: &str,
    ) -> Result<session_log_contract::RuntimeRegistrationOutcome, String> {
        let outcome = SessionDbClient::discover()
            .and_then(|client| {
                client.register_runtime(runtime_id.to_string(), session_id.to_string())
            })
            .map_err(|error| format!("failed to register runtime: {error}"))?;
        let projection = match &outcome {
            session_log_contract::RuntimeRegistrationOutcome::Registered { projection, .. }
            | session_log_contract::RuntimeRegistrationOutcome::AlreadyRegistered {
                projection,
                ..
            } => Some(projection.clone()),
            _ => None,
        };
        if let Some(projection) = projection {
            let Some(write) = self.write_replaced_projection_cache(projection, None, None) else {
                return Err(format!(
                    "session {session_id} runtime registration succeeded but projection cache is missing"
                ));
            };
            self.publish_session_updated(&write);
        }
        Ok(outcome)
    }

    pub fn execute_canonical_session_command_with_status_event(
        &self,
        session_id: &str,
        command: SessionCommand,
    ) -> Result<SessionCommandResult, String> {
        self.execute_canonical_session_command(session_id, command)
    }

    pub fn execute_canonical_session_command_with_message(
        &self,
        command_session_id: &str,
        command: SessionCommand,
        message: Message,
    ) -> Result<(SessionCommandResult, Message), String> {
        let message_projection = SessionRecordProjection {
            session_id: message.session_id.clone(),
            message_id: message.id.clone(),
            role: match message.role {
                MessageRole::User => "user",
                MessageRole::Assistant => "assistant",
                MessageRole::System => "system",
            }
            .to_string(),
            created_at: message.created_at,
            updated_at: message.updated_at,
            record: serde_json::to_value(&message)
                .map_err(|error| format!("failed to encode typed message projection: {error}"))?,
        };
        let result = SessionDbClient::discover()
            .and_then(|client| {
                client.execute_session_command_with_message(
                    command_session_id.to_string(),
                    command,
                    Some(message_projection),
                )
            })
            .map_err(|error| format!("failed to execute canonical session command: {error}"))?;
        let refresh_error = if session_event_ends_runtime(&result.event) {
            self.refresh_messages_from_session_db(command_session_id)
                .err()
        } else {
            self.upsert_feed_message(&message.session_id, message.clone());
            None
        };
        if refresh_error.is_some() {
            self.upsert_feed_message(&message.session_id, message.clone());
        }
        let Some(write) = self.write_replaced_projection_cache(
            result.projection.clone(),
            result.session_name.clone(),
            None,
        ) else {
            return Err(format!(
                "session {command_session_id} command succeeded but projection cache is missing"
            ));
        };
        self.publish_session_updated(&write);
        if let Some(error) = refresh_error {
            return Err(error);
        }
        Ok((result, message))
    }

    pub fn replace_projection_cache(
        &self,
        projection: SessionProjection,
        session_name: Option<String>,
    ) -> Option<ApiSession> {
        self.write_replaced_projection_cache(projection, session_name, None)
            .map(|write| write.session)
    }

    fn write_replaced_projection_cache(
        &self,
        projection: SessionProjection,
        session_name: Option<String>,
        updated_at: Option<i64>,
    ) -> Option<ProjectionCacheWrite> {
        let session_id = projection.session_id.clone();
        let parent_id = projection.parent_id.clone();
        let mut sessions = self.sessions.write();
        let info = sessions.get_mut(&session_id)?;
        let changed = info.projection != projection
            || session_name.as_ref().is_some_and(|name| info.name != *name);
        if changed {
            info.projection = projection;
        }
        if let Some(session_name) = session_name {
            info.name = session_name;
        }
        if let Some(updated_at) = updated_at {
            info.updated_at = updated_at;
        } else if changed {
            info.updated_at = Utc::now().timestamp_millis();
        }
        let session = api_session_from_info(info, parent_id.clone());
        drop(sessions);
        self.replace_parent_cache(&session_id, parent_id);
        Some(ProjectionCacheWrite {
            session,
            changed,
            inserted: false,
        })
    }

    pub fn reduce_projection_cache(
        &self,
        projection: SessionProjection,
        session_name: Option<String>,
        updated_at: i64,
    ) -> Option<ApiSession> {
        self.write_replaced_projection_cache(projection, session_name, Some(updated_at))
            .and_then(|write| write.changed.then_some(write.session))
    }

    pub(crate) fn write_reduced_projection_cache(
        &self,
        projection: SessionProjection,
        session_name: Option<String>,
        updated_at: i64,
    ) -> Option<ProjectionCacheWrite> {
        self.write_replaced_projection_cache(projection, session_name, Some(updated_at))
    }

    pub(crate) fn publish_session_created(&self, write: &ProjectionCacheWrite) {
        if write.inserted {
            self.push_event(GlobalEvent::SessionCreated {
                properties: crate::contracts::SessionCreatedProperties {
                    session_id: write.session.id.clone(),
                    info: write.session.clone(),
                },
            });
        }
    }

    pub(crate) fn publish_session_updated(&self, write: &ProjectionCacheWrite) {
        if write.changed {
            self.push_event(GlobalEvent::SessionUpdated {
                properties: crate::contracts::SessionUpdatedProperties {
                    session_id: write.session.id.clone(),
                    info: write.session.clone(),
                },
            });
            self.push_current_session_status_event(&write.session.id);
        }
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
        info.use_last_tool_call_response = default_use_last_tool_call_response_for_session(
            info.session_type.as_deref().unwrap_or("coding"),
            info.agent.as_deref(),
        );
        info
    }

    #[cfg(any(
        test,
        feature = "business-tests",
        feature = "os-tests",
        feature = "performance-tests"
    ))]
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
        self.execute_api_session_update(
            session_id,
            title,
            model,
            agent,
            session_type,
            kill_processes_on_start,
            validator_enabled,
            force_planning,
            disable_permission_restrictions,
            task_management,
            None,
        )
        .map_err(|error| {
            tracing::warn!(session_id, error, "canonical session update rejected");
        })
        .ok()
    }

    pub fn update_session_from_request(
        &self,
        session_id: &str,
        payload: ApiUpdateSessionRequest,
    ) -> Result<ApiSession, String> {
        self.execute_api_session_update(
            session_id,
            payload.title.or(payload.name),
            payload.model,
            payload.agent,
            payload.session_type,
            payload.kill_processes_on_start,
            payload.validator_enabled,
            payload.force_planning,
            payload.disable_permission_restrictions,
            payload.task_management,
            payload.auto_session_name,
        )
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "internal adapter preserves the existing optional HTTP patch fields"
    )]
    fn execute_api_session_update(
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
        auto_session_name: Option<bool>,
    ) -> Result<ApiSession, String> {
        let current = self
            .sessions
            .read()
            .get(session_id)
            .cloned()
            .ok_or_else(|| format!("session {session_id} not found"))?;
        let name = title
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        let agent_or_type_changed = agent.is_some() || session_type.is_some();
        let (next_agent, next_type, use_last_tool_call_response) = if agent_or_type_changed {
            let next_type =
                normalize_session_type(session_type, agent.as_deref().or(current.agent.as_deref()));
            let next_agent = agent.or_else(|| agent_for_session_type(&next_type));
            let use_last_tool_call_response =
                default_use_last_tool_call_response_for_session(&next_type, next_agent.as_deref());
            (
                Some(next_agent),
                Some(next_type),
                Some(use_last_tool_call_response),
            )
        } else {
            (None, None, None)
        };
        let model = model.or_else(|| {
            agent_or_type_changed.then(|| {
                runtime_provider_for_session(
                    next_type.as_deref().unwrap_or("coding"),
                    next_agent.as_ref().and_then(|agent| agent.as_deref()),
                )
            })?
        });
        let task_plan_patch = task_management.and_then(|task_management| {
            match parse_task_management_patch(task_management, Utc::now()) {
                Ok(patch) => Some(patch),
                Err(error) => {
                    tracing::warn!(session_id, error, "invalid task management patch ignored");
                    None
                }
            }
        });
        let (agent, clear_agent) = match next_agent {
            Some(Some(agent)) => (Some(agent), false),
            Some(None) => (None, true),
            None => (None, false),
        };
        self.execute_canonical_session_update(
            session_id,
            SessionMetadataPatch {
                name,
                model,
                agent,
                clear_agent,
                session_type: next_type,
                kill_processes_on_start,
                validator_enabled,
                force_planning,
                disable_permission_restrictions,
                use_last_tool_call_response,
                auto_session_name,
            },
            task_plan_patch,
        )
    }

    pub fn update_session_auto_session_name(
        &self,
        session_id: &str,
        auto_session_name: bool,
    ) -> Option<ApiSession> {
        self.execute_canonical_session_update(
            session_id,
            SessionMetadataPatch {
                auto_session_name: Some(auto_session_name),
                ..SessionMetadataPatch::default()
            },
            None,
        )
        .ok()
    }

    pub fn delete_session(&self, session_id: &str) -> bool {
        self.remove_session_projection(session_id).is_some()
    }

    pub(crate) fn remove_session_projection(&self, session_id: &str) -> Option<ApiSession> {
        let parent_id = self.parent_for_child(session_id);
        let info = self.sessions.write().remove(session_id)?;
        let session = api_session_from_info(&info, parent_id);
        self.messages.write().remove(session_id);
        self.live_messages.write().remove(session_id);
        let mut todo_cursors = self.todo_cursors.write();
        self.todos.write().remove(session_id);
        todo_cursors.remove(session_id);
        {
            let mut children = self.children.write();
            children.remove(session_id);
            for child_ids in children.values_mut() {
                child_ids.retain(|child_id| child_id != session_id);
            }
        }

        let replacement_current = self.sessions.read().keys().next().cloned();
        let mut current = self.current_session_id.write();
        if current.as_deref() == Some(session_id) {
            *current = replacement_current;
        }
        Some(session)
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
        if info.runtime_usage == usage {
            return false;
        }
        info.runtime_usage = usage;
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
        if info.context_tokens.input == context_tokens.input
            && info.context_tokens.limit == context_tokens.limit
        {
            return false;
        }
        info.context_tokens = lifecycle::ContextTokenStats {
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
                status: serde_json::json!({ "type": info.projection.state.ui_status() }),
                context_tokens,
                usage: session_usage_from_info(&info, context_tokens),
            },
        });
    }

    pub fn claim_due_task_runs(&self, now: DateTime<Utc>) -> Vec<ScheduledTaskRun> {
        let candidates = self
            .sessions
            .read()
            .values()
            .filter(|info| info.projection.state.ui_status() == "idle")
            .filter_map(|info| {
                info.projection
                    .task_plan
                    .detailed_tasks
                    .iter()
                    .find(|task| task.scheduler_eligible(now))
                    .map(|task| {
                        (
                            info.id.clone(),
                            task.task_id.clone(),
                            task.display_summary(&info.projection.task_plan.plan_summary),
                            task.start_condition,
                        )
                    })
            })
            .collect::<Vec<_>>();
        let mut claimed = Vec::new();
        for (session_id, task_id, task_summary, start_condition) in candidates {
            let (prompt, message) =
                self.build_scheduler_message(&session_id, &task_summary, start_condition);
            match self.execute_canonical_session_command_with_message(
                &session_id,
                SessionCommand::StartScheduledTask {
                    task_id,
                    task_summary: task_summary.clone(),
                    start_condition,
                    now,
                },
                message,
            ) {
                Ok((result, _)) => match result.event {
                    SessionEvent::ScheduledTaskClaimed {
                        task_summary: claimed_summary,
                        start_condition: claimed_condition,
                        ..
                    } if claimed_summary == task_summary
                        && claimed_condition == start_condition =>
                    {
                        claimed.push(ScheduledTaskRun {
                            session_id,
                            task_summary,
                            start_condition,
                            prompt,
                        })
                    }
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

        claimed
    }

    fn build_scheduler_message(
        &self,
        session_id: &str,
        task_summary: &str,
        start_condition: StartCondition,
    ) -> (serde_json::Value, Message) {
        let trigger = match start_condition {
            StartCondition::SessionIdle => "session became idle",
            StartCondition::ScheduledTask => "scheduled start time arrived",
            StartCondition::PollingTask => "polling interval became due",
            StartCondition::UserAction => "user action",
        };
        let part_id = format!("part_scheduler_{}", Uuid::new_v4());
        let text = format!("Continue the pending task because the {trigger}: {task_summary}");
        let prompt = serde_json::json!({
            "parts": [{
                "id": part_id,
                "type": "text",
                "text": text,
            }],
            "source": "task_scheduler",
        });
        let message = self.build_message_with_parts(
            session_id,
            MessageRole::User,
            vec![MessagePart {
                id: part_id,
                part_type: "text".to_string(),
                content: Some(text.clone()),
                text: Some(text),
                metadata: None,
                call_id: None,
                tool: None,
                state: None,
            }],
            None,
            Some(serde_json::json!({
                "kind": "task_scheduler",
                "start_condition": start_condition,
            })),
        );
        (prompt, message)
    }

    #[cfg(test)]
    pub fn replace_projection_metrics(
        &self,
        session_id: &str,
        context_tokens: lifecycle::ContextTokenStats,
        runtime_usage: serde_json::Value,
    ) {
        if let Some(info) = self.sessions.write().get_mut(session_id) {
            info.context_tokens = context_tokens;
            info.runtime_usage = runtime_usage;
            info.updated_at = Utc::now().timestamp_millis();
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
    let plan_summary = info.projection.task_plan.plan_summary.trim().to_string();
    let plan_summary = (!plan_summary.is_empty()).then_some(plan_summary);
    let first_task_summary = info
        .projection
        .task_plan
        .detailed_tasks
        .first()
        .map(|task| task.task_summary.trim().to_string())
        .filter(|value| !value.is_empty());
    let session_name = info.name.trim().to_string();
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
        auto_session_name: info.auto_session_name,
        kill_processes_on_start: info.kill_processes_on_start,
        validator_enabled: info.validator_enabled,
        force_planning: info.force_planning,
        disable_permission_restrictions: info.disable_permission_restrictions,
        model_variant: info.model_variant.clone(),
        model_acceleration_enabled: info.model_acceleration_enabled,
        status: match info.projection.state.ui_status() {
            "idle" => ApiSessionStatus::Idle,
            "busy" => ApiSessionStatus::Busy,
            _ => ApiSessionStatus::Error,
        },
        message_count: info.message_count,
        task_management: info.projection.task_management_json(
            DateTime::<Utc>::from_timestamp_millis(info.created_at)
                .unwrap_or(DateTime::<Utc>::UNIX_EPOCH),
        ),
        context_tokens: session_context_tokens(info),
        usage: session_usage_from_info(info, session_context_tokens(info)),
        plan_summary,
        session_display_name,
    }
}

fn session_task_start_at(info: &SessionInfo) -> Option<i64> {
    info.projection
        .task_plan
        .detailed_tasks
        .iter()
        .find(|task| task.status == PlanStatus::Doing)
        .or_else(|| info.projection.task_plan.detailed_tasks.first())
        .map(|task| task.start_at.timestamp_millis())
        .or(Some(info.created_at))
}

fn session_context_tokens(info: &SessionInfo) -> SessionContextTokens {
    SessionContextTokens {
        input: info.context_tokens.input,
        limit: info.context_tokens.limit,
    }
}

fn session_usage_from_info(
    info: &SessionInfo,
    context_tokens: SessionContextTokens,
) -> crate::contracts::SessionUsage {
    crate::contracts::SessionUsage::new(context_tokens, info.runtime_usage.clone())
}

fn session_info_from_snapshot(snapshot: &SessionSnapshot) -> Result<SessionInfo, String> {
    snapshot.validate()?;
    Ok(SessionInfo {
        id: snapshot.session_id.clone(),
        created_at: snapshot.created_at,
        updated_at: snapshot.updated_at,
        last_user_message_at: snapshot.last_user_message_at,
        directory: (!snapshot.workspace.trim().is_empty()).then(|| snapshot.workspace.clone()),
        model: snapshot.metadata.model.clone(),
        agent: snapshot.metadata.agent.clone(),
        session_type: Some(snapshot.metadata.session_type.clone()),
        kill_processes_on_start: snapshot.metadata.kill_processes_on_start,
        validator_enabled: snapshot.metadata.validator_enabled,
        force_planning: snapshot.metadata.force_planning,
        model_variant: snapshot.metadata.model_variant.clone(),
        model_acceleration_enabled: snapshot.metadata.model_acceleration_enabled,
        disable_permission_restrictions: snapshot.metadata.disable_permission_restrictions,
        use_last_tool_call_response: snapshot.metadata.use_last_tool_call_response,
        message_count: snapshot.message_count as usize,
        name: snapshot.name.clone().unwrap_or_default(),
        auto_session_name: snapshot.metadata.auto_session_name,
        session_directory: std::path::PathBuf::from(&snapshot.metadata.session_directory),
        projection: snapshot.lifecycle_projection.clone(),
        context_tokens: snapshot.metadata.context_tokens,
        runtime_usage: snapshot.metadata.runtime_usage.clone(),
    })
}

lazy_static::lazy_static! {
    pub static ref SESSION_STORE: SessionStore = SessionStore::empty();
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
