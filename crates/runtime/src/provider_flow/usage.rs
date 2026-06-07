//! Provider usage accounting helpers.

use chrono::{DateTime, Utc};

use crate::state_machine::runtime_management::{RuntimeManagement, UsageReport};

pub(crate) fn usage_report_from_metrics(
    metrics: Option<tura_llm_rust::CallMetrics>,
    started_at: DateTime<Utc>,
    finished_at: DateTime<Utc>,
    first_token_at: DateTime<Utc>,
) -> Option<UsageReport> {
    let latency_ms = finished_at
        .signed_duration_since(started_at)
        .num_milliseconds()
        .max(0) as u64;
    let time_to_first_token_ms = first_token_at
        .signed_duration_since(started_at)
        .num_milliseconds()
        .max(0) as u64;
    metrics.map(|m| crate::state_machine::runtime_management::UsageReport {
        input_tokens: m.usage.input_tokens.unwrap_or(0),
        output_tokens: m.usage.output_tokens.unwrap_or(0),
        total_tokens: m.usage.total_tokens.unwrap_or(0),
        cached_input_tokens: m.usage.cached_input_tokens.unwrap_or(0),
        cache_write_tokens: m.usage.cache_write_tokens.unwrap_or(0),
        reasoning_tokens: m.usage.reasoning_tokens.unwrap_or(0),
        attachment_input_tokens: 0,
        input_cost: m.cost.input_cost.unwrap_or(0.0),
        output_cost: m.cost.output_cost.unwrap_or(0.0),
        total_cost: m.cost.total_cost.unwrap_or(0.0),
        currency: m.cost.currency.unwrap_or_else(|| "USD".to_string()),
        pricing_source: "provider".to_string(),
        latency_ms,
        time_to_first_token_ms,
        token_per_second: tokens_per_second(m.usage.output_tokens.unwrap_or(0), latency_ms),
    })
}

pub(crate) fn estimated_usage_report_for_interrupted_runtime(
    runtime: &RuntimeManagement,
    started_at: DateTime<Utc>,
    finished_at: DateTime<Utc>,
    first_token_at: DateTime<Utc>,
    pricing_source: impl Into<String>,
) -> UsageReport {
    let latency_ms = finished_at
        .signed_duration_since(started_at)
        .num_milliseconds()
        .max(0) as u64;
    let time_to_first_token_ms = first_token_at
        .signed_duration_since(started_at)
        .num_milliseconds()
        .max(0) as u64;
    let input_tokens = runtime
        .input
        .as_ref()
        .map(approx_json_tokens)
        .unwrap_or_default();
    let output_tokens = runtime
        .output
        .as_ref()
        .map(approx_json_tokens)
        .unwrap_or_default();

    UsageReport {
        input_tokens,
        output_tokens,
        total_tokens: input_tokens + output_tokens,
        cached_input_tokens: 0,
        cache_write_tokens: 0,
        reasoning_tokens: 0,
        attachment_input_tokens: 0,
        input_cost: 0.0,
        output_cost: 0.0,
        total_cost: 0.0,
        currency: "USD".to_string(),
        pricing_source: pricing_source.into(),
        latency_ms,
        time_to_first_token_ms,
        token_per_second: tokens_per_second(output_tokens, latency_ms),
    }
}

fn tokens_per_second(output_tokens: u64, latency_ms: u64) -> f64 {
    if output_tokens == 0 || latency_ms == 0 {
        return 0.0;
    }
    output_tokens as f64 / (latency_ms as f64 / 1000.0)
}

fn approx_json_tokens(value: &serde_json::Value) -> u64 {
    approx_text_tokens(&serde_json::to_string(value).unwrap_or_else(|_| value.to_string()))
}

fn approx_text_tokens(text: &str) -> u64 {
    let chars = text.chars().count() as u64;
    chars.div_ceil(4).max((!text.is_empty()) as u64)
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

#[cfg(test)]
mod tests {
    use super::estimated_usage_report_for_interrupted_runtime;
    use crate::state_machine::agent_management::{ProviderConfig, ToolChoice};
    use crate::state_machine::runtime_management::{RuntimeManagement, RuntimeProviderConfig};
    use chrono::{Duration, Utc};
    use serde_json::json;

    #[test]
    fn interrupted_usage_estimate_counts_input_and_output() {
        let started_at = Utc::now();
        let finished_at = started_at + Duration::milliseconds(2500);
        let mut runtime = RuntimeManagement::new(
            "runtime-estimate".to_string(),
            "session-estimate".to_string(),
            "agent-estimate".to_string(),
            RuntimeProviderConfig {
                base: ProviderConfig {
                    tura_llm_name: "openai".to_string(),
                    stream: true,
                    temperature: 0.0,
                    max_tokens: 0,
                    tool_choice: ToolChoice::Auto,
                    time_out_ms: 1_000,
                },
                thinking: false,
                provider_name: "openai".to_string(),
                model_name: "gpt-test".to_string(),
                provider_url_name: String::new(),
                llm_provider_name: "openai".to_string(),
            },
            started_at,
        );
        runtime.set_input(json!({"messages":[{"role":"user","content":"hello world"}]}));
        runtime.set_output(json!({"streamed_command_run_result":{"commands":["apply_patch"]}}));

        let usage = estimated_usage_report_for_interrupted_runtime(
            &runtime,
            started_at,
            finished_at,
            started_at,
            "runtime_estimate_cancelled",
        );

        assert!(usage.input_tokens > 0);
        assert!(usage.output_tokens > 0);
        assert_eq!(usage.total_tokens, usage.input_tokens + usage.output_tokens);
        assert_eq!(usage.pricing_source, "runtime_estimate_cancelled");
        assert_eq!(usage.latency_ms, 2500);
    }
}
