use serde_json::{json, Value};

use crate::llm::_utils::deep_merge_json;
use crate::tura_llm::{
    default_client, estimate_context_utilization, CallMetrics, CallOptions, CostDetails,
    ProviderResponse, TuraError, UsageDetails,
};

pub async fn google_embed(
    base_url: &str,
    model: &str,
    api_key: &str,
    text: &str,
) -> Result<Vec<f32>, TuraError> {
    let client = default_client(api_key)?;
    let clean_model = model.strip_prefix("models/").unwrap_or(model);
    let url = format!(
        "{}/models/{}:embedContent?key={}",
        base_url.trim_end_matches('/'),
        clean_model,
        api_key
    );
    let payload = json!({
        "model": format!("models/{}", clean_model),
        "content": { "parts": [{ "text": text }] },
        "taskType": "RETRIEVAL_DOCUMENT"
    });

    let resp = client
        .post(url)
        .header("Authorization", "")
        .json(&payload)
        .send()
        .await
        .map_err(|e| TuraError::Network {
            message: e.to_string(),
        })?;
    let status = resp.status();
    let data: Value = resp.json().await.map_err(|e| TuraError::Network {
        message: e.to_string(),
    })?;
    if !status.is_success() {
        return Err(TuraError::HttpStatus {
            status: status.as_u16(),
            body: data.to_string(),
        });
    }
    let embedding = data
        .pointer("/embedding/values")
        .and_then(Value::as_array)
        .ok_or_else(|| TuraError::ProviderRequest {
            provider: "google".into(),
            message: "missing embedding values".into(),
        })?;
    Ok(embedding
        .iter()
        .filter_map(Value::as_f64)
        .map(|v| v as f32)
        .collect())
}

pub async fn call(
    base_url: &str,
    model: &str,
    api_key: &str,
    messages: &[Value],
    options: &CallOptions,
) -> Result<ProviderResponse, TuraError> {
    let client = default_client(api_key)?;
    let clean_model = model.strip_prefix("models/").unwrap_or(model);
    let url = format!(
        "{}/models/{}:generateContent?key={}",
        base_url.trim_end_matches('/'),
        clean_model,
        api_key
    );

    let mut payload = json!({
        "contents": build_contents(messages),
        "generationConfig": {
            "temperature": options.temperature.unwrap_or(0.2),
        }
    });

    if options.search {
        payload["tools"] = json!([{ "googleSearch": {} }]);
    } else if let Some(tools) = &options.tools {
        let declarations: Vec<Value> = tools
            .iter()
            .map(|t| {
                if let Some(func) = t.get("function") {
                    json!({
                        "name": func.get("name").unwrap_or(&json!("")).clone(),
                        "description": func.get("description").unwrap_or(&json!("")).clone(),
                        "parameters": func.get("parameters").unwrap_or(&json!({})).clone()
                    })
                } else {
                    t.clone()
                }
            })
            .collect();
        payload["tools"] = json!([{ "functionDeclarations": declarations }]);
    }

    if let Some(max) = options.max_tokens.or(options.max_completion_tokens) {
        payload["generationConfig"]["maxOutputTokens"] = json!(max);
    }
    if let Some(top_p) = options.top_p {
        payload["generationConfig"]["topP"] = json!(top_p);
    }
    if let Some(extra) = &options.extra_body {
        deep_merge_json(&mut payload, extra.clone());
    }

    let resp = client
        .post(url)
        .header("Authorization", "")
        .json(&payload)
        .send()
        .await
        .map_err(|e| TuraError::Network {
            message: e.to_string(),
        })?;
    let status = resp.status();
    let req_id = resp
        .headers()
        .get("x-request-id")
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);
    let data: Value = resp.json().await.map_err(|e| TuraError::Network {
        message: e.to_string(),
    })?;
    if !status.is_success() {
        return Err(TuraError::HttpStatus {
            status: status.as_u16(),
            body: data.to_string(),
        });
    }

    let content = data
        .pointer("/candidates/0/content")
        .cloned()
        .unwrap_or_else(|| data.clone());
    let mut metrics = CallMetrics {
        usage: UsageDetails {
            input_tokens: pointer_u64(&data, "/usageMetadata/promptTokenCount"),
            output_tokens: pointer_u64(&data, "/usageMetadata/candidatesTokenCount"),
            total_tokens: pointer_u64(&data, "/usageMetadata/totalTokenCount"),
            cached_input_tokens: pointer_u64(&data, "/usageMetadata/cachedContentTokenCount"),
            context_window: options.context_window,
            ..Default::default()
        },
        cost: CostDetails {
            total_cost: None,
            currency: Some("USD".into()),
            ..Default::default()
        },
        cache_hit: pointer_u64(&data, "/usageMetadata/cachedContentTokenCount").unwrap_or(0) > 0,
        cache_triggered_at_input_tokens: pointer_u64(
            &data,
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
        provider_request_id: req_id,
        raw_usage: data.get("usageMetadata").cloned(),
    };
    estimate_context_utilization(&mut metrics);

    Ok(ProviderResponse {
        content,
        raw: data,
        metrics: Some(metrics),
    })
}

fn build_contents(messages: &[Value]) -> Value {
    let contents: Vec<Value> = messages
        .iter()
        .map(|msg| {
            let role = match msg.get("role").and_then(Value::as_str).unwrap_or("user") {
                "assistant" => "model",
                x => x,
            };
            let parts = match msg.get("content") {
                Some(Value::String(text)) => vec![json!({ "text": text })],
                Some(Value::Array(items)) => items.clone(),
                Some(other) => vec![json!({ "text": other.to_string() })],
                None => vec![],
            };
            json!({
                "role": role,
                "parts": parts
            })
        })
        .collect();
    Value::Array(contents)
}

fn pointer_u64(value: &Value, ptr: &str) -> Option<u64> {
    value
        .pointer(ptr)
        .and_then(|v| v.as_u64().or_else(|| v.as_i64().map(|i| i.max(0) as u64)))
}
