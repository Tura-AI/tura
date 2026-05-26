//! Provider / Auth API handlers

use crate::api::types::*;
use crate::mock::global_store;
use axum::extract::{Json, Path, Query};
use axum::response::{Html, IntoResponse};
use base64::Engine;
use chrono::Utc;
use reqwest::Url;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::{Path as FsPath, PathBuf};
use tokio::time::{sleep, timeout, Duration, Instant};
use tura_llm_rust::{AuthMethodKind, OAuthAuthorizeKind};
use uuid::Uuid;

// ============================================================================
// Provider List
// ============================================================================

pub async fn list_providers() -> Json<ProviderListResponse> {
    let settings = tura_llm_rust::Settings::default().await.ok();
    if let Some(settings) = settings.as_deref() {
        if let Some(route) = active_agent_route(settings) {
            return Json(provider_list_for_route(settings, route));
        }
    }

    let providers = global_store().list_providers();
    let mut all = Vec::new();
    let mut defaults = HashMap::new();
    let mut connected = Vec::new();

    for provider in providers {
        let default_model = configured_model_for_provider(settings.as_deref(), &provider.id)
            .unwrap_or_else(|| default_model_for_provider(&provider.id));
        let mut models = configured_models_for_provider(settings.as_deref(), &provider.id)
            .into_iter()
            .map(|model| (model.id.clone(), model))
            .collect::<HashMap<_, _>>();
        models.insert(default_model.id.clone(), default_model.clone());

        defaults.insert(provider.id.clone(), default_model.id);
        if provider.enabled {
            connected.push(provider.id.clone());
        }

        all.push(SdkProvider {
            id: provider.id,
            name: provider.name,
            source: "config".to_string(),
            env: Vec::new(),
            key: None,
            options: HashMap::new(),
            models,
            api: None,
            npm: None,
        });
    }

    enrich_provider_list(&mut all, &mut connected, &providers_enabled_set());

    Json(ProviderListResponse {
        all,
        default: defaults,
        connected,
    })
}

pub(crate) async fn active_agent_model() -> Option<AgentModel> {
    let settings = tura_llm_rust::Settings::default().await.ok()?;
    let route = active_agent_route(settings.as_ref())?;
    let primary = route.providers.first()?;
    Some(AgentModel {
        provider_id: primary.provider.clone(),
        model_id: primary.model.clone(),
    })
}

fn provider_list_for_route(
    settings: &tura_llm_rust::Settings,
    route: &tura_llm_rust::RouteConfig,
) -> ProviderListResponse {
    let mut all = Vec::<SdkProvider>::new();
    let mut indexes = HashMap::<String, usize>::new();
    let mut defaults = HashMap::<String, String>::new();
    let mut connected = Vec::<String>::new();

    for provider in &route.providers {
        let model_id = normalize_model_id(&provider.provider, &provider.model);
        let index = match indexes.get(&provider.provider).copied() {
            Some(index) => index,
            None => {
                let index = all.len();
                indexes.insert(provider.provider.clone(), index);
                defaults.insert(provider.provider.clone(), model_id.clone());
                connected.push(provider.provider.clone());
                all.push(SdkProvider {
                    id: provider.provider.clone(),
                    name: provider_display_name(&provider.provider),
                    source: "config".to_string(),
                    env: Vec::new(),
                    key: None,
                    options: HashMap::new(),
                    models: HashMap::new(),
                    api: None,
                    npm: None,
                });
                index
            }
        };

        all[index].models.insert(
            model_id.clone(),
            sdk_model_from_config(&provider.provider, &model_id),
        );
    }

    for (provider_id, models) in provider_model_catalog() {
        let index = match indexes.get(provider_id).copied() {
            Some(index) => index,
            None => {
                let index = all.len();
                indexes.insert(provider_id.to_string(), index);
                defaults.insert(provider_id.to_string(), models[0].to_string());
                all.push(SdkProvider {
                    id: provider_id.to_string(),
                    name: provider_display_name(provider_id),
                    source: "config".to_string(),
                    env: Vec::new(),
                    key: None,
                    options: HashMap::new(),
                    models: HashMap::new(),
                    api: None,
                    npm: None,
                });
                index
            }
        };

        for model_id in models {
            all[index]
                .models
                .entry(model_id.to_string())
                .or_insert_with(|| sdk_model_from_config(provider_id, model_id));
        }
    }

    for (provider_id, model_ids) in configured_model_catalog(settings) {
        let index = match indexes.get(provider_id.as_str()).copied() {
            Some(index) => index,
            None => {
                let index = all.len();
                indexes.insert(provider_id.clone(), index);
                if let Some(model_id) = model_ids.first() {
                    defaults.insert(provider_id.clone(), model_id.clone());
                }
                all.push(SdkProvider {
                    id: provider_id.clone(),
                    name: provider_display_name(&provider_id),
                    source: "config".to_string(),
                    env: Vec::new(),
                    key: None,
                    options: HashMap::new(),
                    models: HashMap::new(),
                    api: None,
                    npm: None,
                });
                index
            }
        };

        for model_id in model_ids {
            all[index]
                .models
                .entry(model_id.clone())
                .or_insert_with(|| sdk_model_from_config(&provider_id, &model_id));
        }
    }

    let store_connected = global_store()
        .list_providers()
        .into_iter()
        .filter(|provider| provider.enabled)
        .map(|provider| provider.id)
        .collect::<std::collections::HashSet<_>>();

    for (provider_id, model_id) in browser_login_provider_defaults() {
        if indexes.contains_key(provider_id) {
            if store_connected.contains(provider_id)
                && !connected.iter().any(|id| id == provider_id)
            {
                connected.push(provider_id.to_string());
            }
            continue;
        }
        defaults.insert(provider_id.to_string(), model_id.to_string());
        if store_connected.contains(provider_id) && !connected.iter().any(|id| id == provider_id) {
            connected.push(provider_id.to_string());
        }
        all.push(SdkProvider {
            id: provider_id.to_string(),
            name: provider_display_name(provider_id),
            source: "config".to_string(),
            env: Vec::new(),
            key: None,
            options: HashMap::new(),
            models: HashMap::from([(
                model_id.to_string(),
                sdk_model_from_config(provider_id, model_id),
            )]),
            api: None,
            npm: None,
        });
    }

    enrich_provider_list(&mut all, &mut connected, &store_connected);

    ProviderListResponse {
        all,
        default: defaults,
        connected,
    }
}

fn active_agent_route(settings: &tura_llm_rust::Settings) -> Option<&tura_llm_rust::RouteConfig> {
    route_by_name(settings, &active_agent_route_name())
}

fn active_agent_route_name() -> String {
    #[derive(serde::Deserialize)]
    struct RegistryAgent {
        provider: RegistryProvider,
    }

    #[derive(serde::Deserialize)]
    struct RegistryProvider {
        tura_llm_name: String,
    }

    let registry_path = std::env::current_dir()
        .unwrap_or_default()
        .join("agents")
        .join("interface")
        .join("Icoding_agent.json");

    std::fs::read_to_string(registry_path)
        .ok()
        .and_then(|content| serde_json::from_str::<RegistryAgent>(&content).ok())
        .map(|agent| agent.provider.tura_llm_name)
        .filter(|name| !name.trim().is_empty())
        .unwrap_or_else(crate::session::manager::coding_agent_provider)
}

fn route_by_name<'a>(
    settings: &'a tura_llm_rust::Settings,
    name: &str,
) -> Option<&'a tura_llm_rust::RouteConfig> {
    match name {
        "tura_general" => Some(&settings.tura_general),
        "tura_office" => Some(&settings.tura_office),
        "tura_creative" => Some(&settings.tura_creative),
        "tura_translator" => Some(&settings.tura_translator),
        "tura_validator" => Some(&settings.tura_validator),
        "tura_validator_advanced" => Some(&settings.tura_validator_advanced),
        "tura_classifier" => Some(&settings.tura_classifier),
        "tura_embedding" => Some(&settings.tura_embedding),
        "tura_coder" => Some(&settings.tura_coder),
        "tura_coder_advanced" => Some(&settings.tura_coder_advanced),
        "tura_planner" => Some(&settings.tura_planner),
        "tura_planner_advanced" => Some(&settings.tura_planner_advanced),
        "tura_roleplay" => Some(&settings.tura_roleplay),
        "tura_professional" => Some(&settings.tura_professional),
        "tura_math" => Some(&settings.tura_math),
        "tura_academic" => Some(&settings.tura_academic),
        _ => None,
    }
}

fn provider_display_name(provider_id: &str) -> String {
    tura_llm_rust::provider_auth_registry_entry(provider_id)
        .map(|entry| entry.display_name)
        .unwrap_or(provider_id)
        .to_string()
}

fn configured_model_for_provider(
    settings: Option<&tura_llm_rust::Settings>,
    provider_id: &str,
) -> Option<SdkProviderModel> {
    let settings = settings?;
    let route = [
        &settings.tura_general,
        &settings.tura_office,
        &settings.tura_creative,
        &settings.tura_translator,
        &settings.tura_validator,
        &settings.tura_validator_advanced,
        &settings.tura_classifier,
        &settings.tura_embedding,
        &settings.tura_coder,
        &settings.tura_coder_advanced,
        &settings.tura_planner,
        &settings.tura_planner_advanced,
        &settings.tura_roleplay,
        &settings.tura_professional,
        &settings.tura_math,
        &settings.tura_academic,
    ]
    .into_iter()
    .flat_map(|route| route.providers.iter())
    .find(|provider| provider.provider == provider_id)?;

    Some(sdk_model_from_config(
        provider_id,
        &normalize_model_id(provider_id, &route.model),
    ))
}

fn configured_models_for_provider(
    settings: Option<&tura_llm_rust::Settings>,
    provider_id: &str,
) -> Vec<SdkProviderModel> {
    let Some(settings) = settings else {
        return Vec::new();
    };
    configured_model_catalog(settings)
        .remove(provider_id)
        .unwrap_or_default()
        .into_iter()
        .map(|model_id| sdk_model_from_config(provider_id, &model_id))
        .collect()
}

fn configured_model_catalog(settings: &tura_llm_rust::Settings) -> HashMap<String, Vec<String>> {
    let mut catalog = HashMap::<String, Vec<String>>::new();
    for route in all_routes(settings) {
        for provider in &route.providers {
            let model_id = normalize_model_id(&provider.provider, &provider.model);
            let models = catalog.entry(provider.provider.clone()).or_default();
            if !models.iter().any(|existing| existing == &model_id) {
                models.push(model_id);
            }
        }
    }
    for models in catalog.values_mut() {
        models.sort();
    }
    catalog
}

fn all_routes(settings: &tura_llm_rust::Settings) -> [&tura_llm_rust::RouteConfig; 16] {
    [
        &settings.tura_general,
        &settings.tura_office,
        &settings.tura_creative,
        &settings.tura_translator,
        &settings.tura_validator,
        &settings.tura_validator_advanced,
        &settings.tura_classifier,
        &settings.tura_embedding,
        &settings.tura_coder,
        &settings.tura_coder_advanced,
        &settings.tura_planner,
        &settings.tura_planner_advanced,
        &settings.tura_roleplay,
        &settings.tura_professional,
        &settings.tura_math,
        &settings.tura_academic,
    ]
}

fn normalize_model_id(provider_id: &str, model_id: &str) -> String {
    let prefix = format!("{}/", provider_runtime_id(provider_id));
    model_id
        .strip_prefix(&prefix)
        .unwrap_or(model_id)
        .to_string()
}

fn default_model_for_provider(provider_id: &str) -> SdkProviderModel {
    sdk_model_from_config(provider_id, "default")
}

fn sdk_model_from_config(provider_id: &str, model_id: &str) -> SdkProviderModel {
    SdkProviderModel {
        id: model_id.to_string(),
        name: model_id.to_string(),
        family: provider_id.to_string(),
        release_date: "2026-01-01".to_string(),
        attachment: true,
        reasoning: true,
        temperature: true,
        tool_call: true,
        limit: SdkProviderModelLimit {
            context: 200_000,
            input: 200_000,
            output: 16_384,
        },
        modalities: SdkProviderModelModalities {
            input: vec!["text".to_string(), "image".to_string(), "pdf".to_string()],
            output: vec!["text".to_string()],
        },
        options: HashMap::new(),
        status: None,
    }
}

fn browser_login_provider_defaults() -> [(&'static str, &'static str); 3] {
    [
        ("openai", "gpt-5.5"),
        ("anthropic", "claude-sonnet-4.5"),
        ("antigravity", "antigravity-browser"),
    ]
}

fn provider_model_catalog() -> Vec<(&'static str, &'static [&'static str])> {
    tura_llm_rust::provider_auth_registry()
        .iter()
        .filter(|entry| !entry.supported_models.is_empty())
        .map(|entry| (entry.provider_id, entry.supported_models))
        .collect()
}

fn model_supported_by_provider(provider_id: &str, model_id: &str) -> bool {
    provider_model_catalog()
        .into_iter()
        .find(|(id, _)| *id == provider_id)
        .map(|(_, models)| models.iter().any(|candidate| candidate == &model_id))
        .unwrap_or(false)
}

fn provider_runtime_id(provider_id: &str) -> &str {
    tura_llm_rust::runtime_provider_id(provider_id)
}

fn providers_enabled_set() -> std::collections::HashSet<String> {
    global_store()
        .list_providers()
        .into_iter()
        .filter(|provider| provider.enabled)
        .map(|provider| provider.id)
        .collect()
}

fn enrich_provider_list(
    providers: &mut [SdkProvider],
    connected: &mut Vec<String>,
    store_connected: &std::collections::HashSet<String>,
) {
    for provider in providers {
        let env_key = provider_env_key(&provider.id);
        let has_key = provider_key_exists(&provider.id);
        provider.env = vec![env_key.clone()];
        provider.key = has_key.then_some(env_key);
        provider.source = if has_key {
            "env".to_string()
        } else if store_connected.contains(&provider.id) {
            "api".to_string()
        } else {
            "config".to_string()
        };
        if has_key && !connected.iter().any(|id| id == &provider.id) {
            connected.push(provider.id.clone());
        }
    }
}

fn provider_base_url(settings: &tura_llm_rust::Settings, provider_id: &str) -> Option<String> {
    let provider_id = provider_runtime_id(provider_id);
    for route in all_routes(settings) {
        for provider in &route.providers {
            if provider.provider == provider_id {
                return Some(provider.base_url.clone());
            }
        }
    }
    None
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct ValidateModelRequest {
    #[serde(rename = "providerID")]
    pub provider_id: String,
    #[serde(rename = "modelID")]
    pub model_id: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ValidateModelResponse {
    pub ok: bool,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<serde_json::Value>,
}

pub async fn validate_model(
    Json(payload): Json<ValidateModelRequest>,
) -> Json<ValidateModelResponse> {
    let provider_id = payload.provider_id.trim();
    let model_id = payload.model_id.trim();
    if provider_id.is_empty() || model_id.is_empty() {
        return Json(ValidateModelResponse {
            ok: false,
            message: "providerID and modelID are required".to_string(),
            output: None,
        });
    }
    let runtime_provider_id = provider_runtime_id(provider_id);
    let settings = match tura_llm_rust::Settings::default().await {
        Ok(settings) => settings,
        Err(error) => {
            return Json(ValidateModelResponse {
                ok: false,
                message: format!("failed to load provider settings: {error}"),
                output: None,
            });
        }
    };
    let configured_models = configured_model_catalog(settings.as_ref());
    let explicitly_configured = configured_models
        .get(provider_id)
        .map(|models| models.iter().any(|configured| configured == model_id))
        .unwrap_or(false);
    if !model_supported_by_provider(provider_id, model_id) && !explicitly_configured {
        return Json(ValidateModelResponse {
            ok: false,
            message: format!("{provider_id}/{model_id} is not supported by this Codex runtime"),
            output: None,
        });
    }
    let connected_by_route = active_agent_route(settings.as_ref())
        .map(|route| {
            route
                .providers
                .iter()
                .any(|provider| provider.provider == runtime_provider_id)
        })
        .unwrap_or(false);
    let connected_by_auth = global_store()
        .list_providers()
        .into_iter()
        .any(|provider| provider.id == provider_id && provider.enabled)
        || provider_key_exists(provider_id);
    if !connected_by_route && !connected_by_auth {
        return Json(ValidateModelResponse {
            ok: false,
            message: format!("{provider_id} is not connected"),
            output: None,
        });
    }
    let Some(base_url) = provider_base_url(settings.as_ref(), provider_id) else {
        return Json(ValidateModelResponse {
            ok: false,
            message: format!("{provider_id} has no configured base URL"),
            output: None,
        });
    };

    let config = tura_llm_rust::ProviderConfig {
        provider: runtime_provider_id.to_string(),
        base_url,
        model: tura_llm_rust::Settings::normalize_model_name(runtime_provider_id, model_id),
        temperature: 0.0,
    };
    let options = tura_llm_rust::CallOptions {
        max_completion_tokens: Some(4),
        max_tokens: Some(4),
        ..Default::default()
    };
    let messages = vec![serde_json::json!({
        "role": "user",
        "content": "Reply with OK."
    })];
    let conf = tura_llm_rust::TuraConfig::default();
    match timeout(
        Duration::from_secs(20),
        config.call(&conf, messages, options),
    )
    .await
    {
        Ok(Ok(response)) => Json(ValidateModelResponse {
            ok: true,
            message: "model validation succeeded".to_string(),
            output: Some(response.content),
        }),
        Ok(Err(error)) => Json(ValidateModelResponse {
            ok: false,
            message: error.to_string(),
            output: None,
        }),
        Err(_) => Json(ValidateModelResponse {
            ok: false,
            message: "model validation timed out".to_string(),
            output: None,
        }),
    }
}

// ============================================================================
// Auth
// ============================================================================

pub async fn set_auth(
    Path(provider_id): Path<String>,
    Json(payload): Json<ProviderAuth>,
) -> Json<bool> {
    if payload
        .access
        .as_deref()
        .or(payload.key.as_deref())
        .map(str::trim)
        .unwrap_or_default()
        .is_empty()
    {
        return Json(false);
    }
    let saved = persist_provider_auth(&provider_id, &payload).is_ok();
    Json(saved && global_store().set_auth(&provider_id, payload))
}

pub async fn remove_auth(Path(provider_id): Path<String>) -> Json<bool> {
    Json(logout_provider_auth(&provider_id).is_ok() && global_store().remove_auth(&provider_id))
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ProviderAuthStatusResponse {
    pub provider_id: String,
    pub display_name: String,
    pub login: Option<String>,
    pub configured: bool,
    pub authenticated: bool,
    pub expired: Option<bool>,
    pub account_id: Option<String>,
    pub token_env: Option<String>,
    pub login_env: Option<String>,
    pub refresh_env: Option<String>,
    pub expires_env: Option<String>,
    pub updated_at: Option<String>,
    pub auth_state: tura_llm_rust::AuthState,
    pub runtime_state: tura_llm_rust::ProviderRuntimeState,
    pub last_error_category: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ProviderAuthActionResponse {
    pub ok: bool,
    pub provider_id: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<ProviderAuthStatusResponse>,
}

async fn refresh_provider_auth_if_needed(provider_id: &str, force: bool) -> Result<bool, String> {
    let Some(entry) = tura_llm_rust::provider_auth_registry_entry(provider_id) else {
        return Ok(false);
    };
    if !entry.capabilities.supports_oauth_refresh {
        return Ok(false);
    }
    let status = build_provider_auth_status(provider_id);
    if !matches!(status.login.as_deref(), Some("oauth")) {
        return Ok(false);
    }
    if !force && !provider_auth_expires_soon(&status) {
        return Ok(false);
    }
    match provider_id {
        "openai" => refresh_openai_provider_auth(provider_id, &status)
            .await
            .map(|_| true),
        "google" | "gemini" => refresh_google_provider_auth(provider_id, &status)
            .await
            .map(|_| true),
        _ => Ok(false),
    }
}

fn provider_auth_expires_soon(status: &ProviderAuthStatusResponse) -> bool {
    if status.expired == Some(true) {
        return true;
    }
    let Some(expires_at) = status
        .expires_env
        .as_deref()
        .and_then(config_value)
        .and_then(|value| value.parse::<i64>().ok())
    else {
        return false;
    };
    expires_at <= Utc::now().timestamp_millis() + 60_000
}

async fn refresh_openai_provider_auth(
    provider_id: &str,
    status: &ProviderAuthStatusResponse,
) -> Result<(), String> {
    refresh_oauth_provider_auth(
        provider_id,
        status,
        openai_oauth_token_url(),
        vec![("client_id".to_string(), openai_oauth_client_id())],
        "OpenAI",
    )
    .await
}

async fn refresh_google_provider_auth(
    provider_id: &str,
    status: &ProviderAuthStatusResponse,
) -> Result<(), String> {
    let mut extra_params = Vec::new();
    if let Some(client_id) = google_oauth_client_id(provider_id) {
        extra_params.push(("client_id".to_string(), client_id));
    }
    if let Some(client_secret) = google_oauth_client_secret(provider_id) {
        extra_params.push(("client_secret".to_string(), client_secret));
    }
    refresh_oauth_provider_auth(
        provider_id,
        status,
        google_oauth_token_url(),
        extra_params,
        "Google",
    )
    .await
}

async fn refresh_oauth_provider_auth(
    provider_id: &str,
    status: &ProviderAuthStatusResponse,
    token_url: String,
    extra_params: Vec<(String, String)>,
    display_name: &str,
) -> Result<(), String> {
    let refresh_env = status
        .refresh_env
        .as_deref()
        .ok_or_else(|| format!("{display_name} refresh env is not configured"))?;
    let refresh = config_value(refresh_env)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| format!("{refresh_env} is not configured"))?;
    let mut form = vec![
        ("grant_type".to_string(), "refresh_token".to_string()),
        ("refresh_token".to_string(), refresh.clone()),
    ];
    form.extend(extra_params);
    let response = reqwest::Client::new()
        .post(token_url)
        .header("content-type", "application/x-www-form-urlencoded")
        .form(&form)
        .send()
        .await
        .map_err(|error| error.to_string())?;
    let http_status = response.status();
    let body: serde_json::Value = response.json().await.map_err(|error| error.to_string())?;
    if !http_status.is_success() {
        return Err(format!(
            "{display_name} token endpoint returned {http_status}: {body}"
        ));
    }
    let access = body
        .get("access_token")
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| format!("{display_name} refresh response did not include access_token"))?
        .to_string();
    let refresh = body
        .get("refresh_token")
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(refresh.as_str())
        .to_string();
    let expires = Utc::now().timestamp_millis()
        + body
            .get("expires_in")
            .and_then(serde_json::Value::as_i64)
            .unwrap_or(3600)
            * 1000;
    let account_id = body
        .get("id_token")
        .and_then(serde_json::Value::as_str)
        .and_then(extract_account_id_from_jwt)
        .or_else(|| extract_account_id_from_jwt(&access))
        .or_else(|| status.account_id.clone());
    let auth = ProviderAuth {
        auth_type: "oauth".to_string(),
        key: Some(access.clone()),
        access: Some(access),
        refresh: Some(refresh),
        expires: Some(expires),
        account_id,
        metadata: Some(HashMap::from([(
            "login".to_string(),
            serde_json::Value::String("oauth".to_string()),
        )])),
    };
    persist_provider_auth(provider_id, &auth).map_err(|error| error.to_string())
}

pub async fn provider_auth_status(
    Path(provider_id): Path<String>,
) -> Json<ProviderAuthStatusResponse> {
    let _ = refresh_provider_auth_if_needed(&provider_id, false).await;
    Json(build_provider_auth_status(&provider_id))
}

pub async fn provider_auth_validate(
    Path(provider_id): Path<String>,
) -> Json<ProviderAuthActionResponse> {
    let refresh_result = refresh_provider_auth_if_needed(&provider_id, false).await;
    let status = build_provider_auth_status(&provider_id);
    let ok = status.authenticated;
    Json(ProviderAuthActionResponse {
        ok,
        provider_id,
        message: if ok {
            "provider auth is configured".to_string()
        } else if let Err(error) = refresh_result {
            format!("provider auth refresh failed: {error}")
        } else {
            "provider auth is not configured".to_string()
        },
        status: Some(status),
    })
}

pub async fn provider_auth_refresh(
    Path(provider_id): Path<String>,
) -> Json<ProviderAuthActionResponse> {
    let can_refresh = tura_llm_rust::provider_auth_registry_entry(&provider_id)
        .map(|entry| entry.capabilities.supports_oauth_refresh)
        .unwrap_or(false);
    let refresh_result = refresh_provider_auth_if_needed(&provider_id, true).await;
    let status = build_provider_auth_status(&provider_id);
    let ok = can_refresh && status.authenticated && refresh_result.is_ok();
    Json(ProviderAuthActionResponse {
        ok,
        provider_id,
        message: if !can_refresh {
            "provider auth method does not support refresh".to_string()
        } else if let Err(error) = refresh_result {
            format!("provider auth refresh failed: {error}")
        } else if ok {
            "provider auth refreshed".to_string()
        } else {
            "provider auth is not configured".to_string()
        },
        status: Some(status),
    })
}

pub async fn provider_auth_logout(
    Path(provider_id): Path<String>,
) -> Json<ProviderAuthActionResponse> {
    let result = logout_provider_auth(&provider_id);
    let status = build_provider_auth_status(&provider_id);
    Json(ProviderAuthActionResponse {
        ok: result.is_ok(),
        provider_id,
        message: result
            .map(|_| "provider auth logged out".to_string())
            .unwrap_or_else(|error| format!("provider auth logout failed: {error}")),
        status: Some(status),
    })
}

// ============================================================================
// Provider Auth
// ============================================================================

#[derive(Debug, Clone, serde::Deserialize, Default)]
pub struct ProviderAuthQuery {
    pub directory: Option<String>,
    pub workspace: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ProviderAuthMethod {
    #[serde(rename = "type")]
    pub method_type: String,
    pub kind: AuthMethodKind,
    pub login: String,
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompts: Option<Vec<serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_env: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub login_env: Option<String>,
}

pub async fn provider_auth(
    Query(_params): Query<ProviderAuthQuery>,
) -> Json<HashMap<String, Vec<ProviderAuthMethod>>> {
    let mut response = HashMap::new();
    for entry in tura_llm_rust::provider_auth_registry() {
        let methods = provider_auth_methods(entry.provider_id);
        if !methods.is_empty() {
            response.insert(entry.provider_id.to_string(), methods);
        }
    }
    Json(response)
}

fn provider_auth_methods(provider_id: &str) -> Vec<ProviderAuthMethod> {
    let Some(entry) = tura_llm_rust::provider_auth_registry_entry(provider_id) else {
        return Vec::new();
    };
    entry
        .auth_methods
        .iter()
        .map(|method| ProviderAuthMethod {
            method_type: legacy_auth_method_type(method.kind).to_string(),
            kind: method.kind,
            login: method.login.to_string(),
            label: method.label.to_string(),
            prompts: None,
            token_env: entry.token_env.map(ToString::to_string),
            login_env: entry.login_env.map(ToString::to_string),
        })
        .collect()
}

fn legacy_auth_method_type(kind: AuthMethodKind) -> &'static str {
    match kind {
        AuthMethodKind::ApiKey => "api",
        AuthMethodKind::OAuthPkce | AuthMethodKind::BrowserToken | AuthMethodKind::DeviceCode => {
            "oauth"
        }
        AuthMethodKind::LocalCliToken => "local",
        AuthMethodKind::AwsCredentials => "aws",
        AuthMethodKind::None => "none",
    }
}

// ============================================================================
// OAuth
// ============================================================================

pub async fn oauth_authorize(
    Path(provider_id): Path<String>,
    Query(_params): Query<OAuthAuthorizeParams>,
    Json(payload): Json<OAuthAuthorizePayload>,
) -> Json<OAuthAuthorizeResponse> {
    let methods = provider_auth_methods(&provider_id);
    let selected_method = methods.get(payload.method).filter(|method| {
        matches!(
            method.kind,
            AuthMethodKind::OAuthPkce | AuthMethodKind::BrowserToken
        )
    });

    let Some(selected_method) = selected_method else {
        return Json(OAuthAuthorizeResponse {
            url: String::new(),
            method: OAuthMethod::Code,
            instructions: "Invalid auth method".to_string(),
        });
    };

    let entry = tura_llm_rust::provider_auth_registry_entry(&provider_id);
    let authorize_kind = entry
        .and_then(|entry| entry.oauth_authorize_kind)
        .unwrap_or(OAuthAuthorizeKind::Unsupported);

    let (url, method, instructions) = if authorize_kind == OAuthAuthorizeKind::OpenAiPkce {
        let state = oauth_state();
        let code_verifier = oauth_code_verifier();
        let code_challenge = oauth_code_challenge(&code_verifier);
        let url = openai_oauth_authorize_url(&state, &code_challenge);
        global_store().set_oauth_state(
            &provider_id,
            "oauth_pkce".to_string(),
            None,
            url.clone(),
            Some(state),
            Some(code_verifier),
        );
        (
            url,
            OAuthMethod::Auto,
            "Complete authorization in your browser. This window will close automatically."
                .to_string(),
        )
    } else if authorize_kind == OAuthAuthorizeKind::BrowserTokenPaste {
        let code = random_confirmation_code(&provider_id, payload.method);
        let url = browser_login_url(&provider_id);
        global_store().set_oauth_state(
            &provider_id,
            "token".to_string(),
            Some(code.clone()),
            url.clone(),
            Some(oauth_state()),
            None,
        );
        (
            url,
            OAuthMethod::Code,
            format!("Open the login page, copy your browser token, and paste it here. Confirmation: {code}"),
        )
    } else if selected_method.kind == AuthMethodKind::OAuthPkce {
        (
            String::new(),
            OAuthMethod::Code,
            format!(
                "{} OAuth is listed but is not implemented yet.",
                provider_display_name(&provider_id)
            ),
        )
    } else {
        (
            String::new(),
            OAuthMethod::Code,
            "This provider does not support browser authorization.".to_string(),
        )
    };

    Json(OAuthAuthorizeResponse {
        url,
        method,
        instructions,
    })
}

#[derive(Debug, Clone, serde::Deserialize, Default)]
pub struct OAuthAuthorizeParams {
    pub directory: Option<String>,
    pub workspace: Option<String>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct OAuthAuthorizePayload {
    pub method: usize,
    pub inputs: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OAuthMethod {
    #[serde(rename = "auto")]
    Auto,
    #[serde(rename = "code")]
    Code,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct OAuthAuthorizeResponse {
    pub url: String,
    pub method: OAuthMethod,
    pub instructions: String,
}

pub async fn oauth_callback(
    Path(provider_id): Path<String>,
    Query(_params): Query<OAuthCallbackParams>,
    Json(payload): Json<OAuthCallbackPayload>,
) -> Json<bool> {
    if provider_id == "openai"
        && payload
            .code
            .as_deref()
            .unwrap_or_default()
            .trim()
            .is_empty()
    {
        return Json(wait_for_oauth_completed(&provider_id).await);
    }

    let has_pending = global_store().consume_oauth_state(&provider_id);
    if has_pending.is_none() {
        return Json(false);
    }

    let pending = has_pending.expect("pending OAuth state should exist after is_none check");
    if matches!(pending.method.as_str(), "code" | "token" | "oauth_pkce")
        && payload
            .code
            .as_deref()
            .unwrap_or_default()
            .trim()
            .is_empty()
    {
        return Json(false);
    }

    if pending.method == "oauth_pkce"
        && payload.state.is_some()
        && payload.state.as_deref() != pending.state.as_deref()
    {
        return Json(false);
    }

    if pending.method == "code" {
        let expected_code = pending.code.as_ref();
        if payload.code.as_ref() != expected_code {
            return Json(false);
        }
    }

    let tokens = if pending.method == "oauth_pkce" {
        match exchange_openai_oauth_code(payload.code.as_deref().unwrap_or_default(), &pending)
            .await
        {
            Ok(tokens) => Some(tokens),
            Err(_) => return Json(false),
        }
    } else {
        None
    };

    let key = if pending.method == "oauth_pkce" {
        tokens
            .as_ref()
            .map(|tokens| tokens.access_token.clone())
            .unwrap_or_default()
    } else if pending.method == "token" {
        payload
            .code
            .as_deref()
            .unwrap_or_default()
            .trim()
            .to_string()
    } else {
        browser_login_token(&provider_id, pending.code.as_deref())
    };

    let auth = ProviderAuth {
        auth_type: "oauth".to_string(),
        key: Some(key),
        access: tokens.as_ref().map(|tokens| tokens.access_token.clone()),
        refresh: tokens.as_ref().map(|tokens| tokens.refresh_token.clone()),
        expires: tokens
            .as_ref()
            .map(|tokens| Utc::now().timestamp_millis() + tokens.expires_in.unwrap_or(3600) * 1000),
        account_id: tokens.as_ref().and_then(extract_account_id),
        metadata: Some(HashMap::from([
            (
                "login".to_string(),
                serde_json::Value::String(
                    if pending.method == "oauth_pkce" {
                        "oauth"
                    } else {
                        "browser"
                    }
                    .to_string(),
                ),
            ),
            (
                "url".to_string(),
                serde_json::Value::String(pending.url.clone()),
            ),
        ])),
    };

    if persist_provider_auth(&provider_id, &auth).is_err() {
        return Json(false);
    }

    let _ = global_store().set_auth(&provider_id, auth.clone());

    Json(true)
}

pub async fn oauth_redirect_callback(
    Query(params): Query<OAuthRedirectCallbackParams>,
) -> impl IntoResponse {
    let Some(code) = params
        .code
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    else {
        return Html(oauth_callback_html(false, "Missing authorization code"));
    };
    let Some(state) = params
        .state
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    else {
        return Html(oauth_callback_html(false, "Missing OAuth state"));
    };
    let Some((provider_id, pending)) = global_store().consume_oauth_state_by_state(state) else {
        return Html(oauth_callback_html(
            false,
            "OAuth state expired or was not found",
        ));
    };
    if pending.method != "oauth_pkce" {
        return Html(oauth_callback_html(
            false,
            "OAuth callback did not match a PKCE login",
        ));
    }
    let tokens = match exchange_openai_oauth_code(code, &pending).await {
        Ok(tokens) => tokens,
        Err(error) => {
            return Html(oauth_callback_html(
                false,
                &format!("Token exchange failed: {error}"),
            ))
        }
    };
    let auth = ProviderAuth {
        auth_type: "oauth".to_string(),
        key: Some(tokens.access_token.clone()),
        access: Some(tokens.access_token.clone()),
        refresh: Some(tokens.refresh_token.clone()),
        expires: Some(Utc::now().timestamp_millis() + tokens.expires_in.unwrap_or(3600) * 1000),
        account_id: extract_account_id(&tokens),
        metadata: Some(HashMap::from([
            (
                "login".to_string(),
                serde_json::Value::String("oauth".to_string()),
            ),
            (
                "url".to_string(),
                serde_json::Value::String(pending.url.clone()),
            ),
        ])),
    };
    if persist_provider_auth(&provider_id, &auth).is_err() {
        return Html(oauth_callback_html(
            false,
            "Token was received but could not be persisted",
        ));
    }
    let _ = global_store().set_auth(&provider_id, auth.clone());
    global_store().set_oauth_completed(&provider_id, auth);
    Html(oauth_callback_html(
        true,
        "OpenAI OAuth connected. You can close this window.",
    ))
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct OAuthCallbackParams {
    pub directory: Option<String>,
    pub workspace: Option<String>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct OAuthCallbackPayload {
    pub method: usize,
    pub state: Option<String>,
    pub code: Option<String>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct OAuthRedirectCallbackParams {
    pub code: Option<String>,
    pub state: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct OpenAiTokenResponse {
    id_token: Option<String>,
    access_token: String,
    refresh_token: String,
    expires_in: Option<i64>,
}

async fn wait_for_oauth_completed(provider_id: &str) -> bool {
    let deadline = Instant::now() + Duration::from_secs(5 * 60);
    while Instant::now() < deadline {
        if let Some(auth) = global_store().consume_oauth_completed(provider_id) {
            let _ = global_store().set_auth(provider_id, auth);
            return true;
        }
        sleep(Duration::from_millis(500)).await;
    }
    false
}

fn random_confirmation_code(provider: &str, method: usize) -> String {
    format!("{}-{}", provider, method)
}

fn oauth_state() -> String {
    Uuid::new_v4().simple().to_string()
}

fn oauth_code_verifier() -> String {
    format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple())
}

fn oauth_code_challenge(verifier: &str) -> String {
    let digest = Sha256::digest(verifier.as_bytes());
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(digest)
}

fn openai_oauth_client_id() -> String {
    std::env::var("OPENAI_OAUTH_CLIENT_ID")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "app_EMoamEEZ73f0CkXaXp7hrann".to_string())
}

fn openai_oauth_redirect_uri() -> String {
    std::env::var("OPENAI_OAUTH_REDIRECT_URI")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "http://localhost:1455/auth/callback".to_string())
}

fn openai_oauth_token_url() -> String {
    std::env::var("OPENAI_OAUTH_TOKEN_URL")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "https://auth.openai.com/oauth/token".to_string())
}

fn google_oauth_token_url() -> String {
    std::env::var("GOOGLE_OAUTH_TOKEN_URL")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "https://oauth2.googleapis.com/token".to_string())
}

fn google_oauth_client_id(provider_id: &str) -> Option<String> {
    let provider_prefix = provider_id.to_ascii_uppercase().replace('-', "_");
    [
        format!("{provider_prefix}_OAUTH_CLIENT_ID"),
        "GOOGLE_OAUTH_CLIENT_ID".to_string(),
    ]
    .into_iter()
    .find_map(|key| config_value(&key))
}

fn google_oauth_client_secret(provider_id: &str) -> Option<String> {
    let provider_prefix = provider_id.to_ascii_uppercase().replace('-', "_");
    [
        format!("{provider_prefix}_OAUTH_CLIENT_SECRET"),
        "GOOGLE_OAUTH_CLIENT_SECRET".to_string(),
    ]
    .into_iter()
    .find_map(|key| config_value(&key))
}

fn openai_oauth_authorize_url(state: &str, code_challenge: &str) -> String {
    let client_id = openai_oauth_client_id();
    let redirect_uri = openai_oauth_redirect_uri();
    Url::parse_with_params(
        "https://auth.openai.com/oauth/authorize",
        &[
            ("response_type", "code"),
            ("client_id", client_id.as_str()),
            ("redirect_uri", redirect_uri.as_str()),
            ("scope", "openid profile email offline_access"),
            ("code_challenge", code_challenge),
            ("code_challenge_method", "S256"),
            ("id_token_add_organizations", "true"),
            ("codex_cli_simplified_flow", "true"),
            ("state", state),
            ("originator", "opencode"),
        ],
    )
    .expect("static OpenAI OAuth authorize URL is valid")
    .to_string()
}

async fn exchange_openai_oauth_code(
    code: &str,
    pending: &crate::mock::store::PendingOAuth,
) -> anyhow::Result<OpenAiTokenResponse> {
    let code_verifier = pending
        .code_verifier
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("missing PKCE code verifier"))?;
    let client_id = openai_oauth_client_id();
    let redirect_uri = openai_oauth_redirect_uri();
    let response = reqwest::Client::new()
        .post(openai_oauth_token_url())
        .form(&[
            ("grant_type", "authorization_code"),
            ("client_id", client_id.as_str()),
            ("redirect_uri", redirect_uri.as_str()),
            ("code", code),
            ("code_verifier", code_verifier),
        ])
        .send()
        .await?;
    let status = response.status();
    let body: serde_json::Value = response.json().await?;
    if !status.is_success() {
        return Err(anyhow::anyhow!(
            "OpenAI token endpoint returned {status}: {body}"
        ));
    }
    let tokens: OpenAiTokenResponse = serde_json::from_value(body)?;
    if tokens.access_token.trim().is_empty() {
        return Err(anyhow::anyhow!(
            "OpenAI token response did not include access_token"
        ));
    }
    if tokens.refresh_token.trim().is_empty() {
        return Err(anyhow::anyhow!(
            "OpenAI token response did not include refresh_token"
        ));
    }
    Ok(tokens)
}

fn extract_account_id(tokens: &OpenAiTokenResponse) -> Option<String> {
    tokens
        .id_token
        .as_deref()
        .and_then(extract_account_id_from_jwt)
        .or_else(|| extract_account_id_from_jwt(&tokens.access_token))
}

fn extract_account_id_from_jwt(token: &str) -> Option<String> {
    let payload = token.split('.').nth(1)?;
    let decoded = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(payload)
        .or_else(|_| base64::engine::general_purpose::URL_SAFE.decode(payload))
        .ok()?;
    let claims: serde_json::Value = serde_json::from_slice(&decoded).ok()?;
    claims
        .get("chatgpt_account_id")
        .and_then(|value| value.as_str())
        .or_else(|| {
            claims
                .pointer("/https://api.openai.com~1auth/chatgpt_account_id")
                .and_then(|value| value.as_str())
        })
        .or_else(|| {
            claims
                .get("organizations")
                .and_then(|value| value.as_array())
                .and_then(|organizations| organizations.first())
                .and_then(|organization| organization.get("id"))
                .and_then(|value| value.as_str())
        })
        .map(ToString::to_string)
}

fn oauth_callback_html(success: bool, message: &str) -> String {
    let title = if success {
        "OAuth connected"
    } else {
        "OAuth failed"
    };
    format!(
        r#"<!doctype html>
<html>
  <head><meta charset="utf-8"><title>{title}</title></head>
  <body style="font-family: system-ui, sans-serif; padding: 32px;">
    <h1>{title}</h1>
    <p>{}</p>
  </body>
</html>"#,
        html_escape(message)
    )
}

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn browser_login_url(provider_id: &str) -> String {
    match provider_id {
        "openai" => "https://chatgpt.com/auth/login".to_string(),
        "anthropic" => "https://claude.ai/login".to_string(),
        "antigravity" => "https://antigravity.google.com/auth".to_string(),
        other => format!("https://auth.example.com/oauth/{other}"),
    }
}

fn browser_login_token(provider_id: &str, code: Option<&str>) -> String {
    format!(
        "browser-login:{}:{}",
        provider_id,
        code.unwrap_or("confirmed")
    )
}

fn persist_provider_auth(provider_id: &str, auth: &ProviderAuth) -> io::Result<()> {
    let key = auth.access.as_deref().or(auth.key.as_deref());
    let Some(key) = key.filter(|value| !value.trim().is_empty()) else {
        return Ok(());
    };

    let env_path = std::env::var("TURA_ENV_PATH")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            tura_llm_rust::TuraConfig::default()
                .env_path()
                .to_path_buf()
        });
    let api_key = provider_env_key(provider_id);
    upsert_env_value(&env_path, &api_key, key)?;
    std::env::set_var(&api_key, key);

    let requested_login = auth
        .metadata
        .as_ref()
        .and_then(|metadata| metadata.get("login"))
        .and_then(|value| value.as_str())
        .map(ToString::to_string)
        .unwrap_or_else(|| login_value_for_auth(provider_id, auth));

    if auth.auth_type == "api" || requested_login == "api" {
        let login_key = provider_login_key(provider_id);
        upsert_env_value(&env_path, &login_key, "api")?;
        std::env::set_var(&login_key, "api");

        upsert_provider_auth_config(provider_id, "api", Some(&login_key), auth, None)?;
    }

    if auth.auth_type == "oauth" || matches!(requested_login.as_str(), "oauth" | "browser") {
        let login = requested_login.as_str();
        let login_key = provider_login_key(provider_id);
        upsert_env_value(&env_path, &login_key, login)?;
        std::env::set_var(&login_key, login);

        if let Some(registry_entry) = tura_llm_rust::provider_auth_registry_entry(provider_id) {
            if let (Some(refresh_key), Some(refresh)) = (
                registry_entry.refresh_env,
                auth.refresh
                    .as_deref()
                    .filter(|value| !value.trim().is_empty()),
            ) {
                upsert_env_value(&env_path, refresh_key, refresh)?;
                std::env::set_var(refresh_key, refresh);
            }
            if let (Some(expires_key), Some(expires)) = (registry_entry.expires_env, auth.expires) {
                let expires_value = expires.to_string();
                upsert_env_value(&env_path, expires_key, &expires_value)?;
                std::env::set_var(expires_key, expires_value);
            }
            if let (Some(account_key), Some(account_id)) = (
                registry_entry.account_env,
                auth.account_id
                    .as_deref()
                    .filter(|value| !value.trim().is_empty()),
            ) {
                upsert_env_value(&env_path, account_key, account_id)?;
                std::env::set_var(account_key, account_id);
            }
        }

        upsert_provider_auth_config(
            provider_id,
            login,
            Some(&login_key),
            auth,
            auth.metadata
                .as_ref()
                .and_then(|metadata| metadata.get("url"))
                .and_then(|value| value.as_str())
                .map(ToString::to_string)
                .or_else(|| Some(browser_login_url(provider_id))),
        )?;
    }

    Ok(())
}

fn login_value_for_auth(provider_id: &str, auth: &ProviderAuth) -> String {
    if let Some(login) = auth
        .metadata
        .as_ref()
        .and_then(|metadata| metadata.get("login"))
        .and_then(|value| value.as_str())
        .filter(|value| !value.trim().is_empty())
    {
        return login.to_string();
    }
    if auth.auth_type == "api" {
        return "api".to_string();
    }
    tura_llm_rust::provider_default_auth_method(provider_id)
        .map(|method| method.login.to_string())
        .unwrap_or_else(|| {
            if auth.auth_type == "oauth" {
                "oauth".to_string()
            } else {
                auth.auth_type.clone()
            }
        })
}

fn build_provider_auth_status(provider_id: &str) -> ProviderAuthStatusResponse {
    let entry = tura_llm_rust::provider_auth_registry_entry(provider_id);
    let config_entry = read_provider_auth_config(provider_id);
    let login = config_entry
        .as_ref()
        .and_then(|entry| entry.login.clone())
        .or_else(|| {
            entry
                .and_then(|entry| entry.login_env)
                .and_then(|key| std::env::var(key).ok())
        })
        .or_else(|| {
            entry
                .and_then(|entry| entry.login_env)
                .and_then(|key| tura_llm_rust::TuraConfig::default().get(key))
        });
    let token_env = config_entry
        .as_ref()
        .and_then(|entry| entry.token_env.clone())
        .or_else(|| {
            entry
                .and_then(|entry| entry.token_env)
                .map(ToString::to_string)
        });
    let refresh_env = config_entry
        .as_ref()
        .and_then(|entry| entry.refresh_env.clone())
        .or_else(|| {
            entry
                .and_then(|entry| entry.refresh_env)
                .map(ToString::to_string)
        });
    let expires_env = config_entry
        .as_ref()
        .and_then(|entry| entry.expires_env.clone())
        .or_else(|| {
            entry
                .and_then(|entry| entry.expires_env)
                .map(ToString::to_string)
        });
    let account_env = config_entry
        .as_ref()
        .and_then(|entry| entry.account_env.clone())
        .or_else(|| {
            entry
                .and_then(|entry| entry.account_env)
                .map(ToString::to_string)
        });

    let local_codex_auth = openai_codex_auth(provider_id);
    let configured = provider_key_exists(provider_id)
        || local_codex_auth.is_some()
        || bedrock_credentials_exist(provider_id);
    let expires_at = expires_env
        .as_deref()
        .and_then(config_value)
        .and_then(|value| value.parse::<i64>().ok());
    let expired = expires_at.map(|expires_at| expires_at <= Utc::now().timestamp_millis());
    let authenticated = (configured && expired != Some(true))
        || local_codex_auth
            .as_ref()
            .is_some_and(|auth| !auth.access_token.trim().is_empty());
    let auth_state = if authenticated {
        if matches!(login.as_deref(), Some("api")) {
            tura_llm_rust::AuthState::ApiKeyConfigured
        } else {
            tura_llm_rust::AuthState::Authenticated
        }
    } else if expired == Some(true) {
        tura_llm_rust::AuthState::Expired
    } else {
        tura_llm_rust::AuthState::NotConfigured
    };
    let runtime_state = if entry.and_then(|entry| entry.disabled_reason).is_some() {
        tura_llm_rust::ProviderRuntimeState::Disabled
    } else if authenticated {
        tura_llm_rust::ProviderRuntimeState::Ready
    } else if configured {
        tura_llm_rust::ProviderRuntimeState::Configured
    } else {
        tura_llm_rust::ProviderRuntimeState::MissingAuth
    };

    ProviderAuthStatusResponse {
        provider_id: provider_id.to_string(),
        display_name: provider_display_name(provider_id),
        login,
        configured,
        authenticated,
        expired,
        account_id: config_entry
            .as_ref()
            .and_then(|entry| entry.account_id.clone())
            .or_else(|| account_env.as_deref().and_then(config_value))
            .or_else(|| local_codex_auth.and_then(|auth| auth.account_id)),
        token_env,
        login_env: config_entry
            .as_ref()
            .and_then(|entry| entry.login_env.clone())
            .or_else(|| {
                entry
                    .and_then(|entry| entry.login_env)
                    .map(ToString::to_string)
            }),
        refresh_env,
        expires_env,
        updated_at: config_entry.and_then(|entry| entry.updated_at),
        auth_state,
        runtime_state,
        last_error_category: entry
            .and_then(|entry| entry.disabled_reason)
            .map(ToString::to_string),
    }
}

#[derive(Debug, Clone)]
struct OpenAiCodexAuth {
    access_token: String,
    account_id: Option<String>,
}

fn openai_codex_auth(provider_id: &str) -> Option<OpenAiCodexAuth> {
    if provider_id != "openai" {
        return None;
    }
    let home = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME"))?;
    let path = PathBuf::from(home).join(".codex").join("auth.json");
    let value: serde_json::Value = serde_json::from_str(&fs::read_to_string(path).ok()?).ok()?;
    let tokens = value.get("tokens")?;
    let access_token = tokens
        .get("access_token")
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.trim().is_empty())?
        .to_string();
    let account_id = tokens
        .get("account_id")
        .or_else(|| value.get("account_id"))
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(ToString::to_string);
    Some(OpenAiCodexAuth {
        access_token,
        account_id,
    })
}

fn read_provider_auth_config(provider_id: &str) -> Option<tura_llm_rust::ProviderAuthConfig> {
    let path = tura_llm_config_path();
    let content = fs::read_to_string(path).ok()?;
    let root: tura_llm_rust::RootConfig = serde_json::from_str(&content).ok()?;
    root.provider_auth.get(provider_id).cloned()
}

fn config_value(key: &str) -> Option<String> {
    std::env::var(key)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| tura_llm_rust::TuraConfig::default().get(key))
        .filter(|value| !value.trim().is_empty())
}

fn bedrock_credentials_exist(provider_id: &str) -> bool {
    provider_id == "bedrock"
        && [
            "AWS_ACCESS_KEY_ID",
            "AWS_PROFILE",
            "AWS_REGION",
            "AWS_DEFAULT_REGION",
        ]
        .iter()
        .any(|key| config_value(key).is_some())
}

fn logout_provider_auth(provider_id: &str) -> io::Result<()> {
    let status = build_provider_auth_status(provider_id);
    let env_path = std::env::var("TURA_ENV_PATH")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            tura_llm_rust::TuraConfig::default()
                .env_path()
                .to_path_buf()
        });

    let current_login = status.login.as_deref();
    let should_clear_shared_token = match provider_id {
        "openai" => current_login == Some("oauth"),
        "openai-api" => current_login != Some("oauth"),
        "anthropic" | "antigravity" => current_login == Some("browser"),
        "anthropic-api" | "antigravity-api" => current_login != Some("browser"),
        _ => true,
    };

    if should_clear_shared_token {
        if let Some(token_env) = status.token_env.as_deref() {
            upsert_env_value(&env_path, token_env, "")?;
            std::env::remove_var(token_env);
        }
    }
    if let Some(login_env) = status.login_env.as_deref() {
        upsert_env_value(&env_path, login_env, "")?;
        std::env::remove_var(login_env);
    }
    for key in [
        status.refresh_env.as_deref(),
        status.expires_env.as_deref(),
        tura_llm_rust::provider_auth_registry_entry(provider_id)
            .and_then(|entry| entry.account_env),
    ]
    .into_iter()
    .flatten()
    {
        upsert_env_value(&env_path, key, "")?;
        std::env::remove_var(key);
    }

    update_provider_auth_config_status(provider_id, "revoked")
}

fn update_provider_auth_config_status(provider_id: &str, status: &str) -> io::Result<()> {
    let path = tura_llm_config_path();
    let content = fs::read_to_string(&path)?;
    let mut root: serde_json::Value = serde_json::from_str(&content)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
    if let Some(entry) = root
        .get_mut("provider_auth")
        .and_then(|value| value.as_object_mut())
        .and_then(|provider_auth| provider_auth.get_mut(provider_id))
        .and_then(|entry| entry.as_object_mut())
    {
        entry.insert(
            "status".to_string(),
            serde_json::Value::String(status.to_string()),
        );
        entry.insert(
            "updated_at".to_string(),
            serde_json::Value::String(Utc::now().to_rfc3339()),
        );
    }
    let formatted = serde_json::to_string_pretty(&root)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
    fs::write(path, format!("{formatted}\n"))
}

fn tura_llm_config_path() -> std::path::PathBuf {
    std::env::var("TURALLM_CONFIG")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            std::env::current_dir()
                .unwrap_or_default()
                .join("crates")
                .join("provider")
                .join("config")
                .join("tura_llm_config.json")
        })
}

fn upsert_provider_auth_config(
    provider_id: &str,
    login: &str,
    login_env: Option<&str>,
    auth: &ProviderAuth,
    auth_url: Option<String>,
) -> io::Result<()> {
    let path = tura_llm_config_path();
    let content = fs::read_to_string(&path)?;
    let mut root: serde_json::Value = serde_json::from_str(&content)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;

    if !root.is_object() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "tura llm config root must be a JSON object",
        ));
    }

    let provider_auth = root
        .as_object_mut()
        .expect("checked object")
        .entry("provider_auth")
        .or_insert_with(|| serde_json::json!({}));
    if !provider_auth.is_object() {
        *provider_auth = serde_json::json!({});
    }

    let auth_type = match login {
        "api" => "api_key",
        "browser" => "browser_token",
        "local" => "local_cli_token",
        "device" => "device_code",
        "aws" => "aws_credentials",
        _ => "oauth",
    };
    let registry_entry = tura_llm_rust::provider_auth_registry_entry(provider_id);

    let mut entry = serde_json::json!({
        "type": auth_type,
        "login": login,
        "status": "connected",
        "provider": provider_runtime_id(provider_id),
        "auth_url": auth_url,
        "token_env": provider_env_key(provider_id),
        "login_env": login_env,
        "updated_at": Utc::now().to_rfc3339(),
    });
    if let Some(registry_entry) = registry_entry {
        if let Some(refresh_env) = registry_entry.refresh_env {
            entry["refresh_env"] = serde_json::Value::String(refresh_env.to_string());
        }
        if let Some(expires_env) = registry_entry.expires_env {
            entry["expires_env"] = serde_json::Value::String(expires_env.to_string());
        }
        if let Some(account_env) = registry_entry.account_env {
            entry["account_env"] = serde_json::Value::String(account_env.to_string());
        }
        if !registry_entry.default_base_url.is_empty() {
            entry["endpoint"] =
                serde_json::Value::String(registry_entry.default_base_url.to_string());
        }
        if let Some(reason) = registry_entry.disabled_reason {
            entry["unsupported_reason"] = serde_json::Value::String(reason.to_string());
        }
    }
    if provider_id == "openai" && login == "oauth" {
        entry["endpoint"] = serde_json::Value::String(
            "https://chatgpt.com/backend-api/codex/responses".to_string(),
        );
        entry["refresh_env"] = serde_json::Value::String("OPENAI_REFRESH_TOKEN".to_string());
        entry["expires_env"] = serde_json::Value::String("OPENAI_TOKEN_EXPIRES".to_string());
        entry["account_env"] = serde_json::Value::String("OPENAI_ACCOUNT_ID".to_string());
        if let Some(account_id) = auth.account_id.as_deref() {
            entry["account_id"] = serde_json::Value::String(account_id.to_string());
        }
    }
    provider_auth
        .as_object_mut()
        .expect("provider_auth is object")
        .insert(provider_id.to_string(), entry);

    let formatted = serde_json::to_string_pretty(&root)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
    fs::write(path, format!("{formatted}\n"))
}

fn provider_env_key(provider_id: &str) -> String {
    tura_llm_rust::provider_token_env(provider_id)
        .map(ToString::to_string)
        .unwrap_or_else(|| format!("{}_API_KEY", provider_id.to_ascii_uppercase()))
}

fn provider_login_key(provider_id: &str) -> String {
    tura_llm_rust::provider_login_env(provider_id)
        .map(ToString::to_string)
        .unwrap_or_else(|| format!("{}_LOGIN", provider_id.to_ascii_uppercase()))
}

fn provider_key_exists(provider_id: &str) -> bool {
    let key = provider_env_key(provider_id);
    let has_key = std::env::var(&key)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .is_some()
        || tura_llm_rust::TuraConfig::default()
            .get(&key)
            .filter(|value| !value.trim().is_empty())
            .is_some();
    if !has_key {
        return false;
    }
    if matches!(
        provider_id,
        "openai" | "openai-api" | "anthropic" | "anthropic-api" | "antigravity" | "antigravity-api"
    ) {
        let login_key = provider_login_key(provider_id);
        let login = std::env::var(&login_key)
            .ok()
            .or_else(|| tura_llm_rust::TuraConfig::default().get(&login_key));
        return match provider_id {
            "openai" => login.as_deref() == Some("oauth"),
            "anthropic" | "antigravity" => login.as_deref() == Some("browser"),
            "openai-api" | "anthropic-api" | "antigravity-api" => {
                !matches!(login.as_deref(), Some("oauth" | "browser"))
            }
            _ => true,
        };
    }
    true
}

fn upsert_env_value(path: &FsPath, key: &str, value: &str) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let mut lines = fs::read_to_string(path)
        .map(|content| content.lines().map(ToString::to_string).collect::<Vec<_>>())
        .unwrap_or_default();
    let prefix = format!("{key}=");
    let next = format!("{key}={}", quote_env_value(value));
    let mut replaced = false;

    for line in &mut lines {
        if line.trim_start().starts_with(&prefix) {
            *line = next.clone();
            replaced = true;
            break;
        }
    }

    if !replaced {
        lines.push(next);
    }

    let mut content = lines.join("\n");
    content.push('\n');
    fs::write(path, content)
}

fn quote_env_value(value: &str) -> String {
    let escaped = value.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{escaped}\"")
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::extract::Path;
    use std::io::{Read, Write};
    use tokio::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::const_new(());

    #[test]
    fn provider_auth_methods_are_projected_from_registry() {
        let openai = provider_auth_methods("openai");
        assert_eq!(openai.len(), 1);
        assert_eq!(openai[0].kind, AuthMethodKind::OAuthPkce);
        assert_eq!(openai[0].method_type, "oauth");
        assert_eq!(openai[0].token_env.as_deref(), Some("OPENAI_API_KEY"));

        let anthropic = provider_auth_methods("anthropic");
        assert_eq!(anthropic[0].kind, AuthMethodKind::BrowserToken);
        assert_eq!(anthropic[0].login, "browser");

        let openrouter = provider_auth_methods("openrouter");
        assert_eq!(openrouter[0].kind, AuthMethodKind::ApiKey);
        assert_eq!(openrouter[0].method_type, "api");
        assert_eq!(
            openrouter[0].token_env.as_deref(),
            Some("OPENROUTER_API_KEY")
        );
    }

    #[test]
    fn provider_env_keys_use_registry_compatibility_aliases() {
        assert_eq!(provider_env_key("openai-api"), "OPENAI_API_KEY");
        assert_eq!(provider_login_key("anthropic-api"), "ANTHROPIC_LOGIN");
        assert_eq!(provider_env_key("gemini-api"), "GEMINI_API_KEY");
    }

    #[test]
    fn login_value_for_auth_prefers_metadata_and_registry_defaults() {
        let auth = ProviderAuth {
            auth_type: "oauth".to_string(),
            key: Some("secret".to_string()),
            access: None,
            refresh: None,
            expires: None,
            account_id: None,
            metadata: None,
        };
        assert_eq!(login_value_for_auth("anthropic", &auth), "browser");

        let with_metadata = ProviderAuth {
            metadata: Some(HashMap::from([(
                "login".to_string(),
                serde_json::Value::String("oauth".to_string()),
            )])),
            ..auth
        };
        assert_eq!(login_value_for_auth("anthropic", &with_metadata), "oauth");
    }

    #[tokio::test]
    async fn provider_auth_refresh_updates_expired_openai_oauth_env_and_config() {
        let _guard = ENV_LOCK.lock().await;
        let temp_dir = std::env::temp_dir().join(format!(
            "tura-openai-oauth-refresh-test-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).expect("temp dir");
        let env_path = temp_dir.join(".env");
        let config_path = temp_dir.join("tura_llm_config.json");
        std::fs::write(
            &env_path,
            "OPENAI_LOGIN=oauth\nOPENAI_API_KEY=old-access\nOPENAI_REFRESH_TOKEN=old-refresh\nOPENAI_TOKEN_EXPIRES=0\n",
        )
        .expect("env");
        std::fs::write(&config_path, r#"{"provider_auth":{}}"#).expect("config");

        let (addr, server) = spawn_openai_token_server(
            "old-refresh",
            r#"{"access_token":"new-access","refresh_token":"new-refresh","expires_in":3600}"#,
        );

        set_env("TURA_ENV_PATH", env_path.to_string_lossy().as_ref());
        set_env("TURALLM_CONFIG", config_path.to_string_lossy().as_ref());
        set_env(
            "OPENAI_OAUTH_TOKEN_URL",
            &format!("http://{addr}/oauth/token"),
        );
        set_env("OPENAI_LOGIN", "oauth");
        set_env("OPENAI_API_KEY", "old-access");
        set_env("OPENAI_REFRESH_TOKEN", "old-refresh");
        set_env("OPENAI_TOKEN_EXPIRES", "0");
        assert_eq!(
            std::env::var("OPENAI_REFRESH_TOKEN").as_deref(),
            Ok("old-refresh")
        );

        let Json(response) = provider_auth_refresh(Path("openai".to_string())).await;

        assert!(response.ok, "{}", response.message);
        assert_eq!(
            response.status.as_ref().map(|status| status.authenticated),
            Some(true)
        );
        assert_eq!(std::env::var("OPENAI_API_KEY").as_deref(), Ok("new-access"));
        assert_eq!(
            std::env::var("OPENAI_REFRESH_TOKEN").as_deref(),
            Ok("new-refresh")
        );
        assert!(std::env::var("OPENAI_TOKEN_EXPIRES")
            .ok()
            .and_then(|value| value.parse::<i64>().ok())
            .is_some_and(|expires| expires > Utc::now().timestamp_millis()));
        let config = std::fs::read_to_string(&config_path).expect("read config");
        assert!(
            config.contains("\"status\": \"connected\"")
                || config.contains("\"status\":\"connected\"")
        );
        assert!(config.contains("OPENAI_REFRESH_TOKEN"));
        server.join().expect("token server should finish");

        clear_openai_refresh_test_env();
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[tokio::test]
    async fn provider_auth_status_refreshes_expired_openai_oauth_before_reporting() {
        let _guard = ENV_LOCK.lock().await;
        let temp_dir = std::env::temp_dir().join(format!(
            "tura-openai-oauth-status-refresh-test-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).expect("temp dir");
        let env_path = temp_dir.join(".env");
        let config_path = temp_dir.join("tura_llm_config.json");
        std::fs::write(
            &env_path,
            "OPENAI_LOGIN=oauth\nOPENAI_API_KEY=expired-access\nOPENAI_REFRESH_TOKEN=status-refresh-token\nOPENAI_TOKEN_EXPIRES=0\n",
        )
        .expect("env");
        std::fs::write(&config_path, r#"{"provider_auth":{}}"#).expect("config");

        let (addr, server) = spawn_openai_token_server(
            "status-refresh-token",
            r#"{"access_token":"status-access","expires_in":3600}"#,
        );

        set_env("TURA_ENV_PATH", env_path.to_string_lossy().as_ref());
        set_env("TURALLM_CONFIG", config_path.to_string_lossy().as_ref());
        set_env(
            "OPENAI_OAUTH_TOKEN_URL",
            &format!("http://{addr}/oauth/token"),
        );
        set_env("OPENAI_LOGIN", "oauth");
        set_env("OPENAI_API_KEY", "expired-access");
        set_env("OPENAI_REFRESH_TOKEN", "status-refresh-token");
        set_env("OPENAI_TOKEN_EXPIRES", "0");
        assert_eq!(
            std::env::var("OPENAI_REFRESH_TOKEN").as_deref(),
            Ok("status-refresh-token")
        );

        let Json(status) = provider_auth_status(Path("openai".to_string())).await;

        assert!(status.authenticated);
        assert_eq!(status.expired, Some(false));
        assert_eq!(status.auth_state, tura_llm_rust::AuthState::Authenticated);
        assert_eq!(
            status.runtime_state,
            tura_llm_rust::ProviderRuntimeState::Ready
        );
        assert_eq!(
            std::env::var("OPENAI_API_KEY").as_deref(),
            Ok("status-access")
        );
        assert_eq!(
            std::env::var("OPENAI_REFRESH_TOKEN").as_deref(),
            Ok("status-refresh-token")
        );
        server.join().expect("token server should finish");

        clear_openai_refresh_test_env();
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[tokio::test]
    async fn provider_auth_refresh_covers_google_and_gemini_oauth_methods() {
        let _guard = ENV_LOCK.lock().await;

        for case in [
            OAuthRefreshCase {
                provider_id: "google",
                login_env: "GOOGLE_LOGIN",
                token_env: "GOOGLE_API_KEY",
                old_access: "google-expired-access",
                new_access: "google-new-access",
            },
            OAuthRefreshCase {
                provider_id: "gemini",
                login_env: "GEMINI_LOGIN",
                token_env: "GEMINI_API_KEY",
                old_access: "gemini-expired-access",
                new_access: "gemini-new-access",
            },
        ] {
            clear_openai_refresh_test_env();
            let temp_dir = std::env::temp_dir().join(format!(
                "tura-{provider}-oauth-refresh-test-{}",
                std::process::id(),
                provider = case.provider_id
            ));
            let _ = std::fs::remove_dir_all(&temp_dir);
            std::fs::create_dir_all(&temp_dir).expect("temp dir");
            let env_path = temp_dir.join(".env");
            let config_path = temp_dir.join("tura_llm_config.json");
            std::fs::write(
                &env_path,
                format!(
                    "{login_env}=oauth\n{token_env}={old_access}\nGOOGLE_REFRESH_TOKEN={refresh}\nGOOGLE_TOKEN_EXPIRES=0\n",
                    login_env = case.login_env,
                    token_env = case.token_env,
                    old_access = case.old_access,
                    refresh = case.refresh_token()
                ),
            )
            .expect("env");
            std::fs::write(&config_path, r#"{"provider_auth":{}}"#).expect("config");

            let (addr, server) = spawn_openai_token_server(
                case.refresh_token(),
                Box::leak(
                    format!(
                        r#"{{"access_token":"{}","refresh_token":"{}","expires_in":3600}}"#,
                        case.new_access,
                        case.refresh_token()
                    )
                    .into_boxed_str(),
                ),
            );

            set_env("TURA_ENV_PATH", env_path.to_string_lossy().as_ref());
            set_env("TURALLM_CONFIG", config_path.to_string_lossy().as_ref());
            set_env(
                "GOOGLE_OAUTH_TOKEN_URL",
                &format!("http://{addr}/oauth/token"),
            );
            set_env(case.login_env, "oauth");
            set_env(case.token_env, case.old_access);
            set_env("GOOGLE_REFRESH_TOKEN", case.refresh_token());
            set_env("GOOGLE_TOKEN_EXPIRES", "0");

            let Json(response) = provider_auth_refresh(Path(case.provider_id.to_string())).await;

            assert!(response.ok, "{}", response.message);
            assert_eq!(
                std::env::var(case.token_env).as_deref(),
                Ok(case.new_access)
            );
            assert_eq!(
                std::env::var("GOOGLE_REFRESH_TOKEN").as_deref(),
                Ok(case.refresh_token())
            );
            assert!(std::env::var("GOOGLE_TOKEN_EXPIRES")
                .ok()
                .and_then(|value| value.parse::<i64>().ok())
                .is_some_and(|expires| expires > Utc::now().timestamp_millis()));
            let config = std::fs::read_to_string(&config_path).expect("read config");
            assert!(config.contains(case.provider_id));
            assert!(config.contains("GOOGLE_REFRESH_TOKEN"));
            server.join().expect("token server should finish");

            clear_openai_refresh_test_env();
            let _ = std::fs::remove_dir_all(&temp_dir);
        }
    }

    struct OAuthRefreshCase {
        provider_id: &'static str,
        login_env: &'static str,
        token_env: &'static str,
        old_access: &'static str,
        new_access: &'static str,
    }

    impl OAuthRefreshCase {
        fn refresh_token(&self) -> &'static str {
            match self.provider_id {
                "google" => "google-refresh-token",
                "gemini" => "gemini-refresh-token",
                _ => "refresh-token",
            }
        }
    }

    fn spawn_openai_token_server(
        expected_refresh_token: &'static str,
        token_body: &'static str,
    ) -> (std::net::SocketAddr, std::thread::JoinHandle<()>) {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind token server");
        let addr = listener.local_addr().expect("token server addr");
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept refresh request");
            let request = read_http_request(&mut stream);
            let (_, body) = request
                .split_once("\r\n\r\n")
                .expect("refresh request should include body separator");
            assert!(body.contains("grant_type=refresh_token"), "{body}");
            assert!(
                body.contains(&format!("refresh_token={expected_refresh_token}")),
                "{body}"
            );
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                token_body.len(),
                token_body
            );
            stream
                .write_all(response.as_bytes())
                .expect("write refresh response");
        });
        (addr, server)
    }

    fn read_http_request(stream: &mut std::net::TcpStream) -> String {
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(2)))
            .expect("set read timeout");
        let mut data = Vec::new();
        let mut buffer = [0_u8; 1024];
        loop {
            let size = stream.read(&mut buffer).expect("read refresh request");
            assert!(
                size > 0,
                "refresh request stream closed before body completed"
            );
            data.extend_from_slice(&buffer[..size]);
            if http_request_complete(&data) {
                return String::from_utf8_lossy(&data).into_owned();
            }
        }
    }

    fn http_request_complete(data: &[u8]) -> bool {
        let request = String::from_utf8_lossy(data);
        let Some((headers, body)) = request.split_once("\r\n\r\n") else {
            return false;
        };
        let content_length = headers
            .lines()
            .find_map(|line| line.strip_prefix("content-length:"))
            .or_else(|| {
                headers
                    .lines()
                    .find_map(|line| line.strip_prefix("Content-Length:"))
            })
            .and_then(|value| value.trim().parse::<usize>().ok())
            .unwrap_or(0);
        body.len() >= content_length
    }

    fn clear_openai_refresh_test_env() {
        for key in [
            "TURA_ENV_PATH",
            "TURALLM_CONFIG",
            "OPENAI_OAUTH_TOKEN_URL",
            "OPENAI_LOGIN",
            "OPENAI_API_KEY",
            "OPENAI_REFRESH_TOKEN",
            "OPENAI_TOKEN_EXPIRES",
            "GOOGLE_OAUTH_TOKEN_URL",
            "GOOGLE_OAUTH_CLIENT_ID",
            "GOOGLE_OAUTH_CLIENT_SECRET",
            "GOOGLE_LOGIN",
            "GOOGLE_API_KEY",
            "GEMINI_LOGIN",
            "GEMINI_API_KEY",
            "GOOGLE_REFRESH_TOKEN",
            "GOOGLE_TOKEN_EXPIRES",
        ] {
            std::env::remove_var(key);
        }
    }

    fn set_env(key: &str, value: &str) {
        std::env::set_var(key, value);
    }
}
