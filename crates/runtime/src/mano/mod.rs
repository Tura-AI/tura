mod process;

use crate::state_machine::agent_management::AgentManagement;
use crate::state_machine::session_management::{SessionInput, SessionManagement};
pub use process::{
    orchestrate, orchestrate_for_session, orchestrate_for_session_in_directory,
    process_from_user_internal,
};
use std::path::PathBuf;

pub type SessionFactory = fn(SessionInput) -> Result<SessionManagement, String>;
pub type ManasEntry = fn(&SessionManagement) -> Result<Vec<AgentManagement>, String>;

#[derive(Debug, Clone, PartialEq)]
pub struct ManoProcessResult {
    pub session: SessionManagement,
    pub agents: Vec<AgentManagement>,
}

#[derive(Clone, Copy, Default)]
pub struct ManoOverrides {
    pub session_factory: Option<SessionFactory>,
    pub manas_entry: Option<ManasEntry>,
}

pub fn process_from_user(input: SessionInput) -> Result<ManoProcessResult, String> {
    orchestrate(input)
}

pub fn process_from_gateway_session(
    session_id: String,
    input: SessionInput,
) -> Result<ManoProcessResult, String> {
    orchestrate_for_session(input, session_id)
}

pub fn process_from_gateway_session_in_directory(
    session_id: String,
    input: SessionInput,
    session_directory: PathBuf,
) -> Result<ManoProcessResult, String> {
    orchestrate_for_session_in_directory(input, session_id, session_directory)
}

pub fn process_from_user_with_overrides(
    input: SessionInput,
    overrides: ManoOverrides,
) -> Result<ManoProcessResult, String> {
    if overrides.session_factory.is_none() && overrides.manas_entry.is_none() {
        return orchestrate(input);
    }
    process_from_user_internal(input, overrides)
}
