use std::path::Path;
use std::sync::{Arc, Barrier};
use tura_agents::store::{
    default_agent_config, delete_dynamic_agent, discover_agents, load_agent, save_dynamic_agent,
    AgentSource, AGENTS_DIR, AGENT_CONFIG_FILE, AGENT_PROMPT_FILE,
};

#[test]
fn agent_business_lifecycle_saves_discovers_alias_loads_and_deletes_dynamic_agent() {
    let project = temp_project();
    let mut config = default_agent_config(project.path(), "Code-Reviewer").expect("default config");
    config.description = Some("Reviews code and reports risks".to_string());
    config.aliases = vec!["reviewer".to_string(), "code_review".to_string()];
    config.provider["default_model_tier"] = serde_json::json!("flagship_fast");
    config.provider["tura_llm_name"] = serde_json::json!("flagship_fast");

    let saved = save_dynamic_agent(
        project.path(),
        &config,
        Some("Review diffs, call out risks first, and keep summaries short."),
    )
    .expect("save dynamic agent");

    assert_eq!(saved.summary.id, "code-reviewer");
    assert_eq!(saved.summary.name, "Code Reviewer");
    assert_eq!(saved.summary.source, AgentSource::Dynamic);
    assert_eq!(saved.summary.provider.as_deref(), Some("flagship_fast"));
    assert_eq!(saved.summary.aliases, vec!["reviewer", "code_review"]);
    assert!(saved
        .summary
        .capabilities
        .iter()
        .any(|capability| capability == "command_run" || capability == "shells"));
    assert_eq!(
        saved.prompt.as_deref(),
        Some("Review diffs, call out risks first, and keep summaries short.")
    );

    let by_alias = load_agent(project.path(), "REVIEWER").expect("load by alias");
    assert_eq!(by_alias.summary.id, "code-reviewer");
    let discovered = discover_agents(project.path());
    assert_eq!(discovered.len(), 1);
    assert_eq!(discovered[0].summary.id, "code-reviewer");
    assert!(project
        .path()
        .join(AGENTS_DIR)
        .join("code-reviewer")
        .join(AGENT_CONFIG_FILE)
        .exists());
    assert!(project
        .path()
        .join(AGENTS_DIR)
        .join("code-reviewer")
        .join(AGENT_PROMPT_FILE)
        .exists());

    assert!(delete_dynamic_agent(project.path(), "code_review").expect("delete by alias"));
    assert!(load_agent(project.path(), "code-reviewer").is_none());
    assert!(!delete_dynamic_agent(project.path(), "missing-agent").expect("delete missing"));
}

#[test]
fn agent_business_rule_rejects_deleting_static_default_agent() {
    let project = temp_project();
    let mut config = default_agent_config(project.path(), "built-in-helper").expect("config");
    config.default_config = true;
    config.aliases = vec!["builtin".to_string()];
    let agent_dir = project.path().join(AGENTS_DIR).join("built-in-helper");
    std::fs::create_dir_all(&agent_dir).expect("agent dir");
    std::fs::write(
        agent_dir.join(AGENT_CONFIG_FILE),
        serde_json::to_string_pretty(&config).expect("config json"),
    )
    .expect("write config");
    std::fs::write(agent_dir.join(AGENT_PROMPT_FILE), "Static prompt").expect("write prompt");

    let loaded = load_agent(project.path(), "builtin").expect("load static alias");
    assert_eq!(loaded.summary.id, "built-in-helper");
    assert_eq!(loaded.summary.source, AgentSource::Static);
    assert_eq!(loaded.prompt.as_deref(), Some("Static prompt"));

    let error = delete_dynamic_agent(project.path(), "builtin")
        .expect_err("static default agent cannot be deleted through dynamic API");
    assert!(error.contains("default_config"), "{error}");
    assert!(
        agent_dir.exists(),
        "static default agent directory must remain after rejected delete"
    );
}

#[test]
fn agent_business_flow_skips_malformed_entries_and_preserves_valid_discovery() {
    let project = temp_project();
    let missing_config_dir = project.path().join(AGENTS_DIR).join("missing-config");
    let malformed_dir = project.path().join(AGENTS_DIR).join("malformed");
    std::fs::create_dir_all(&missing_config_dir).expect("missing config dir");
    std::fs::create_dir_all(&malformed_dir).expect("malformed dir");
    std::fs::write(malformed_dir.join(AGENT_CONFIG_FILE), "{not valid json")
        .expect("malformed config");
    std::fs::write(malformed_dir.join(AGENT_PROMPT_FILE), "bad prompt").expect("bad prompt");

    let mut valid = default_agent_config(project.path(), "Valid-Agent").expect("valid config");
    valid.aliases = vec!["valid_alias".to_string()];
    save_dynamic_agent(project.path(), &valid, Some("Valid prompt")).expect("save valid");

    let discovered = discover_agents(project.path());

    assert_eq!(discovered.len(), 1);
    assert_eq!(discovered[0].summary.id, "valid-agent");
    assert_eq!(
        load_agent(project.path(), "VALID_ALIAS")
            .expect("valid alias should still load")
            .prompt
            .as_deref(),
        Some("Valid prompt")
    );
}

#[test]
fn agent_business_concurrent_save_discover_and_delete_keeps_canonical_agent_set() {
    let project = Arc::new(temp_project());
    let barrier = Arc::new(Barrier::new(6));
    let mut handles = Vec::new();
    for index in 0..6 {
        let project = Arc::clone(&project);
        let barrier = Arc::clone(&barrier);
        handles.push(std::thread::spawn(move || {
            let agent_name = format!("Concurrent-Agent-{index}");
            let mut config =
                default_agent_config(project.path(), &agent_name).expect("concurrent config");
            config.description = Some(format!("Concurrent description {index}"));
            config.aliases = vec![format!("concurrent_alias_{index}")];
            barrier.wait();
            let saved = save_dynamic_agent(
                project.path(),
                &config,
                Some(&format!("Concurrent prompt {index}")),
            )
            .expect("concurrent save");
            assert_eq!(saved.summary.id, format!("concurrent-agent-{index}"));
        }));
    }
    for handle in handles {
        handle.join().expect("concurrent save thread");
    }

    let discovered = discover_agents(project.path());
    let ids = discovered
        .iter()
        .map(|agent| agent.summary.id.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        ids,
        vec![
            "concurrent-agent-0",
            "concurrent-agent-1",
            "concurrent-agent-2",
            "concurrent-agent-3",
            "concurrent-agent-4",
            "concurrent-agent-5",
        ]
    );
    for index in 0..6 {
        let loaded = load_agent(project.path(), &format!("CONCURRENT_ALIAS_{index}"))
            .expect("load concurrent alias");
        assert_eq!(loaded.summary.id, format!("concurrent-agent-{index}"));
        assert_eq!(
            loaded.prompt.as_deref(),
            Some(format!("Concurrent prompt {index}").as_str())
        );
    }

    assert!(delete_dynamic_agent(project.path(), "concurrent_alias_3").expect("delete alias"));
    assert!(load_agent(project.path(), "concurrent-agent-3").is_none());
    assert_eq!(discover_agents(project.path()).len(), 5);
}

fn temp_project() -> tempfile::TempDir {
    let project = tempfile::tempdir().expect("temp project");
    std::fs::create_dir_all(project.path().join(Path::new(AGENTS_DIR))).expect("agents dir");
    project
}
