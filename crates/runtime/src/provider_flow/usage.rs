//! Provider usage accounting helpers.

use chrono::{DateTime, Utc};

use crate::state_machine::runtime_management::UsageReport;

pub(crate) fn usage_report_from_metrics(
    runtime: &crate::state_machine::runtime_management::RuntimeManagement,
    metrics: Option<tura_llm_rust::CallMetrics>,
    started_at: DateTime<Utc>,
    finished_at: DateTime<Utc>,
    first_token_at: DateTime<Utc>,
) -> Option<UsageReport> {
    let called_at = runtime.called_at.unwrap_or(runtime.created_at);
    let routing_ms = called_at
        .signed_duration_since(runtime.created_at)
        .num_milliseconds()
        .max(0) as u64;
    let provider_queue_ms = started_at
        .signed_duration_since(called_at)
        .num_milliseconds()
        .max(0) as u64;
    let latency_ms = finished_at
        .signed_duration_since(started_at)
        .num_milliseconds()
        .max(0) as u64;
    let time_to_first_token_ms = first_token_at
        .signed_duration_since(started_at)
        .num_milliseconds()
        .max(0) as u64;
    metrics.and_then(|m| {
        let input_tokens = m.usage.input_tokens?;
        let output_tokens = m.usage.output_tokens?;
        let total_tokens = m.usage.total_tokens?;
        Some(crate::state_machine::runtime_management::UsageReport {
            input_tokens,
            output_tokens,
            total_tokens,
            cached_input_tokens: m.usage.cached_input_tokens.unwrap_or(0),
            cache_write_tokens: m.usage.cache_write_tokens.unwrap_or(0),
            reasoning_tokens: m.usage.reasoning_tokens.unwrap_or(0),
            attachment_input_tokens: 0,
            input_cost: m.cost.input_cost.unwrap_or(0.0),
            output_cost: m.cost.output_cost.unwrap_or(0.0),
            total_cost: m.cost.total_cost.unwrap_or(0.0),
            currency: m.cost.currency.unwrap_or_else(|| "USD".to_string()),
            pricing_source: "provider".to_string(),
            routing_ms,
            provider_queue_ms,
            latency_ms,
            time_to_first_token_ms,
            token_per_second: tokens_per_second(output_tokens, latency_ms),
        })
    })
}

fn tokens_per_second(output_tokens: u64, latency_ms: u64) -> f64 {
    if output_tokens == 0 || latency_ms == 0 {
        return 0.0;
    }
    output_tokens as f64 / (latency_ms as f64 / 1000.0)
}

pub(crate) fn runtime_cache_diagnostics(
    runtime: &crate::state_machine::runtime_management::RuntimeManagement,
) -> serde_json::Value {
    let input = runtime.input.as_ref();
    let messages = input
        .and_then(|input| input.get("messages"))
        .and_then(serde_json::Value::as_array);
    let tools = input
        .and_then(|input| input.get("tools"))
        .and_then(serde_json::Value::as_array);
    let options = input.and_then(|input| input.get("options"));
    serde_json::json!({
        "input_hash": input.map(stable_json_hash).unwrap_or_default(),
        "message_count": messages.map(|messages| messages.len()).unwrap_or_default(),
        "tool_count": tools.map(|tools| tools.len()).unwrap_or_default(),
        "first_message_hash": messages
            .and_then(|messages| messages.first())
            .map(stable_json_hash)
            .unwrap_or_default(),
        "last_message_hash": messages
            .and_then(|messages| messages.last())
            .map(stable_json_hash)
            .unwrap_or_default(),
        "tools_hash": tools
            .map(|tools| stable_json_hash(&serde_json::Value::Array(tools.clone())))
            .unwrap_or_default(),
        "prompt_cache_key": options
            .and_then(|options| options.get("prompt_cache_key"))
            .cloned()
            .unwrap_or(serde_json::Value::Null),
    })
}

fn stable_json_hash(value: &serde_json::Value) -> String {
    let serialized = serde_json::to_string(value).unwrap_or_else(|_| value.to_string());
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in serialized.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}
