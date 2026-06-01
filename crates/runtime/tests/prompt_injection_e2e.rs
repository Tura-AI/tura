use std::path::{Path, PathBuf};

use code_tools_suite::agent_router::activate_agents_by_session_type;
use code_tools_suite::manas::load_agent_system_prompt_messages;
use code_tools_suite::session::activate_session_with_topic;
use code_tools_suite::state_machine::session_management::SessionInput;

#[test]
fn coding_agents_inject_persona_style_then_agent_prompt() {
    let project_root = find_project_root();
    let tura_persona_dir = project_root
        .join("personas")
        .join("src")
        .join("tura")
        .join("prompt");
    let persona = read_prompt(&tura_persona_dir.join("persona.md"));
    let communication_style = read_prompt(&tura_persona_dir.join("communication_style.md"));

    for (agent_name, agent_prompt_path) in [
        (
            "coding_agent_planning",
            project_root
                .join("agents")
                .join("src")
                .join("coding_agent_planning")
                .join("prompt.md"),
        ),
        (
            "coding_agent_fast",
            project_root
                .join("agents")
                .join("src")
                .join("coding_agent_fast")
                .join("prompt.md"),
        ),
        (
            "coding_agent_instant",
            project_root
                .join("agents")
                .join("src")
                .join("coding_agent_instant")
                .join("prompt.md"),
        ),
    ] {
        let agent_prompt = read_prompt(&agent_prompt_path);
        let session = activate_session_with_topic(
            project_root.clone(),
            "coding",
            SessionInput {
                user_input: "check prompt injection".to_string(),
                file_input: vec![],
                agent: Some(agent_name.to_string()),
                runtime_context: None,
            },
        )
        .expect("session should be created");
        let agents = activate_agents_by_session_type(&session).expect("agent should activate");
        let agent = agents.first().expect("one agent should activate");

        assert_eq!(agent.agent_name, agent_name);
        assert_eq!(agent.agent_persona.len(), 1);
        assert_eq!(agent.agent_persona[0].persona_name, "tura");
        assert_eq!(agent.agent_persona[0].persona_directory, tura_persona_dir);
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

        assert_eq!(
            contents,
            vec![
                persona.clone(),
                communication_style.clone(),
                agent_prompt.clone()
            ],
            "{agent_name} should inject tura persona, tura communication style, then agent prompt"
        );
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
                .join("coding_agent")
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
