use crate::state_machine::agent_management::{AgentId, ProviderConfig};
use crate::state_machine::runtime_management::{
    RuntimeId, RuntimeManagement, RuntimeProviderConfig,
};
use crate::state_machine::session_management::SessionId;
use chrono::Utc;

use super::call_runtime::route_by_name;
use super::types::RuntimeQueueItem;

pub struct CreateRuntimeInput {
    pub session_id: SessionId,
    pub agent_id: AgentId,
    pub messages: Vec<serde_json::Value>,
    pub tools: Vec<serde_json::Value>,
    pub provider_config: ProviderConfig,
    pub tura_settings: std::sync::Arc<tura_llm_rust::Settings>,
    pub thinking: bool,
}

pub async fn create_runtime(
    input: CreateRuntimeInput,
) -> Result<(RuntimeManagement, RuntimeQueueItem), String> {
    let runtime_id = generate_runtime_id();
    let now = Utc::now();

    let runtime_provider_config = runtime_provider_config_from_tura(
        &input.provider_config,
        input.tura_settings.as_ref(),
        input.thinking,
    )?;

    let runtime = RuntimeManagement::new(
        runtime_id.clone(),
        input.session_id.clone(),
        input.agent_id.clone(),
        runtime_provider_config.clone(),
        now,
    );

    let queue_item = RuntimeQueueItem {
        runtime_id: runtime_id.clone(),
        session_id: input.session_id.clone(),
        agent_id: input.agent_id.clone(),
        messages: input.messages,
        tools: input.tools,
        provider_name: runtime_provider_config.provider_name.clone(),
        created_at: now,
    };

    Ok((runtime, queue_item))
}

pub fn runtime_provider_config_from_tura(
    provider_config: &ProviderConfig,
    settings: &tura_llm_rust::Settings,
    thinking: bool,
) -> Result<RuntimeProviderConfig, String> {
    let route = route_by_name(settings, &provider_config.tura_llm_name)
        .ok_or_else(|| format!("unknown provider route: {}", provider_config.tura_llm_name))?;
    let primary = route.providers.first().ok_or_else(|| {
        format!(
            "provider route '{}' has no configured providers",
            provider_config.tura_llm_name
        )
    })?;
    let selected = session_model_override()
        .and_then(|(provider, model)| {
            provider_base_url(settings, &provider).map(|base_url| tura_llm_rust::ProviderConfig {
                provider,
                base_url,
                model,
                temperature: primary.temperature,
            })
        })
        .unwrap_or_else(|| primary.clone());

    Ok(RuntimeProviderConfig {
        base: provider_config.clone(),
        thinking,
        provider_name: provider_config.tura_llm_name.clone(),
        model_name: selected.model.clone(),
        provider_url_name: selected.base_url.clone(),
        provider_router_name: selected.provider.clone(),
    })
}

fn session_model_override() -> Option<(String, String)> {
    let value = std::env::var("TURA_SESSION_MODEL_OVERRIDE").ok()?;
    let (provider, model) = value.trim().split_once('/')?;
    let provider = provider.trim();
    let model = model.trim();
    if provider.is_empty() || model.is_empty() {
        return None;
    }
    Some((
        provider.to_string(),
        tura_llm_rust::Settings::normalize_model_name(provider, model),
    ))
}

fn provider_base_url(settings: &tura_llm_rust::Settings, provider: &str) -> Option<String> {
    for route in [
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
    ] {
        if let Some(config) = route
            .providers
            .iter()
            .find(|item| item.provider == provider)
        {
            return Some(config.base_url.clone());
        }
    }
    match provider {
        "antigravity" => Some("https://antigravity.google.com/v1".to_string()),
        "anthropic" => Some("https://api.anthropic.com/v1".to_string()),
        "openai" => Some("https://api.openai.com/v1".to_string()),
        _ => None,
    }
}

pub async fn enqueue_runtime(queue_item: RuntimeQueueItem, redis_url: &str) -> Result<(), String> {
    let client = redis::Client::open(redis_url)
        .map_err(|e| format!("failed to create redis client: {}", e))?;

    let mut con = client
        .get_multiplexed_async_connection()
        .await
        .map_err(|e| format!("failed to get redis connection: {}", e))?;

    let queue_key = format!("runtime:queue:{}", queue_item.session_id);
    let payload = serde_json::to_string(&queue_item)
        .map_err(|e| format!("failed to serialize queue item: {}", e))?;

    redis::cmd("RPUSH")
        .arg(&queue_key)
        .arg(&payload)
        .query_async::<_, ()>(&mut con)
        .await
        .map_err(|e| format!("failed to enqueue runtime: {}", e))?;

    Ok(())
}

fn generate_runtime_id() -> RuntimeId {
    format!(
        "runtime-{:x}",
        Utc::now().timestamp_nanos_opt().unwrap_or_default()
    )
}
