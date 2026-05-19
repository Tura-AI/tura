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
use std::path::Path as FsPath;
use tokio::time::{sleep, timeout, Duration, Instant};
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
    match provider_id {
        "antigravity" => "Antigravity Oauth",
        "antigravity-api" => "Antigravity API",
        "minimax" => "MiniMax",
        "openrouter" => "OpenRouter",
        "openai" => "OpenAI Codex",
        "openai-api" => "OpenAI API",
        "anthropic" => "Claude Oauth",
        "anthropic-api" => "Anthropic API",
        "google" => "Google",
        "xai" => "xAI",
        other => other,
    }
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

fn provider_model_catalog() -> [(&'static str, &'static [&'static str]); 9] {
    [
        (
            "openai",
            &[
                "gpt-5.5",
                "gpt-5.4",
                "gpt-5.4-mini",
                "gpt-5.3-codex",
                "gpt-5.3-codex-spark",
                "gpt-5.2",
            ],
        ),
        (
            "openai-api",
            &[
                "gpt-5.5",
                "gpt-5.4",
                "gpt-5.4-mini",
                "gpt-5.2",
                "gpt-4.1",
                "gpt-4.1-mini",
                "o4-mini",
            ],
        ),
        ("anthropic", &["claude-sonnet-4.5", "claude-opus-4.6"]),
        (
            "anthropic-api",
            &["claude-sonnet-4.5", "claude-opus-4.6", "claude-haiku-4.5"],
        ),
        ("antigravity", &["antigravity-browser"]),
        ("antigravity-api", &["gemini-3-pro", "gemini-3-flash"]),
        ("minimax", &["minimax-m2.7", "minimax-m2.5", "minimax-m2.1"]),
        (
            "openrouter",
            &[
                "minimax/minimax-m2.7",
                "minimax/minimax-m2.5",
                "anthropic/claude-opus-4.6",
                "anthropic/claude-sonnet-4.5",
                "openai/gpt-5.4",
                "openai/gpt-5.3-codex",
                "google/gemini-3-pro",
                "google/gemini-3-flash",
            ],
        ),
        (
            "google",
            &["gemini-3-pro", "gemini-3-flash", "gemini-2.5-pro"],
        ),
    ]
}

fn model_supported_by_provider(provider_id: &str, model_id: &str) -> bool {
    provider_model_catalog()
        .into_iter()
        .find(|(id, _)| *id == provider_id)
        .map(|(_, models)| models.iter().any(|candidate| candidate == &model_id))
        .unwrap_or(false)
}

fn provider_runtime_id(provider_id: &str) -> &str {
    match provider_id {
        "openai-api" => "openai",
        "anthropic-api" => "anthropic",
        "antigravity-api" => "antigravity",
        other => other,
    }
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
    let saved = persist_provider_auth(&provider_id, &payload).is_ok();
    Json(saved && global_store().set_auth(&provider_id, payload))
}

pub async fn remove_auth(Path(provider_id): Path<String>) -> Json<bool> {
    Json(global_store().remove_auth(&provider_id))
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
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompts: Option<Vec<serde_json::Value>>,
}

pub async fn provider_auth(
    Query(_params): Query<ProviderAuthQuery>,
) -> Json<HashMap<String, Vec<ProviderAuthMethod>>> {
    let mut response = HashMap::new();
    for provider in global_store().list_providers() {
        let methods = provider_auth_methods(&provider.id);
        if !methods.is_empty() {
            response.insert(provider.id.clone(), methods);
        }
    }
    Json(response)
}

fn provider_auth_methods(provider_id: &str) -> Vec<ProviderAuthMethod> {
    match provider_id {
        "openai" => vec![ProviderAuthMethod {
            method_type: "oauth".to_string(),
            label: "ChatGPT Pro/Plus (browser)".to_string(),
            prompts: None,
        }],
        "openai-api" => vec![ProviderAuthMethod {
            method_type: "api".to_string(),
            label: "API Key".to_string(),
            prompts: None,
        }],
        "antigravity" => vec![ProviderAuthMethod {
            method_type: "oauth".to_string(),
            label: "Antigravity Browser Token".to_string(),
            prompts: None,
        }],
        "antigravity-api" => vec![ProviderAuthMethod {
            method_type: "api".to_string(),
            label: "API Key".to_string(),
            prompts: None,
        }],
        "anthropic" => vec![ProviderAuthMethod {
            method_type: "oauth".to_string(),
            label: "Claude Browser Token".to_string(),
            prompts: None,
        }],
        "anthropic-api" => vec![ProviderAuthMethod {
            method_type: "api".to_string(),
            label: "API Key".to_string(),
            prompts: None,
        }],
        _ => vec![ProviderAuthMethod {
            method_type: "api".to_string(),
            label: "API Key".to_string(),
            prompts: None,
        }],
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
    let method = methods.get(payload.method).and_then(|method| {
        if method.method_type == "oauth" {
            Some(OAuthMethod::Auto)
        } else {
            None
        }
    });

    if method.is_none() {
        return Json(OAuthAuthorizeResponse {
            url: String::new(),
            method: OAuthMethod::Code,
            instructions: "Invalid auth method".to_string(),
        });
    }

    let (url, method, instructions) = if provider_id == "openai" {
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
    } else if matches!(provider_id.as_str(), "antigravity" | "anthropic") {
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
    } else {
        let url = format!("https://auth.example.com/oauth/{provider_id}");
        let code = random_confirmation_code(&provider_id, payload.method);
        global_store().set_oauth_state(
            &provider_id,
            "code".to_string(),
            Some(code),
            url.clone(),
            Some(oauth_state()),
            None,
        );
        let instructions = format!("Open {url} in your browser and complete authentication");
        (url, OAuthMethod::Code, instructions)
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

    let pending = has_pending.unwrap();
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
        .post("https://auth.openai.com/oauth/token")
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

    if auth.auth_type == "api"
        && matches!(
            provider_id,
            "openai-api" | "anthropic-api" | "antigravity-api"
        )
    {
        let login_key = provider_login_key(provider_id);
        upsert_env_value(&env_path, &login_key, "api")?;
        std::env::set_var(&login_key, "api");
    }

    if auth.auth_type == "oauth" {
        let login = auth
            .metadata
            .as_ref()
            .and_then(|metadata| metadata.get("login"))
            .and_then(|value| value.as_str())
            .unwrap_or("oauth");
        let login_key = provider_login_key(provider_id);
        upsert_env_value(&env_path, &login_key, login)?;
        std::env::set_var(&login_key, login);

        if provider_id == "openai" {
            if let Some(refresh) = auth
                .refresh
                .as_deref()
                .filter(|value| !value.trim().is_empty())
            {
                let refresh_key = "OPENAI_REFRESH_TOKEN";
                upsert_env_value(&env_path, refresh_key, refresh)?;
                std::env::set_var(refresh_key, refresh);
            }
            if let Some(expires) = auth.expires {
                let expires_key = "OPENAI_TOKEN_EXPIRES";
                let expires_value = expires.to_string();
                upsert_env_value(&env_path, expires_key, &expires_value)?;
                std::env::set_var(expires_key, expires_value);
            }
            if let Some(account_id) = auth
                .account_id
                .as_deref()
                .filter(|value| !value.trim().is_empty())
            {
                let account_key = "OPENAI_ACCOUNT_ID";
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

    let mut entry = serde_json::json!({
        "type": "oauth",
        "login": login,
        "status": "connected",
        "provider": provider_runtime_id(provider_id),
        "auth_url": auth_url,
        "token_env": provider_env_key(provider_id),
        "login_env": login_env,
        "updated_at": Utc::now().to_rfc3339(),
    });
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
    match provider_id {
        "openai-api" => return "OPENAI_API_KEY".to_string(),
        "anthropic-api" => return "ANTHROPIC_API_KEY".to_string(),
        "antigravity-api" => return "ANTIGRAVITY_API_KEY".to_string(),
        _ => {}
    }
    format!("{}_API_KEY", provider_id.to_ascii_uppercase())
}

fn provider_login_key(provider_id: &str) -> String {
    match provider_id {
        "openai-api" => return "OPENAI_LOGIN".to_string(),
        "anthropic-api" => return "ANTHROPIC_LOGIN".to_string(),
        "antigravity-api" => return "ANTIGRAVITY_LOGIN".to_string(),
        _ => {}
    }
    format!("{}_LOGIN", provider_id.to_ascii_uppercase())
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
