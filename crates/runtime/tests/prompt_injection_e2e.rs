use std::path::{Path, PathBuf};

use runtime::agent_router::activate_agents_by_session_type;
use runtime::manas::load_agent_system_prompt_messages;
use runtime::session::activate_session_with_directory;
use runtime::state_machine::session_management::SessionInput;

#[test]
fn coding_agents_inject_agent_prompt_without_persona_binding() {
    let project_root = find_project_root();
    for (agent_name, agent_prompt_path) in [
        (
            "balanced",
            project_root
                .join("agents")
                .join("src")
                .join("balanced")
                .join("prompt.md"),
        ),
        (
            "thoughtful",
            project_root
                .join("agents")
                .join("src")
                .join("thoughtful")
                .join("prompt.md"),
        ),
        (
            "direct",
            project_root
                .join("agents")
                .join("src")
                .join("direct")
                .join("prompt.md"),
        ),
        (
            "direct-text-only",
            project_root
                .join("agents")
                .join("src")
                .join("direct-text-only")
                .join("prompt.md"),
        ),
    ] {
        let agent_prompt = read_prompt(&agent_prompt_path);
        let session = activate_session_with_directory(
            project_root.clone(),
            SessionInput {
                user_input: "check prompt injection".to_string(),
                file_input: vec![],
                agent: Some(agent_name.to_string()),
                runtime_context: None,
                planning_mode_override: None,
            },
        )
        .expect("session should be created");
        let agents = activate_agents_by_session_type(&session).expect("agent should activate");
        let agent = agents.first().expect("one agent should activate");

        assert_eq!(agent.agent_name, agent_name);
        assert_eq!(agent.agent_prompt.len(), 1);
        assert_eq!(agent.agent_prompt[0].agent_prompt, agent_name);

        let contents = load_agent_system_prompt_messages(agent)
            .expect("system prompt messages should load")
            .into_iter()
            .map(|message| {
                message
                    .get("content")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or_default()
                    .to_string()
            })
            .collect::<Vec<_>>();

        assert_eq!(contents, vec![agent_prompt.clone()]);
    }
}

fn find_project_root() -> PathBuf {
    let current = std::env::current_dir().expect("current directory should resolve");
    current
        .ancestors()
        .find(|candidate| {
            candidate
                .join("agents")
                .join("src")
                .join("thoughtful")
                .join("agent_config.json")
                .exists()
        })
        .expect("project root should be discoverable")
        .to_path_buf()
}

fn read_prompt(path: &Path) -> String {
    std::fs::read_to_string(path)
        .unwrap_or_else(|err| panic!("failed to read prompt {}: {err}", path.display()))
}
