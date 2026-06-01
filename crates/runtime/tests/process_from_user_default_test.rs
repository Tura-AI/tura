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
    assert_eq!(agents[0].provider.tura_llm_name, "fast");
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
    assert_eq!(agents[0].agent_name, "coding_agent_planning");
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
    assert!(agents[0]
        .agent_capabilities
        .iter()
        .any(|capability| capability.capability_name == "task_status"));
}

#[test]
fn default_coding_agents_expose_expected_command_run_capabilities() {
    let project_root = std::env::current_dir().expect("current dir should resolve");

    for (agent_name, expected, forbidden, provider) in [
        (
            "coding_agent_planning",
            vec![
                "command_run",
                "apply_patch",
                "shell_command",
                "read_media",
                "web_discover",
                "compact_context",
                "task_status",
            ],
            vec![],
            "flagship_thinking",
        ),
        (
            "coding_agent_fast",
            vec![
                "command_run",
                "apply_patch",
                "shell_command",
                "read_media",
                "web_discover",
                "compact_context",
                "task_status",
            ],
            vec!["multiple_tasks"],
            "flagship_thinking",
        ),
        (
            "coding_agent_instant",
            vec![
                "command_run",
                "apply_patch",
                "shell_command",
                "web_discover",
                "compact_context",
                "task_status",
            ],
            vec!["multiple_tasks", "read_media"],
            "fast",
        ),
    ] {
        let session = activate_session_with_topic(
            project_root.clone(),
            "coding",
            SessionInput {
                user_input: "check capabilities".to_string(),
                file_input: vec![],
                agent: Some(agent_name.to_string()),
                runtime_context: None,
            },
        )
        .expect("session should be created");
        let agents = activate_agents_by_session_type(&session).expect("agent should load");
        let agent = agents.first().expect("agent should exist");
        let capabilities = agent
            .agent_capabilities
            .iter()
            .map(|capability| capability.capability_name.as_str())
            .collect::<std::collections::HashSet<_>>();

        assert_eq!(agent.agent_name, agent_name);
        assert_eq!(agent.provider.tura_llm_name, provider);
        for capability in expected {
            assert!(
                capabilities.contains(capability),
                "{agent_name} missing {capability}"
            );
        }
        for capability in forbidden {
            assert!(
                !capabilities.contains(capability),
                "{agent_name} should not expose {capability}"
            );
        }
    }
}
