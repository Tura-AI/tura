use axum::extract::Path;
use axum::Json;
use gateway::api::agent::{create_agent, delete_agent, get_agent, list_agents, update_agent};
use gateway::api::persona::{create_persona, delete_persona, get_persona, list_personas};
use gateway::contracts::{UpsertAgentRequest, UpsertPersonaRequest};
use serde_json::json;
use std::ffi::OsString;
use tokio::sync::Mutex;
use tura_agents::store::{default_agent_config, AgentSource};
use tura_persona::store::{default_persona_config, PersonaSource};

static ENV_LOCK: Mutex<()> = Mutex::const_new(());

#[tokio::test]
async fn gateway_agent_persona_business_flow_round_trips_settings_handlers() {
    let _env_guard = ENV_LOCK.lock().await;
    let temp = tempfile::tempdir().expect("temp root");
    let _project_root = ProjectRootGuard::set(temp.path().as_os_str().to_os_string());

    let Json(created_persona) = create_persona(Json(UpsertPersonaRequest {
        id: Some("ui-persona".to_string()),
        config: None,
        persona: Some("Persona prompt from gateway business flow.".to_string()),
        communication_style: Some("Be concise and operational.".to_string()),
    }))
    .await
    .expect("create persona");
    assert_eq!(created_persona.summary.id, "ui-persona");
    assert_eq!(created_persona.summary.source, PersonaSource::Dynamic);
    assert_eq!(
        created_persona.persona.as_deref(),
        Some("Persona prompt from gateway business flow.")
    );

    let Json(loaded_persona) = get_persona(Path("UI-PERSONA".to_string()))
        .await
        .expect("get persona by case-insensitive id");
    assert_eq!(loaded_persona.summary.id, "ui-persona");
    let Json(personas) = list_personas().await;
    assert_eq!(personas.len(), 1);

    let mut agent_config = default_agent_config(temp.path(), "ui-agent").expect("agent config");
    agent_config.description = Some("Gateway editable agent".to_string());
    agent_config.aliases = vec!["ui-helper".to_string()];
    agent_config.provider["tura_llm_name"] = json!("flagship_fast");
    agent_config.agent_persona = vec![json!({
        "persona_name": "ui-persona",
        "persona_directory": "personas/ui-persona"
    })];

    let Json(created_agent) = create_agent(Json(UpsertAgentRequest {
        id: None,
        config: Some(agent_config),
        prompt: Some("Agent prompt from gateway business flow.".to_string()),
    }))
    .await
    .expect("create agent");
    assert_eq!(created_agent.summary.id, "ui-agent");
    assert_eq!(created_agent.summary.source, AgentSource::Dynamic);
    assert_eq!(
        created_agent.prompt.as_deref(),
        Some("Agent prompt from gateway business flow.")
    );

    let Json(loaded_agent) = get_agent(Path("ui-helper".to_string()))
        .await
        .expect("get agent by alias");
    assert_eq!(loaded_agent.summary.id, "ui-agent");
    assert_eq!(
        loaded_agent.summary.provider.as_deref(),
        Some("flagship_fast")
    );

    let Json(frontend_agents) = list_agents().await;
    let frontend_agent = frontend_agents
        .iter()
        .find(|agent| agent.name == "ui-agent")
        .expect("frontend agent projection");
    assert_eq!(frontend_agent.description, "Gateway editable agent");
    assert!(!frontend_agent.native);
    assert_eq!(frontend_agent.options["aliases"], json!(["ui-helper"]));
    assert_eq!(frontend_agent.options["source"], json!("dynamic"));
    assert_eq!(
        frontend_agent.options["personas"][0]["summary"]["id"],
        "ui-persona"
    );

    let Json(updated_agent) = update_agent(
        Path("ui-agent".to_string()),
        Json(UpsertAgentRequest {
            id: None,
            config: None,
            prompt: Some("Updated prompt through gateway handler.".to_string()),
        }),
    )
    .await
    .expect("update agent prompt");
    assert_eq!(
        updated_agent.prompt.as_deref(),
        Some("Updated prompt through gateway handler.")
    );

    let Json(deleted_agent) = delete_agent(Path("ui-helper".to_string()))
        .await
        .expect("delete agent by alias");
    assert!(deleted_agent);
    assert!(get_agent(Path("ui-agent".to_string())).await.is_err());

    let Json(deleted_persona) = delete_persona(Path("ui-persona".to_string()))
        .await
        .expect("delete persona");
    assert!(deleted_persona);
    assert!(get_persona(Path("ui-persona".to_string())).await.is_err());
}

#[tokio::test]
async fn gateway_agent_persona_business_flow_reports_bad_requests_without_writes() {
    let _env_guard = ENV_LOCK.lock().await;
    let temp = tempfile::tempdir().expect("temp root");
    let _project_root = ProjectRootGuard::set(temp.path().as_os_str().to_os_string());

    let create_agent_error = create_agent(Json(UpsertAgentRequest {
        id: None,
        config: None,
        prompt: None,
    }))
    .await
    .expect_err("agent id is required");
    assert_eq!(create_agent_error.0, axum::http::StatusCode::BAD_REQUEST);
    assert!(create_agent_error
        .1
         .0
        .error
        .contains("agent id is required"));

    let mut bad_persona = default_persona_config(temp.path(), "bad-persona").expect("config");
    bad_persona.default_config = true;
    let create_persona_error = create_persona(Json(UpsertPersonaRequest {
        id: None,
        config: Some(bad_persona),
        persona: None,
        communication_style: None,
    }))
    .await
    .expect_err("user persona cannot set default_config");
    assert_eq!(create_persona_error.0, axum::http::StatusCode::BAD_REQUEST);
    assert!(create_persona_error
        .1
         .0
        .error
        .contains("default_config=true"));
    assert!(!temp.path().join("agents").exists());
    assert!(!temp.path().join("personas").join("bad-persona").exists());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn gateway_agent_persona_business_flow_concurrent_settings_writes_keep_files_distinct() {
    let _env_guard = ENV_LOCK.lock().await;
    let temp = tempfile::tempdir().expect("temp root");
    let _project_root = ProjectRootGuard::set(temp.path().as_os_str().to_os_string());
    let write_count = 8;

    let persona_results = futures::future::join_all((0..write_count).map(|index| async move {
        create_persona(Json(UpsertPersonaRequest {
            id: Some(format!("concurrent-persona-{index}")),
            config: None,
            persona: Some(format!(
                "Persona prompt {index} from concurrent settings write."
            )),
            communication_style: Some(format!("Style {index}")),
        }))
        .await
        .map(|Json(persona)| persona)
    }))
    .await;
    for (index, result) in persona_results.into_iter().enumerate() {
        let persona = result.unwrap_or_else(|error| {
            panic!(
                "concurrent persona {index} should write successfully: {}",
                error.1 .0.error
            )
        });
        assert_eq!(persona.summary.id, format!("concurrent-persona-{index}"));
        assert_eq!(
            persona.persona.as_deref(),
            Some(format!("Persona prompt {index} from concurrent settings write.").as_str())
        );
    }

    let agent_results = futures::future::join_all((0..write_count).map(|index| {
        let root = temp.path().to_path_buf();
        async move {
            let mut config = default_agent_config(&root, &format!("concurrent-agent-{index}"))
                .expect("agent config");
            config.description = Some(format!("Concurrent agent {index}"));
            config.aliases = vec![format!("concurrent-alias-{index}")];
            config.provider["tura_llm_name"] = json!(format!("local-route-{index}"));
            config.agent_persona = vec![json!({
                "persona_name": format!("concurrent-persona-{index}"),
                "persona_directory": format!("personas/concurrent-persona-{index}")
            })];
            create_agent(Json(UpsertAgentRequest {
                id: None,
                config: Some(config),
                prompt: Some(format!(
                    "Agent prompt {index} from concurrent settings write."
                )),
            }))
            .await
            .map(|Json(agent)| agent)
        }
    }))
    .await;
    for (index, result) in agent_results.into_iter().enumerate() {
        let agent = result.unwrap_or_else(|error| {
            panic!(
                "concurrent agent {index} should write successfully: {}",
                error.1 .0.error
            )
        });
        assert_eq!(agent.summary.id, format!("concurrent-agent-{index}"));
        assert_eq!(
            agent.prompt.as_deref(),
            Some(format!("Agent prompt {index} from concurrent settings write.").as_str())
        );
        assert_eq!(
            agent.summary.aliases,
            vec![format!("concurrent-alias-{index}")]
        );
    }

    let Json(personas) = list_personas().await;
    assert_eq!(personas.len(), write_count);
    for index in 0..write_count {
        let Json(persona) = get_persona(Path(format!("CONCURRENT-PERSONA-{index}")))
            .await
            .expect("load concurrent persona");
        assert_eq!(persona.summary.id, format!("concurrent-persona-{index}"));
        assert_eq!(
            persona.communication_style.as_deref(),
            Some(format!("Style {index}").as_str())
        );
        assert!(
            temp.path()
                .join("personas")
                .join(format!("concurrent-persona-{index}"))
                .join("prompt")
                .join("persona.md")
                .exists(),
            "persona prompt file should exist for concurrent-persona-{index}"
        );

        let Json(agent) = get_agent(Path(format!("concurrent-alias-{index}")))
            .await
            .expect("load concurrent agent by alias");
        assert_eq!(agent.summary.id, format!("concurrent-agent-{index}"));
        assert_eq!(
            agent.summary.provider.as_deref(),
            Some(format!("local-route-{index}").as_str())
        );
        assert_eq!(
            agent.config.agent_persona[0]["persona_name"],
            json!(format!("concurrent-persona-{index}"))
        );
        assert!(
            temp.path()
                .join("agents")
                .join("src")
                .join(format!("concurrent-agent-{index}"))
                .join("agent_config.json")
                .exists(),
            "agent config file should exist for concurrent-agent-{index}"
        );
    }

    let Json(frontend_agents) = list_agents().await;
    assert_eq!(frontend_agents.len(), write_count);
    for index in 0..write_count {
        let frontend_agent = frontend_agents
            .iter()
            .find(|agent| agent.name == format!("concurrent-agent-{index}"))
            .unwrap_or_else(|| panic!("frontend agent {index} should be listed"));
        assert_eq!(
            frontend_agent.description,
            format!("Concurrent agent {index}")
        );
        assert_eq!(
            frontend_agent.options["aliases"],
            json!([format!("concurrent-alias-{index}")])
        );
        assert_eq!(
            frontend_agent.options["personas"][0]["summary"]["id"],
            json!(format!("concurrent-persona-{index}"))
        );
    }
}

struct ProjectRootGuard {
    previous: Option<OsString>,
}

impl ProjectRootGuard {
    fn set(root: OsString) -> Self {
        let previous = std::env::var_os("TURA_PROJECT_ROOT");
        std::env::set_var("TURA_PROJECT_ROOT", root);
        Self { previous }
    }
}

impl Drop for ProjectRootGuard {
    fn drop(&mut self) {
        match self.previous.take() {
            Some(value) => std::env::set_var("TURA_PROJECT_ROOT", value),
            None => std::env::remove_var("TURA_PROJECT_ROOT"),
        }
    }
}
