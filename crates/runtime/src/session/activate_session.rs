use std::path::PathBuf;

use lifecycle::{SessionInput, SessionManagement};

const DEFAULT_SESSION_DIRECTORY: &str = "test_session";

pub fn activate_session(input: SessionInput) -> Result<SessionManagement, String> {
    let session_directory = std::env::current_dir()
        .map_err(|err| format!("failed to resolve project directory: {err}"))?
        .join(DEFAULT_SESSION_DIRECTORY);

    activate_session_with_directory(session_directory, input)
}

pub fn activate_session_with_directory(
    session_directory: PathBuf,
    input: SessionInput,
) -> Result<SessionManagement, String> {
    super::create_session(session_directory, input)
}
