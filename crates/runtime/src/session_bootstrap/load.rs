use std::path::PathBuf;

use crate::session::activate_session_with_topic;
use crate::state_machine::session_management::{SessionInput, SessionManagement};

pub(crate) fn create_session_with_topic(
    input: SessionInput,
    session_directory_override: Option<PathBuf>,
) -> Result<SessionManagement, String> {
    let project_directory = std::env::current_dir()
        .map_err(|err| format!("failed to resolve project directory: {err}"))?;

    let session_topic = infer_session_topic(&input.user_input);
    let session_directory =
        session_directory_override.unwrap_or_else(|| project_directory.join("sessions"));
    crate::workspace_git::ensure_workspace_git_repo(&session_directory)?;
    activate_session_with_topic(session_directory, session_topic, input)
}

fn infer_session_topic(user_input: &str) -> String {
    let input_lower = user_input.to_lowercase();

    if input_lower.contains("code")
        || input_lower.contains("function")
        || input_lower.contains("implement")
        || input_lower.contains("bug")
        || input_lower.contains("refactor")
        || input_lower.contains("shell")
        || input_lower.contains("pwd")
        || input_lower.contains("class")
        || input_lower.contains("file")
    {
        return "coding".to_string();
    }

    if input_lower.contains("test") || input_lower.contains("testing") {
        return "testing".to_string();
    }

    "general".to_string()
}
