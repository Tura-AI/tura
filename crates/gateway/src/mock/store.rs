//! Mock data store for API testing
//! This module provides in-memory mock data that simulates the OpenCode backend

use crate::api::types::*;
use crate::session::config::DEFAULT_SESSION_REASONING_EFFORT;
use crate::session::manager::{coding_agent_provider, CODING_AGENT_NAME};
use crate::types::OutboundAction;
use chrono::Utc;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

// ============================================================================
// Store
// ============================================================================

#[derive(Debug, Clone)]
pub struct Store {
    pub sessions: Arc<RwLock<HashMap<String, Session>>>,
    pub messages: Arc<RwLock<HashMap<String, Vec<Message>>>>,
    pub projects: Arc<RwLock<HashMap<String, Project>>>,
    pub providers: Arc<RwLock<HashMap<String, Provider>>>,
    pub permissions: Arc<RwLock<HashMap<String, PermissionRequest>>>,
    pub permission_replies: Arc<RwLock<HashMap<String, bool>>>,
    pub questions: Arc<RwLock<HashMap<String, QuestionRequest>>>,
    pub config: Arc<RwLock<Config>>,
    pub current_directory: Arc<RwLock<Option<String>>>,
    pub outbound_actions: Arc<RwLock<Vec<OutboundAction>>>,
    pub pending_oauth: Arc<RwLock<HashMap<String, PendingOAuth>>>,
    pub completed_oauth: Arc<RwLock<HashMap<String, ProviderAuth>>>,
}

impl Default for Store {
    fn default() -> Self {
        Self::new()
    }
}

impl Store {
    pub fn new() -> Self {
        let store = Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            messages: Arc::new(RwLock::new(HashMap::new())),
            projects: Arc::new(RwLock::new(HashMap::new())),
            providers: Arc::new(RwLock::new(HashMap::new())),
            permissions: Arc::new(RwLock::new(HashMap::new())),
            permission_replies: Arc::new(RwLock::new(HashMap::new())),
            questions: Arc::new(RwLock::new(HashMap::new())),
            config: Arc::new(RwLock::new(Config::default())),
            current_directory: Arc::new(RwLock::new(None)),
            outbound_actions: Arc::new(RwLock::new(Vec::new())),
            pending_oauth: Arc::new(RwLock::new(HashMap::new())),
            completed_oauth: Arc::new(RwLock::new(HashMap::new())),
        };
        store.init_mock_data();
        store
    }

    fn init_mock_data(&self) {
        // Create a default session
        let session_id = Uuid::new_v4().to_string();
        let now = Utc::now().timestamp_millis();

        let session = Session {
            id: session_id.clone(),
            name: Some("Welcome Session".to_string()),
            parent_id: None,
            created_at: now,
            updated_at: now,
            directory: None,
            model: Some(coding_agent_provider()),
            agent: Some(CODING_AGENT_NAME.to_string()),
            session_type: Some("coding".to_string()),
            lsp: Some(serde_json::json!({"mode":"auto","enabled":[],"disabled":[]})),
            kill_processes_on_start: false,
            validator_enabled: false,
            force_multiple_tasks: false,
            disable_permission_restrictions: false,
            model_variant: Some(DEFAULT_SESSION_REASONING_EFFORT.to_string()),
            model_acceleration_enabled: true,
            status: SessionStatus::Idle,
            message_count: 0,
            task_management: serde_json::json!({}),
            plan_summary: None,
            session_display_name: None,
        };
        self.sessions.write().insert(session_id.clone(), session);

        // Create a welcome message
        let message_id = new_message_id(now);
        let message = Message {
            id: message_id,
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
        self.messages.write().insert(session_id, vec![message]);

        // Create a default project (if directory is set)
        // ... (projects can be added when directory is known)

        // Create default providers
        let default_providers = vec![
            Provider {
                id: "openai".to_string(),
                name: "OpenAI Codex".to_string(),
                auth_type: "oauth".to_string(),
                enabled: false,
            },
            Provider {
                id: "openai-api".to_string(),
                name: "OpenAI API".to_string(),
                auth_type: "api_key".to_string(),
                enabled: false,
            },
            Provider {
                id: "anthropic".to_string(),
                name: "Claude Oauth".to_string(),
                auth_type: "oauth".to_string(),
                enabled: false,
            },
            Provider {
                id: "anthropic-api".to_string(),
                name: "Anthropic API".to_string(),
                auth_type: "api_key".to_string(),
                enabled: false,
            },
            Provider {
                id: "antigravity".to_string(),
                name: "Antigravity Oauth".to_string(),
                auth_type: "oauth".to_string(),
                enabled: false,
            },
            Provider {
                id: "antigravity-api".to_string(),
                name: "Antigravity API".to_string(),
                auth_type: "api_key".to_string(),
                enabled: false,
            },
        ];
        let mut providers = self.providers.write();
        for provider in default_providers {
            providers.insert(provider.id.clone(), provider);
        }
    }

    // ========================================================================
    // Session Operations
    // ========================================================================

    pub fn list_sessions(&self) -> Vec<Session> {
        self.sessions.read().values().cloned().collect()
    }

    pub fn get_session(&self, session_id: &str) -> Option<Session> {
        self.sessions.read().get(session_id).cloned()
    }

    pub fn create_session(
        &self,
        directory: Option<String>,
        _model: Option<String>,
        _agent: Option<String>,
    ) -> Session {
        let session_id = Uuid::new_v4().to_string();
        let now = Utc::now().timestamp_millis();

        let session = Session {
            id: session_id.clone(),
            name: Some("New Session".to_string()),
            parent_id: None,
            created_at: now,
            updated_at: now,
            directory: directory.clone(),
            model: Some(coding_agent_provider()),
            agent: Some(CODING_AGENT_NAME.to_string()),
            session_type: Some("coding".to_string()),
            lsp: Some(serde_json::json!({"mode":"auto","enabled":[],"disabled":[]})),
            kill_processes_on_start: false,
            validator_enabled: false,
            force_multiple_tasks: false,
            disable_permission_restrictions: false,
            model_variant: Some(DEFAULT_SESSION_REASONING_EFFORT.to_string()),
            model_acceleration_enabled: true,
            status: SessionStatus::Idle,
            message_count: 0,
            task_management: serde_json::json!({}),
            plan_summary: None,
            session_display_name: None,
        };

        self.sessions
            .write()
            .insert(session_id.clone(), session.clone());
        self.messages.write().insert(session_id, Vec::new());

        if let Some(dir) = directory {
            *self.current_directory.write() = Some(dir);
        }

        session
    }

    pub fn delete_session(&self, session_id: &str) -> bool {
        let _ = self.sessions.write().remove(session_id).is_some();
        self.messages.write().remove(session_id);
        true
    }

    pub fn get_messages(&self, session_id: &str) -> Vec<Message> {
        self.messages
            .read()
            .get(session_id)
            .cloned()
            .unwrap_or_default()
    }

    pub fn add_message(&self, session_id: &str, role: MessageRole, content: String) -> Message {
        let now = Utc::now().timestamp_millis();
        let message_id = new_message_id(now);

        let part_id = Uuid::new_v4().to_string();
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
            id: message_id,
            session_id: session_id.to_string(),
            role,
            parent_id,
            parts: vec![MessagePart {
                id: part_id,
                part_type: "text".to_string(),
                content: Some(content.clone()),
                text: Some(content),
                metadata: None,
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

        // Update session
        if let Some(session) = self.sessions.write().get_mut(session_id) {
            session.message_count = session_messages.len();
            session.updated_at = now;
        }

        message
    }

    pub fn update_session_status(&self, session_id: &str, status: SessionStatus) {
        if let Some(session) = self.sessions.write().get_mut(session_id) {
            session.status = status;
            session.updated_at = Utc::now().timestamp_millis();
        }
    }

    // ========================================================================
    // Project Operations
    // ========================================================================

    pub fn list_projects(&self) -> Vec<Project> {
        self.projects.read().values().cloned().collect()
    }

    pub fn get_project(&self, project_id: &str) -> Option<Project> {
        self.projects.read().get(project_id).cloned()
    }

    pub fn add_project(&self, worktree: String, name: Option<String>) -> Project {
        let project_id = Uuid::new_v4().to_string();
        let now = Utc::now().timestamp_millis();

        let project = Project {
            id: project_id.clone(),
            worktree: worktree.clone(),
            vcs: Some("git".to_string()),
            name,
            icon: None,
            time: ProjectTime {
                created: now,
                updated: now,
                initialized: None,
            },
        };

        self.projects.write().insert(project_id, project.clone());
        project
    }

    // ========================================================================
    // Config Operations
    // ========================================================================

    pub fn get_config(&self) -> Config {
        self.config.read().clone()
    }

    pub fn update_config(&self, patch: ConfigPatch) -> Config {
        let mut config = self.config.write();

        if let Some(language) = patch.language {
            config.language = Some(language);
        }
        if let Some(theme) = patch.theme {
            config.theme = Some(theme);
        }
        if let Some(model) = patch.model {
            config.model = Some(model);
        }
        if let Some(agent) = patch.agent {
            config.agent = Some(agent);
        }
        if let Some(skill_folders) = patch.skill_folders {
            config.skill_folders = skill_folders;
        }

        config.clone()
    }

    // ========================================================================
    // Provider Operations
    // ========================================================================

    pub fn list_providers(&self) -> Vec<Provider> {
        self.providers.read().values().cloned().collect()
    }

    pub fn set_auth(&self, provider_id: &str, auth: ProviderAuth) -> bool {
        if let Some(provider) = self.providers.write().get_mut(provider_id) {
            // In real implementation, would store the auth securely
            provider.auth_type = auth.auth_type;
            provider.enabled = true;
            return true;
        }
        false
    }

    pub fn remove_auth(&self, _provider_id: &str) -> bool {
        // In real implementation, would remove stored auth
        true
    }

    pub fn set_oauth_state(
        &self,
        provider_id: &str,
        method: String,
        code: Option<String>,
        url: String,
        state: Option<String>,
        code_verifier: Option<String>,
    ) {
        let now = Utc::now().timestamp_millis();
        self.pending_oauth.write().insert(
            provider_id.to_string(),
            PendingOAuth {
                method,
                code,
                url,
                state,
                code_verifier,
                expires_at: now + 15 * 60_000,
            },
        );
    }

    pub fn consume_oauth_state(&self, provider_id: &str) -> Option<PendingOAuth> {
        self.pending_oauth
            .write()
            .remove(provider_id)
            .filter(|pending| pending.expires_at > Utc::now().timestamp_millis())
    }

    pub fn pending_oauth_method(&self, provider_id: &str) -> Option<String> {
        self.pending_oauth
            .read()
            .get(provider_id)
            .filter(|pending| pending.expires_at > Utc::now().timestamp_millis())
            .map(|pending| pending.method.clone())
    }

    pub fn consume_oauth_state_by_state(&self, state: &str) -> Option<(String, PendingOAuth)> {
        let mut pending = self.pending_oauth.write();
        let provider_id = pending
            .iter()
            .find(|(_, value)| {
                value.state.as_deref() == Some(state)
                    && value.expires_at > Utc::now().timestamp_millis()
            })
            .map(|(provider_id, _)| provider_id.clone())?;
        let value = pending.remove(&provider_id)?;
        Some((provider_id, value))
    }

    pub fn set_oauth_completed(&self, provider_id: &str, auth: ProviderAuth) {
        self.completed_oauth
            .write()
            .insert(provider_id.to_string(), auth);
    }

    pub fn consume_oauth_completed(&self, provider_id: &str) -> Option<ProviderAuth> {
        self.completed_oauth.write().remove(provider_id)
    }

    // ========================================================================
    // Permission Operations
    // ========================================================================

    pub fn create_permission(
        &self,
        session_id: String,
        permission: String,
        args: HashMap<String, serde_json::Value>,
    ) -> PermissionRequest {
        let id = Uuid::new_v4().to_string();
        let request = PermissionRequest {
            id: id.clone(),
            session_id,
            permission,
            args,
        };
        self.permissions.write().insert(id, request.clone());
        request
    }

    pub fn reply_permission(&self, request_id: &str, approve: bool) -> bool {
        let existed = self.permissions.write().remove(request_id).is_some();
        if existed {
            self.permission_replies
                .write()
                .insert(request_id.to_string(), approve);
        }
        existed
    }

    pub fn permission_reply(&self, request_id: &str) -> Option<bool> {
        self.permission_replies.read().get(request_id).copied()
    }

    pub fn list_permissions(&self, session_id: &str) -> Vec<PermissionRequest> {
        self.permissions
            .read()
            .values()
            .filter(|p| p.session_id == session_id)
            .cloned()
            .collect()
    }

    // ========================================================================
    // Question Operations
    // ========================================================================

    pub fn create_question(
        &self,
        session_id: String,
        question: String,
        metadata: HashMap<String, serde_json::Value>,
    ) -> QuestionRequest {
        let id = Uuid::new_v4().to_string();
        let request = QuestionRequest {
            id: id.clone(),
            session_id,
            question,
            metadata,
        };
        self.questions.write().insert(id, request.clone());
        request
    }

    pub fn reply_question(&self, request_id: &str, _response: &str) -> bool {
        self.questions.write().remove(request_id);
        // In real implementation, would send the response to the agent
        true
    }

    pub fn reject_question(&self, request_id: &str) -> bool {
        self.questions.write().remove(request_id);
        true
    }

    // ========================================================================
    // Gateway-specific Operations (保留原有功能)
    // ========================================================================

    pub fn add_outbound_action(&self, action: OutboundAction) {
        self.outbound_actions.write().push(action);
    }

    pub fn get_outbound_actions(&self) -> Vec<OutboundAction> {
        self.outbound_actions.read().clone()
    }

    pub fn clear_outbound_actions(&self) {
        self.outbound_actions.write().clear();
    }

    pub fn get_or_create_session(&self) -> Session {
        let sessions = self.sessions.read();
        if let Some(session) = sessions.values().next() {
            return session.clone();
        }
        drop(sessions);
        self.create_session(None, None, None)
    }

    pub fn set_current_directory(&self, directory: String) {
        *self.current_directory.write() = Some(directory);
    }

    pub fn get_current_directory(&self) -> Option<String> {
        self.current_directory.read().clone()
    }
}

fn new_message_id(now: i64) -> String {
    format!("msg-{now:013}-{}", Uuid::new_v4())
}

#[derive(Debug, Clone)]
pub struct PendingOAuth {
    pub method: String,
    pub code: Option<String>,
    pub url: String,
    pub state: Option<String>,
    pub code_verifier: Option<String>,
    pub expires_at: i64,
}

// ============================================================================
// Global Store Instance
// ============================================================================

lazy_static::lazy_static! {
    pub static ref GLOBAL_STORE: Store = Store::new();
}

pub fn global_store() -> &'static Store {
    &GLOBAL_STORE
}
