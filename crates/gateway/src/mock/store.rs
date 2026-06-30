//! Mock data store for API testing
//! This module provides in-memory mock data that simulates the OpenCode backend

use crate::contracts::*;
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
            last_user_message_at: None,
            task_start_at: Some(now),
            directory: None,
            model: Some(coding_agent_provider()),
            agent: Some(CODING_AGENT_NAME.to_string()),
            session_type: Some("coding".to_string()),
            auto_session_name: true,
            kill_processes_on_start: false,
            validator_enabled: false,
            force_planning: false,
            disable_permission_restrictions: false,
            model_variant: Some(DEFAULT_SESSION_REASONING_EFFORT.to_string()),
            model_acceleration_enabled: true,
            status: SessionStatus::Idle,
            message_count: 0,
            task_management: serde_json::json!({}),
            context_tokens: SessionContextTokens::default(),
            usage: Default::default(),
            plan_summary: None,
            session_display_name: None,
        };
        self.sessions.write().insert(session_id.clone(), session);

        // Create a welcome message
        let message_id = new_message_id(now);
        let message = Message {
            id: message_id.clone(),
            session_id: session_id.clone(),
            role: MessageRole::Assistant,
            parent_id: None,
            parts: vec![MessagePart {
                id: Uuid::new_v4().to_string(),
                session_id: session_id.clone(),
                message_id,
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
            last_user_message_at: None,
            task_start_at: Some(now),
            directory: directory.clone(),
            model: Some(coding_agent_provider()),
            agent: Some(CODING_AGENT_NAME.to_string()),
            session_type: Some("coding".to_string()),
            auto_session_name: true,
            kill_processes_on_start: false,
            validator_enabled: false,
            force_planning: false,
            disable_permission_restrictions: false,
            model_variant: Some(DEFAULT_SESSION_REASONING_EFFORT.to_string()),
            model_acceleration_enabled: true,
            status: SessionStatus::Idle,
            message_count: 0,
            task_management: serde_json::json!({}),
            context_tokens: SessionContextTokens::default(),
            usage: Default::default(),
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
            id: message_id.clone(),
            session_id: session_id.to_string(),
            role,
            parent_id,
            parts: vec![MessagePart {
                id: part_id,
                session_id: session_id.to_string(),
                message_id,
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
            worktree,
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

    pub fn peek_oauth_state(&self, provider_id: &str) -> Option<PendingOAuth> {
        self.pending_oauth
            .read()
            .get(provider_id)
            .filter(|pending| pending.expires_at > Utc::now().timestamp_millis())
            .cloned()
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
    // Gateway-specific operations.
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

    #[cfg(test)]
    pub fn clear_current_directory(&self) {
        *self.current_directory.write() = None;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contracts::{ConfigPatch, MessageRole, ProviderAuth, SessionStatus};

    fn auth(auth_type: &str) -> ProviderAuth {
        ProviderAuth {
            auth_type: auth_type.to_string(),
            key: Some("secret".to_string()),
            access: None,
            refresh: None,
            expires: None,
            account_id: None,
            metadata: None,
        }
    }

    #[test]
    fn new_store_contains_welcome_session_message_and_default_providers() {
        let store = Store::new();

        let sessions = store.list_sessions();
        assert_eq!(sessions.len(), 1);
        let session = &sessions[0];
        assert_eq!(session.name.as_deref(), Some("Welcome Session"));
        assert_eq!(session.status, SessionStatus::Idle);
        assert_eq!(session.message_count, 0);
        assert_eq!(session.agent.as_deref(), Some(CODING_AGENT_NAME));

        let messages = store.get_messages(&session.id);
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].role, MessageRole::Assistant);
        assert!(messages[0].parts[0]
            .text
            .as_deref()
            .unwrap_or_default()
            .contains("ready to help"));

        let providers = store
            .list_providers()
            .into_iter()
            .map(|provider| (provider.id, provider.enabled))
            .collect::<HashMap<_, _>>();
        assert_eq!(providers.get("openai"), Some(&false));
        assert_eq!(providers.get("openai-api"), Some(&false));
        assert_eq!(providers.get("anthropic"), Some(&false));
        assert_eq!(providers.get("antigravity"), Some(&false));
    }

    #[test]
    fn create_session_tracks_current_directory_and_initializes_message_bucket() {
        let store = Store::new();

        let session = store.create_session(Some("C:/work/project".to_string()), None, None);

        assert_eq!(session.name.as_deref(), Some("New Session"));
        assert_eq!(session.directory.as_deref(), Some("C:/work/project"));
        assert_eq!(
            session.model.as_deref(),
            Some(coding_agent_provider().as_str())
        );
        assert_eq!(session.agent.as_deref(), Some(CODING_AGENT_NAME));
        assert_eq!(
            store.get_current_directory().as_deref(),
            Some("C:/work/project")
        );
        assert!(store.get_messages(&session.id).is_empty());
        assert_eq!(
            store.get_session(&session.id).map(|item| item.id),
            Some(session.id)
        );
    }

    #[test]
    fn delete_session_removes_messages_and_is_idempotent_success() {
        let store = Store::new();
        let session = store.create_session(None, None, None);
        store.add_message(&session.id, MessageRole::User, "hello".to_string());
        assert!(!store.get_messages(&session.id).is_empty());

        assert!(store.delete_session(&session.id));
        assert!(store.get_session(&session.id).is_none());
        assert!(store.get_messages(&session.id).is_empty());
        assert!(store.delete_session(&session.id));
    }

    #[test]
    fn add_message_sets_assistant_parent_to_latest_user_and_updates_count() {
        let store = Store::new();
        let session = store.create_session(None, None, None);

        let user_1 = store.add_message(&session.id, MessageRole::User, "first".to_string());
        let assistant_1 =
            store.add_message(&session.id, MessageRole::Assistant, "reply".to_string());
        let user_2 = store.add_message(&session.id, MessageRole::User, "second".to_string());
        let assistant_2 =
            store.add_message(&session.id, MessageRole::Assistant, "reply 2".to_string());

        assert_eq!(assistant_1.parent_id.as_deref(), Some(user_1.id.as_str()));
        assert_eq!(assistant_2.parent_id.as_deref(), Some(user_2.id.as_str()));
        assert_eq!(store.get_messages(&session.id).len(), 4);
        assert_eq!(
            store
                .get_session(&session.id)
                .map(|session| session.message_count),
            Some(4)
        );
    }

    #[test]
    fn adding_message_to_unknown_session_creates_message_bucket_without_session() {
        let store = Store::new();

        let message = store.add_message("missing-session", MessageRole::User, "hello".to_string());

        assert_eq!(message.session_id, "missing-session");
        assert_eq!(store.get_messages("missing-session").len(), 1);
        assert!(store.get_session("missing-session").is_none());
    }

    #[test]
    fn update_session_status_is_noop_for_missing_session() {
        let store = Store::new();
        let session = store.create_session(None, None, None);

        store.update_session_status(&session.id, SessionStatus::Busy);
        assert_eq!(
            store.get_session(&session.id).map(|session| session.status),
            Some(SessionStatus::Busy)
        );

        store.update_session_status("missing", SessionStatus::Idle);
        assert!(store.get_session("missing").is_none());
    }

    #[test]
    fn projects_config_and_current_directory_are_mutated_independently() {
        let store = Store::new();

        let project = store.add_project("C:/repo".to_string(), Some("Repo".to_string()));
        assert_eq!(
            store.get_project(&project.id).map(|item| item.worktree),
            Some("C:/repo".to_string())
        );
        assert_eq!(store.list_projects().len(), 1);

        let updated = store.update_config(ConfigPatch {
            language: Some("zh-CN".to_string()),
            theme: Some("dark".to_string()),
            model: Some("gpt-5.5".to_string()),
            agent: Some("coding".to_string()),
            skill_folders: Some(vec!["skills".to_string()]),
        });
        assert_eq!(updated.language.as_deref(), Some("zh-CN"));
        assert_eq!(updated.theme.as_deref(), Some("dark"));
        assert_eq!(updated.model.as_deref(), Some("gpt-5.5"));
        assert_eq!(updated.agent.as_deref(), Some("coding"));
        assert_eq!(updated.skill_folders, vec!["skills"]);

        store.set_current_directory("D:/other".to_string());
        assert_eq!(store.get_current_directory().as_deref(), Some("D:/other"));
    }

    #[test]
    fn provider_auth_updates_existing_provider_and_rejects_unknown_provider() {
        let store = Store::new();

        assert!(store.set_auth("openai", auth("api_key")));
        let openai = store
            .list_providers()
            .into_iter()
            .find(|provider| provider.id == "openai")
            .expect("openai provider");
        assert!(openai.enabled);
        assert_eq!(openai.auth_type, "api_key");

        assert!(!store.set_auth("missing", auth("api_key")));
        assert!(store.remove_auth("missing"));
    }

    #[test]
    fn oauth_state_peek_consume_and_state_lookup_are_single_use() {
        let store = Store::new();
        store.set_oauth_state(
            "codex",
            "oauth_pkce".to_string(),
            Some("code".to_string()),
            "http://localhost/callback".to_string(),
            Some("state-1".to_string()),
            Some("verifier".to_string()),
        );

        assert_eq!(
            store.pending_oauth_method("codex").as_deref(),
            Some("oauth_pkce")
        );
        assert_eq!(
            store.peek_oauth_state("codex").and_then(|state| state.code),
            Some("code".to_string())
        );
        let (provider_id, pending) = store
            .consume_oauth_state_by_state("state-1")
            .expect("consume by state");
        assert_eq!(provider_id, "codex");
        assert_eq!(pending.code_verifier.as_deref(), Some("verifier"));
        assert!(store.consume_oauth_state("codex").is_none());
        assert!(store.peek_oauth_state("codex").is_none());
        assert!(store.consume_oauth_state_by_state("state-1").is_none());
    }

    #[test]
    fn completed_oauth_auth_is_single_use() {
        let store = Store::new();

        store.set_oauth_completed("codex", auth("oauth"));
        assert_eq!(
            store
                .consume_oauth_completed("codex")
                .map(|auth| auth.auth_type),
            Some("oauth".to_string())
        );
        assert!(store.consume_oauth_completed("codex").is_none());
    }

    #[test]
    fn outbound_actions_are_append_read_and_clear() {
        let store = Store::new();

        store.add_outbound_action(OutboundAction::Typing);
        store.add_outbound_action(OutboundAction::SendText {
            text: "hello".to_string(),
            reply_to_message_id: Some("msg-1".to_string()),
        });

        let actions = store.get_outbound_actions();
        assert_eq!(actions.len(), 2);
        assert!(matches!(actions[0], OutboundAction::Typing));
        assert!(matches!(
            &actions[1],
            OutboundAction::SendText {
                text,
                reply_to_message_id: Some(reply)
            } if text == "hello" && reply == "msg-1"
        ));

        store.clear_outbound_actions();
        assert!(store.get_outbound_actions().is_empty());
    }

    #[test]
    fn get_or_create_session_reuses_existing_then_creates_when_empty() {
        let store = Store::new();
        let existing = store.get_or_create_session();
        assert_eq!(store.get_or_create_session().id, existing.id);

        for session in store.list_sessions() {
            store.delete_session(&session.id);
        }
        let created = store.get_or_create_session();
        assert!(store.get_session(&created.id).is_some());
    }

    #[test]
    fn cloned_store_handles_concurrent_message_writes() {
        let store = Store::new();
        let session = store.create_session(None, None, None);
        let mut handles = Vec::new();

        for index in 0..16 {
            let store = store.clone();
            let session_id = session.id.clone();
            handles.push(std::thread::spawn(move || {
                store.add_message(&session_id, MessageRole::User, format!("message {index}"));
            }));
        }

        for handle in handles {
            handle.join().expect("writer thread should not panic");
        }

        assert_eq!(store.get_messages(&session.id).len(), 16);
        assert_eq!(
            store
                .get_session(&session.id)
                .map(|session| session.message_count),
            Some(16)
        );
    }
}
