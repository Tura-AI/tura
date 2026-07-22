mod agent_prompts;
pub mod child_dispatch;
pub(crate) mod constants;
pub(crate) mod final_response;
mod process;
pub(crate) mod prompt_messages;
pub(crate) mod runtime_turn;
pub(crate) mod tool_arguments;
pub(crate) mod tool_catalog;

#[cfg(test)]
pub(crate) static TEST_ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

pub(crate) use constants::{COMMAND_RUN_TOOL, TASK_STATUS_COMMAND};
pub(crate) use final_response::{user_visible_runtime_output_text, user_visible_runtime_text};
pub(crate) use process::{process_manas_internal, ManasInput};

use crate::state_machine::agent_management::AgentManagement;
use lifecycle::SessionManagement;

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

    crate::agent_router::activate_agents_by_session_type(session)
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
            initial_runtime_id: None,
            runtime_event_writer: None,
            session_delta_writer: None,
        },
        overrides,
    )
    .map(|r| r.agents)
}

pub mod input {
    use crate::state_machine::agent_management::AgentManagement;
    use lifecycle::SessionManagement;

    pub struct ManasOrchestrationInput<'a> {
        pub agents: &'a mut [AgentManagement],
        pub session: &'a mut SessionManagement,
        pub initial_messages: Vec<serde_json::Value>,
        pub redis_url: &'a str,
    }
}
