use std::path::PathBuf;

use crate::session::activate_session_with_directory;
use lifecycle::{SessionInput, SessionManagement};

pub(crate) fn create_session_with_topic(
    input: SessionInput,
    session_directory_override: Option<PathBuf>,
) -> Result<SessionManagement, String> {
    let project_directory = std::env::current_dir()
        .map_err(|err| format!("failed to resolve project directory: {err}"))?;

    let session_directory =
        session_directory_override.unwrap_or_else(|| project_directory.join("sessions"));
    crate::workspace_git::ensure_workspace_git_repo(&session_directory)?;
    activate_session_with_directory(session_directory, input)
}
