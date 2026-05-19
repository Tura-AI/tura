use std::path::PathBuf;

use chrono::Utc;
use code_tools_suite::agent_router::{activate_agents_by_session_type, coding_agent_provider_name};
use code_tools_suite::session::{activate_session_with_topic, create_session_with_directory};
use code_tools_suite::state_machine::session_management::{FileInput, SessionInput};

#[test]
fn default_agent_registry_loads_general_agent() {
    let input = SessionInput {
        user_input: "build a rust agent workflow".to_string(),
        file_input: vec![FileInput {
            file_name: "spec.md".to_string(),
            file_path: PathBuf::from("/tmp/spec.md"),
            file_size_bytes: 128,
            last_modified_at: Utc::now(),
            description: Some("task specification".to_string()),
        }],
        agent: None,
        runtime_context: None,
    };

    let session = create_session_with_directory(PathBuf::from("sessions"), input.clone())
        .expect("session should be created");
    let agents = activate_agents_by_session_type(&session).expect("agent registry should load");

    assert_eq!(session.input, input);
    assert_eq!(agents.len(), 1);
    assert_eq!(agents[0].agent_name, "general");
    assert!(agents[0].report_to_user);
    assert_eq!(agents[0].provider.tura_llm_name, "tura_general");
    assert!(agents[0].validator.need_validator);
    assert!(!agents[0]
        .agent_capabilities
        .iter()
        .any(|capability| capability.capability_name == "grep"));
    assert!(agents[0]
        .agent_capabilities
        .iter()
        .any(|capability| capability.capability_name == "command_run"));
    assert!(!agents[0]
        .agent_capabilities
        .iter()
        .any(|capability| capability.capability_name == "recall_memory"));
    assert!(!agents[0]
        .agent_capabilities
        .iter()
        .any(|capability| capability.capability_name == "remember_memory"));
    assert!(!agents[0]
        .agent_capabilities
        .iter()
        .any(|capability| capability.capability_name == "search_services"));
    assert!(!agents[0]
        .agent_capabilities
        .iter()
        .any(|capability| capability.capability_name == "persist_tool"));
    assert!(!agents[0]
        .agent_capabilities
        .iter()
        .any(|capability| capability.capability_name == "multiple_tasks"));
    assert!(!agents[0]
        .agent_capabilities
        .iter()
        .any(|capability| capability.capability_name == "send_message_to_user"));
    assert_eq!(agents[0].agent_capabilities.len(), 1);
}

#[test]
fn coding_topic_registry_loads_coding_agent() {
    let input = SessionInput {
        user_input: "build a rust agent workflow".to_string(),
        file_input: vec![],
        agent: None,
        runtime_context: None,
    };

    let session = activate_session_with_topic(PathBuf::from("."), "coding", input)
        .expect("session should be created");
    let agents = activate_agents_by_session_type(&session).expect("agent registry should load");

    assert_eq!(agents.len(), 1);
    assert_eq!(agents[0].agent_name, "coding_agent");
    assert_eq!(
        agents[0].provider.tura_llm_name,
        coding_agent_provider_name()
    );
    assert!(!agents[0].validator.need_validator);
    assert!(!agents[0]
        .agent_capabilities
        .iter()
        .any(|capability| capability.capability_name == "write_file"));
    assert!(!agents[0]
        .agent_capabilities
        .iter()
        .any(|capability| capability.capability_name == "apply_diff"));
    assert!(!agents[0]
        .agent_capabilities
        .iter()
        .any(|capability| capability.capability_name == "delete_file"));
    assert!(agents[0]
        .agent_capabilities
        .iter()
        .any(|capability| capability.capability_name == "command_run"));
    assert!(!agents[0]
        .agent_capabilities
        .iter()
        .any(|capability| capability.capability_name == "multiple_tasks"));
    assert!(!agents[0]
        .agent_capabilities
        .iter()
        .any(|capability| capability.capability_name == "send_message_to_user"));
    assert_eq!(agents[0].agent_capabilities.len(), 1);
}
