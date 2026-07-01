//! Session management using mano state machine
//!
//! This module provides session creation and management using the mano service's
//! session state machine (SessionManagement, SessionState, etc.)

use chrono::Utc;
use runtime::state_machine::session_management::{
    SessionId, SessionInput, SessionManagement, SessionState, UserGoal,
};
use std::path::PathBuf;
use uuid::Uuid;

use crate::session::config::DEFAULT_SESSION_REASONING_EFFORT;

const DEFAULT_SESSION_DIRECTORY: &str = "sessions";
pub const CODING_AGENT_NAME: &str = "direct";
pub const THINKING_AGENT_NAME: &str = "thoughtful";
pub const CODING_AGENT_FAST_NAME: &str = "direct";
pub const CODING_AGENT_FAST_TEXT_ONLY_NAME: &str = "direct-text-only";

pub fn coding_agent_provider() -> String {
    runtime::agent_router::coding_agent_provider_name()
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

        let dir_clone = directory.clone();
        let session_directory = directory.map(PathBuf::from).unwrap_or_else(|| {
            std::env::current_dir()
                .unwrap_or_default()
                .join(DEFAULT_SESSION_DIRECTORY)
        });
        if let Err(error) = runtime::workspace_git::ensure_workspace_git_repo(&session_directory) {
            tracing::warn!(
                directory = %session_directory.display(),
                error = %error,
                "failed to ensure workspace git repository"
            );
        }
        let session_id = Self::generate_session_id(&session_directory, now);
        let session_name = format!("Session-{}", now.format("%Y%m%d%H%M%S"));

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
            planning_mode_override: None,
        };

        let mut management = SessionManagement::new(
            session_id.clone(),
            session_name,
            session_directory,
            false,
            Vec::<String>::new(),
            input,
            user_goal,
            now,
        );
        management.use_last_tool_call_response = use_last_tool_call_response;

        SessionInfo {
            id: session_id,
            created_at: now.timestamp_millis(),
            updated_at: now.timestamp_millis(),
            last_user_message_at: Some(now.timestamp_millis()),
            directory: dir_clone,
            model,
            agent,
            session_type: Some(session_type),
            kill_processes_on_start: false,
            validator_enabled: false,
            force_planning: false,
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

    fn generate_session_id(
        session_directory: &std::path::Path,
        now: chrono::DateTime<Utc>,
    ) -> SessionId {
        let prefix = session_directory
            .file_name()
            .and_then(|value| value.to_str())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("session")
            .chars()
            .take(8)
            .collect::<String>();
        format!("{prefix}-{}-{}", now.timestamp_millis(), Uuid::new_v4())
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SessionInfo {
    pub id: SessionId,
    pub created_at: i64,
    pub updated_at: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_user_message_at: Option<i64>,
    pub directory: Option<String>,
    pub model: Option<String>,
    pub agent: Option<String>,
    pub session_type: Option<String>,
    pub kill_processes_on_start: bool,
    pub validator_enabled: bool,
    #[serde(default)]
    pub force_planning: bool,
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

impl SessionInfo {
    pub fn from_management(management: &SessionManagement) -> Self {
        Self {
            id: management.session_id.clone(),
            created_at: management.session_created_at.timestamp_millis(),
            updated_at: management.session_last_update_at.timestamp_millis(),
            last_user_message_at: Some(management.session_last_user_message_at.timestamp_millis()),
            directory: Some(management.session_directory.to_string_lossy().to_string()),
            model: Some(coding_agent_provider()),
            agent: Some(CODING_AGENT_NAME.to_string()),
            session_type: Some("coding".to_string()),
            kill_processes_on_start: false,
            validator_enabled: false,
            force_planning: false,
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
        Some("coding")
        | Some(CODING_AGENT_NAME)
        | Some("fast")
        | Some(THINKING_AGENT_NAME)
        | Some("thinking-planning")
        | Some("thinking")
        | None => "coding".to_string(),
        Some(CODING_AGENT_FAST_TEXT_ONLY_NAME) => "coding".to_string(),
        Some("fast-text-only") => "coding".to_string(),
        Some("coding_agent") | Some("coding_agent_planning") | Some("coding_agent_thinking") => {
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
        (_, Some(CODING_AGENT_NAME))
        | (_, Some(CODING_AGENT_FAST_TEXT_ONLY_NAME))
        | (_, Some("fast"))
        | (_, Some("fast-text-only"))
        | (_, Some("coding_agent_fast")) => Some("fast".to_string()),
        ("coding", _)
        | (_, Some(THINKING_AGENT_NAME))
        | (_, Some("thinking-planning"))
        | (_, Some("thinking"))
        | (_, Some("coding_agent"))
        | (_, Some("coding_agent_planning"))
        | (_, Some("coding_agent_thinking")) => Some(coding_agent_provider()),
        ("general", _) | (_, Some("general")) => Some("fast".to_string()),
        _ => None,
    }
}

pub fn default_use_last_tool_call_response_for_session(
    session_type: &str,
    agent: Option<&str>,
) -> bool {
    !matches!(
        (session_type, agent),
        ("coding", _)
            | (_, Some(CODING_AGENT_NAME))
            | (_, Some("fast"))
            | (_, Some(THINKING_AGENT_NAME))
            | (_, Some("thinking-planning"))
            | (_, Some("thinking"))
            | (_, Some(CODING_AGENT_FAST_TEXT_ONLY_NAME))
            | (_, Some("fast-text-only"))
            | (_, Some("coding_agent"))
            | (_, Some("coding_agent_planning"))
            | (_, Some("coding_agent_fast"))
            | (_, Some("coding_agent_thinking"))
    )
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
            SessionState::Failed | SessionState::Cancelled | SessionState::Interrupted => {
                SessionStatus::Error
            }
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
            Some(CODING_AGENT_NAME.to_string()),
            Some("coding".to_string()),
        );

        assert_eq!(info.agent.as_deref(), Some(CODING_AGENT_NAME));
        assert_eq!(info.model.as_deref(), Some("fast"));
        assert_eq!(info.session_type.as_deref(), Some("coding"));
        assert!(info.management.task_type.is_empty());
        assert!(!info.use_last_tool_call_response);
        assert!(!info.management.use_last_tool_call_response);
        assert!(info.management.auto_session_name);
        assert!(info.id.starts_with("sessions-"));
    }

    #[test]
    fn generated_session_ids_are_unique_within_the_same_millisecond() {
        let now = Utc::now();
        let session_directory = PathBuf::from("concurrent-session-a");

        let first = SessionManager::generate_session_id(&session_directory, now);
        let second = SessionManager::generate_session_id(&session_directory, now);

        assert_ne!(first, second);
        let expected_prefix = format!("concurre-{}-", now.timestamp_millis());
        assert!(first.starts_with(&expected_prefix));
        assert!(second.starts_with(&expected_prefix));
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
        assert!(info.management.task_type.is_empty());
        assert!(info.use_last_tool_call_response);
        assert!(info.management.use_last_tool_call_response);
    }
}
