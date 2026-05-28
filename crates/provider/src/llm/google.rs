use serde_json::{json, Value};

use crate::metrics::extract_google_metrics;
use crate::streaming::send_provider_request_first_response;
use crate::tura_llm::{default_client, CallOptions, ProviderResponse, TuraError};
use crate::utils::deep_merge_json;

pub async fn embed(
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

    let resp = send_provider_request_first_response(
        client.post(url).header("Authorization", "").json(&payload),
    )
    .await?;
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
                    let mut parameters = func.get("parameters").unwrap_or(&json!({})).clone();
                    sanitize_google_schema(&mut parameters);
                    json!({
                        "name": func.get("name").unwrap_or(&json!("")).clone(),
                        "description": func.get("description").unwrap_or(&json!("")).clone(),
                        "parameters": parameters
                    })
                } else {
                    let mut tool = t.clone();
                    sanitize_google_schema(&mut tool);
                    tool
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

    let resp = send_provider_request_first_response(
        client.post(url).header("Authorization", "").json(&payload),
    )
    .await?;
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
    let metrics = extract_google_metrics(&data, options.context_window, req_id);

    Ok(ProviderResponse {
        content,
        raw: data,
        metrics: Some(metrics),
    })
}

fn sanitize_google_schema(value: &mut Value) {
    match value {
        Value::Object(object) => {
            object.remove("additionalProperties");
            for child in object.values_mut() {
                sanitize_google_schema(child);
            }
        }
        Value::Array(items) => {
            for child in items {
                sanitize_google_schema(child);
            }
        }
        _ => {}
    }
}

fn build_contents(messages: &[Value]) -> Value {
    let mut call_names = std::collections::HashMap::<String, String>::new();
    let contents: Vec<Value> = messages
        .iter()
        .filter_map(|msg| {
            if msg.get("type").and_then(Value::as_str) == Some("function_call") {
                let name = msg
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or("function")
                    .to_string();
                if let Some(call_id) = msg.get("call_id").and_then(Value::as_str) {
                    call_names.insert(call_id.to_string(), name.clone());
                }
                let args = msg
                    .get("arguments")
                    .cloned()
                    .map(parse_json_string_value)
                    .unwrap_or_else(|| json!({}));
                let mut part = json!({ "functionCall": { "name": name, "args": args } });
                if let Some(signature) = google_thought_signature(msg) {
                    part["thoughtSignature"] = json!(signature);
                }
                return Some(json!({
                    "role": "model",
                    "parts": [part]
                }));
            }

            if msg.get("type").and_then(Value::as_str) == Some("function_call_output") {
                let call_id = msg
                    .get("call_id")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                let name = call_names
                    .get(call_id)
                    .cloned()
                    .unwrap_or_else(|| "function".to_string());
                let response = msg
                    .get("output")
                    .cloned()
                    .map(parse_json_string_value)
                    .unwrap_or_else(|| json!({}));
                return Some(json!({
                    "role": "function",
                    "parts": [{ "functionResponse": { "name": name, "response": response } }]
                }));
            }

            let role = match msg.get("role").and_then(Value::as_str).unwrap_or("user") {
                "assistant" => "model",
                "system" => "user",
                x => x,
            };
            let parts = match msg.get("content") {
                Some(Value::String(text)) => vec![json!({ "text": text })],
                Some(Value::Array(items)) => items.clone(),
                Some(other) => vec![json!({ "text": other.to_string() })],
                None => vec![],
            };
            (!parts.is_empty()).then(|| {
                json!({
                    "role": role,
                    "parts": parts
                })
            })
        })
        .collect();
    Value::Array(contents)
}

fn google_thought_signature(msg: &Value) -> Option<String> {
    msg.get("provider_metadata")
        .and_then(|metadata| {
            metadata
                .get("google_thought_signature")
                .or_else(|| metadata.get("thoughtSignature"))
                .or_else(|| metadata.get("thought_signature"))
        })
        .and_then(Value::as_str)
        .map(ToString::to_string)
}

fn parse_json_string_value(value: Value) -> Value {
    match value {
        Value::String(text) => {
            serde_json::from_str(&text).unwrap_or_else(|_| json!({ "output": text }))
        }
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{build_contents, parse_json_string_value, sanitize_google_schema};

    #[test]
    fn google_schema_sanitization_drops_unsupported_additional_properties() {
        let mut schema = json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "nested": {
                    "type": "object",
                    "additionalProperties": false
                }
            }
        });

        sanitize_google_schema(&mut schema);

        assert!(schema.get("additionalProperties").is_none());
        assert!(schema["properties"]["nested"]
            .get("additionalProperties")
            .is_none());
    }

    #[test]
    fn google_contents_map_roles_and_text_parts() {
        let contents = build_contents(&[
            json!({"role": "system", "content": "rules"}),
            json!({"role": "assistant", "content": "ok"}),
            json!({"role": "user", "content": [{"text": "hi"}]}),
        ]);

        assert_eq!(contents[0]["role"], "user");
        assert_eq!(contents[0]["parts"][0]["text"], "rules");
        assert_eq!(contents[1]["role"], "model");
        assert_eq!(contents[2]["parts"][0]["text"], "hi");
    }

    #[test]
    fn google_contents_map_function_call_and_output() {
        let contents = build_contents(&[
            json!({
                "type": "function_call",
                "call_id": "call_1",
                "name": "echo",
                "arguments": "{\"answer\":\"pong\"}",
                "provider_metadata": {"thoughtSignature": "sig"}
            }),
            json!({
                "type": "function_call_output",
                "call_id": "call_1",
                "output": "{\"ok\":true}"
            }),
        ]);

        assert_eq!(contents[0]["role"], "model");
        assert_eq!(contents[0]["parts"][0]["functionCall"]["name"], "echo");
        assert_eq!(
            contents[0]["parts"][0]["functionCall"]["args"]["answer"],
            "pong"
        );
        assert_eq!(contents[0]["parts"][0]["thoughtSignature"], "sig");
        assert_eq!(contents[1]["role"], "function");
        assert_eq!(contents[1]["parts"][0]["functionResponse"]["name"], "echo");
        assert_eq!(
            contents[1]["parts"][0]["functionResponse"]["response"]["ok"],
            true
        );
    }

    #[test]
    fn parse_json_string_value_wraps_plain_text_outputs() {
        assert_eq!(
            parse_json_string_value(json!("not-json")),
            json!({"output": "not-json"})
        );
        assert_eq!(
            parse_json_string_value(json!("{\"ok\":true}")),
            json!({"ok": true})
        );
    }
}
