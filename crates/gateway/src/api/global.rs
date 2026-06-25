//! Global API handlers (health, config, events)

use crate::contracts::*;
use crate::mock::global_store;
use crate::session::session_store;
use axum::{
    extract::Path as AxumPath,
    response::sse::{Event as SseEvent, KeepAlive, Sse},
    Json,
};
use serde_json::Value;
use std::convert::Infallible;
use std::path::{Path, PathBuf};
use std::time::Duration;

// ============================================================================
// Health
// ============================================================================

pub async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        healthy: true,
        version: env!("CARGO_PKG_VERSION").to_string(),
        root: gateway_identity_root(),
        exe_dir: gateway_exe_dir(),
        dev_log_path: gateway_dev_log_path(),
    })
}

/// Returns the provider LLM call log directory when dev logging is active.
///
/// Logging is active when `LOG_PATH` is explicitly set, or for `dev` build-kind
/// (the repo-local `bin/` package always writes). A `release` build only logs
/// via the explicit `LOG_PATH` opt-in. Uses TURA_PROJECT_ROOT for the default
/// path so the reported location matches what the gateway actually writes to.
fn gateway_dev_log_path() -> Option<String> {
    let log_path_env = std::env::var("LOG_PATH").ok();
    let dev_build = tura_path::build_kind() == "dev";
    if log_path_env.is_none() && !dev_build {
        return None;
    }
    let root = log_path_env.map(PathBuf::from).unwrap_or_else(|| {
        std::env::var_os("TURA_PROJECT_ROOT")
            .map(PathBuf::from)
            .filter(|p| p.exists())
            .or_else(|| std::env::current_dir().ok())
            .unwrap_or_default()
            .join("log")
            .join("provider")
    });
    Some(canonical_string(&root))
}

/// Canonical runtime root the gateway is serving. Clients compare this against
/// their own package root to decide whether a reachable gateway is "their own".
pub(crate) fn gateway_identity_root() -> String {
    let root = std::env::var_os("TURA_PROJECT_ROOT")
        .map(PathBuf::from)
        .filter(|path| path.exists())
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_default();
    canonical_string(&root)
}

fn gateway_exe_dir() -> String {
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(PathBuf::from))
        .unwrap_or_default();
    canonical_string(&exe_dir)
}

fn canonical_string(path: &std::path::Path) -> String {
    let resolved = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    strip_verbatim_prefix(&resolved.to_string_lossy())
}

/// Strip the Windows `\\?\` (and `\\?\UNC\`) verbatim prefix so paths compare
/// equal to the plain forms other tools (Node `realpathSync`, etc.) produce.
///
/// Delegates to [`tura_path::strip_verbatim_prefix`] — the single source of
/// truth for path normalization — and is re-exported here for existing callers.
pub fn strip_verbatim_prefix(path: &str) -> String {
    tura_path::strip_verbatim_prefix(path)
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

fn read_tura_config_response() -> TuraConfigResponse {
    let path = crate::api::provider::config::provider_config_path();
    match read_json_config(&path) {
        Ok(root) => tura_config_response_from_value(path, root),
        Err(error) => TuraConfigResponse {
            path: path.to_string_lossy().to_string(),
            tiers: Vec::new(),
            error: Some(error),
        },
    }
}

fn read_json_config(path: &Path) -> Result<Value, String> {
    let content = std::fs::read_to_string(path).map_err(|err| {
        format!(
            "failed to read Tura provider config {}: {err}",
            path.display()
        )
    })?;
    serde_json::from_str::<Value>(&content).map_err(|err| {
        format!(
            "failed to parse Tura provider config {}: {err}",
            path.display()
        )
    })
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
    let provider_id = provider.get("provider")?.as_str()?;
    let model_id = provider.get("model")?.as_str()?;
    if is_hidden_model(root, provider_id, tier, model_id) {
        return None;
    }
    Some(TuraConfigSelection {
        provider: provider_id.to_string(),
        model: model_id.to_string(),
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
            if model_is_hidden(model) || looks_like_claude_model(model) {
                continue;
            }
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

fn is_hidden_model(root: &Value, provider_id: &str, tier: &str, selected_model_id: &str) -> bool {
    let Some(models) = root
        .pointer("/model_catalog/providers")
        .and_then(Value::as_object)
        .and_then(|providers| providers.get(provider_id))
        .and_then(|provider| provider.get("models"))
        .and_then(|models| models.get(tier))
        .and_then(Value::as_array)
    else {
        return looks_like_claude_id(selected_model_id);
    };
    models
        .iter()
        .find(|model| model_id(model).is_some_and(|id| id == selected_model_id))
        .map(|model| model_is_hidden(model) || looks_like_claude_model(model))
        .unwrap_or_else(|| looks_like_claude_id(selected_model_id))
}

fn model_is_hidden(model: &Value) -> bool {
    model
        .get("visible")
        .and_then(Value::as_bool)
        .is_some_and(|visible| !visible)
}

fn looks_like_claude_model(model: &Value) -> bool {
    let fields = [
        model_id(model),
        model.get("name").and_then(Value::as_str),
        model.get("family").and_then(Value::as_str),
    ];
    fields.into_iter().flatten().any(looks_like_claude_id)
}

fn looks_like_claude_id(value: &str) -> bool {
    let value = value.trim().to_ascii_lowercase();
    value == "claude"
        || value.starts_with("claude-")
        || value.starts_with("anthropic.claude-")
        || value.contains("/claude-")
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

fn update_tura_config_tier(path: &Path, payload: &TuraConfigUpdate) -> Result<(), String> {
    let content = std::fs::read_to_string(path).map_err(|err| {
        format!(
            "failed to read Tura provider config {}: {err}",
            path.display()
        )
    })?;
    let mut root: Value = serde_json::from_str(&content).map_err(|err| {
        format!(
            "failed to parse Tura provider config {}: {err}",
            path.display()
        )
    })?;
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
    let formatted = serde_json::to_string_pretty(&root)
        .map_err(|err| format!("failed to serialize Tura provider config: {err}"))?;
    std::fs::write(path, format!("{formatted}\n")).map_err(|err| {
        format!(
            "failed to write Tura provider config {}: {err}",
            path.display()
        )
    })
}

fn option_exists(root: &Value, tier: &str, provider_id: &str, model: &str) -> bool {
    configured_key_options(root, tier)
        .iter()
        .any(|option| option.provider == provider_id && option.model == model)
}

// ============================================================================
// Global Events (SSE)
// ============================================================================

pub async fn global_event() -> Sse<impl futures::Stream<Item = Result<SseEvent, Infallible>>> {
    let state = EventStreamState {
        first: true,
        event_cursor: session_store().event_cursor(),
        session_id: None,
    };
    Sse::new(event_stream(state)).keep_alive(KeepAlive::default())
}

pub async fn session_event(
    AxumPath(session_id): AxumPath<String>,
) -> Sse<impl futures::Stream<Item = Result<SseEvent, Infallible>>> {
    let state = EventStreamState {
        first: true,
        event_cursor: session_store().event_cursor(),
        session_id: Some(session_id),
    };
    Sse::new(event_stream(state)).keep_alive(KeepAlive::default())
}

fn event_stream(
    state: EventStreamState,
) -> impl futures::Stream<Item = Result<SseEvent, Infallible>> {
    let stream = futures::stream::unfold(state, |mut state| async move {
        loop {
            let event = if state.first {
                state.first = false;
                Some(GlobalEvent::ServerConnected {
                    properties: std::collections::HashMap::new(),
                })
            } else {
                session_store().next_event(&mut state.event_cursor)
            };

            if let Some(event) = event {
                if !event_visible_to_frontend(&event) {
                    continue;
                }
                if !event_matches_session_filter(&event, state.session_id.as_deref()) {
                    continue;
                }
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
    stream
}

fn event_visible_to_frontend(event: &GlobalEvent) -> bool {
    match event {
        GlobalEvent::MessageUpdated { properties } => properties.info.role != MessageRole::System,
        _ => true,
    }
}

struct EventStreamState {
    first: bool,
    event_cursor: u64,
    session_id: Option<String>,
}

fn event_matches_session_filter(event: &GlobalEvent, session_id: Option<&str>) -> bool {
    let Some(session_id) = session_id else {
        return true;
    };
    matches!(event, GlobalEvent::ServerConnected { .. })
        || event_session_id(event).is_some_and(|event_session_id| event_session_id == session_id)
}

fn event_session_id(event: &GlobalEvent) -> Option<&str> {
    match event {
        GlobalEvent::SessionCreated { properties } => Some(properties.session_id.as_str()),
        GlobalEvent::SessionUpdated { properties } => Some(properties.session_id.as_str()),
        GlobalEvent::SessionDeleted { properties } => Some(properties.session_id.as_str()),
        GlobalEvent::SessionStatus { properties } => Some(properties.session_id.as_str()),
        GlobalEvent::MessageUpdated { properties } => Some(properties.session_id.as_str()),
        GlobalEvent::MessageRemoved { properties } => Some(properties.session_id.as_str()),
        GlobalEvent::MessagePartDelta { properties } => Some(properties.session_id.as_str()),
        GlobalEvent::MessagePartUpdated { properties } => Some(properties.session_id.as_str()),
        GlobalEvent::CommandUpdated { properties } => Some(properties.session_id.as_str()),
        GlobalEvent::TodoUpdated { properties } => {
            properties.get("sessionID").and_then(|value| value.as_str())
        }
        GlobalEvent::ServerConnected { .. }
        | GlobalEvent::ServerInstanceDisposed { .. }
        | GlobalEvent::ProjectUpdated { .. } => None,
    }
}

fn event_directory(event: &GlobalEvent) -> String {
    let session_id = match event {
        GlobalEvent::SessionCreated { properties } => {
            return properties.info.directory.clone().unwrap_or_default();
        }
        GlobalEvent::SessionUpdated { properties } => {
            return properties.info.directory.clone().unwrap_or_default();
        }
        GlobalEvent::SessionDeleted { properties } => {
            return properties.info.directory.clone().unwrap_or_default();
        }
        GlobalEvent::SessionStatus { properties } => Some(properties.session_id.as_str()),
        GlobalEvent::MessageUpdated { properties } => Some(properties.session_id.as_str()),
        GlobalEvent::MessageRemoved { properties } => Some(properties.session_id.as_str()),
        GlobalEvent::MessagePartDelta { properties } => Some(properties.session_id.as_str()),
        GlobalEvent::MessagePartUpdated { properties } => Some(properties.session_id.as_str()),
        GlobalEvent::CommandUpdated { properties } => Some(properties.session_id.as_str()),
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
        properties: Box::new(global_store().get_or_create_session()),
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

#[cfg(test)]
mod tests {
    use super::{
        event_matches_session_filter, event_visible_to_frontend, read_json_config,
        update_tura_config_tier, TuraConfigUpdate,
    };
    use crate::contracts::{
        GlobalEvent, Message, MessageRole, MessageUpdatedProperties, SessionStatusProperties,
    };

    #[test]
    fn read_json_config_reports_missing_path_context() {
        let temp = tempfile::tempdir().expect("tempdir");
        let path = temp.path().join("missing-provider.json");

        let error = read_json_config(&path).expect_err("missing config should fail");

        let message = &error;
        assert!(
            message.contains("failed to read Tura provider config"),
            "error should describe the failed operation: {message}"
        );
        assert!(
            message.contains(&path.to_string_lossy().to_string()),
            "error should include the config path: {message}"
        );
    }

    #[test]
    fn update_tura_config_tier_reports_parse_path_context() {
        let temp = tempfile::tempdir().expect("tempdir");
        let path = temp.path().join("provider.json");
        std::fs::write(&path, "{not-json").expect("write invalid config");

        let error = update_tura_config_tier(
            &path,
            &TuraConfigUpdate {
                tier: "fast".to_string(),
                provider: "codex".to_string(),
                model: "gpt-5.5".to_string(),
            },
        )
        .expect_err("invalid config should fail");

        let message = &error;
        assert!(
            message.contains("failed to parse Tura provider config"),
            "error should describe the failed operation: {message}"
        );
        assert!(
            message.contains(&path.to_string_lossy().to_string()),
            "error should include the config path: {message}"
        );
    }

    #[test]
    fn session_event_filter_keeps_only_matching_session_events_and_connection_events() {
        let matching = GlobalEvent::MessageUpdated {
            properties: MessageUpdatedProperties {
                session_id: "session-a".to_string(),
                info: Message {
                    id: "runtime-1.message".to_string(),
                    session_id: "session-a".to_string(),
                    role: MessageRole::Assistant,
                    parts: Vec::new(),
                    created_at: 1,
                    updated_at: 1,
                    parent_id: None,
                },
            },
        };
        let other = GlobalEvent::SessionStatus {
            properties: SessionStatusProperties {
                session_id: "session-b".to_string(),
                updated_at: 1,
                status: serde_json::json!({"state": "busy"}),
                context_tokens: Default::default(),
                usage: Default::default(),
            },
        };
        let connected = GlobalEvent::ServerConnected {
            properties: std::collections::HashMap::new(),
        };

        assert!(event_matches_session_filter(&matching, Some("session-a")));
        assert!(!event_matches_session_filter(&other, Some("session-a")));
        assert!(event_matches_session_filter(&connected, Some("session-a")));
        assert!(event_matches_session_filter(&other, None));
    }

    #[test]
    fn event_visibility_filters_system_message_updates() {
        let system_update = GlobalEvent::MessageUpdated {
            properties: MessageUpdatedProperties {
                session_id: "session-a".to_string(),
                info: Message {
                    id: "msg-system".to_string(),
                    session_id: "session-a".to_string(),
                    role: MessageRole::System,
                    parts: Vec::new(),
                    created_at: 1,
                    updated_at: 1,
                    parent_id: None,
                },
            },
        };
        let assistant_update = GlobalEvent::MessageUpdated {
            properties: MessageUpdatedProperties {
                session_id: "session-a".to_string(),
                info: Message {
                    id: "msg-assistant".to_string(),
                    session_id: "session-a".to_string(),
                    role: MessageRole::Assistant,
                    parts: Vec::new(),
                    created_at: 1,
                    updated_at: 1,
                    parent_id: None,
                },
            },
        };

        assert!(!event_visible_to_frontend(&system_update));
        assert!(event_visible_to_frontend(&assistant_update));
    }
}
