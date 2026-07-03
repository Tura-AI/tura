use crate::api::registry;
use crate::contracts::*;
use axum::{extract::Path, http::StatusCode, Json};
use std::collections::HashMap;

pub async fn list_agents() -> Json<Vec<Agent>> {
    Json(list_agents_value())
}

pub fn list_agents_value() -> Vec<Agent> {
    list_agents_from_store()
}

pub async fn get_agent(
    Path(agent_id): Path<String>,
) -> Result<Json<tura_agents::store::StoredAgent>, (StatusCode, Json<BadRequestError>)> {
    get_agent_value(agent_id)
        .map(Json)
        .map_err(|(status, error)| api_error(status, error))
}

pub fn get_agent_value(
    agent_id: String,
) -> Result<tura_agents::store::StoredAgent, (StatusCode, String)> {
    let root = registry::project_root();
    tura_agents::store::load_agent(&root, &agent_id).ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            format!("agent `{agent_id}` not found"),
        )
    })
}

pub async fn create_agent(
    Json(payload): Json<UpsertAgentRequest>,
) -> Result<Json<tura_agents::store::StoredAgent>, (StatusCode, Json<BadRequestError>)> {
    create_agent_value(payload).map(Json).map_err(|err| {
        api_error(
            StatusCode::BAD_REQUEST,
            format!("failed to create agent: {err}"),
        )
    })
}

pub fn create_agent_value(
    payload: UpsertAgentRequest,
) -> Result<tura_agents::store::StoredAgent, String> {
    upsert_agent_in_store(None, payload)
}

pub async fn update_agent(
    Path(agent_id): Path<String>,
    Json(payload): Json<UpsertAgentRequest>,
) -> Result<Json<tura_agents::store::StoredAgent>, (StatusCode, Json<BadRequestError>)> {
    update_agent_value(agent_id, payload)
        .map(Json)
        .map_err(|err| {
            api_error(
                StatusCode::BAD_REQUEST,
                format!("failed to update agent: {err}"),
            )
        })
}

pub fn update_agent_value(
    agent_id: String,
    payload: UpsertAgentRequest,
) -> Result<tura_agents::store::StoredAgent, String> {
    upsert_agent_in_store(Some(agent_id), payload)
}

pub async fn delete_agent(
    Path(agent_id): Path<String>,
) -> Result<Json<bool>, (StatusCode, Json<BadRequestError>)> {
    delete_agent_value(agent_id)
        .map(Json)
        .map_err(|err| api_error(StatusCode::BAD_REQUEST, err))
}

pub fn delete_agent_value(agent_id: String) -> Result<bool, String> {
    tura_agents::store::delete_dynamic_agent(&registry::project_root(), &agent_id)
}

fn api_error(status: StatusCode, error: String) -> (StatusCode, Json<BadRequestError>) {
    (status, Json(BadRequestError { error }))
}

fn list_agents_from_store() -> Vec<Agent> {
    tura_agents::store::discover_agents(&registry::project_root())
        .into_iter()
        .map(agent_from_stored_agent)
        .collect()
}

fn agent_from_stored_agent(agent: tura_agents::store::StoredAgent) -> Agent {
    let mut options = HashMap::new();
    options.insert(
        "source".to_string(),
        serde_json::json!(agent.summary.source),
    );
    options.insert("path".to_string(), serde_json::json!(agent.summary.path));
    options.insert(
        "aliases".to_string(),
        serde_json::json!(agent.summary.aliases),
    );
    if let Some(icon_emoji) = agent.config.icon_emoji.as_deref() {
        options.insert("icon_emoji".to_string(), serde_json::json!(icon_emoji));
    }
    options.insert(
        "capabilities".to_string(),
        serde_json::json!(agent.summary.capabilities),
    );
    options.insert(
        "default_config".to_string(),
        serde_json::json!(agent.config.default_config),
    );
    options.insert("provider".to_string(), agent.config.provider.clone());
    Agent {
        name: agent.summary.id,
        description: agent.summary.description,
        mode: "primary".to_string(),
        native: agent.summary.source == tura_agents::store::AgentSource::Static,
        hidden: agent.summary.hidden,
        model: None,
        options,
        permission: PermissionRuleset {
            allow: vec!["*".to_string()],
            deny: Vec::new(),
        },
    }
}

fn upsert_agent_in_store(
    agent_id: Option<String>,
    payload: UpsertAgentRequest,
) -> Result<tura_agents::store::StoredAgent, String> {
    let root = registry::project_root();
    let agent_id = agent_id
        .or(payload.id)
        .or_else(|| {
            payload
                .config
                .as_ref()
                .map(|config| config.agent_name.clone())
        })
        .ok_or_else(|| "agent id is required".to_string())?;
    let mut config = payload.config.unwrap_or(
        tura_agents::store::load_agent(&root, &agent_id)
            .map(|agent| agent.config)
            .unwrap_or(tura_agents::store::default_agent_config(&root, &agent_id)?),
    );
    config.agent_name = agent_id;
    tura_agents::store::save_dynamic_agent(&root, &config, payload.prompt.as_deref())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tempfile::TempDir;
    use tokio::sync::Mutex;

    static AGENT_ENV_LOCK: Mutex<()> = Mutex::const_new(());

    #[test]
    fn api_error_preserves_status_and_message() {
        let (status, Json(body)) = api_error(StatusCode::BAD_REQUEST, "invalid agent".to_string());

        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body.error, "invalid agent");
    }

    #[test]
    fn agent_from_stored_agent_projects_static_agent_for_frontend() {
        let stored = stored_agent(
            "coding",
            tura_agents::store::AgentSource::Static,
            false,
            true,
            Some("code"),
            vec!["edit".to_string(), "review".to_string()],
        );

        let agent = agent_from_stored_agent(stored);

        assert_eq!(agent.name, "coding");
        assert_eq!(agent.description, "Coding agent");
        assert_eq!(agent.mode, "primary");
        assert!(agent.native);
        assert!(!agent.hidden);
        assert_eq!(agent.permission.allow, vec!["*"]);
        assert!(agent.permission.deny.is_empty());
        assert_eq!(agent.options["source"], serde_json::json!("static"));
        assert_eq!(
            agent.options["aliases"],
            serde_json::json!(["edit", "review"])
        );
        assert_eq!(agent.options["icon_emoji"], serde_json::json!("code"));
        assert_eq!(
            agent.options["capabilities"],
            serde_json::json!(["write", "shell"])
        );
        assert_eq!(
            agent.options["provider"],
            serde_json::json!({ "tura_llm_name": "flagship" })
        );
        assert_eq!(agent.options["default_config"], serde_json::json!(true));
    }

    #[test]
    fn agent_from_stored_agent_projects_dynamic_hidden_agent_without_icon() {
        let stored = stored_agent(
            "research",
            tura_agents::store::AgentSource::Dynamic,
            true,
            false,
            None,
            Vec::new(),
        );

        let agent = agent_from_stored_agent(stored);

        assert_eq!(agent.name, "research");
        assert!(!agent.native);
        assert!(agent.hidden);
        assert!(!agent.options.contains_key("icon_emoji"));
        assert_eq!(agent.options["source"], serde_json::json!("dynamic"));
        assert_eq!(agent.options["aliases"], serde_json::json!([]));
        assert!(!agent.options.contains_key("personas"));
    }

    #[test]
    fn upsert_agent_requires_an_id_from_route_payload_or_config() {
        let error = upsert_agent_in_store(
            None,
            UpsertAgentRequest {
                id: None,
                config: None,
                prompt: None,
            },
        )
        .expect_err("missing id should fail");

        assert_eq!(error, "agent id is required");
    }

    #[tokio::test]
    async fn upsert_agent_creates_default_dynamic_agent_in_project_root() {
        let _guard = AGENT_ENV_LOCK.lock().await;
        let previous_root = std::env::var_os("TURA_PROJECT_ROOT");
        let temp = TempDir::new().expect("temp root");
        std::env::set_var("TURA_PROJECT_ROOT", temp.path());

        let stored = upsert_agent_in_store(
            Some("helper-agent".to_string()),
            UpsertAgentRequest {
                id: None,
                config: None,
                prompt: Some("Help with local tasks.".to_string()),
            },
        )
        .expect("create agent");

        assert_eq!(stored.summary.id, "helper-agent");
        assert_eq!(stored.config.agent_name, "helper-agent");
        assert_eq!(stored.prompt.as_deref(), Some("Help with local tasks."));
        assert!(temp
            .path()
            .join("agents")
            .join("src")
            .join("helper-agent")
            .join("agent_config.json")
            .exists());
        assert!(temp
            .path()
            .join("agents")
            .join("src")
            .join("helper-agent")
            .join("prompt.md")
            .exists());

        restore_project_root(previous_root);
    }

    #[tokio::test]
    async fn upsert_agent_rejects_invalid_ids_before_writing_files() {
        let _guard = AGENT_ENV_LOCK.lock().await;
        let previous_root = std::env::var_os("TURA_PROJECT_ROOT");
        let temp = TempDir::new().expect("temp root");
        std::env::set_var("TURA_PROJECT_ROOT", temp.path());

        let error = upsert_agent_in_store(
            Some("../escape".to_string()),
            UpsertAgentRequest {
                id: None,
                config: None,
                prompt: None,
            },
        )
        .expect_err("invalid id should fail");

        assert!(error.contains("invalid agent id"));
        assert!(!temp.path().join("agents").exists());

        restore_project_root(previous_root);
    }

    fn stored_agent(
        id: &str,
        source: tura_agents::store::AgentSource,
        hidden: bool,
        default_config: bool,
        icon: Option<&str>,
        aliases: Vec<String>,
    ) -> tura_agents::store::StoredAgent {
        let path = PathBuf::from("agents/src").join(id);
        tura_agents::store::StoredAgent {
            summary: tura_agents::store::AgentSummary {
                id: id.to_string(),
                name: format!("{id} name"),
                description: format!("{id} agent").replace(id, &capitalize(id)),
                source,
                path: path.clone(),
                aliases: aliases.clone(),
                capabilities: vec!["write".to_string(), "shell".to_string()],
                provider: Some("flagship".to_string()),
                hidden,
            },
            config: tura_agents::store::AgentConfig {
                agent_name: id.to_string(),
                description: Some(format!("{id} agent")),
                aliases,
                icon_emoji: icon.map(ToString::to_string),
                agent_directory: path,
                parent_agent_id: None,
                report_to_user: true,
                default_config,
                provider: serde_json::json!({ "tura_llm_name": "flagship" }),
                agent_prompt: Vec::new(),
                agent_capabilities: Vec::new(),
                reflection: false,
                op_manual: true,
                self_reflection: false,
                validator: serde_json::json!({ "need_validator": false }),
            },
            prompt: None,
        }
    }

    fn capitalize(value: &str) -> String {
        let mut chars = value.chars();
        match chars.next() {
            Some(first) => first.to_ascii_uppercase().to_string() + chars.as_str(),
            None => String::new(),
        }
    }

    fn restore_project_root(previous: Option<std::ffi::OsString>) {
        match previous {
            Some(value) => std::env::set_var("TURA_PROJECT_ROOT", value),
            None => std::env::remove_var("TURA_PROJECT_ROOT"),
        }
    }
}
