use std::path::PathBuf;

use chrono::Utc;
use uuid::Uuid;

use crate::state_machine::session_management::{SessionInput, SessionManagement};

const DEFAULT_SESSION_DIRECTORY: &str = "test_session";
const CODING_SESSION_TOPIC: &str = "coding";

pub fn activate_session(input: SessionInput) -> Result<SessionManagement, String> {
    let session_directory = std::env::current_dir()
        .map_err(|err| format!("failed to resolve project directory: {err}"))?
        .join(DEFAULT_SESSION_DIRECTORY);

    activate_session_with_topic(session_directory, "general", input)
}

pub fn activate_session_with_topic(
    session_directory: PathBuf,
    session_topic: impl Into<String>,
    input: SessionInput,
) -> Result<SessionManagement, String> {
    let session_topic = session_topic.into();
    let mut session = create_session_for_topic(session_directory, session_topic.clone(), input)?;
    session.use_last_tool_call_response = session_topic != CODING_SESSION_TOPIC;
    Ok(session)
}

fn create_session_for_topic(
    session_directory: PathBuf,
    session_topic: String,
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
        session_topic,
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
