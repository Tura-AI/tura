mod activate_session;
mod create_session;

use std::path::PathBuf;

use crate::state_machine::session_management::{SessionInput, SessionManagement};

pub use activate_session::{activate_session, activate_session_with_topic};
pub use create_session::create_session;

pub fn create_session_with_directory(
    session_directory: PathBuf,
    input: SessionInput,
) -> Result<SessionManagement, String> {
    create_session(session_directory, input)
}
