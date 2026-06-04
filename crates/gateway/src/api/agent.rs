use crate::api::{registry, types::*};
use axum::{extract::Path, http::StatusCode, Json};
use std::collections::HashMap;

pub async fn list_agents() -> Json<Vec<Agent>> {
    Json(list_agents_from_store())
}

pub async fn get_agent(
    Path(agent_id): Path<String>,
) -> Result<Json<tura_agents::store::StoredAgent>, (StatusCode, Json<BadRequestError>)> {
    let root = registry::project_root();
    tura_agents::store::load_agent(&root, &agent_id)
        .map(Json)
        .ok_or_else(|| {
            api_error(
                StatusCode::NOT_FOUND,
                format!("agent `{agent_id}` not found"),
            )
        })
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct UpsertAgentRequest {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub config: Option<tura_agents::store::AgentConfig>,
    #[serde(default)]
    pub prompt: Option<String>,
}

pub async fn create_agent(
    Json(payload): Json<UpsertAgentRequest>,
) -> Result<Json<tura_agents::store::StoredAgent>, (StatusCode, Json<BadRequestError>)> {
    upsert_agent_in_store(None, payload)
        .map(Json)
        .map_err(|err| {
            api_error(
                StatusCode::BAD_REQUEST,
                format!("failed to create agent: {err}"),
            )
        })
}

pub async fn update_agent(
    Path(agent_id): Path<String>,
    Json(payload): Json<UpsertAgentRequest>,
) -> Result<Json<tura_agents::store::StoredAgent>, (StatusCode, Json<BadRequestError>)> {
    upsert_agent_in_store(Some(agent_id), payload)
        .map(Json)
        .map_err(|err| {
            api_error(
                StatusCode::BAD_REQUEST,
                format!("failed to update agent: {err}"),
            )
        })
}

pub async fn delete_agent(
    Path(agent_id): Path<String>,
) -> Result<Json<bool>, (StatusCode, Json<BadRequestError>)> {
    tura_agents::store::delete_dynamic_agent(&registry::project_root(), &agent_id)
        .map(Json)
        .map_err(|err| api_error(StatusCode::BAD_REQUEST, err))
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
        "personas".to_string(),
        serde_json::json!(resolve_agent_personas(&agent.config)),
    );
    options.insert(
        "default_config".to_string(),
        serde_json::json!(agent.config.default_config),
    );
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

fn resolve_agent_personas(
    config: &tura_agents::store::AgentConfig,
) -> Vec<tura_persona::store::StoredPersona> {
    let root = registry::project_root();
    config
        .agent_persona
        .iter()
        .filter_map(|item| {
            item.get("persona_name")
                .and_then(serde_json::Value::as_str)
                .and_then(|name| tura_persona::store::load_persona(&root, name))
        })
        .collect()
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
