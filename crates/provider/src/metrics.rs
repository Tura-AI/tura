use serde_json::{json, Value};

use crate::tura_llm::{estimate_context_utilization, CallMetrics, CostDetails, UsageDetails};

pub(crate) fn fill_missing_estimated_usage(
    metrics: &mut CallMetrics,
    payload: &Value,
    content: &Value,
    reason: &str,
) {
    if metrics.usage.total_tokens.unwrap_or(0) > 0 {
        return;
    }

    let input_tokens = estimate_token_count(&payload.to_string()).max(1);
    let output_tokens = estimate_token_count(&content.to_string()).max(1);
    metrics.usage.input_tokens = Some(input_tokens);
    metrics.usage.output_tokens = Some(output_tokens);
    metrics.usage.total_tokens = Some(input_tokens + output_tokens);
    metrics.raw_usage = Some(json!({
        "estimated": true,
        "reason": reason,
        "input_tokens": input_tokens,
        "output_tokens": output_tokens,
        "total_tokens": input_tokens + output_tokens
    }));
}

pub(crate) fn estimate_token_count(text: &str) -> u64 {
    let chars = text.chars().count() as u64;
    chars.div_ceil(4)
}

pub(crate) fn extract_openapi_metrics(data: &Value, context_window: Option<u64>) -> CallMetrics {
    let usage = data.get("usage").cloned().unwrap_or(Value::Null);
    let input_tokens =
        pointer_u64(&usage, "/prompt_tokens").or_else(|| pointer_u64(&usage, "/input_tokens"));
    let output_tokens =
        pointer_u64(&usage, "/completion_tokens").or_else(|| pointer_u64(&usage, "/output_tokens"));
    let cached_input_tokens = pointer_u64(&usage, "/prompt_tokens_details/cached_tokens")
        .or_else(|| pointer_u64(&usage, "/input_tokens_details/cached_tokens"))
        .or_else(|| pointer_u64(&usage, "/cache_read_input_tokens"))
        .or_else(|| pointer_u64(&usage, "/cache_read_tokens"));
    let cache_write_tokens = pointer_u64(&usage, "/prompt_tokens_details/cache_creation_tokens")
        .or_else(|| pointer_u64(&usage, "/input_tokens_details/cache_write_tokens"))
        .or_else(|| pointer_u64(&usage, "/cache_creation_input_tokens"))
        .or_else(|| pointer_u64(&usage, "/cache_write_tokens"));
    let reasoning_tokens = pointer_u64(&usage, "/completion_tokens_details/reasoning_tokens")
        .or_else(|| pointer_u64(&usage, "/output_tokens_details/reasoning_tokens"));

    let mut metrics = CallMetrics {
        usage: UsageDetails {
            input_tokens,
            output_tokens,
            total_tokens: pointer_u64(&usage, "/total_tokens").or_else(|| {
                input_tokens.zip(output_tokens).map(|(input, output)| {
                    input
                        + output
                        + cached_input_tokens.unwrap_or(0)
                        + cache_write_tokens.unwrap_or(0)
                })
            }),
            cached_input_tokens,
            cache_write_tokens,
            reasoning_tokens,
            audio_input_tokens: pointer_u64(&usage, "/prompt_tokens_details/audio_tokens"),
            audio_output_tokens: pointer_u64(&usage, "/completion_tokens_details/audio_tokens"),
            context_window,
            ..Default::default()
        },
        cost: extract_openapi_costs(
            data,
            input_tokens,
            output_tokens,
            cached_input_tokens,
            cache_write_tokens,
            reasoning_tokens,
        ),
        cache_hit: cached_input_tokens.unwrap_or(0) > 0,
        cache_triggered_at_input_tokens: cached_input_tokens,
        tool_call_count: data
            .pointer("/choices/0/message/tool_calls")
            .and_then(Value::as_array)
            .map(|v| v.len())
            .unwrap_or(0),
        finish_reason: data
            .pointer("/choices/0/finish_reason")
            .and_then(Value::as_str)
            .map(str::to_string),
        provider_request_id: None,
        raw_usage: if usage.is_null() { None } else { Some(usage) },
    };
    estimate_context_utilization(&mut metrics);
    metrics
}

pub(crate) fn extract_google_metrics(
    data: &Value,
    context_window: Option<u64>,
    provider_request_id: Option<String>,
) -> CallMetrics {
    let mut metrics = CallMetrics {
        usage: UsageDetails {
            input_tokens: pointer_u64(data, "/usageMetadata/promptTokenCount"),
            output_tokens: pointer_u64(data, "/usageMetadata/candidatesTokenCount"),
            total_tokens: pointer_u64(data, "/usageMetadata/totalTokenCount"),
            cached_input_tokens: pointer_u64(data, "/usageMetadata/cachedContentTokenCount"),
            context_window,
            ..Default::default()
        },
        cost: CostDetails {
            total_cost: None,
            currency: Some("USD".into()),
            ..Default::default()
        },
        cache_hit: pointer_u64(data, "/usageMetadata/cachedContentTokenCount").unwrap_or(0) > 0,
        cache_triggered_at_input_tokens: pointer_u64(
            data,
            "/usageMetadata/cachedContentTokenCount",
        ),
        tool_call_count: data
            .pointer("/candidates/0/content/parts")
            .and_then(Value::as_array)
            .map(|parts| {
                parts
                    .iter()
                    .filter(|p| p.get("functionCall").is_some())
                    .count()
            })
            .unwrap_or(0),
        finish_reason: data
            .pointer("/candidates/0/finishReason")
            .and_then(Value::as_str)
            .map(str::to_string),
        provider_request_id,
        raw_usage: data.get("usageMetadata").cloned(),
    };
    estimate_context_utilization(&mut metrics);
    metrics
}

pub(crate) fn pointer_u64(value: &Value, ptr: &str) -> Option<u64> {
    value
        .pointer(ptr)
        .and_then(|v| v.as_u64().or_else(|| v.as_i64().map(|i| i.max(0) as u64)))
}

fn extract_openapi_costs(
    data: &Value,
    input_tokens: Option<u64>,
    output_tokens: Option<u64>,
    cached_input_tokens: Option<u64>,
    cache_write_tokens: Option<u64>,
    reasoning_tokens: Option<u64>,
) -> CostDetails {
    let direct = data.get("cost");
    CostDetails {
        input_cost: direct.and_then(|v| v.get("input")).and_then(Value::as_f64),
        output_cost: direct.and_then(|v| v.get("output")).and_then(Value::as_f64),
        cache_read_cost: direct
            .and_then(|v| v.get("cache_read"))
            .and_then(Value::as_f64),
        cache_write_cost: direct
            .and_then(|v| v.get("cache_write"))
            .and_then(Value::as_f64),
        reasoning_cost: direct
            .and_then(|v| v.get("reasoning"))
            .and_then(Value::as_f64),
        total_cost: direct
            .and_then(|v| v.get("total"))
            .and_then(Value::as_f64)
            .or_else(|| {
                approx_total_cost(
                    input_tokens,
                    output_tokens,
                    cached_input_tokens,
                    cache_write_tokens,
                    reasoning_tokens,
                )
            }),
        currency: direct
            .and_then(|v| v.get("currency"))
            .and_then(Value::as_str)
            .map(str::to_string)
            .or_else(|| Some("USD".to_string())),
    }
}

fn approx_total_cost(
    input_tokens: Option<u64>,
    output_tokens: Option<u64>,
    cached_input_tokens: Option<u64>,
    cache_write_tokens: Option<u64>,
    reasoning_tokens: Option<u64>,
) -> Option<f64> {
    let input = input_tokens.unwrap_or(0) as f64 / 1_000_000.0 * 0.15;
    let output = output_tokens.unwrap_or(0) as f64 / 1_000_000.0 * 0.60;
    let cache_read = cached_input_tokens.unwrap_or(0) as f64 / 1_000_000.0 * 0.03;
    let cache_write = cache_write_tokens.unwrap_or(0) as f64 / 1_000_000.0 * 0.15;
    let reasoning = reasoning_tokens.unwrap_or(0) as f64 / 1_000_000.0 * 0.10;
    let total = input + output + cache_read + cache_write + reasoning;
    if total > 0.0 {
        Some(total)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::{
        extract_google_metrics, extract_openapi_metrics, fill_missing_estimated_usage, pointer_u64,
    };
    use serde_json::json;

    #[test]
    fn openapi_metrics_read_all_known_usage_variants() {
        let metrics = extract_openapi_metrics(
            &json!({
                "usage": {
                    "prompt_tokens": 100,
                    "completion_tokens": 20,
                    "prompt_tokens_details": {
                        "cached_tokens": 40,
                        "cache_creation_tokens": 7,
                        "audio_tokens": 3
                    },
                    "completion_tokens_details": {
                        "reasoning_tokens": 5,
                        "audio_tokens": 2
                    }
                },
                "choices": [{
                    "finish_reason": "tool_calls",
                    "message": {"tool_calls": [{}, {}]}
                }]
            }),
            Some(200),
        );

        assert_eq!(metrics.usage.input_tokens, Some(100));
        assert_eq!(metrics.usage.output_tokens, Some(20));
        assert_eq!(metrics.usage.cached_input_tokens, Some(40));
        assert_eq!(metrics.usage.cache_write_tokens, Some(7));
        assert_eq!(metrics.usage.reasoning_tokens, Some(5));
        assert_eq!(metrics.usage.audio_input_tokens, Some(3));
        assert_eq!(metrics.usage.audio_output_tokens, Some(2));
        assert_eq!(metrics.usage.total_tokens, Some(167));
        assert_eq!(metrics.tool_call_count, 2);
        assert_eq!(metrics.finish_reason.as_deref(), Some("tool_calls"));
        assert!(metrics.cache_hit);
    }

    #[test]
    fn openapi_metrics_prefer_direct_cost_when_provider_returns_it() {
        let metrics = extract_openapi_metrics(
            &json!({
                "usage": {"input_tokens": 10, "output_tokens": 20, "total_tokens": 30},
                "cost": {
                    "input": 0.1,
                    "output": 0.2,
                    "cache_read": 0.03,
                    "cache_write": 0.04,
                    "reasoning": 0.05,
                    "total": 0.42,
                    "currency": "EUR"
                }
            }),
            None,
        );

        assert_eq!(metrics.cost.input_cost, Some(0.1));
        assert_eq!(metrics.cost.total_cost, Some(0.42));
        assert_eq!(metrics.cost.currency.as_deref(), Some("EUR"));
    }

    #[test]
    fn google_metrics_read_usage_metadata_and_tool_calls() {
        let metrics = extract_google_metrics(
            &json!({
                "usageMetadata": {
                    "promptTokenCount": 12,
                    "candidatesTokenCount": 8,
                    "totalTokenCount": 20,
                    "cachedContentTokenCount": 4
                },
                "candidates": [{
                    "finishReason": "STOP",
                    "content": {
                        "parts": [
                            {"functionCall": {"name": "echo"}},
                            {"text": "done"}
                        ]
                    }
                }]
            }),
            Some(128),
            Some("req-1".to_string()),
        );

        assert_eq!(metrics.usage.input_tokens, Some(12));
        assert_eq!(metrics.usage.output_tokens, Some(8));
        assert_eq!(metrics.usage.cached_input_tokens, Some(4));
        assert_eq!(metrics.tool_call_count, 1);
        assert_eq!(metrics.finish_reason.as_deref(), Some("STOP"));
        assert_eq!(metrics.provider_request_id.as_deref(), Some("req-1"));
        assert!(metrics.cache_hit);
    }

    #[test]
    fn estimated_usage_does_not_override_real_usage() {
        let mut metrics = extract_openapi_metrics(
            &json!({"usage": {"input_tokens": 2, "output_tokens": 3, "total_tokens": 5}}),
            None,
        );
        fill_missing_estimated_usage(&mut metrics, &json!({"input": "long"}), &json!("out"), "x");
        assert_eq!(metrics.usage.total_tokens, Some(5));
        assert_eq!(
            metrics
                .raw_usage
                .as_ref()
                .and_then(|value| value.get("estimated")),
            None
        );
    }

    #[test]
    fn pointer_u64_clamps_negative_signed_values() {
        assert_eq!(pointer_u64(&json!({"n": -2}), "/n"), Some(0));
        assert_eq!(pointer_u64(&json!({"n": 9}), "/n"), Some(9));
    }
}
