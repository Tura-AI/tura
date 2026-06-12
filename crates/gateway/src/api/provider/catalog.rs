use std::collections::HashMap;

use axum::extract::Json;
use tokio::time::{timeout, Duration};

use crate::api::types::*;
use crate::mock::global_store;

use super::{provider_env_key, provider_key_exists, provider_key_value_for_env};

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
        if model_visible_for_picker(settings.as_deref(), &provider.id, &default_model.id) {
            models.insert(default_model.id.clone(), default_model.clone());
        }

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

pub(super) fn provider_list_for_route(
    settings: &tura_llm_rust::Settings,
    route: &tura_llm_rust::RouteConfig,
) -> ProviderListResponse {
    let mut all = Vec::<SdkProvider>::new();
    let mut indexes = HashMap::<String, usize>::new();
    let mut defaults = HashMap::<String, String>::new();
    let mut connected = Vec::<String>::new();

    for provider in &route.providers {
        let model_id = normalize_model_id(&provider.provider, &provider.model);
        if !model_visible_for_picker(Some(settings), &provider.provider, &model_id) {
            continue;
        }
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

    for (provider_id, models) in provider_model_catalog(Some(settings)) {
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
        let model_ids = model_ids
            .into_iter()
            .filter(|model_id| model_visible_for_picker(Some(settings), &provider_id, model_id))
            .collect::<Vec<_>>();
        if model_ids.is_empty() {
            continue;
        }
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
        if !model_visible_for_picker(Some(settings), provider_id, model_id) {
            continue;
        }
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

pub(super) fn active_agent_route(
    settings: &tura_llm_rust::Settings,
) -> Option<&tura_llm_rust::RouteConfig> {
    route_by_name(settings, &active_agent_route_name())
}

pub(super) fn active_agent_route_name() -> String {
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

pub(super) fn route_by_name<'a>(
    settings: &'a tura_llm_rust::Settings,
    name: &str,
) -> Option<&'a tura_llm_rust::RouteConfig> {
    settings.route_by_name(name)
}

pub(super) fn provider_display_name(provider_id: &str) -> String {
    tura_llm_rust::provider_auth_registry_entry(provider_id)
        .map(|entry| entry.display_name)
        .unwrap_or(provider_id)
        .to_string()
}

pub(super) fn configured_model_for_provider(
    settings: Option<&tura_llm_rust::Settings>,
    provider_id: &str,
) -> Option<SdkProviderModel> {
    let settings = settings?;
    let route = settings
        .routes()
        .flat_map(|route| route.providers.iter())
        .find(|provider| {
            provider.provider == provider_id
                && model_visible_for_picker(
                    Some(settings),
                    provider_id,
                    &normalize_model_id(provider_id, &provider.model),
                )
        })?;

    Some(sdk_model_from_settings(
        Some(settings),
        provider_id,
        &normalize_model_id(provider_id, &route.model),
    ))
}

pub(super) fn configured_models_for_provider(
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
        .filter(|model_id| model_visible_for_picker(Some(settings), provider_id, model_id))
        .map(|model_id| sdk_model_from_settings(Some(settings), provider_id, &model_id))
        .collect()
}

pub(super) fn configured_model_catalog(
    settings: &tura_llm_rust::Settings,
) -> HashMap<String, Vec<String>> {
    settings.configured_model_catalog()
}

pub(super) fn provider_display_name_from_settings(
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

pub(super) fn provider_env_from_settings(
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

pub(super) fn provider_api_from_settings(
    settings: Option<&tura_llm_rust::Settings>,
    provider_id: &str,
) -> Option<String> {
    settings
        .and_then(|settings| settings.model_catalog.providers.get(provider_id))
        .and_then(|provider| {
            (!provider.base_url.trim().is_empty()).then(|| provider.base_url.clone())
        })
}

pub(super) fn provider_options_from_settings(
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

pub(super) fn insert_option(
    options: &mut HashMap<String, serde_json::Value>,
    key: &str,
    value: &str,
) {
    if !value.trim().is_empty() {
        options.insert(
            key.to_string(),
            serde_json::Value::String(value.to_string()),
        );
    }
}

pub(super) fn normalize_model_id(provider_id: &str, model_id: &str) -> String {
    let prefix = format!("{}/", provider_runtime_id(provider_id));
    model_id
        .strip_prefix(&prefix)
        .unwrap_or(model_id)
        .to_string()
}

pub(super) fn default_model_for_provider(provider_id: &str) -> SdkProviderModel {
    sdk_model_from_config(provider_id, "default")
}

pub(super) fn sdk_model_from_config(provider_id: &str, model_id: &str) -> SdkProviderModel {
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

pub(super) fn sdk_model_from_settings(
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

pub(super) fn catalog_model_detail<'a>(
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

pub(super) fn apply_catalog_model_detail(
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

pub(super) fn browser_login_provider_defaults() -> [(&'static str, &'static str); 4] {
    [
        ("codex", "gpt-5.1-codex"),
        ("anthropic", "claude-sonnet-4-20250514"),
        ("antigravity", "antigravity-browser"),
        ("github-copilot", "copilot-chat"),
    ]
}

pub(super) fn provider_model_catalog(
    settings: Option<&tura_llm_rust::Settings>,
) -> Vec<(String, Vec<String>)> {
    tura_llm_rust::provider_auth_registry()
        .iter()
        .filter(|entry| !entry.supported_models.is_empty())
        .filter_map(|entry| {
            let models = entry
                .supported_models
                .iter()
                .filter(|model| model_visible_for_picker(settings, entry.provider_id, model))
                .map(|model| model.to_string())
                .collect::<Vec<_>>();
            if models.is_empty() {
                return None;
            }
            Some((entry.provider_id.to_string(), models))
        })
        .collect()
}

pub(super) fn model_visible_for_picker(
    settings: Option<&tura_llm_rust::Settings>,
    provider_id: &str,
    model_id: &str,
) -> bool {
    if looks_like_claude_model(provider_id, model_id) {
        return false;
    }
    let Some(settings) = settings else {
        return true;
    };
    catalog_model_detail(settings, provider_id, model_id).is_none_or(|detail| detail.visible)
}

pub(super) fn looks_like_claude_model(provider_id: &str, model_id: &str) -> bool {
    let provider_id = provider_id.trim().to_ascii_lowercase();
    let model_id = model_id.trim().to_ascii_lowercase();
    provider_id == "claude-code"
        || model_id == "claude"
        || model_id.starts_with("claude-")
        || model_id.starts_with("anthropic.claude-")
        || model_id.contains("/claude-")
}

pub(super) fn model_supported_by_provider(provider_id: &str, model_id: &str) -> bool {
    tura_llm_rust::provider_auth_registry()
        .iter()
        .find(|entry| entry.provider_id == provider_id)
        .map(|entry| {
            entry
                .supported_models
                .iter()
                .any(|candidate| candidate == &model_id)
        })
        .unwrap_or(false)
}

pub(super) fn provider_runtime_id(provider_id: &str) -> &str {
    tura_llm_rust::runtime_provider_id(provider_id)
}

pub(super) fn providers_enabled_set() -> std::collections::HashSet<String> {
    global_store()
        .list_providers()
        .into_iter()
        .filter(|provider| provider.enabled)
        .map(|provider| provider.id)
        .collect()
}

pub(super) fn enrich_provider_list(
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

pub(super) fn provider_base_url(
    settings: &tura_llm_rust::Settings,
    provider_id: &str,
) -> Option<String> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn provider_config(provider: &str, model: &str) -> tura_llm_rust::ProviderConfig {
        tura_llm_rust::ProviderConfig {
            provider: provider.to_string(),
            base_url: format!("https://{provider}.example/v1"),
            model: tura_llm_rust::Settings::normalize_model_name(provider, model),
            temperature: 0.2,
        }
    }

    fn catalog_settings() -> tura_llm_rust::Settings {
        let visible_detail = tura_llm_rust::CatalogModelDetail {
            id: "local/model-a".to_string(),
            visible: true,
            name: "Model A".to_string(),
            family: "local-family".to_string(),
            release_date: "2026-06-12".to_string(),
            attachment: true,
            reasoning: false,
            temperature: true,
            tool_call: false,
            limit: tura_llm_rust::CatalogModelLimit {
                context: 32_000,
                input: 16_000,
                output: 4_000,
            },
            modalities: tura_llm_rust::CatalogModelModalities {
                input: vec!["text".to_string()],
                output: vec!["text".to_string(), "json".to_string()],
            },
            options: serde_json::Map::from_iter([(
                "tier".to_string(),
                serde_json::json!("business"),
            )]),
            status: Some("stable".to_string()),
        };
        let hidden_detail = tura_llm_rust::CatalogModelDetail {
            id: "hidden-model".to_string(),
            visible: false,
            name: "Hidden".to_string(),
            ..Default::default()
        };
        tura_llm_rust::Settings {
            provider_base_url: HashMap::from([(
                "local".to_string(),
                "https://local.example/v1".to_string(),
            )]),
            routes: HashMap::from([(
                "coding".to_string(),
                tura_llm_rust::RouteConfig {
                    default_temperature: 0.2,
                    providers: vec![
                        provider_config("local", "local/model-a"),
                        provider_config("local", "hidden-model"),
                        provider_config("openai", "openai/gpt-visible"),
                    ],
                },
            )]),
            model_catalog: tura_llm_rust::ModelCatalog {
                tiers: vec!["fast".to_string()],
                providers: HashMap::from([(
                    "local".to_string(),
                    tura_llm_rust::ProviderCatalogConfig {
                        display_name: "Local Provider".to_string(),
                        runtime_provider: "openai".to_string(),
                        api_style: "openai_compatible".to_string(),
                        base_url: "https://local.example/v1".to_string(),
                        token_env: Some("LOCAL_TOKEN".to_string()),
                        env: vec!["LOCAL_TOKEN".to_string(), "LOCAL_FALLBACK".to_string()],
                        domains: vec!["llm".to_string(), "local".to_string()],
                        capabilities: vec!["llm.chat".to_string()],
                        auth_methods: vec!["api_key".to_string()],
                        api_docs: Some("https://local.example/docs".to_string()),
                        status: Some("beta".to_string()),
                        models: HashMap::from([(
                            "fast".to_string(),
                            vec![
                                tura_llm_rust::CatalogModelConfig::Detailed(visible_detail),
                                tura_llm_rust::CatalogModelConfig::Detailed(hidden_detail),
                                tura_llm_rust::CatalogModelConfig::Id("claude-legacy".to_string()),
                            ],
                        )]),
                    },
                )]),
            },
            provider_enums: tura_llm_rust::ProviderEnumCatalog::default(),
        }
    }

    #[test]
    fn catalog_provider_metadata_helpers_project_settings_fields() {
        let settings = catalog_settings();

        assert_eq!(
            provider_display_name_from_settings(Some(&settings), "local").as_deref(),
            Some("Local Provider")
        );
        assert_eq!(
            provider_env_from_settings(Some(&settings), "local"),
            vec!["LOCAL_TOKEN".to_string(), "LOCAL_FALLBACK".to_string()]
        );
        assert_eq!(
            provider_api_from_settings(Some(&settings), "local").as_deref(),
            Some("https://local.example/v1")
        );

        let options = provider_options_from_settings(Some(&settings), "local");
        assert_eq!(options["api_style"], "openai_compatible");
        assert_eq!(options["runtime_provider"], "openai");
        assert_eq!(options["token_env"], "LOCAL_TOKEN");
        assert_eq!(options["domains"], serde_json::json!(["llm", "local"]));
        assert_eq!(options["capabilities"], serde_json::json!(["llm.chat"]));
        assert_eq!(options["auth_methods"], serde_json::json!(["api_key"]));
        assert_eq!(options["api_docs"], "https://local.example/docs");
        assert_eq!(options["status"], "beta");

        assert!(provider_display_name_from_settings(Some(&settings), "missing").is_none());
        assert!(provider_env_from_settings(None, "local").is_empty());
        assert!(provider_api_from_settings(Some(&settings), "missing").is_none());
        assert!(provider_options_from_settings(None, "local").is_empty());
    }

    #[test]
    fn catalog_model_detail_lookup_and_sdk_model_projection_are_canonical() {
        let settings = catalog_settings();

        assert_eq!(normalize_model_id("openai", "openai/model-a"), "model-a");
        assert_eq!(normalize_model_id("local", "local/model-a"), "model-a");
        assert_eq!(
            normalize_model_id("local", "openai/model-a"),
            "openai/model-a"
        );
        assert_eq!(normalize_model_id("local", "model-a"), "model-a");

        let detail = catalog_model_detail(&settings, "local", "model-a").expect("model detail");
        assert_eq!(detail.name, "Model A");
        assert_eq!(detail.status.as_deref(), Some("stable"));

        let model = sdk_model_from_settings(Some(&settings), "local", "model-a");
        assert_eq!(model.id, "model-a");
        assert_eq!(model.name, "Model A");
        assert_eq!(model.family, "local-family");
        assert_eq!(model.release_date, "2026-06-12");
        assert!(model.attachment);
        assert!(!model.reasoning);
        assert!(model.temperature);
        assert!(!model.tool_call);
        assert_eq!(model.limit.context, 32_000);
        assert_eq!(model.limit.input, 16_000);
        assert_eq!(model.limit.output, 4_000);
        assert_eq!(model.modalities.input, vec!["text"]);
        assert_eq!(model.modalities.output, vec!["text", "json"]);
        assert_eq!(model.options["tier"], "business");
        assert_eq!(model.status.as_deref(), Some("stable"));
    }

    #[test]
    fn catalog_visibility_rejects_hidden_and_claude_like_models() {
        let settings = catalog_settings();

        assert!(model_visible_for_picker(
            Some(&settings),
            "local",
            "model-a"
        ));
        assert!(!model_visible_for_picker(
            Some(&settings),
            "local",
            "hidden-model"
        ));
        for (provider, model) in [
            ("claude-code", "anything"),
            ("anthropic", "claude-3-opus"),
            ("bedrock", "anthropic.claude-3-5-sonnet"),
            ("custom", "vendor/claude-test"),
            ("custom", "claude"),
        ] {
            assert!(looks_like_claude_model(provider, model));
            assert!(!model_visible_for_picker(Some(&settings), provider, model));
        }
        assert!(!looks_like_claude_model("anthropic", "sonnet-compatible"));
    }

    #[test]
    fn provider_list_for_route_merges_route_catalog_and_configured_models() {
        let settings = catalog_settings();
        let route = settings.route_by_name("coding").expect("coding route");

        let response = provider_list_for_route(&settings, route);
        let local = response
            .all
            .iter()
            .find(|provider| provider.id == "local")
            .expect("local provider");

        assert_eq!(response.default["local"], "model-a");
        assert!(response
            .connected
            .iter()
            .any(|provider| provider == "local"));
        assert_eq!(local.name, "Local Provider");
        assert_eq!(local.env, vec!["LOCAL_TOKEN", "LOCAL_FALLBACK"]);
        assert_eq!(local.api.as_deref(), Some("https://local.example/v1"));
        assert!(local.models.contains_key("model-a"));
        assert!(!local.models.contains_key("hidden-model"));
        assert!(!local.models.contains_key("claude-legacy"));

        let openai = response
            .all
            .iter()
            .find(|provider| provider.id == "openai")
            .expect("route-only openai provider");
        assert!(openai.models.contains_key("gpt-visible"));
    }
}
