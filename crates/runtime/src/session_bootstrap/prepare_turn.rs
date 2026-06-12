use chrono::{DateTime, Utc};
use std::path::PathBuf;

use crate::session_bootstrap::load::create_session_with_topic;
use crate::session_bootstrap::persisted::load_persisted_gateway_session;
use crate::state_machine::session_management::{SessionInput, SessionManagement};

pub(crate) fn bootstrap_orchestration_session(
    input: SessionInput,
    session_directory: Option<PathBuf>,
    gateway_session_id: Option<String>,
    now: DateTime<Utc>,
) -> Result<SessionManagement, String> {
    if let Some(session_id) = gateway_session_id {
        if let Some(directory) = session_directory.as_ref() {
            if let Some(mut persisted) = load_persisted_gateway_session(directory, &session_id)? {
                persisted.prepare_for_new_user_turn(input, now);
                if let Some(directory) = session_directory {
                    persisted.session_directory = directory;
                }
                persisted.session_id = session_id;
                return Ok(persisted);
            }
        }

        let mut session = create_session_with_topic(input, session_directory)?;
        session.session_id = session_id;
        return Ok(session);
    }

    create_session_with_topic(input, session_directory)
}
