//! Session management using mano state machine
//!
//! This module provides session creation and management using the mano service's
//! session state machine (SessionManagement, SessionState, etc.)

use chrono::Utc;
use code_tools_suite::state_machine::session_management::{
    SessionId, SessionInput, SessionManagement, SessionName, SessionState, UserGoal,
};
use std::path::PathBuf;
use uuid::Uuid;

use crate::session::config::DEFAULT_SESSION_REASONING_EFFORT;

const DEFAULT_SESSION_DIRECTORY: &str = "sessions";
pub const CODING_AGENT_NAME: &str = "coding_agent";
pub const CODING_AGENT_FAST_NAME: &str = "coding_agent_fast";

pub fn coding_agent_provider() -> String {
    code_tools_suite::agent_router::coding_agent_provider_name()
}

pub struct SessionManager;

impl SessionManager {
    pub fn create_session(
        directory: Option<String>,
        model: Option<String>,
        agent: Option<String>,
        session_type: Option<String>,
    ) -> SessionInfo {
        let now = Utc::now();
        let session_id = Self::generate_session_id();
        let session_name = format!("Session-{}", now.format("%Y%m%d%H%M%S"));

        let dir_clone = directory.clone();
        let session_directory = directory.map(PathBuf::from).unwrap_or_else(|| {
            std::env::current_dir()
                .unwrap_or_default()
                .join(DEFAULT_SESSION_DIRECTORY)
        });

        let user_goal: UserGoal = String::new();

        let session_type = normalize_session_type(session_type, agent.as_deref());
        let agent = agent.or_else(|| agent_for_session_type(&session_type));
        let model = model.or_else(|| runtime_provider_for_session(&session_type, agent.as_deref()));
        let use_last_tool_call_response =
            default_use_last_tool_call_response_for_session(&session_type, agent.as_deref());
        let input = SessionInput {
            user_input: String::new(),
            file_input: vec![],
            agent: agent.clone(),
            runtime_context: None,
        };

        let session_topic = session_topic_for_session_type(&session_type);

        let mut management = SessionManagement::new(
            session_id.clone(),
            session_name.clone(),
            session_directory,
            false,
            session_topic,
            input,
            user_goal,
            now,
        );
        management.use_last_tool_call_response = use_last_tool_call_response;

        SessionInfo {
            id: session_id,
            name: Some(session_name),
            created_at: now.timestamp_millis(),
            updated_at: now.timestamp_millis(),
            directory: dir_clone,
            model,
            agent,
            session_type: Some(session_type),
            lsp: LspSessionConfig::default(),
            kill_processes_on_start: false,
            validator_enabled: false,
            force_multiple_tasks: false,
            model_variant: Some(DEFAULT_SESSION_REASONING_EFFORT.to_string()),
            model_acceleration_enabled: true,
            disable_permission_restrictions: false,
            use_last_tool_call_response,
            status: SessionStatus::from_state(management.state),
            message_count: 0,
            management,
        }
    }

    pub fn select_session(management: &SessionManagement) -> SessionInfo {
        SessionInfo::from_management(management)
    }

    pub fn transition_session(
        management: &mut SessionManagement,
        next_state: SessionState,
    ) -> Result<(), String> {
        let now = Utc::now();
        management.transition(next_state, now)
    }

    pub fn get_state(management: &SessionManagement) -> SessionState {
        management.state
    }

    fn generate_session_id() -> SessionId {
        format!("sess-{}", Uuid::new_v4())
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SessionInfo {
    pub id: SessionId,
    pub name: Option<SessionName>,
    pub created_at: i64,
    pub updated_at: i64,
    pub directory: Option<String>,
    pub model: Option<String>,
    pub agent: Option<String>,
    pub session_type: Option<String>,
    pub lsp: LspSessionConfig,
    pub kill_processes_on_start: bool,
    pub validator_enabled: bool,
    pub force_multiple_tasks: bool,
    #[serde(default)]
    pub model_variant: Option<String>,
    #[serde(default)]
    pub model_acceleration_enabled: bool,
    #[serde(default)]
    pub disable_permission_restrictions: bool,
    #[serde(default = "default_true")]
    pub use_last_tool_call_response: bool,
    pub status: SessionStatus,
    pub message_count: usize,
    pub management: SessionManagement,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LspSessionConfig {
    pub mode: LspMode,
    pub enabled: Vec<String>,
    pub disabled: Vec<String>,
}

impl Default for LspSessionConfig {
    fn default() -> Self {
        Self {
            mode: LspMode::Auto,
            enabled: Vec::new(),
            disabled: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum LspMode {
    Auto,
    Manual,
}

impl SessionInfo {
    pub fn from_management(management: &SessionManagement) -> Self {
        Self {
            id: management.session_id.clone(),
            name: Some(management.session_name.clone()),
            created_at: management.session_created_at.timestamp_millis(),
            updated_at: management.session_last_update_at.timestamp_millis(),
            directory: Some(management.session_directory.to_string_lossy().to_string()),
            model: Some(coding_agent_provider()),
            agent: Some(CODING_AGENT_NAME.to_string()),
            session_type: Some("coding".to_string()),
            lsp: LspSessionConfig::default(),
            kill_processes_on_start: false,
            validator_enabled: false,
            force_multiple_tasks: false,
            model_variant: Some(DEFAULT_SESSION_REASONING_EFFORT.to_string()),
            model_acceleration_enabled: true,
            disable_permission_restrictions: management.disable_permission_restrictions,
            use_last_tool_call_response: management.use_last_tool_call_response,
            status: SessionStatus::from_state(management.state),
            message_count: management.session_current_turn as usize,
            management: management.clone(),
        }
    }

    pub fn transition(&mut self, next_state: SessionState) -> Result<(), String> {
        let now = Utc::now();
        self.management.transition(next_state, now)
    }
}

pub fn normalize_session_type(session_type: Option<String>, agent: Option<&str>) -> String {
    match session_type.as_deref().or(agent) {
        Some("coding") | Some(CODING_AGENT_NAME) | Some(CODING_AGENT_FAST_NAME) | None => {
            "coding".to_string()
        }
        Some("general") => "general".to_string(),
        Some(other) => other.to_string(),
    }
}

pub fn agent_for_session_type(session_type: &str) -> Option<String> {
    match session_type {
        "coding" => Some(CODING_AGENT_NAME.to_string()),
        "general" => Some("general".to_string()),
        _ => None,
    }
}

pub fn runtime_provider_for_session(session_type: &str, agent: Option<&str>) -> Option<String> {
    match (session_type, agent) {
        ("coding", _) | (_, Some(CODING_AGENT_NAME)) | (_, Some(CODING_AGENT_FAST_NAME)) => {
            Some(coding_agent_provider())
        }
        ("general", _) | (_, Some("general")) => Some("tura_general".to_string()),
        _ => None,
    }
}

pub fn default_use_last_tool_call_response_for_session(
    session_type: &str,
    agent: Option<&str>,
) -> bool {
    !matches!(
        (session_type, agent),
        ("coding", _) | (_, Some(CODING_AGENT_NAME)) | (_, Some(CODING_AGENT_FAST_NAME))
    )
}

fn session_topic_for_session_type(session_type: &str) -> String {
    match session_type {
        "coding" | "programming" | "development" | "testing" => "coding".to_string(),
        other => other.to_string(),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SessionStatus {
    Idle,
    Busy,
    Error,
}

impl SessionStatus {
    pub fn from_state(state: SessionState) -> Self {
        match state {
            SessionState::Created | SessionState::Completed => SessionStatus::Idle,
            SessionState::Running | SessionState::Paused => SessionStatus::Busy,
            SessionState::Failed | SessionState::Cancelled => SessionStatus::Error,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coding_session_uses_agent_configured_provider() {
        let info = SessionManager::create_session(
            None,
            None,
            Some("coding_agent".to_string()),
            Some("coding".to_string()),
        );

        assert_eq!(info.agent.as_deref(), Some(CODING_AGENT_NAME));
        assert_eq!(info.model, Some(coding_agent_provider()));
        assert_eq!(info.session_type.as_deref(), Some("coding"));
        assert_eq!(info.management.session_topic, "coding");
        assert!(!info.use_last_tool_call_response);
        assert!(!info.management.use_last_tool_call_response);
    }

    #[test]
    fn non_coding_session_keeps_requested_provider() {
        let info = SessionManager::create_session(
            None,
            Some("openai/gpt-5".to_string()),
            Some("general_agent".to_string()),
            Some("general".to_string()),
        );

        assert_eq!(info.agent.as_deref(), Some("general_agent"));
        assert_eq!(info.model.as_deref(), Some("openai/gpt-5"));
        assert_eq!(info.session_type.as_deref(), Some("general"));
        assert_eq!(info.management.session_topic, "general");
        assert!(info.use_last_tool_call_response);
        assert!(info.management.use_last_tool_call_response);
    }
}
