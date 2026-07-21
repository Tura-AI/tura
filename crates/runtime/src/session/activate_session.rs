use chrono::Utc;
use std::path::PathBuf;
use uuid::Uuid;

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
    crate::workspace_git::ensure_workspace_git_repo(&session_directory)?;
    create_session_for_directory(session_directory, input)
}

fn create_session_for_directory(
    session_directory: PathBuf,
    input: SessionInput,
) -> Result<SessionManagement, String> {
    let now = Utc::now();
    let session_id = generate_session_id(&session_directory, now);
    let session_name = format!("temp-session-{}", now.format("%Y%m%d%H%M%S"));
    let user_goal = input.user_input.clone();

    let mut session = SessionManagement::new(
        session_id,
        session_name,
        session_directory,
        false,
        Vec::<String>::new(),
        input,
        user_goal,
        now,
    );
    session.is_child_session = std::env::var("TURA_PARENT_SESSION_ID")
        .ok()
        .is_some_and(|value| !value.trim().is_empty());
    Ok(session)
}

fn generate_session_id(session_directory: &std::path::Path, now: chrono::DateTime<Utc>) -> String {
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
