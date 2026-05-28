//! Global API handlers (health, config, events)

use crate::api::types::*;
use crate::mock::global_store;
use crate::session::session_store;
use axum::{
    http::{header, StatusCode},
    response::sse::{Event as SseEvent, KeepAlive, Sse},
    response::Response,
    Json,
};
use serde_json::Value;
use std::collections::HashMap;
use std::convert::Infallible;
use std::path::PathBuf;
use std::time::Duration;

// ============================================================================
// Health
// ============================================================================

pub async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        healthy: true,
        version: env!("CARGO_PKG_VERSION").to_string(),
    })
}

// ============================================================================
// Config
// ============================================================================

pub async fn get_config() -> Json<Config> {
    Json(global_store().get_config())
}

pub async fn patch_config(Json(payload): Json<ConfigPatch>) -> Json<Config> {
    Json(global_store().update_config(payload))
}

pub async fn get_gui_config() -> Response<String> {
    match std::fs::read_to_string(gui_config_path()) {
        Ok(content) => text_response(StatusCode::OK, content),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            text_response(StatusCode::OK, String::new())
        }
        Err(err) => text_response(StatusCode::INTERNAL_SERVER_ERROR, err.to_string()),
    }
}

pub async fn put_gui_config(body: String) -> Response<String> {
    let path = gui_config_path();
    if let Some(parent) = path.parent() {
        if let Err(err) = std::fs::create_dir_all(parent) {
            return text_response(StatusCode::INTERNAL_SERVER_ERROR, err.to_string());
        }
    }
    match std::fs::write(path, body.as_bytes()) {
        Ok(()) => text_response(StatusCode::OK, body),
        Err(err) => text_response(StatusCode::INTERNAL_SERVER_ERROR, err.to_string()),
    }
}

pub async fn get_tura_config() -> Json<TuraConfigResponse> {
    Json(read_tura_config_response())
}

pub async fn put_tura_config(Json(payload): Json<TuraConfigUpdate>) -> Json<TuraConfigResponse> {
    let path = crate::api::provider::config::provider_config_path();
    let write_result = update_tura_config_tier(&path, &payload);
    let mut response = read_tura_config_response();
    if let Err(error) = write_result {
        response.error = Some(error);
    }
    Json(response)
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct TuraConfigResponse {
    pub path: String,
    pub tiers: Vec<TuraConfigTier>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct TuraConfigUpdate {
    pub tier: String,
    pub provider: String,
    pub model: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct TuraConfigTier {
    pub tier: String,
    pub current: Option<TuraConfigSelection>,
    pub options: Vec<TuraConfigOption>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct TuraConfigSelection {
    pub provider: String,
    pub model: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct TuraConfigOption {
    pub provider: String,
    pub provider_name: String,
    pub model: String,
    pub model_name: String,
}

fn read_tura_config_response() -> TuraConfigResponse {
    let path = crate::api::provider::config::provider_config_path();
    match std::fs::read_to_string(&path)
        .map_err(|err| err.to_string())
        .and_then(|content| serde_json::from_str::<Value>(&content).map_err(|err| err.to_string()))
    {
        Ok(root) => tura_config_response_from_value(path, root),
        Err(error) => TuraConfigResponse {
            path: path.to_string_lossy().to_string(),
            tiers: Vec::new(),
            error: Some(error),
        },
    }
}

fn tura_config_response_from_value(path: PathBuf, root: Value) -> TuraConfigResponse {
    TuraConfigResponse {
        path: path.to_string_lossy().to_string(),
        tiers: tura_config_tiers(&root),
        error: None,
    }
}

fn tura_config_tiers(root: &Value) -> Vec<TuraConfigTier> {
    let tier_names = configured_tier_names(root);
    tier_names
        .into_iter()
        .map(|tier| TuraConfigTier {
            current: current_tier_selection(root, &tier),
            options: configured_key_options(root, &tier),
            tier,
        })
        .collect()
}

fn configured_tier_names(root: &Value) -> Vec<String> {
    root.pointer("/model_catalog/tiers")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(ToString::to_string)
                .collect::<Vec<_>>()
        })
        .filter(|items| !items.is_empty())
        .unwrap_or_else(|| {
            root.get("routes")
                .and_then(Value::as_object)
                .map(|routes| routes.keys().cloned().collect())
                .unwrap_or_default()
        })
}

fn current_tier_selection(root: &Value, tier: &str) -> Option<TuraConfigSelection> {
    let provider = root
        .get("routes")?
        .get(tier)?
        .get("providers")?
        .as_array()?
        .first()?;
    Some(TuraConfigSelection {
        provider: provider.get("provider")?.as_str()?.to_string(),
        model: provider.get("model")?.as_str()?.to_string(),
    })
}

fn configured_key_options(root: &Value, tier: &str) -> Vec<TuraConfigOption> {
    let Some(providers) = root
        .pointer("/model_catalog/providers")
        .and_then(Value::as_object)
    else {
        return Vec::new();
    };
    let mut options = Vec::new();
    for (provider_id, provider) in providers {
        if !provider_has_configured_key(provider_id, provider) {
            continue;
        }
        let provider_name = provider
            .get("display_name")
            .and_then(Value::as_str)
            .unwrap_or(provider_id)
            .to_string();
        let Some(models) = provider
            .get("models")
            .and_then(|models| models.get(tier))
            .and_then(Value::as_array)
        else {
            continue;
        };
        for model in models {
            let Some(model_id) = model_id(model) else {
                continue;
            };
            options.push(TuraConfigOption {
                provider: provider_id.to_string(),
                provider_name: provider_name.clone(),
                model: model_id.to_string(),
                model_name: model
                    .get("name")
                    .and_then(Value::as_str)
                    .filter(|value| !value.trim().is_empty())
                    .unwrap_or(model_id)
                    .to_string(),
            });
        }
    }
    options
}

fn model_id(model: &Value) -> Option<&str> {
    model
        .as_str()
        .or_else(|| model.get("id").and_then(Value::as_str))
        .filter(|value| !value.trim().is_empty())
}

fn provider_has_configured_key(provider_id: &str, provider: &Value) -> bool {
    let mut env_names = provider
        .get("env")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect::<Vec<_>>();
    if let Some(token_env) = provider.get("token_env").and_then(Value::as_str) {
        env_names.push(token_env);
    }
    if let Some(entry) = tura_llm_rust::provider_auth_registry_entry(provider_id) {
        if let Some(token_env) = entry.token_env {
            env_names.push(token_env);
        }
        if let Some(refresh_env) = entry.refresh_env {
            env_names.push(refresh_env);
        }
    }
    env_names.into_iter().any(config_key_exists)
}

fn config_key_exists(key: &str) -> bool {
    std::env::var(key)
        .ok()
        .or_else(|| tura_llm_rust::TuraConfig::default().get(key))
        .is_some_and(|value| !value.trim().is_empty())
}

fn update_tura_config_tier(path: &PathBuf, payload: &TuraConfigUpdate) -> Result<(), String> {
    let content = std::fs::read_to_string(path).map_err(|err| err.to_string())?;
    let mut root: Value = serde_json::from_str(&content).map_err(|err| err.to_string())?;
    if !option_exists(&root, &payload.tier, &payload.provider, &payload.model) {
        return Err(format!(
            "{} / {} is not available for tier {} with configured credentials",
            payload.provider, payload.model, payload.tier
        ));
    }
    let routes = root
        .get_mut("routes")
        .and_then(Value::as_object_mut)
        .ok_or_else(|| "routes must be an object".to_string())?;
    let route = routes
        .entry(payload.tier.clone())
        .or_insert_with(|| serde_json::json!({ "providers": [] }));
    let route = route
        .as_object_mut()
        .ok_or_else(|| format!("route {} must be an object", payload.tier))?;
    let providers = route
        .entry("providers".to_string())
        .or_insert_with(|| serde_json::json!([]));
    let providers = providers
        .as_array_mut()
        .ok_or_else(|| format!("route {} providers must be an array", payload.tier))?;
    let next = serde_json::json!({
        "provider": payload.provider,
        "model": payload.model
    });
    if let Some(first) = providers.first_mut() {
        *first = next;
    } else {
        providers.push(next);
    }
    let formatted = serde_json::to_string_pretty(&root).map_err(|err| err.to_string())?;
    std::fs::write(path, format!("{formatted}\n")).map_err(|err| err.to_string())
}

fn option_exists(root: &Value, tier: &str, provider_id: &str, model: &str) -> bool {
    configured_key_options(root, tier)
        .iter()
        .any(|option| option.provider == provider_id && option.model == model)
}

fn gui_config_path() -> PathBuf {
    std::env::current_dir()
        .unwrap_or_default()
        .join("config")
        .join("gui_config.toml")
}

fn text_response(status: StatusCode, body: String) -> Response<String> {
    Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
        .body(body)
        .expect("text response is valid")
}

// ============================================================================
// Global Events (SSE)
// ============================================================================

pub async fn global_event() -> Sse<impl futures::Stream<Item = Result<SseEvent, Infallible>>> {
    let state = EventStreamState {
        first: true,
        seen_messages: seen_message_counts(),
    };
    let stream = futures::stream::unfold(state, |mut state| async move {
        loop {
            let event = if state.first {
                state.first = false;
                Some(GlobalEvent::ServerConnected {
                    properties: std::collections::HashMap::new(),
                })
            } else {
                session_store()
                    .pop_event()
                    .or_else(|| scan_message_events(&mut state.seen_messages))
            };

            if let Some(event) = event {
                let directory = event_directory(&event);
                let data = serde_json::json!({
                    "directory": directory,
                    "payload": event,
                });
                let item = SseEvent::default().data(data.to_string());
                return Some((Ok(item), state));
            }

            tokio::time::sleep(Duration::from_millis(250)).await;
        }
    });

    Sse::new(stream).keep_alive(KeepAlive::default())
}

struct EventStreamState {
    first: bool,
    seen_messages: HashMap<String, usize>,
}

fn seen_message_counts() -> HashMap<String, usize> {
    session_store()
        .list_sessions()
        .into_iter()
        .map(|session| {
            let count = session_store().get_messages(&session.id).len();
            (session.id, count)
        })
        .collect()
}

fn scan_message_events(seen: &mut HashMap<String, usize>) -> Option<GlobalEvent> {
    for session in session_store().list_sessions() {
        let messages = session_store().get_messages(&session.id);
        let count = messages.len();
        let previous = seen.entry(session.id.clone()).or_insert(0);
        if count <= *previous {
            continue;
        }

        let message = messages.get(*previous).cloned()?;
        *previous += 1;
        return Some(GlobalEvent::MessageUpdated {
            properties: MessageUpdatedProperties {
                session_id: session.id,
                info: crate::api::session::api_message_from_store(message),
            },
        });
    }

    None
}

fn event_directory(event: &GlobalEvent) -> String {
    let session_id = match event {
        GlobalEvent::SessionCreated { properties } => {
            return properties.info.directory.clone().unwrap_or_default()
        }
        GlobalEvent::SessionUpdated { properties } => {
            return properties.info.directory.clone().unwrap_or_default()
        }
        GlobalEvent::SessionDeleted { properties } => {
            return properties.info.directory.clone().unwrap_or_default()
        }
        GlobalEvent::SessionStatus { properties } => Some(properties.session_id.as_str()),
        GlobalEvent::MessageUpdated { properties } => Some(properties.session_id.as_str()),
        GlobalEvent::MessageRemoved { properties } => Some(properties.session_id.as_str()),
        GlobalEvent::MessagePartDelta { properties } => Some(properties.session_id.as_str()),
        GlobalEvent::MessagePartUpdated { properties } => Some(properties.session_id.as_str()),
        GlobalEvent::TodoUpdated { properties } => {
            properties.get("sessionID").and_then(|value| value.as_str())
        }
        GlobalEvent::ServerInstanceDisposed { properties } => return properties.directory.clone(),
        GlobalEvent::ProjectUpdated { properties } => return properties.worktree.clone(),
        GlobalEvent::ServerConnected { .. } => return "global".to_string(),
    };

    session_id
        .and_then(|id| session_store().get_session(id))
        .and_then(|session| session.directory)
        .unwrap_or_else(|| "global".to_string())
}

pub async fn sync_event() -> Json<SyncEvent> {
    Json(SyncEvent::SessionUpdated {
        properties: global_store().get_or_create_session(),
    })
}

// ============================================================================
// Global Dispose
// ============================================================================

pub async fn dispose() -> Json<bool> {
    Json(true)
}

// ============================================================================
// Global Upgrade
// ============================================================================

pub async fn upgrade(Json(_payload): Json<UpgradeRequest>) -> Json<UpgradeResponse> {
    Json(UpgradeResponse {
        success: false,
        version: Some(env!("CARGO_PKG_VERSION").to_string()),
        error: Some("Self-upgrade is not implemented by this gateway build.".to_string()),
    })
}
