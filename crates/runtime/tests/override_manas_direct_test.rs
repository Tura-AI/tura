use std::path::PathBuf;

use chrono::Utc;
use runtime::agent_router::coding_agent_provider_name;
use runtime::manas::{self, ManasOverrides};
use runtime::state_machine::agent_management::{
    AgentManagement, ProviderConfig, ToolChoice, ValidatorConfig,
};
use runtime::state_machine::session_management::{SessionInput, SessionManagement};

fn hardcoded_agents(_session: &SessionManagement) -> Result<Vec<AgentManagement>, String> {
    let now = Utc::now();
    let provider = ProviderConfig {
        tura_llm_name: coding_agent_provider_name(),
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
            "test-agent-1".to_string(),
            "test_planner".to_string(),
            PathBuf::from("/tmp/test/agent/one"),
            None,
            true,
            false,
            provider.clone(),
            validator.clone(),
            now,
        ),
        AgentManagement::new(
            "test-agent-2".to_string(),
            "test_executor".to_string(),
            PathBuf::from("/tmp/test/agent/two"),
            None,
            false,
            false,
            provider,
            validator,
            now,
        ),
    ])
}

#[test]
fn process_from_session_can_override_only_manas() {
    let now = Utc::now();
    let session = SessionManagement::new(
        "session-direct".to_string(),
        "direct-session".to_string(),
        PathBuf::from("/tmp/direct/session"),
        false,
        "general".to_string(),
        SessionInput {
            user_input: "direct session input".to_string(),
            file_input: vec![],
            agent: None,
            runtime_context: None,
            planning_mode_override: None,
        },
        "direct session input".to_string(),
        now,
    );

    let agents = manas::process_from_session_with_overrides(
        &session,
        ManasOverrides {
            agent_loader: Some(hardcoded_agents),
        },
    )
    .expect("manas override should succeed");

    assert_eq!(agents.len(), 2);
    assert_eq!(agents[0].agent_name, "test_planner");
    assert_eq!(agents[1].agent_name, "test_executor");
}
