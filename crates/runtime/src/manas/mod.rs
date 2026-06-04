mod agent_prompts;
mod change_tracker;
pub mod child_dispatch;
mod constants;
mod final_response;
mod gateway_events;
mod permission_gate;
mod process;
pub(crate) mod prompt_messages;
mod runtime_turn;
mod tool_arguments;
mod tool_catalog;
mod tool_execution;
mod validator_feedback;

pub use process::{process_manas_internal, ManasInput, ManasResult};

use crate::state_machine::agent_management::AgentManagement;
use crate::state_machine::session_management::SessionManagement;

pub type AgentLoader = fn(&SessionManagement) -> Result<Vec<AgentManagement>, String>;

pub fn load_agent_system_prompt_messages(
    agent: &AgentManagement,
) -> Result<Vec<serde_json::Value>, String> {
    agent_prompts::load_agent_system_prompt_messages(agent)
}

#[derive(Clone, Copy, Default)]
pub struct ManasOverrides {
    pub agent_loader: Option<AgentLoader>,
}

pub fn process_from_session(session: &SessionManagement) -> Result<Vec<AgentManagement>, String> {
    process_from_session_with_overrides(session, ManasOverrides::default())
}

pub fn process_from_session_with_overrides(
    session: &SessionManagement,
    overrides: ManasOverrides,
) -> Result<Vec<AgentManagement>, String> {
    if let Some(agent_loader) = overrides.agent_loader {
        return agent_loader(session);
    }

    crate::agent_router::activate_agents_by_session_type(session).and_then(|mut agents| {
        crate::agent_router::initialize_agent_state_machine(&mut agents, session)?;
        Ok(agents)
    })
}

pub fn run_session(
    session: &SessionManagement,
    overrides: ManasOverrides,
) -> Result<Vec<AgentManagement>, String> {
    process_manas_internal(
        ManasInput {
            agents: &mut [],
            session: &mut session.clone(),
            initial_messages: Vec::new(),
            redis_url: "redis://localhost:6379",
        },
        overrides,
    )
    .map(|r| r.agents)
}

pub mod input {
    use crate::state_machine::agent_management::AgentManagement;
    use crate::state_machine::session_management::SessionManagement;

    pub struct ManasOrchestrationInput<'a> {
        pub agents: &'a mut [AgentManagement],
        pub session: &'a mut SessionManagement,
        pub initial_messages: Vec<serde_json::Value>,
        pub redis_url: &'a str,
    }
}
