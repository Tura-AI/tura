//! Provider / Auth API handlers

use crate::api::types::*;
use crate::mock::global_store;
use axum::extract::{Json, Path, Query};
use axum::response::{Html, IntoResponse};
use base64::Engine;
use chrono::Utc;
use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::PathBuf;
use tokio::time::{sleep, timeout, Duration, Instant};
use tura_llm_rust::{AuthMethodKind, OAuthAuthorizeKind};

mod auth_refresh;
mod auth_registry;
mod auth_scheduler;
mod auth_update;
mod auth_validator;
pub(crate) mod config;
mod metadata;
mod oauth_support;

use config::{config_value, upsert_env_value};
use metadata::{
    legacy_auth_method_type, oauth_authorize_endpoint, oauth_token_endpoint, provider_api_key_url,
    provider_auth_docs_url,
};
use oauth_support::{
    anthropic_oauth_client_id, anthropic_oauth_redirect_uri, anthropic_oauth_token_url,
    browser_login_token, browser_login_url, github_copilot_oauth_client_id,
    github_copilot_oauth_scope, github_device_code_url, github_oauth_token_url,
    google_oauth_client_id, google_oauth_client_secret, google_oauth_token_url,
    oauth_authorize_url, oauth_callback_html, oauth_code_challenge, oauth_code_verifier,
    oauth_state, openai_oauth_client_id, openai_oauth_redirect_uri, openai_oauth_token_url,
    provider_google_oauth_redirect_uri, random_confirmation_code,
};

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
            id: provider.id.clone(),
            name: provider_display_name_from_settings(settings.as_deref(), &provider.id)
                .unwrap_or(provider.name),
            source: "config".to_string(),
            env: provider_env_from_settings(settings.as_deref(), &provider.id),
            key: None,
            options: provider_options_from_settings(settings.as_deref(), &provider.id),
            models,
            api: provider_api_from_settings(settings.as_deref(), &provider.id),
            npm: None,
        });
    }

    enrich_provider_list(&mut all, &mut connected, &providers_enabled_set());

    Json(ProviderListResponse {
        all,
        default: defaults,
        connected,
        enums: settings
            .as_deref()
            .map(|settings| settings.provider_enums.clone())
            .unwrap_or_default(),
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
                    name: provider_display_name_from_settings(Some(settings), &provider.provider)
                        .unwrap_or_else(|| provider_display_name(&provider.provider)),
                    source: "config".to_string(),
                    env: provider_env_from_settings(Some(settings), &provider.provider),
                    key: None,
                    options: provider_options_from_settings(Some(settings), &provider.provider),
                    models: HashMap::new(),
                    api: provider_api_from_settings(Some(settings), &provider.provider),
                    npm: None,
                });
                index
            }
        };

        all[index].models.insert(
            model_id.clone(),
            sdk_model_from_settings(Some(settings), &provider.provider, &model_id),
        );
    }

    for (provider_id, models) in provider_model_catalog() {
        let index = match indexes.get(&provider_id).copied() {
            Some(index) => index,
            None => {
                let index = all.len();
                indexes.insert(provider_id.to_string(), index);
                defaults.insert(provider_id.to_string(), models[0].to_string());
                all.push(SdkProvider {
                    id: provider_id.to_string(),
                    name: provider_display_name_from_settings(Some(settings), &provider_id)
                        .unwrap_or_else(|| provider_display_name(&provider_id)),
                    source: "config".to_string(),
                    env: provider_env_from_settings(Some(settings), &provider_id),
                    key: None,
                    options: provider_options_from_settings(Some(settings), &provider_id),
                    models: HashMap::new(),
                    api: provider_api_from_settings(Some(settings), &provider_id),
                    npm: None,
                });
                index
            }
        };

        for model_id in models {
            all[index]
                .models
                .entry(model_id.to_string())
                .or_insert_with(|| {
                    sdk_model_from_settings(Some(settings), &provider_id, &model_id)
                });
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
                    name: provider_display_name_from_settings(Some(settings), &provider_id)
                        .unwrap_or_else(|| provider_display_name(&provider_id)),
                    source: "config".to_string(),
                    env: provider_env_from_settings(Some(settings), &provider_id),
                    key: None,
                    options: provider_options_from_settings(Some(settings), &provider_id),
                    models: HashMap::new(),
                    api: provider_api_from_settings(Some(settings), &provider_id),
                    npm: None,
                });
                index
            }
        };

        for model_id in model_ids {
            all[index]
                .models
                .entry(model_id.clone())
                .or_insert_with(|| {
                    sdk_model_from_settings(Some(settings), &provider_id, &model_id)
                });
        }
    }

    for provider_id in settings.model_catalog.providers.keys() {
        if indexes.contains_key(provider_id) {
            continue;
        }
        let index = all.len();
        indexes.insert(provider_id.clone(), index);
        all.push(SdkProvider {
            id: provider_id.clone(),
            name: provider_display_name_from_settings(Some(settings), provider_id)
                .unwrap_or_else(|| provider_display_name(provider_id)),
            source: "config".to_string(),
            env: provider_env_from_settings(Some(settings), provider_id),
            key: None,
            options: provider_options_from_settings(Some(settings), provider_id),
            models: HashMap::new(),
            api: provider_api_from_settings(Some(settings), provider_id),
            npm: None,
        });
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
            name: provider_display_name_from_settings(Some(settings), provider_id)
                .unwrap_or_else(|| provider_display_name(provider_id)),
            source: "config".to_string(),
            env: provider_env_from_settings(Some(settings), provider_id),
            key: None,
            options: provider_options_from_settings(Some(settings), provider_id),
            models: HashMap::from([(
                model_id.to_string(),
                sdk_model_from_settings(Some(settings), provider_id, model_id),
            )]),
            api: provider_api_from_settings(Some(settings), provider_id),
            npm: None,
        });
    }

    enrich_provider_list(&mut all, &mut connected, &store_connected);

    ProviderListResponse {
        all,
        default: defaults,
        connected,
        enums: settings.provider_enums.clone(),
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
    settings.route_by_name(name)
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
    let route = settings
        .routes()
        .flat_map(|route| route.providers.iter())
        .find(|provider| provider.provider == provider_id)?;

    Some(sdk_model_from_settings(
        Some(settings),
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
        .map(|model_id| sdk_model_from_settings(Some(settings), provider_id, &model_id))
        .collect()
}

fn configured_model_catalog(settings: &tura_llm_rust::Settings) -> HashMap<String, Vec<String>> {
    settings.configured_model_catalog()
}

fn provider_display_name_from_settings(
    settings: Option<&tura_llm_rust::Settings>,
    provider_id: &str,
) -> Option<String> {
    settings?
        .model_catalog
        .providers
        .get(provider_id)
        .map(|provider| provider.display_name.trim())
        .filter(|name| !name.is_empty())
        .map(ToString::to_string)
}

fn provider_env_from_settings(
    settings: Option<&tura_llm_rust::Settings>,
    provider_id: &str,
) -> Vec<String> {
    let Some(provider) =
        settings.and_then(|settings| settings.model_catalog.providers.get(provider_id))
    else {
        return Vec::new();
    };
    let mut env = provider.env.clone();
    if let Some(token_env) = provider
        .token_env
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        if !env.iter().any(|item| item == token_env) {
            env.insert(0, token_env.to_string());
        }
    }
    env
}

fn provider_api_from_settings(
    settings: Option<&tura_llm_rust::Settings>,
    provider_id: &str,
) -> Option<String> {
    settings
        .and_then(|settings| settings.model_catalog.providers.get(provider_id))
        .and_then(|provider| {
            (!provider.base_url.trim().is_empty()).then(|| provider.base_url.clone())
        })
}

fn provider_options_from_settings(
    settings: Option<&tura_llm_rust::Settings>,
    provider_id: &str,
) -> HashMap<String, serde_json::Value> {
    let Some(provider) =
        settings.and_then(|settings| settings.model_catalog.providers.get(provider_id))
    else {
        return HashMap::new();
    };
    let mut options = HashMap::new();
    insert_option(&mut options, "api_style", &provider.api_style);
    insert_option(&mut options, "runtime_provider", &provider.runtime_provider);
    if let Some(token_env) = &provider.token_env {
        insert_option(&mut options, "token_env", token_env);
    }
    if !provider.domains.is_empty() {
        options.insert("domains".to_string(), serde_json::json!(provider.domains));
    }
    if !provider.capabilities.is_empty() {
        options.insert(
            "capabilities".to_string(),
            serde_json::json!(provider.capabilities),
        );
    }
    if !provider.auth_methods.is_empty() {
        options.insert(
            "auth_methods".to_string(),
            serde_json::json!(provider.auth_methods),
        );
    }
    if let Some(api_docs) = &provider.api_docs {
        insert_option(&mut options, "api_docs", api_docs);
    }
    if let Some(status) = &provider.status {
        insert_option(&mut options, "status", status);
    }
    options
}

fn insert_option(options: &mut HashMap<String, serde_json::Value>, key: &str, value: &str) {
    if !value.trim().is_empty() {
        options.insert(
            key.to_string(),
            serde_json::Value::String(value.to_string()),
        );
    }
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

fn sdk_model_from_settings(
    settings: Option<&tura_llm_rust::Settings>,
    provider_id: &str,
    model_id: &str,
) -> SdkProviderModel {
    let mut model = sdk_model_from_config(provider_id, model_id);
    if let Some(detail) =
        settings.and_then(|settings| catalog_model_detail(settings, provider_id, model_id))
    {
        apply_catalog_model_detail(&mut model, provider_id, detail);
    }
    model
}

fn catalog_model_detail<'a>(
    settings: &'a tura_llm_rust::Settings,
    provider_id: &str,
    model_id: &str,
) -> Option<&'a tura_llm_rust::CatalogModelDetail> {
    let provider = settings.model_catalog.providers.get(provider_id)?;
    provider
        .models
        .values()
        .flatten()
        .find(|entry| {
            tura_llm_rust::Settings::normalize_model_name(provider_id, entry.id()) == model_id
                || entry.id() == model_id
        })?
        .detail()
}

fn apply_catalog_model_detail(
    model: &mut SdkProviderModel,
    provider_id: &str,
    detail: &tura_llm_rust::CatalogModelDetail,
) {
    if !detail.name.trim().is_empty() {
        model.name = detail.name.clone();
    }
    if !detail.family.trim().is_empty() {
        model.family = detail.family.clone();
    } else {
        model.family = provider_id.to_string();
    }
    if !detail.release_date.trim().is_empty() {
        model.release_date = detail.release_date.clone();
    }
    model.attachment = detail.attachment;
    model.reasoning = detail.reasoning;
    model.temperature = detail.temperature;
    model.tool_call = detail.tool_call;
    model.limit = SdkProviderModelLimit {
        context: detail.limit.context,
        input: detail.limit.input,
        output: detail.limit.output,
    };
    model.modalities = SdkProviderModelModalities {
        input: detail.modalities.input.clone(),
        output: detail.modalities.output.clone(),
    };
    model.options = detail.options.clone().into_iter().collect();
    model.status = detail.status.clone();
}

fn browser_login_provider_defaults() -> [(&'static str, &'static str); 4] {
    [
        ("codex", "gpt-5.1-codex"),
        ("anthropic", "claude-sonnet-4-20250514"),
        ("antigravity", "antigravity-browser"),
        ("github-copilot", "copilot-chat"),
    ]
}

fn provider_model_catalog() -> Vec<(String, Vec<String>)> {
    tura_llm_rust::provider_auth_registry()
        .iter()
        .filter(|entry| !entry.supported_models.is_empty())
        .map(|entry| {
            (
                entry.provider_id.to_string(),
                entry
                    .supported_models
                    .iter()
                    .map(|model| model.to_string())
                    .collect(),
            )
        })
        .collect()
}

fn model_supported_by_provider(provider_id: &str, model_id: &str) -> bool {
    provider_model_catalog()
        .into_iter()
        .find(|(id, _)| id == provider_id)
        .map(|(_, models)| models.iter().any(|candidate| candidate == model_id))
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
        if let Some(entry) = tura_llm_rust::provider_auth_registry_entry(&provider.id) {
            provider
                .options
                .entry("domains".to_string())
                .or_insert_with(|| serde_json::json!(["llm"]));
            provider
                .options
                .entry("capabilities".to_string())
                .or_insert_with(|| serde_json::json!(["llm.chat", "llm.tool_call", "oauth.login"]));
            provider.options.insert(
                "auth_methods".to_string(),
                serde_json::json!(entry
                    .auth_methods
                    .iter()
                    .map(|method| format!("{:?}", method.kind))
                    .collect::<Vec<_>>()),
            );
        }
        let fallback_env_key = provider_env_key(&provider.id);
        if provider.env.is_empty() {
            provider.env.push(fallback_env_key.clone());
        }
        let key = provider
            .env
            .iter()
            .find_map(|env_key| provider_key_value_for_env(&provider.id, env_key));
        let has_key = key.is_some();
        provider.key = key;
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
    settings.provider_base_url(provider_id)
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

    // claude-code uses the subscription OAuth token against the native Anthropic
    // Messages API, so it must keep its own provider id (the runtime maps it to
    // "anthropic", which would resolve ANTHROPIC_API_KEY and the wrong call path).
    let runtime_call_provider = if provider_id == "claude-code" {
        provider_id
    } else {
        runtime_provider_id
    };
    let config = tura_llm_rust::ProviderConfig {
        provider: runtime_call_provider.to_string(),
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
    let _ = global_store().set_auth(&provider_id, payload);
    Json(saved)
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
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub level: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub details: Vec<ProviderAuthActionDetail>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<ProviderAuthStatusResponse>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ProviderAuthActionDetail {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
}

pub(super) async fn refresh_provider_auth_if_needed(
    provider_id: &str,
    force: bool,
) -> Result<bool, String> {
    auth_refresh::refresh_provider_auth_if_needed(provider_id, force).await
}

pub(crate) fn start_provider_auth_scheduler() {
    auth_scheduler::start_provider_auth_scheduler();
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
    let status = build_provider_auth_status(&provider_id);
    let receipt = validate_provider_auth_config(&provider_id, &status).await;
    Json(ProviderAuthActionResponse {
        ok: receipt.ok,
        provider_id,
        code: receipt.code,
        message: receipt.message,
        level: Some(receipt.level),
        details: receipt.details,
        status: Some(status),
    })
}

struct ProviderValidationReceipt {
    ok: bool,
    level: String,
    code: String,
    message: String,
    details: Vec<ProviderAuthActionDetail>,
}

async fn validate_provider_auth_config(
    provider_id: &str,
    status: &ProviderAuthStatusResponse,
) -> ProviderValidationReceipt {
    let settings = tura_llm_rust::Settings::default().await.ok();
    let provider_config = settings
        .as_deref()
        .and_then(|settings| settings.model_catalog.providers.get(provider_id));
    let mut env_keys = Vec::new();
    push_unique_env(&mut env_keys, status.token_env.as_deref());
    push_unique_env(&mut env_keys, status.login_env.as_deref());
    push_unique_env(&mut env_keys, status.refresh_env.as_deref());
    if let Some(settings) = settings.as_ref() {
        for env in provider_env_from_settings(Some(settings), provider_id) {
            push_unique_env(&mut env_keys, Some(&env));
        }
    }
    if let Some(entry) = tura_llm_rust::provider_auth_registry_entry(provider_id) {
        push_unique_env(&mut env_keys, entry.token_env);
        push_unique_env(&mut env_keys, entry.login_env);
        push_unique_env(&mut env_keys, entry.refresh_env);
    }

    let present: Vec<String> = env_keys
        .iter()
        .filter(|key| config_value(key).is_some())
        .cloned()
        .collect();
    let missing: Vec<String> = env_keys
        .iter()
        .filter(|key| config_value(key).is_none())
        .cloned()
        .collect();

    let base_url = provider_api_from_settings(settings.as_deref(), provider_id).or_else(|| {
        tura_llm_rust::provider_auth_registry_entry(provider_id)
            .map(|entry| entry.default_base_url.to_string())
            .filter(|url| !url.trim().is_empty())
    });
    let url_ok = base_url
        .as_deref()
        .map(|url| reqwest::Url::parse(url).is_ok())
        .unwrap_or(true);
    let token_env = status
        .token_env
        .as_deref()
        .or_else(|| provider_config.and_then(|provider| provider.token_env.as_deref()))
        .or_else(|| {
            tura_llm_rust::provider_auth_registry_entry(provider_id)
                .and_then(|entry| entry.token_env)
        });
    let token = token_env.and_then(config_value);
    let external_validation = validate_provider_credentials_remotely(
        provider_id,
        provider_config,
        base_url.as_deref(),
        token.as_deref(),
    )
    .await;
    let warning = matches!(
        external_validation,
        ProviderCredentialValidation::Warning(_)
    );
    let unsupported = matches!(
        external_validation,
        ProviderCredentialValidation::Unsupported(_)
    );
    let configured = status.authenticated || status.configured || !present.is_empty();
    let ok = if unsupported {
        url_ok && configured
    } else {
        url_ok
            && matches!(
                external_validation,
                ProviderCredentialValidation::Passed(_) | ProviderCredentialValidation::Warning(_)
            )
    };

    let mut parts = Vec::new();
    let mut details = Vec::new();
    if unsupported {
        parts.push("credential validation unavailable".to_string());
        details.push(validation_detail("provider.validation.unavailable", None));
    } else if ok {
        parts.push("credential validation passed".to_string());
        details.push(validation_detail("provider.validation.passed", None));
    } else {
        parts.push("credential validation failed".to_string());
        details.push(validation_detail("provider.validation.failed", None));
    }
    if let Some(base_url) = base_url {
        details.push(validation_detail(
            if url_ok {
                "provider.base_url.ok"
            } else {
                "provider.base_url.invalid"
            },
            Some(base_url.clone()),
        ));
        parts.push(format!(
            "base_url {} ({})",
            if url_ok { "ok" } else { "invalid" },
            base_url
        ));
    }
    if !present.is_empty() {
        details.push(validation_detail(
            "provider.env.present",
            Some(present.join(", ")),
        ));
        parts.push(format!("present env: {}", present.join(", ")));
    }
    if !missing.is_empty() {
        details.push(validation_detail(
            "provider.env.missing",
            Some(missing.join(", ")),
        ));
        parts.push(format!("missing env: {}", missing.join(", ")));
    }
    if env_keys.is_empty() {
        details.push(validation_detail("provider.env.none_registered", None));
        parts.push("no credential env is registered for this provider".to_string());
    }
    match external_validation {
        ProviderCredentialValidation::Passed(detail)
        | ProviderCredentialValidation::Warning(detail)
        | ProviderCredentialValidation::Failed(detail)
        | ProviderCredentialValidation::Unsupported(detail) => {
            parts.push(detail.message.clone());
            details.push(detail);
        }
    }
    details.push(validation_detail("provider.request.no_paid_model", None));
    parts.push("no paid model request was sent".to_string());
    let level = if unsupported {
        "unsupported"
    } else if ok && warning {
        "warning"
    } else if ok {
        "valid"
    } else {
        "invalid"
    };
    let code = match level {
        "valid" => "provider.validation.valid",
        "warning" => "provider.validation.warning",
        "unsupported" => "provider.validation.unsupported",
        _ => "provider.validation.invalid",
    };
    ProviderValidationReceipt {
        ok,
        level: level.to_string(),
        code: code.to_string(),
        message: parts.join("; "),
        details,
    }
}

enum ProviderCredentialValidation {
    Passed(ProviderAuthActionDetail),
    Warning(ProviderAuthActionDetail),
    Failed(ProviderAuthActionDetail),
    Unsupported(ProviderAuthActionDetail),
}

async fn validate_provider_credentials_remotely(
    provider_id: &str,
    provider_config: Option<&tura_llm_rust::ProviderCatalogConfig>,
    base_url: Option<&str>,
    token: Option<&str>,
) -> ProviderCredentialValidation {
    let api_style = provider_config
        .map(|provider| provider.api_style.as_str())
        .unwrap_or_default();
    let registry_entry = tura_llm_rust::provider_auth_registry_entry(provider_id);
    let has_llm_domain = provider_config
        .map(|provider| provider_has_domain(provider, "llm"))
        .unwrap_or_else(|| {
            registry_entry
                .map(|entry| entry.capabilities.supports_model_validation)
                .unwrap_or(false)
                || is_openai_compatible_provider(provider_id)
        });
    let runtime_provider = provider_config
        .map(|provider| provider.runtime_provider.as_str())
        .filter(|value| !value.trim().is_empty())
        .or_else(|| registry_entry.map(|entry| entry.runtime_provider_id))
        .unwrap_or(provider_id);
    let token = token.map(str::trim).filter(|value| !value.is_empty());
    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(12))
        .build()
    {
        Ok(client) => client,
        Err(error) => {
            return ProviderCredentialValidation::Failed(validation_detail(
                "provider.validation.client_setup_failed",
                Some(error.to_string()),
            ));
        }
    };

    if matches!(api_style, "codex" | "claude_code")
        || matches!(provider_id, "codex" | "claude-code" | "antigravity")
    {
        let Some(token) = token else {
            return ProviderCredentialValidation::Failed(validation_detail(
                "provider.credential.oauth_token_missing",
                None,
            ));
        };
        if !looks_like_bearer_token(token) {
            return ProviderCredentialValidation::Failed(validation_detail(
                "provider.credential.oauth_token_invalid_format",
                None,
            ));
        }
    }

    if api_style == "codex" || provider_id == "codex" {
        let Some(token) = token else {
            return ProviderCredentialValidation::Failed(validation_detail(
                "provider.credential.oauth_token_missing",
                Some("Codex".to_string()),
            ));
        };
        return validate_bearer_get(
            &client,
            "https://chatgpt.com/backend-api/models",
            token,
            &[],
            "Codex subscription model list",
        )
        .await;
    }

    if provider_id == "antigravity" || api_style == "antigravity" {
        let Some(token) = token else {
            return ProviderCredentialValidation::Failed(validation_detail(
                "provider.credential.oauth_token_missing",
                Some("Antigravity".to_string()),
            ));
        };
        return validate_bearer_get(
            &client,
            "https://www.googleapis.com/oauth2/v3/userinfo",
            token,
            &[],
            "Google OAuth userinfo",
        )
        .await;
    }

    if matches!(provider_id, "github" | "github-copilot") {
        let Some(token) = token else {
            return ProviderCredentialValidation::Failed(validation_detail(
                "provider.credential.token_missing",
                Some("GitHub".to_string()),
            ));
        };
        return validate_bearer_get(
            &client,
            "https://api.github.com/user",
            token,
            &[("user-agent", "tura-gateway")],
            "GitHub /user",
        )
        .await;
    }

    if provider_id == "openrouter" {
        let Some(token) = token else {
            return ProviderCredentialValidation::Failed(validation_detail(
                "provider.credential.api_key_missing",
                Some("OpenRouter".to_string()),
            ));
        };
        return validate_bearer_get(
            &client,
            "https://openrouter.ai/api/v1/key",
            token,
            &[],
            "OpenRouter current key",
        )
        .await;
    }

    if provider_id == "huggingface" {
        let Some(token) = token else {
            return ProviderCredentialValidation::Failed(validation_detail(
                "provider.credential.token_missing",
                Some("Hugging Face".to_string()),
            ));
        };
        return validate_bearer_get(
            &client,
            "https://huggingface.co/api/whoami-v2",
            token,
            &[],
            "Hugging Face whoami-v2",
        )
        .await;
    }

    if provider_id == "replicate" {
        let Some(token) = token else {
            return ProviderCredentialValidation::Failed(validation_detail(
                "provider.credential.token_missing",
                Some("Replicate".to_string()),
            ));
        };
        return validate_bearer_get(
            &client,
            "https://api.replicate.com/v1/account",
            token,
            &[],
            "Replicate account",
        )
        .await;
    }

    if provider_id == "fireworks" {
        let Some(token) = token else {
            return ProviderCredentialValidation::Failed(validation_detail(
                "provider.credential.api_key_missing",
                Some("Fireworks".to_string()),
            ));
        };
        return validate_bearer_get(
            &client,
            "https://api.fireworks.ai/v1/accounts",
            token,
            &[],
            "Fireworks accounts",
        )
        .await;
    }

    if provider_id == "cohere" {
        let Some(token) = token else {
            return ProviderCredentialValidation::Failed(validation_detail(
                "provider.credential.token_missing",
                Some("Cohere".to_string()),
            ));
        };
        return validate_bearer_get(
            &client,
            "https://api.cohere.com/v1/models",
            token,
            &[],
            "Cohere /models",
        )
        .await;
    }

    if provider_id == "perplexity" {
        return ProviderCredentialValidation::Unsupported(validation_detail(
            "provider.validation.public_model_list_unsupported",
            Some("Perplexity".to_string()),
        ));
    }

    if provider_id == "line" {
        let Some(token) = token else {
            return ProviderCredentialValidation::Failed(validation_detail(
                "provider.credential.token_missing",
                Some("LINE".to_string()),
            ));
        };
        return validate_bearer_get(
            &client,
            "https://api.line.me/v2/bot/info",
            token,
            &[],
            "LINE bot info",
        )
        .await;
    }

    if provider_id == "slack" {
        let Some(token) = token else {
            return ProviderCredentialValidation::Failed(validation_detail(
                "provider.credential.token_missing",
                Some("Slack".to_string()),
            ));
        };
        return validate_bearer_get(
            &client,
            "https://slack.com/api/auth.test",
            token,
            &[],
            "Slack auth.test",
        )
        .await;
    }

    if provider_id == "claude-code" || api_style == "claude_code" {
        let Some(token) = token else {
            return ProviderCredentialValidation::Failed(validation_detail(
                "provider.credential.oauth_token_missing",
                Some("Claude Code".to_string()),
            ));
        };
        let Some(url) = validation_url(base_url, "models") else {
            return ProviderCredentialValidation::Failed(validation_detail(
                "provider.base_url.invalid",
                Some("Claude Code".to_string()),
            ));
        };
        return validate_anthropic_oauth_models(&client, &url, token).await;
    }

    if (has_llm_domain && runtime_provider == "anthropic")
        || provider_id.contains("anthropic")
        || provider_id.contains("claude")
        || base_url
            .map(|url| url.contains("anthropic.com"))
            .unwrap_or(false)
    {
        let Some(token) = token else {
            return ProviderCredentialValidation::Failed(validation_detail(
                "provider.credential.token_missing",
                Some("Anthropic".to_string()),
            ));
        };
        let Some(url) = validation_url(base_url, "models") else {
            return ProviderCredentialValidation::Failed(validation_detail(
                "provider.base_url.invalid",
                Some("Anthropic".to_string()),
            ));
        };
        return validate_anthropic_models(&client, &url, token).await;
    }

    if has_llm_domain && (api_style == "google" || runtime_provider == "google") {
        let Some(token) = token else {
            return ProviderCredentialValidation::Failed(validation_detail(
                "provider.credential.api_key_missing",
                Some("Google".to_string()),
            ));
        };
        let Some(url) = validation_url(base_url, "models") else {
            return ProviderCredentialValidation::Failed(validation_detail(
                "provider.base_url.invalid",
                Some("Google".to_string()),
            ));
        };
        return validate_google_models(&client, &url, token).await;
    }

    if has_llm_domain
        && (matches!(api_style, "openapi" | "ollama") || is_openai_compatible_provider(provider_id))
    {
        let Some(url) = validation_url(base_url, "models") else {
            return ProviderCredentialValidation::Failed(validation_detail(
                "provider.base_url.invalid",
                Some("OpenAI-compatible".to_string()),
            ));
        };
        if !is_local_no_token_provider(provider_id, base_url) && token.is_none() {
            return ProviderCredentialValidation::Failed(validation_detail(
                "provider.credential.api_key_missing",
                Some("OpenAI-compatible".to_string()),
            ));
        }
        return validate_openai_compatible_models(&client, &url, token).await;
    }

    ProviderCredentialValidation::Unsupported(validation_detail(
        "provider.validation.gateway_not_configured",
        None,
    ))
}

fn provider_has_domain(provider: &tura_llm_rust::ProviderCatalogConfig, domain: &str) -> bool {
    provider
        .domains
        .iter()
        .any(|value| value.eq_ignore_ascii_case(domain))
}

fn validation_detail(code: &str, value: Option<String>) -> ProviderAuthActionDetail {
    let english = validation_detail_english(code);
    let message = match value.as_deref() {
        Some(value) if !value.is_empty() => format!("{english}: {value}"),
        _ => english.to_string(),
    };
    ProviderAuthActionDetail {
        code: code.to_string(),
        message,
        value,
    }
}

fn validation_detail_english(code: &str) -> &'static str {
    match code {
        "provider.validation.passed" => "credential validation passed",
        "provider.validation.failed" => "credential validation failed",
        "provider.validation.unavailable" => "credential validation unavailable",
        "provider.base_url.ok" => "base URL is valid",
        "provider.base_url.invalid" => "base URL is invalid",
        "provider.env.present" => "configured environment variables",
        "provider.env.missing" => "missing environment variables",
        "provider.env.none_registered" => "no credential environment variable is registered",
        "provider.remote.accepted" => "remote validation accepted credentials",
        "provider.remote.permission_limited" => {
            "remote validation authenticated credentials but reported limited permission"
        }
        "provider.remote.rejected" => "remote validation rejected credentials",
        "provider.remote.request_failed" => "remote validation request failed",
        "provider.validation.client_setup_failed" => "validation client setup failed",
        "provider.credential.oauth_token_missing" => "OAuth access token is missing",
        "provider.credential.oauth_token_invalid_format" => "OAuth access token format is invalid",
        "provider.credential.token_missing" => "token is missing",
        "provider.credential.api_key_missing" => "API key is missing",
        "provider.validation.public_model_list_unsupported" => {
            "public model list cannot validate credentials"
        }
        "provider.validation.gateway_not_configured" => {
            "validation gateway is not configured for this provider"
        }
        "provider.request.no_paid_model" => "no paid model request was sent",
        "provider.auth.refresh.unsupported" => "auth refresh is unsupported",
        "provider.auth.refresh.failed" => "auth refresh failed",
        "provider.auth.refresh.succeeded" => "auth refresh succeeded",
        "provider.auth.not_configured" => "provider auth is not configured",
        "provider.auth.logout.succeeded" => "provider auth logged out",
        "provider.auth.logout.failed" => "provider auth logout failed",
        _ => "provider validation detail",
    }
}

fn is_local_no_token_provider(provider_id: &str, base_url: Option<&str>) -> bool {
    matches!(provider_id, "ollama" | "lmstudio")
        || base_url
            .map(|url| url.contains("localhost") || url.contains("127.0.0.1"))
            .unwrap_or(false)
}

fn is_openai_compatible_provider(provider_id: &str) -> bool {
    matches!(
        provider_id,
        "openai"
            | "openai-api"
            | "gpt"
            | "openrouter"
            | "deepseek"
            | "qwen"
            | "qwen-cn"
            | "mistral"
            | "xai"
            | "grok"
            | "kimi"
            | "moonshot"
            | "huggingface"
            | "replicate"
            | "together"
            | "fireworks"
            | "groq"
            | "perplexity"
            | "volcengine"
            | "baidu_qianfan"
    )
}

fn validation_url(base_url: Option<&str>, suffix: &str) -> Option<String> {
    let base_url = base_url?.trim();
    if base_url.is_empty() || base_url.contains('{') || base_url.contains('}') {
        return None;
    }
    reqwest::Url::parse(base_url).ok()?;
    Some(format!(
        "{}/{}",
        base_url.trim_end_matches('/'),
        suffix.trim_start_matches('/')
    ))
}

async fn validate_openai_compatible_models(
    client: &reqwest::Client,
    url: &str,
    token: Option<&str>,
) -> ProviderCredentialValidation {
    let mut request = client.get(url);
    if let Some(token) = token {
        request = request.bearer_auth(token);
    }
    validate_response(request, "OpenAI-compatible /models").await
}

async fn validate_anthropic_models(
    client: &reqwest::Client,
    url: &str,
    token: &str,
) -> ProviderCredentialValidation {
    validate_response(
        client
            .get(url)
            .header("x-api-key", token)
            .header("anthropic-version", "2023-06-01"),
        "Anthropic /models",
    )
    .await
}

async fn validate_anthropic_oauth_models(
    client: &reqwest::Client,
    url: &str,
    token: &str,
) -> ProviderCredentialValidation {
    validate_response(
        client
            .get(url)
            .bearer_auth(token)
            .header("anthropic-version", "2023-06-01")
            .header("anthropic-beta", "oauth-2025-04-20"),
        "Claude Code OAuth /models",
    )
    .await
}

async fn validate_google_models(
    client: &reqwest::Client,
    url: &str,
    token: &str,
) -> ProviderCredentialValidation {
    validate_response(
        client.get(url).query(&[("key", token)]),
        "Google list models",
    )
    .await
}

async fn validate_bearer_get(
    client: &reqwest::Client,
    url: &str,
    token: &str,
    headers: &[(&str, &str)],
    label: &str,
) -> ProviderCredentialValidation {
    let mut request = client.get(url).bearer_auth(token);
    for (key, value) in headers {
        request = request.header(*key, *value);
    }
    validate_response(request, label).await
}

async fn validate_response(
    request: reqwest::RequestBuilder,
    label: &str,
) -> ProviderCredentialValidation {
    match request.send().await {
        Ok(response) => {
            let status = response.status();
            let text = if status.is_success() {
                String::new()
            } else {
                response.text().await.unwrap_or_default()
            };
            if status.is_success() {
                ProviderCredentialValidation::Passed(validation_detail(
                    "provider.remote.accepted",
                    Some(label.to_string()),
                ))
            } else if status == reqwest::StatusCode::FORBIDDEN
                && response_forbidden_but_authenticated(&text)
            {
                ProviderCredentialValidation::Warning(validation_detail(
                    "provider.remote.permission_limited",
                    Some(label.to_string()),
                ))
            } else {
                ProviderCredentialValidation::Failed(validation_detail(
                    "provider.remote.rejected",
                    Some(format!(
                        "{label} HTTP {status}: {}",
                        truncate_validation_body(&text)
                    )),
                ))
            }
        }
        Err(error) => ProviderCredentialValidation::Failed(validation_detail(
            "provider.remote.request_failed",
            Some(format!("{label}: {error}")),
        )),
    }
}

fn response_forbidden_but_authenticated(body: &str) -> bool {
    let body = body.to_ascii_lowercase();
    body.contains("insufficient permissions")
        || body.contains("missing scopes")
        || body.contains("permission")
}

fn truncate_validation_body(body: &str) -> String {
    let compact = body.split_whitespace().collect::<Vec<_>>().join(" ");
    if compact.len() > 240 {
        format!("{}...", &compact[..240])
    } else if compact.is_empty() {
        "<empty response>".to_string()
    } else {
        compact
    }
}

fn looks_like_bearer_token(token: &str) -> bool {
    token.split('.').count() >= 3 || token.len() >= 24
}

fn push_unique_env(keys: &mut Vec<String>, key: Option<&str>) {
    let Some(key) = key.map(str::trim).filter(|value| !value.is_empty()) else {
        return;
    };
    if !keys.iter().any(|item| item == key) {
        keys.push(key.to_string());
    }
}

pub async fn provider_auth_refresh(
    Path(provider_id): Path<String>,
) -> Json<ProviderAuthActionResponse> {
    let can_refresh = auth_validator::provider_auth_can_refresh(&provider_id);
    let refresh_result = refresh_provider_auth_if_needed(&provider_id, true).await;
    let status = build_provider_auth_status(&provider_id);
    let ok = can_refresh && status.authenticated && refresh_result.is_ok();
    let (code, message, details) = if !can_refresh {
        (
            "provider.auth.refresh.unsupported",
            "provider auth method does not support refresh".to_string(),
            vec![validation_detail("provider.auth.refresh.unsupported", None)],
        )
    } else if let Err(error) = refresh_result {
        (
            "provider.auth.refresh.failed",
            format!("provider auth refresh failed: {error}"),
            vec![validation_detail(
                "provider.auth.refresh.failed",
                Some(error),
            )],
        )
    } else if ok {
        (
            "provider.auth.refresh.succeeded",
            "provider auth refreshed".to_string(),
            vec![validation_detail("provider.auth.refresh.succeeded", None)],
        )
    } else {
        (
            "provider.auth.not_configured",
            "provider auth is not configured".to_string(),
            vec![validation_detail("provider.auth.not_configured", None)],
        )
    };
    Json(ProviderAuthActionResponse {
        ok,
        provider_id,
        code: code.to_string(),
        message,
        level: Some(if ok { "valid" } else { "invalid" }.to_string()),
        details,
        status: Some(status),
    })
}

pub async fn provider_auth_logout(
    Path(provider_id): Path<String>,
) -> Json<ProviderAuthActionResponse> {
    let result = logout_provider_auth(&provider_id);
    let status = build_provider_auth_status(&provider_id);
    let ok = result.is_ok();
    let (code, message, details) = result
        .map(|_| {
            (
                "provider.auth.logout.succeeded",
                "provider auth logged out".to_string(),
                vec![validation_detail("provider.auth.logout.succeeded", None)],
            )
        })
        .unwrap_or_else(|error| {
            (
                "provider.auth.logout.failed",
                format!("provider auth logout failed: {error}"),
                vec![validation_detail(
                    "provider.auth.logout.failed",
                    Some(error.to_string()),
                )],
            )
        });
    Json(ProviderAuthActionResponse {
        ok,
        provider_id,
        code: code.to_string(),
        message,
        level: Some(if ok { "valid" } else { "invalid" }.to_string()),
        details,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authorize_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub docs_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub configured_value: Option<String>,
    pub available: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unavailable_reason: Option<String>,
    pub supports_refresh: bool,
}

pub async fn provider_auth(
    Query(_params): Query<ProviderAuthQuery>,
) -> Json<HashMap<String, Vec<ProviderAuthMethod>>> {
    let mut response = HashMap::new();
    for entry in auth_registry::entries() {
        let methods = provider_auth_methods(entry.provider_id);
        if !methods.is_empty() {
            response.insert(entry.provider_id.to_string(), methods);
        }
    }
    if let Ok(settings) = tura_llm_rust::Settings::default().await {
        for (provider_id, provider) in &settings.model_catalog.providers {
            if response.contains_key(provider_id) {
                continue;
            }
            let methods = provider_auth_methods_from_config(provider_id, provider);
            if !methods.is_empty() {
                response.insert(provider_id.to_string(), methods);
            }
        }
    }
    Json(response)
}

fn provider_auth_methods(provider_id: &str) -> Vec<ProviderAuthMethod> {
    let Some(entry) = auth_registry::entry(provider_id) else {
        return Vec::new();
    };
    entry
        .auth_methods
        .iter()
        .map(|method| {
            let unavailable_reason = auth_method_unavailable_reason(
                provider_id,
                method.kind,
                entry.oauth_authorize_kind,
            );
            ProviderAuthMethod {
                method_type: legacy_auth_method_type(method.kind).to_string(),
                kind: method.kind,
                login: method.login.to_string(),
                label: method.label.to_string(),
                prompts: None,
                token_env: entry.token_env.map(ToString::to_string),
                login_env: entry.login_env.map(ToString::to_string),
                authorize_url: oauth_authorize_endpoint(entry.oauth_authorize_kind),
                token_url: oauth_token_endpoint(entry.oauth_authorize_kind),
                api_key_url: provider_api_key_url(provider_id),
                docs_url: provider_auth_docs_url(provider_id),
                configured_value: entry
                    .token_env
                    .and_then(|token_env| provider_auth_method_value(provider_id, token_env)),
                available: unavailable_reason.is_none(),
                unavailable_reason,
                supports_refresh: entry.capabilities.supports_oauth_refresh,
            }
        })
        .collect()
}

fn provider_auth_methods_from_config(
    provider_id: &str,
    provider: &tura_llm_rust::ProviderCatalogConfig,
) -> Vec<ProviderAuthMethod> {
    let envs = provider_auth_envs_from_config(provider);
    envs.into_iter()
        .map(|env| ProviderAuthMethod {
            method_type: "api".to_string(),
            kind: AuthMethodKind::ApiKey,
            login: "api".to_string(),
            label: env.clone(),
            prompts: None,
            token_env: Some(env.clone()),
            login_env: None,
            authorize_url: None,
            token_url: None,
            api_key_url: provider_api_key_url(provider_id),
            docs_url: provider_auth_docs_url(provider_id).or_else(|| provider.api_docs.clone()),
            configured_value: provider_auth_method_value(provider_id, &env),
            available: true,
            unavailable_reason: None,
            supports_refresh: false,
        })
        .collect()
}

fn provider_auth_envs_from_config(provider: &tura_llm_rust::ProviderCatalogConfig) -> Vec<String> {
    let mut envs = Vec::new();
    if let Some(token_env) = provider
        .token_env
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        envs.push(token_env.to_string());
    }
    for env in &provider.env {
        if !env.trim().is_empty() && !envs.iter().any(|item| item == env) {
            envs.push(env.clone());
        }
    }
    envs
}

fn configured_provider_value(provider_id: &str, env: &str) -> Option<String> {
    if auth_registry::entry(provider_id).is_some() {
        provider_key_value_for_env(provider_id, env)
    } else {
        config_value(env).map(|value| value.trim().to_string())
    }
}

fn provider_auth_method_value(provider_id: &str, env: &str) -> Option<String> {
    configured_provider_value(provider_id, env)
        .or_else(|| config_value(env).map(|value| value.trim().to_string()))
        .filter(|value| !value.is_empty())
}

fn auth_method_unavailable_reason(
    provider_id: &str,
    kind: AuthMethodKind,
    authorize_kind: Option<OAuthAuthorizeKind>,
) -> Option<String> {
    if !matches!(kind, AuthMethodKind::OAuthPkce | AuthMethodKind::DeviceCode) {
        return None;
    }
    match authorize_kind.unwrap_or(OAuthAuthorizeKind::Unsupported) {
        OAuthAuthorizeKind::OpenAiPkce => None,
        OAuthAuthorizeKind::AnthropicPkce => None,
        OAuthAuthorizeKind::GooglePkce => {
            google_oauth_client_id(provider_id).is_none().then(|| {
                "GOOGLE_OAUTH_CLIENT_ID is required before Google OAuth can start".to_string()
            })
        }
        OAuthAuthorizeKind::GithubDevice => github_copilot_oauth_client_id().is_none().then(|| {
            "GITHUB_COPILOT_CLIENT_ID is required before GitHub Copilot OAuth can start".to_string()
        }),
        OAuthAuthorizeKind::BrowserTokenPaste | OAuthAuthorizeKind::Unsupported => Some(format!(
            "{} OAuth is listed but is not implemented yet",
            provider_display_name(provider_id)
        )),
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
            AuthMethodKind::OAuthPkce | AuthMethodKind::LocalCliToken | AuthMethodKind::DeviceCode
        )
    });

    let Some(selected_method) = selected_method else {
        return Json(OAuthAuthorizeResponse {
            url: String::new(),
            method: OAuthMethod::Code,
            instructions: "Invalid auth method".to_string(),
        });
    };

    if let Some(reason) = selected_method.unavailable_reason.as_deref() {
        return Json(OAuthAuthorizeResponse {
            url: String::new(),
            method: OAuthMethod::Code,
            instructions: reason.to_string(),
        });
    }

    let entry = tura_llm_rust::provider_auth_registry_entry(&provider_id);
    let authorize_kind = entry
        .and_then(|entry| entry.oauth_authorize_kind)
        .unwrap_or(OAuthAuthorizeKind::Unsupported);

    let (url, method, instructions) = if authorize_kind == OAuthAuthorizeKind::GithubDevice {
        match start_github_copilot_device_flow(&provider_id).await {
            Ok(device) => {
                global_store().set_oauth_state(
                    &provider_id,
                    "device_code".to_string(),
                    Some(device.user_code.clone()),
                    device.verification_uri.clone(),
                    Some(oauth_state()),
                    Some(device.device_code.clone()),
                );
                (
                    device.verification_uri.clone(),
                    OAuthMethod::Code,
                    format!(
                        "Open {} and enter code {}. After GitHub authorizes Copilot, submit the same code here.",
                        device.verification_uri, device.user_code
                    ),
                )
            }
            Err(error) => {
                return Json(OAuthAuthorizeResponse {
                    url: String::new(),
                    method: OAuthMethod::Code,
                    instructions: format!("GitHub Copilot OAuth cannot start: {error}"),
                });
            }
        }
    } else if matches!(
        authorize_kind,
        OAuthAuthorizeKind::OpenAiPkce
            | OAuthAuthorizeKind::AnthropicPkce
            | OAuthAuthorizeKind::GooglePkce
    ) {
        let state = oauth_state();
        let code_verifier = oauth_code_verifier();
        let code_challenge = oauth_code_challenge(&code_verifier);
        let Some(url) = oauth_authorize_url(&provider_id, authorize_kind, &state, &code_challenge)
        else {
            return Json(OAuthAuthorizeResponse {
                url: String::new(),
                method: OAuthMethod::Code,
                instructions: format!(
                    "{} OAuth cannot start because its OAuth client configuration is incomplete.",
                    provider_display_name(&provider_id)
                ),
            });
        };
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
            format!(
                "Complete {} authorization in your browser. This window will close automatically.",
                provider_display_name(&provider_id)
            ),
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
) -> Json<ProviderAuthActionResponse> {
    Json(complete_oauth_callback(provider_id, payload).await)
}

pub async fn oauth_callback_info(Path(provider_id): Path<String>) -> Html<String> {
    Html(format!(
        "{} OAuth callback expects a POST from the GUI. Paste the copied code/token into the provider dialog instead of opening this URL directly.",
        provider_display_name(&provider_id)
    ))
}

async fn complete_oauth_callback(
    provider_id: String,
    payload: OAuthCallbackPayload,
) -> ProviderAuthActionResponse {
    if payload
        .code
        .as_deref()
        .unwrap_or_default()
        .trim()
        .is_empty()
    {
        let pending_method = global_store().pending_oauth_method(&provider_id);
        let ok = match pending_method.as_deref() {
            Some("oauth_pkce") => wait_for_oauth_completed(&provider_id).await,
            Some("device_code") => complete_pending_device_oauth(&provider_id).await,
            _ => false,
        };
        return oauth_callback_response(
            &provider_id,
            ok,
            if ok {
                "provider.oauth.completed"
            } else {
                "provider.oauth.code_missing"
            },
            if ok {
                "provider OAuth completed"
            } else {
                "Paste the copied authorization code before submitting"
            },
        );
    }

    let has_pending = global_store().peek_oauth_state(&provider_id);
    if has_pending.is_none() {
        let normalized_code = normalize_oauth_code(payload.code.as_deref().unwrap_or_default());
        if provider_id == "claude-code" && looks_like_claude_code_oauth_token(&normalized_code.code)
        {
            return persist_direct_claude_code_oauth_token(&provider_id, normalized_code.code);
        }
        if provider_id == "claude-code" && normalized_code.verifier.is_some() {
            return complete_claude_code_manual_oauth(&provider_id, &normalized_code).await;
        }
        return oauth_callback_response(
            &provider_id,
            false,
            "provider.oauth.pending_missing",
            "No pending OAuth login was found. Click OAuth login again, then paste the new code.",
        );
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
        return oauth_callback_response(
            &provider_id,
            false,
            "provider.oauth.code_missing",
            "Paste the copied authorization code before submitting",
        );
    }

    if pending.method == "oauth_pkce"
        && payload.state.is_some()
        && payload.state.as_deref() != pending.state.as_deref()
    {
        return oauth_callback_response(
            &provider_id,
            false,
            "provider.oauth.state_mismatch",
            "OAuth state did not match. Click OAuth login again and paste the new code.",
        );
    }

    if pending.method == "code" {
        let expected_code = pending.code.as_ref();
        if payload.code.as_ref() != expected_code {
            return oauth_callback_response(
                &provider_id,
                false,
                "provider.oauth.code_mismatch",
                "OAuth confirmation code did not match",
            );
        }
    }

    let normalized_code = normalize_oauth_code(payload.code.as_deref().unwrap_or_default());
    if provider_id == "claude-code" && looks_like_claude_code_oauth_token(&normalized_code.code) {
        let response = persist_direct_claude_code_oauth_token(&provider_id, normalized_code.code);
        if response.ok {
            let _ = global_store().consume_oauth_state(&provider_id);
        }
        return response;
    }
    if provider_id == "claude-code" && normalized_code.verifier.is_some() {
        let response = complete_claude_code_manual_oauth(&provider_id, &normalized_code).await;
        if response.ok {
            let _ = global_store().consume_oauth_state(&provider_id);
        }
        return response;
    }
    if pending.method == "oauth_pkce" {
        if let Some(state) = normalized_code.state.as_deref() {
            if pending.state.as_deref() != Some(state) {
                return oauth_callback_response(
                    &provider_id,
                    false,
                    "provider.oauth.state_mismatch",
                    "OAuth state does not match the pending authorization request. Start login again and paste the newest code.",
                );
            }
        }
    }
    let tokens = if matches!(pending.method.as_str(), "oauth_pkce" | "device_code") {
        match exchange_oauth_code(&provider_id, &normalized_code, &pending).await {
            Ok(tokens) => Some(tokens),
            Err(error) => {
                return oauth_callback_response(
                    &provider_id,
                    false,
                    "provider.oauth.exchange_failed",
                    &format!("OAuth token exchange failed: {error}"),
                );
            }
        }
    } else {
        None
    };

    let key = if matches!(pending.method.as_str(), "oauth_pkce" | "device_code") {
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

    let login = if pending.method == "oauth_pkce" {
        "oauth"
    } else if pending.method == "device_code" {
        "device"
    } else {
        "browser"
    };
    let auth = auth_update::oauth_auth(
        key,
        tokens
            .as_ref()
            .and_then(|tokens| tokens.refresh_token.clone()),
        tokens
            .as_ref()
            .map(|tokens| Utc::now().timestamp_millis() + tokens.expires_in.unwrap_or(3600) * 1000),
        tokens.as_ref().and_then(extract_account_id),
        login,
        Some(pending.url.clone()),
    );

    if let Err(error) = persist_provider_auth(&provider_id, &auth) {
        return oauth_callback_response(
            &provider_id,
            false,
            "provider.oauth.persist_failed",
            &format!("OAuth token was received but could not be saved: {error}"),
        );
    }

    let _ = global_store().consume_oauth_state(&provider_id);
    let _ = global_store().set_auth(&provider_id, auth.clone());

    oauth_callback_response(
        &provider_id,
        true,
        "provider.oauth.completed",
        "provider OAuth completed",
    )
}

fn oauth_callback_response(
    provider_id: &str,
    ok: bool,
    code: &str,
    message: &str,
) -> ProviderAuthActionResponse {
    ProviderAuthActionResponse {
        ok,
        provider_id: provider_id.to_string(),
        code: code.to_string(),
        message: message.to_string(),
        level: Some(if ok { "valid" } else { "invalid" }.to_string()),
        details: vec![validation_detail(code, None)],
        status: Some(build_provider_auth_status(provider_id)),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct NormalizedOAuthCode {
    code: String,
    state: Option<String>,
    verifier: Option<String>,
}

fn normalize_oauth_code(code: &str) -> NormalizedOAuthCode {
    let trimmed = code.trim();
    if let Some(code) = trimmed.strip_prefix("code=") {
        let normalized = code
            .split('&')
            .next()
            .unwrap_or_default()
            .split('#')
            .next()
            .unwrap_or_default()
            .trim()
            .to_string();
        let state = trimmed
            .split('#')
            .nth(1)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToString::to_string);
        return NormalizedOAuthCode {
            code: normalized,
            state,
            verifier: None,
        };
    }
    if let Ok(url) = reqwest::Url::parse(trimmed) {
        if let Some(code) = url
            .query_pairs()
            .find_map(|(key, value)| (key == "code").then(|| value.to_string()))
        {
            let state = url
                .query_pairs()
                .find_map(|(key, value)| (key == "state").then(|| value.to_string()))
                .or_else(|| {
                    url.fragment()
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                        .map(ToString::to_string)
                });
            return NormalizedOAuthCode {
                code: code.trim().to_string(),
                state,
                verifier: None,
            };
        }
    }
    let mut parts = trimmed.splitn(2, '#');
    let code = parts.next().unwrap_or_default().trim().to_string();
    let fragment = parts
        .next()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string);
    let verifier = fragment
        .as_deref()
        .filter(|value| looks_like_pkce_verifier(value))
        .map(ToString::to_string);
    let state = if verifier.is_some() { None } else { fragment };
    NormalizedOAuthCode {
        code,
        state,
        verifier,
    }
}

fn looks_like_pkce_verifier(value: &str) -> bool {
    let len = value.len();
    (43..=128).contains(&len)
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~'))
}

fn looks_like_claude_code_oauth_token(value: &str) -> bool {
    let token = value.trim();
    token.starts_with("sk-ant-oat") || token.starts_with("sk-ant-ort")
}

fn persist_direct_claude_code_oauth_token(
    provider_id: &str,
    token: String,
) -> ProviderAuthActionResponse {
    let auth = auth_update::oauth_auth(token, None, None, None, "oauth", None);
    if let Err(error) = persist_provider_auth(provider_id, &auth) {
        return oauth_callback_response(
            provider_id,
            false,
            "provider.oauth.persist_failed",
            &format!("OAuth token could not be saved: {error}"),
        );
    }
    let _ = global_store().set_auth(provider_id, auth);
    oauth_callback_response(
        provider_id,
        true,
        "provider.oauth.completed",
        "provider OAuth token saved",
    )
}

async fn complete_claude_code_manual_oauth(
    provider_id: &str,
    normalized_code: &NormalizedOAuthCode,
) -> ProviderAuthActionResponse {
    let pending = crate::mock::store::PendingOAuth {
        method: "oauth_pkce".to_string(),
        code: None,
        url: String::new(),
        state: None,
        code_verifier: normalized_code.verifier.clone(),
        expires_at: Utc::now().timestamp_millis() + 15 * 60_000,
    };
    let tokens = match exchange_anthropic_oauth_code(normalized_code, &pending).await {
        Ok(tokens) => tokens,
        Err(error) => {
            return oauth_callback_response(
                provider_id,
                false,
                "provider.oauth.exchange_failed",
                &format!("OAuth token exchange failed: {error}"),
            );
        }
    };
    let auth = auth_update::oauth_auth(
        tokens.access_token.clone(),
        tokens.refresh_token.clone(),
        Some(Utc::now().timestamp_millis() + tokens.expires_in.unwrap_or(3600) * 1000),
        extract_account_id(&tokens),
        "oauth",
        None,
    );
    if let Err(error) = persist_provider_auth(provider_id, &auth) {
        return oauth_callback_response(
            provider_id,
            false,
            "provider.oauth.persist_failed",
            &format!("OAuth token was received but could not be saved: {error}"),
        );
    }
    let _ = global_store().set_auth(provider_id, auth);
    oauth_callback_response(
        provider_id,
        true,
        "provider.oauth.completed",
        "provider OAuth completed",
    )
}

async fn complete_pending_device_oauth(provider_id: &str) -> bool {
    let Some(pending) = global_store().consume_oauth_state(provider_id) else {
        return false;
    };
    if pending.method != "device_code" {
        return false;
    }
    let tokens = match exchange_oauth_code(
        provider_id,
        &NormalizedOAuthCode {
            code: String::new(),
            state: None,
            verifier: None,
        },
        &pending,
    )
    .await
    {
        Ok(tokens) => tokens,
        Err(_) => return false,
    };
    let auth = auth_update::oauth_auth(
        tokens.access_token.clone(),
        tokens.refresh_token.clone(),
        Some(Utc::now().timestamp_millis() + tokens.expires_in.unwrap_or(3600) * 1000),
        extract_account_id(&tokens),
        "device",
        Some(pending.url.clone()),
    );
    if persist_provider_auth(provider_id, &auth).is_err() {
        return false;
    }
    let _ = global_store().set_auth(provider_id, auth);
    true
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
    let tokens = match exchange_oauth_code(
        &provider_id,
        &NormalizedOAuthCode {
            code: code.to_string(),
            state: pending.state.clone(),
            verifier: None,
        },
        &pending,
    )
    .await
    {
        Ok(tokens) => tokens,
        Err(error) => {
            return Html(oauth_callback_html(
                false,
                &format!("Token exchange failed: {error}"),
            ))
        }
    };
    let auth = auth_update::oauth_auth(
        tokens.access_token.clone(),
        tokens.refresh_token.clone(),
        Some(Utc::now().timestamp_millis() + tokens.expires_in.unwrap_or(3600) * 1000),
        extract_account_id(&tokens),
        "oauth",
        Some(pending.url.clone()),
    );
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
        &format!(
            "{} OAuth connected. You can close this window.",
            provider_display_name(&provider_id)
        ),
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
struct OAuthTokenResponse {
    id_token: Option<String>,
    access_token: String,
    refresh_token: Option<String>,
    expires_in: Option<i64>,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct GithubDeviceCodeResponse {
    device_code: String,
    user_code: String,
    verification_uri: String,
    #[allow(dead_code)]
    expires_in: Option<i64>,
    #[allow(dead_code)]
    interval: Option<i64>,
}

async fn start_github_copilot_device_flow(
    provider_id: &str,
) -> anyhow::Result<GithubDeviceCodeResponse> {
    let client_id = github_copilot_oauth_client_id()
        .ok_or_else(|| anyhow::anyhow!("GITHUB_COPILOT_CLIENT_ID is not configured"))?;
    let scope = github_copilot_oauth_scope();
    let response = reqwest::Client::new()
        .post(github_device_code_url())
        .header("accept", "application/json")
        .form(&[("client_id", client_id.as_str()), ("scope", scope.as_str())])
        .send()
        .await?;
    let status = response.status();
    let body: serde_json::Value = response.json().await?;
    if !status.is_success() {
        return Err(anyhow::anyhow!(
            "{} device-code endpoint returned {status}: {body}",
            provider_display_name(provider_id)
        ));
    }
    let device: GithubDeviceCodeResponse = serde_json::from_value(body)?;
    if device.device_code.trim().is_empty()
        || device.user_code.trim().is_empty()
        || device.verification_uri.trim().is_empty()
    {
        return Err(anyhow::anyhow!(
            "{} device-code response was incomplete",
            provider_display_name(provider_id)
        ));
    }
    Ok(device)
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

async fn exchange_oauth_code(
    provider_id: &str,
    normalized_code: &NormalizedOAuthCode,
    pending: &crate::mock::store::PendingOAuth,
) -> anyhow::Result<OAuthTokenResponse> {
    let kind = tura_llm_rust::provider_auth_registry_entry(provider_id)
        .and_then(|entry| entry.oauth_callback_kind)
        .unwrap_or(OAuthAuthorizeKind::Unsupported);
    match kind {
        OAuthAuthorizeKind::OpenAiPkce => {
            exchange_openai_oauth_code(&normalized_code.code, pending).await
        }
        OAuthAuthorizeKind::AnthropicPkce => {
            exchange_anthropic_oauth_code(normalized_code, pending).await
        }
        OAuthAuthorizeKind::GooglePkce => {
            exchange_google_oauth_code(provider_id, &normalized_code.code, pending).await
        }
        OAuthAuthorizeKind::GithubDevice => {
            exchange_github_copilot_device_code(provider_id, pending).await
        }
        OAuthAuthorizeKind::BrowserTokenPaste | OAuthAuthorizeKind::Unsupported => Err(
            anyhow::anyhow!("unsupported OAuth callback provider: {provider_id}"),
        ),
    }
}

async fn exchange_github_copilot_device_code(
    provider_id: &str,
    pending: &crate::mock::store::PendingOAuth,
) -> anyhow::Result<OAuthTokenResponse> {
    let client_id = github_copilot_oauth_client_id()
        .ok_or_else(|| anyhow::anyhow!("GITHUB_COPILOT_CLIENT_ID is not configured"))?;
    let device_code = pending
        .code_verifier
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("missing GitHub device code"))?;
    let deadline = Instant::now() + Duration::from_secs(5 * 60);
    let mut interval = Duration::from_secs(5);
    let body = loop {
        let response = reqwest::Client::new()
            .post(github_oauth_token_url())
            .header("accept", "application/json")
            .form(&[
                ("client_id", client_id.as_str()),
                ("device_code", device_code),
                ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
            ])
            .send()
            .await?;
        let status = response.status();
        let body: serde_json::Value = response.json().await?;
        if !status.is_success() {
            return Err(anyhow::anyhow!(
                "{} token endpoint returned {status}: {body}",
                provider_display_name(provider_id)
            ));
        }
        match body.get("error").and_then(serde_json::Value::as_str) {
            Some("authorization_pending") if Instant::now() < deadline => {
                sleep(interval).await;
                continue;
            }
            Some("slow_down") if Instant::now() < deadline => {
                interval += Duration::from_secs(5);
                sleep(interval).await;
                continue;
            }
            Some(error) => {
                return Err(anyhow::anyhow!(
                    "{} token endpoint returned {error}: {}",
                    provider_display_name(provider_id),
                    body.get("error_description")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or_default()
                ));
            }
            None => break body,
        }
    };
    let access_token = body
        .get("access_token")
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| {
            anyhow::anyhow!(
                "{} token response did not include access_token",
                provider_display_name(provider_id)
            )
        })?
        .to_string();
    Ok(OAuthTokenResponse {
        id_token: body
            .get("id_token")
            .and_then(serde_json::Value::as_str)
            .map(ToString::to_string),
        access_token,
        refresh_token: body
            .get("refresh_token")
            .and_then(serde_json::Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .map(ToString::to_string),
        expires_in: body.get("expires_in").and_then(serde_json::Value::as_i64),
    })
}

async fn exchange_openai_oauth_code(
    code: &str,
    pending: &crate::mock::store::PendingOAuth,
) -> anyhow::Result<OAuthTokenResponse> {
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
    let tokens: OAuthTokenResponse = serde_json::from_value(body)?;
    if tokens.access_token.trim().is_empty() {
        return Err(anyhow::anyhow!(
            "OpenAI token response did not include access_token"
        ));
    }
    if tokens
        .refresh_token
        .as_deref()
        .unwrap_or_default()
        .trim()
        .is_empty()
    {
        return Err(anyhow::anyhow!(
            "OpenAI token response did not include refresh_token"
        ));
    }
    Ok(tokens)
}

async fn exchange_anthropic_oauth_code(
    normalized_code: &NormalizedOAuthCode,
    pending: &crate::mock::store::PendingOAuth,
) -> anyhow::Result<OAuthTokenResponse> {
    let code_verifier = pending
        .code_verifier
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("missing PKCE code verifier"))?;
    let client_id = anthropic_oauth_client_id();
    let redirect_uri = anthropic_oauth_redirect_uri();
    let state = normalized_code
        .state
        .as_deref()
        .or(pending.state.as_deref());
    let mut form = vec![
        ("grant_type", "authorization_code"),
        ("client_id", client_id.as_str()),
        ("redirect_uri", redirect_uri.as_str()),
        ("code", normalized_code.code.as_str()),
        ("code_verifier", code_verifier),
    ];
    if let Some(state) = state {
        form.push(("state", state));
    }
    let response = reqwest::Client::builder()
        .timeout(Duration::from_secs(120))
        .build()?
        .post(anthropic_oauth_token_url())
        .header("content-type", "application/x-www-form-urlencoded")
        .form(&form)
        .send()
        .await?;
    let status = response.status();
    let body: serde_json::Value = response.json().await?;
    if !status.is_success() {
        return Err(anyhow::anyhow!(
            "Anthropic token endpoint returned {status}: {body}"
        ));
    }
    let tokens: OAuthTokenResponse = serde_json::from_value(body)?;
    if tokens.access_token.trim().is_empty() {
        return Err(anyhow::anyhow!(
            "Anthropic token response did not include access_token"
        ));
    }
    if tokens
        .refresh_token
        .as_deref()
        .unwrap_or_default()
        .trim()
        .is_empty()
    {
        return Err(anyhow::anyhow!(
            "Anthropic token response did not include refresh_token"
        ));
    }
    Ok(tokens)
}

async fn exchange_google_oauth_code(
    provider_id: &str,
    code: &str,
    pending: &crate::mock::store::PendingOAuth,
) -> anyhow::Result<OAuthTokenResponse> {
    let code_verifier = pending
        .code_verifier
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("missing PKCE code verifier"))?;
    let client_id = google_oauth_client_id(provider_id)
        .ok_or_else(|| anyhow::anyhow!("missing Google OAuth client id"))?;
    let redirect_uri = provider_google_oauth_redirect_uri(provider_id);
    let mut form = vec![
        ("grant_type".to_string(), "authorization_code".to_string()),
        ("client_id".to_string(), client_id),
        ("redirect_uri".to_string(), redirect_uri),
        ("code".to_string(), code.to_string()),
        ("code_verifier".to_string(), code_verifier.to_string()),
    ];
    if let Some(client_secret) = google_oauth_client_secret(provider_id) {
        form.push(("client_secret".to_string(), client_secret));
    }
    let response = reqwest::Client::new()
        .post(google_oauth_token_url())
        .form(&form)
        .send()
        .await?;
    let status = response.status();
    let body: serde_json::Value = response.json().await?;
    if !status.is_success() {
        return Err(anyhow::anyhow!(
            "Google token endpoint returned {status}: {body}"
        ));
    }
    let tokens: OAuthTokenResponse = serde_json::from_value(body)?;
    if tokens.access_token.trim().is_empty() {
        return Err(anyhow::anyhow!(
            "Google token response did not include access_token"
        ));
    }
    Ok(tokens)
}

fn extract_account_id(tokens: &OAuthTokenResponse) -> Option<String> {
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
    let api_key = auth
        .metadata
        .as_ref()
        .and_then(|metadata| metadata.get("token_env"))
        .and_then(|value| value.as_str())
        .filter(|value| !value.trim().is_empty())
        .map(|value| value.trim().to_string())
        .unwrap_or_else(|| provider_env_key(provider_id));
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
    let configured = token_env.as_deref().and_then(config_value).is_some()
        || provider_key_exists(provider_id)
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
    let path = config::provider_config_path();
    let content = fs::read_to_string(path).ok()?;
    let root: tura_llm_rust::RootConfig = serde_json::from_str(&content).ok()?;
    root.provider_auth.get(provider_id).cloned()
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
        "claude-code" => current_login == Some("oauth"),
        "anthropic" => !matches!(current_login, Some("oauth" | "browser")),
        "antigravity" => current_login == Some("oauth"),
        "github-copilot" => matches!(current_login, Some("device" | "oauth" | "api")),
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
    let path = config::provider_config_path();
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

fn upsert_provider_auth_config(
    provider_id: &str,
    login: &str,
    login_env: Option<&str>,
    auth: &ProviderAuth,
    auth_url: Option<String>,
) -> io::Result<()> {
    let path = config::provider_config_path();
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
    if provider_id == "codex" && login == "oauth" {
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
    if provider_id == "github-copilot" {
        return ["COPILOT_GITHUB_TOKEN", "GH_TOKEN", "GITHUB_TOKEN"]
            .into_iter()
            .any(|key| provider_key_value_for_env(provider_id, key).is_some());
    }
    let key = provider_env_key(provider_id);
    provider_key_value_for_env(provider_id, &key).is_some()
}

fn provider_key_value_for_env(provider_id: &str, key: &str) -> Option<String> {
    let value = std::env::var(key)
        .ok()
        .or_else(|| tura_llm_rust::TuraConfig::default().get(key))
        .filter(|value| !value.trim().is_empty())
        .map(|value| value.trim().to_string())?;
    if matches!(
        provider_id,
        "codex"
            | "openai"
            | "openai-api"
            | "claude-code"
            | "anthropic"
            | "anthropic-api"
            | "antigravity"
            | "antigravity-api"
            | "github-copilot"
    ) {
        let login_key = provider_login_key(provider_id);
        let login = std::env::var(&login_key)
            .ok()
            .or_else(|| tura_llm_rust::TuraConfig::default().get(&login_key));
        let login_matches = match provider_id {
            "codex" => login.as_deref() == Some("oauth"),
            "openai" => !matches!(login.as_deref(), Some("oauth" | "browser")),
            "claude-code" => login.as_deref() == Some("oauth"),
            "anthropic" => !matches!(login.as_deref(), Some("oauth" | "browser")),
            "antigravity" => login.as_deref() == Some("oauth"),
            "github-copilot" => matches!(login.as_deref(), Some("device" | "oauth" | "api")),
            "openai-api" | "anthropic-api" | "antigravity-api" => {
                !matches!(login.as_deref(), Some("oauth" | "browser"))
            }
            _ => true,
        };
        if !login_matches {
            return None;
        }
    }
    Some(value)
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
        let codex = provider_auth_methods("codex");
        assert_eq!(codex.len(), 1);
        assert_eq!(codex[0].kind, AuthMethodKind::OAuthPkce);
        assert_eq!(codex[0].method_type, "oauth");
        assert_eq!(codex[0].token_env.as_deref(), Some("OPENAI_API_KEY"));

        let openai = provider_auth_methods("openai");
        assert_eq!(openai[0].kind, AuthMethodKind::ApiKey);
        assert_eq!(openai[0].method_type, "api");

        let anthropic = provider_auth_methods("anthropic");
        assert_eq!(anthropic[0].kind, AuthMethodKind::ApiKey);
        assert_eq!(anthropic[0].method_type, "api");
        assert_eq!(anthropic[0].login, "api");

        let claude_code = provider_auth_methods("claude-code");
        assert_eq!(claude_code[0].kind, AuthMethodKind::LocalCliToken);
        assert_eq!(claude_code[0].method_type, "oauth");
        assert_eq!(
            claude_code[0].token_env.as_deref(),
            Some("CLAUDE_CODE_OAUTH_TOKEN")
        );

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

    #[tokio::test]
    async fn provider_auth_method_value_is_available_for_hover_reveal() {
        let _guard = ENV_LOCK.lock().await;
        clear_openai_refresh_test_env();
        set_env("OPENAI_LOGIN", "oauth");
        set_env("OPENAI_API_KEY", "sk-test-hover-reveal");

        let openai = provider_auth_methods("openai");
        assert_eq!(
            openai[0].configured_value.as_deref(),
            Some("sk-test-hover-reveal")
        );

        let openai_api = provider_auth_methods("openai-api");
        assert_eq!(
            openai_api[0].configured_value.as_deref(),
            Some("sk-test-hover-reveal")
        );

        clear_openai_refresh_test_env();
    }

    #[tokio::test]
    async fn non_llm_openapi_catalog_provider_does_not_use_llm_models_validator() {
        let provider = tura_llm_rust::ProviderCatalogConfig {
            api_style: "openapi".to_string(),
            base_url: "https://example.com/v1".to_string(),
            domains: vec!["infrastructure".to_string()],
            ..Default::default()
        };

        let validation = validate_provider_credentials_remotely(
            "example_infrastructure",
            Some(&provider),
            Some(&provider.base_url),
            Some("fake-token"),
        )
        .await;

        assert!(matches!(
            validation,
            ProviderCredentialValidation::Unsupported(detail)
                if detail.code == "provider.validation.gateway_not_configured"
        ));
    }

    #[tokio::test]
    async fn provider_list_projects_non_llm_catalog_entries() {
        let _guard = ENV_LOCK.lock().await;
        let settings = tura_llm_rust::Settings::default()
            .await
            .expect("load settings");
        let route = settings
            .route_by_name("fast")
            .expect("fast route should be configured");
        let response = provider_list_for_route(settings.as_ref(), route);
        let feishu = response
            .all
            .iter()
            .find(|provider| provider.id == "feishu")
            .expect("Feishu service provider should be listed");
        assert!(response
            .enums
            .domains
            .iter()
            .any(|domain| domain == "communication"));
        assert!(response.enums.api_styles.iter().any(|style| style == "mcp"));
        assert!(feishu.models.is_empty());
        assert_eq!(
            feishu.api.as_deref(),
            Some("https://open.feishu.cn/open-apis")
        );
        assert_eq!(
            feishu
                .options
                .get("domains")
                .and_then(|value| value.as_array())
                .and_then(|items| items.first())
                .and_then(|value| value.as_str()),
            Some("productivity")
        );

        let line = response
            .all
            .iter()
            .find(|provider| provider.id == "line")
            .expect("LINE service provider should be listed");
        assert!(line.models.is_empty());
        assert_eq!(line.api.as_deref(), Some("https://api.line.me/v2/bot"));
        assert!(line
            .env
            .iter()
            .any(|env| env == "LINE_CHANNEL_ACCESS_TOKEN"));
        assert_eq!(
            line.options
                .get("auth_methods")
                .and_then(|value| value.as_array())
                .and_then(|items| items.first())
                .and_then(|value| value.as_str()),
            Some("channel_access_token")
        );

        let duckduckgo = response
            .all
            .iter()
            .find(|provider| provider.id == "duckduckgo_search")
            .expect("DuckDuckGo search provider should be listed");
        assert!(duckduckgo.models.is_empty());
        assert_eq!(
            duckduckgo.api.as_deref(),
            Some("https://html.duckduckgo.com/html/")
        );
        assert!(duckduckgo
            .env
            .iter()
            .any(|env| env == "TURA_DUCKDUCKGO_SEARCH_ENDPOINT"));
        assert_eq!(
            duckduckgo
                .options
                .get("api_style")
                .and_then(|value| value.as_str()),
            Some("duckduckgo_html")
        );

        let exa = response
            .all
            .iter()
            .find(|provider| provider.id == "exa_search")
            .expect("Exa search provider should be listed");
        assert!(exa.models.is_empty());
        assert_eq!(exa.api.as_deref(), Some("https://mcp.exa.ai/mcp"));
        assert!(exa.env.iter().any(|env| env == "TURA_EXA_MCP_ENDPOINT"));
        assert_eq!(
            exa.options
                .get("api_style")
                .and_then(|value| value.as_str()),
            Some("mcp")
        );
    }

    #[tokio::test]
    async fn provider_list_returns_configured_key_value() {
        let _guard = ENV_LOCK.lock().await;
        std::env::set_var("LINE_CHANNEL_ACCESS_TOKEN", "line-test-token");

        let settings = tura_llm_rust::Settings::default()
            .await
            .expect("load settings");
        let route = settings
            .route_by_name("fast")
            .expect("fast route should be configured");
        let response = provider_list_for_route(settings.as_ref(), route);
        let line = response
            .all
            .iter()
            .find(|provider| provider.id == "line")
            .expect("LINE service provider should be listed");
        assert_eq!(line.key.as_deref(), Some("line-test-token"));

        std::env::remove_var("LINE_CHANNEL_ACCESS_TOKEN");
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
        assert_eq!(login_value_for_auth("anthropic", &auth), "api");

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
        let config_path = temp_dir.join("provider_config.json");
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

        let Json(response) = provider_auth_refresh(Path("codex".to_string())).await;

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
        let config_path = temp_dir.join("provider_config.json");
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

        let Json(status) = provider_auth_status(Path("codex".to_string())).await;

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
                refresh_env: "GOOGLE_REFRESH_TOKEN",
                expires_env: "GOOGLE_TOKEN_EXPIRES",
                old_access: "google-expired-access",
                new_access: "google-new-access",
            },
            OAuthRefreshCase {
                provider_id: "gemini",
                login_env: "GEMINI_LOGIN",
                token_env: "GEMINI_API_KEY",
                refresh_env: "GOOGLE_REFRESH_TOKEN",
                expires_env: "GOOGLE_TOKEN_EXPIRES",
                old_access: "gemini-expired-access",
                new_access: "gemini-new-access",
            },
            OAuthRefreshCase {
                provider_id: "antigravity",
                login_env: "ANTIGRAVITY_LOGIN",
                token_env: "ANTIGRAVITY_API_KEY",
                refresh_env: "ANTIGRAVITY_REFRESH_TOKEN",
                expires_env: "ANTIGRAVITY_TOKEN_EXPIRES",
                old_access: "antigravity-expired-access",
                new_access: "antigravity-new-access",
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
            let config_path = temp_dir.join("provider_config.json");
            std::fs::write(
                &env_path,
                format!(
                    "{login_env}=oauth\n{token_env}={old_access}\n{refresh_env}={refresh}\n{expires_env}=0\n",
                    login_env = case.login_env,
                    token_env = case.token_env,
                    refresh_env = case.refresh_env,
                    expires_env = case.expires_env,
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
            set_env(case.refresh_env, case.refresh_token());
            set_env(case.expires_env, "0");

            let Json(response) = provider_auth_refresh(Path(case.provider_id.to_string())).await;

            assert!(response.ok, "{}", response.message);
            assert_eq!(
                std::env::var(case.token_env).as_deref(),
                Ok(case.new_access)
            );
            assert_eq!(
                std::env::var(case.refresh_env).as_deref(),
                Ok(case.refresh_token())
            );
            assert!(std::env::var(case.expires_env)
                .ok()
                .and_then(|value| value.parse::<i64>().ok())
                .is_some_and(|expires| expires > Utc::now().timestamp_millis()));
            let config = std::fs::read_to_string(&config_path).expect("read config");
            assert!(config.contains(case.provider_id));
            assert!(config.contains(case.refresh_env));
            server.join().expect("token server should finish");

            clear_openai_refresh_test_env();
            let _ = std::fs::remove_dir_all(&temp_dir);
        }
    }

    #[tokio::test]
    async fn provider_auth_refresh_covers_claude_code_oauth_method() {
        let _guard = ENV_LOCK.lock().await;
        clear_openai_refresh_test_env();
        let temp_dir = std::env::temp_dir().join(format!(
            "tura-claude-code-oauth-refresh-test-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).expect("temp dir");
        let env_path = temp_dir.join(".env");
        let config_path = temp_dir.join("provider_config.json");
        std::fs::write(
            &env_path,
            "ANTHROPIC_LOGIN=oauth\nCLAUDE_CODE_OAUTH_TOKEN=claude-old-access\nCLAUDE_CODE_REFRESH_TOKEN=claude-refresh-token\nCLAUDE_CODE_TOKEN_EXPIRES=0\n",
        )
        .expect("env");
        std::fs::write(&config_path, r#"{"provider_auth":{}}"#).expect("config");

        let (addr, server) = spawn_openai_token_server(
            "claude-refresh-token",
            r#"{"access_token":"claude-new-access","refresh_token":"claude-new-refresh","expires_in":3600}"#,
        );

        set_env("TURA_ENV_PATH", env_path.to_string_lossy().as_ref());
        set_env("TURALLM_CONFIG", config_path.to_string_lossy().as_ref());
        set_env(
            "ANTHROPIC_OAUTH_TOKEN_URL",
            &format!("http://{addr}/oauth/token"),
        );
        set_env("ANTHROPIC_LOGIN", "oauth");
        set_env("CLAUDE_CODE_OAUTH_TOKEN", "claude-old-access");
        set_env("CLAUDE_CODE_REFRESH_TOKEN", "claude-refresh-token");
        set_env("CLAUDE_CODE_TOKEN_EXPIRES", "0");

        let Json(response) = provider_auth_refresh(Path("claude-code".to_string())).await;

        assert!(response.ok, "{}", response.message);
        assert_eq!(
            std::env::var("CLAUDE_CODE_OAUTH_TOKEN").as_deref(),
            Ok("claude-new-access")
        );
        assert_eq!(
            std::env::var("CLAUDE_CODE_REFRESH_TOKEN").as_deref(),
            Ok("claude-new-refresh")
        );
        assert!(std::env::var("CLAUDE_CODE_TOKEN_EXPIRES")
            .ok()
            .and_then(|value| value.parse::<i64>().ok())
            .is_some_and(|expires| expires > Utc::now().timestamp_millis()));
        let config = std::fs::read_to_string(&config_path).expect("read config");
        assert!(config.contains("claude-code"));
        assert!(config.contains("CLAUDE_CODE_REFRESH_TOKEN"));
        server.join().expect("token server should finish");

        clear_openai_refresh_test_env();
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    struct OAuthRefreshCase {
        provider_id: &'static str,
        login_env: &'static str,
        token_env: &'static str,
        refresh_env: &'static str,
        expires_env: &'static str,
        old_access: &'static str,
        new_access: &'static str,
    }

    impl OAuthRefreshCase {
        fn refresh_token(&self) -> &'static str {
            match self.provider_id {
                "google" => "google-refresh-token",
                "gemini" => "gemini-refresh-token",
                "antigravity" => "antigravity-refresh-token",
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
            "TURA_PROVIDER_CONFIG",
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
            "ANTIGRAVITY_LOGIN",
            "ANTIGRAVITY_API_KEY",
            "ANTIGRAVITY_REFRESH_TOKEN",
            "ANTIGRAVITY_TOKEN_EXPIRES",
            "ANTIGRAVITY_OAUTH_CLIENT_ID",
            "ANTIGRAVITY_OAUTH_CLIENT_SECRET",
            "ANTIGRAVITY_OAUTH_REDIRECT_URI",
            "ANTIGRAVITY_OAUTH_SCOPE",
            "ANTHROPIC_OAUTH_TOKEN_URL",
            "ANTHROPIC_LOGIN",
            "CLAUDE_CODE_OAUTH_TOKEN",
            "CLAUDE_CODE_REFRESH_TOKEN",
            "CLAUDE_CODE_TOKEN_EXPIRES",
        ] {
            std::env::remove_var(key);
        }
    }

    fn set_env(key: &str, value: &str) {
        std::env::set_var(key, value);
    }
}
