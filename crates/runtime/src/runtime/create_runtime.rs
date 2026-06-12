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
        runtime_id,
        session_id: input.session_id.clone(),
        agent_id: input.agent_id.clone(),
        messages: input.messages,
        tools: input.tools,
        provider_name: runtime_provider_config.provider_name,
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

    // Latency level is chosen by the tier flag (the route / tura_llm_name),
    // never by the thinking parameter. Install the tier's timeouts globally so
    // streaming.rs picks them up for first/idle/total deadlines.
    let tier_timeouts = tura_llm_rust::apply_latency_for_tier(&provider_config.tura_llm_name);

    let mut base = provider_config.clone();
    let provider_total_timeout_ms = std::env::var("TURA_PROVIDER_TOTAL_TIMEOUT_MS")
        .ok()
        .and_then(|value| value.trim().parse::<u64>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(tier_timeouts.total_timeout_ms);
    base.time_out_ms = provider_total_timeout_ms;

    Ok(RuntimeProviderConfig {
        base,
        thinking,
        provider_name: provider_config.tura_llm_name.clone(),
        model_name: selected.model,
        provider_url_name: selected.base_url,
        llm_provider_name: selected.provider,
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
    settings.provider_base_url(provider)
}

pub async fn enqueue_runtime(queue_item: RuntimeQueueItem, redis_url: &str) -> Result<(), String> {
    let client = redis::Client::open(redis_url)
        .map_err(|e| format!("failed to create redis client: {e}"))?;

    let mut con = client
        .get_multiplexed_async_connection()
        .await
        .map_err(|e| format!("failed to get redis connection: {e}"))?;

    let queue_key = format!("runtime:queue:{}", queue_item.session_id);
    let payload = serde_json::to_string(&queue_item)
        .map_err(|e| format!("failed to serialize queue item: {e}"))?;

    redis::cmd("RPUSH")
        .arg(&queue_key)
        .arg(&payload)
        .query_async::<_, ()>(&mut con)
        .await
        .map_err(|e| format!("failed to enqueue runtime: {e}"))?;

    Ok(())
}

fn generate_runtime_id() -> RuntimeId {
    format!(
        "runtime-{:x}",
        Utc::now().timestamp_nanos_opt().unwrap_or_default()
    )
}
