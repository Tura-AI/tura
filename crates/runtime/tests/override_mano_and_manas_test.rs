use std::path::PathBuf;

use chrono::Utc;
use runtime::agent_router::coding_agent_provider_name;
use runtime::mano::{self, ManoOverrides};
use runtime::state_machine::agent_management::{
    AgentManagement, ProviderConfig, ToolChoice, ValidatorConfig,
};
use runtime::state_machine::session_management::{SessionInput, SessionManagement};

fn hardcoded_session(_input: SessionInput) -> Result<SessionManagement, String> {
    let now = Utc::now();
    Ok(SessionManagement::new(
        "session-hardcoded".to_string(),
        "hardcoded-session".to_string(),
        PathBuf::from("/hardcoded/session"),
        false,
        "override".to_string(),
        SessionInput {
            user_input: "hardcoded user goal".to_string(),
            file_input: vec![],
            agent: None,
            runtime_context: None,
            planning_mode_override: None,
        },
        "hardcoded user goal".to_string(),
        now,
    ))
}

fn hardcoded_agents(_session: &SessionManagement) -> Result<Vec<AgentManagement>, String> {
    let now = Utc::now();
    let provider = ProviderConfig {
        tura_llm_name: coding_agent_provider_name(),
        default_model_tier: None,
        current_model: None,
        stream: true,
        temperature: 0.5,
        max_tokens: 0,
        tool_choice: ToolChoice::Auto,
        time_out_ms: 120_000,
    };
    let validator = ValidatorConfig {
        need_validator: false,
        validator_name: None,
    };

    Ok(vec![
        AgentManagement::new(
            "agent-1".to_string(),
            "override_planner".to_string(),
            PathBuf::from("/hardcoded/agent/one"),
            None,
            true,
            false,
            false,
            false,
            provider.clone(),
            validator.clone(),
            now,
        ),
        AgentManagement::new(
            "agent-2".to_string(),
            "override_executor".to_string(),
            PathBuf::from("/hardcoded/agent/two"),
            None,
            false,
            false,
            false,
            false,
            provider,
            validator,
            now,
        ),
    ])
}

#[test]
fn process_from_user_can_override_mano_and_manas_together() {
    let result = mano::process_from_user_with_overrides(
        SessionInput {
            user_input: "ignored".to_string(),
            file_input: vec![],
            agent: None,
            runtime_context: None,
            planning_mode_override: None,
        },
        ManoOverrides {
            session_factory: Some(hardcoded_session),
            manas_entry: Some(hardcoded_agents),
        },
    )
    .expect("override path should succeed");

    assert_eq!(result.session.session_id, "session-hardcoded");
    assert_eq!(result.session.user_goal, "hardcoded user goal");
    assert_eq!(result.agents.len(), 2);
    assert_eq!(result.agents[0].agent_name, "override_planner");
    assert_eq!(result.agents[1].agent_name, "override_executor");
}
