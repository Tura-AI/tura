use serde_json::{json, Value};

use crate::metrics::extract_google_metrics;
use crate::streaming::send_provider_request_first_response;
use crate::tura_llm::{default_client, CallOptions, ProviderResponse, TuraError};
use crate::utils::{deep_merge_json, google_parts_from_canonical, text_from_content};

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

    // Gemini has a dedicated `systemInstruction` field — use it instead of
    // collapsing system turns into `user` content (which loses the
    // instruction/dialogue separation the model relies on).
    if let Some(system) = build_system_instruction(messages) {
        payload["systemInstruction"] = system;
    }

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

        // Forced / constrained tool choice → Gemini `toolConfig`.
        if let Some(tool_config) = build_tool_config(options.tool_choice.as_ref()) {
            payload["toolConfig"] = tool_config;
        }
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
    let mut contents = Vec::new();
    for msg in messages {
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
            contents.push(json!({
                "role": "model",
                "parts": [part]
            }));
            continue;
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
            let output_value = msg.get("output");
            let response = output_value
                .filter(|value| value.is_array())
                .and_then(|_| text_from_content(output_value))
                .map(|text| json!({ "output": text }))
                .or_else(|| output_value.cloned().map(parse_json_string_value))
                .or_else(|| msg.get("content").cloned().map(parse_json_string_value))
                .unwrap_or_else(|| json!({}));
            push_google_function_response(
                &mut contents,
                json!({ "functionResponse": { "name": name, "response": response } }),
            );
            if let Some(media_parts) = google_parts_from_canonical(output_value)
                .filter(|parts| parts.iter().any(|part| part.get("inlineData").is_some()))
            {
                contents.push(json!({
                    "role": "user",
                    "parts": media_parts
                }));
            }
            continue;
        }

        // System turns are lifted into `systemInstruction`; skip them here.
        let raw_role = msg.get("role").and_then(Value::as_str).unwrap_or("user");
        if matches!(raw_role, "system" | "developer") {
            continue;
        }
        let role = match raw_role {
            "assistant" => "model",
            x => x,
        };
        let parts = google_parts_from_canonical(msg.get("content")).unwrap_or_default();
        if !parts.is_empty() {
            contents.push(json!({
                "role": role,
                "parts": parts
            }));
        }
    }
    Value::Array(contents)
}

fn _removed_build_contents_old_marker() {}

fn push_google_function_response(contents: &mut Vec<Value>, part: Value) {
    let Some(last) = contents.last_mut() else {
        contents.push(json!({ "role": "user", "parts": [part] }));
        return;
    };
    let last_has_function_response =
        last.get("parts")
            .and_then(Value::as_array)
            .is_some_and(|parts| {
                parts
                    .iter()
                    .any(|part| part.get("functionResponse").is_some())
            });
    if last.get("role").and_then(Value::as_str) == Some("user") && last_has_function_response {
        if let Some(parts) = last.get_mut("parts").and_then(Value::as_array_mut) {
            parts.push(part);
            return;
        }
    }
    contents.push(json!({ "role": "user", "parts": [part] }));
}

/// Concatenate all `system`/`developer` turns into a single Gemini
/// `systemInstruction` object, preserving them as genuine instructions rather
/// than user dialogue. Returns `None` when there is no system text.
fn build_system_instruction(messages: &[Value]) -> Option<Value> {
    let mut parts = Vec::new();
    for msg in messages {
        if msg.get("type").and_then(Value::as_str).is_some() {
            continue; // function_call / function_call_output items
        }
        let role = msg.get("role").and_then(Value::as_str).unwrap_or("user");
        if !matches!(role, "system" | "developer") {
            continue;
        }
        match msg.get("content") {
            Some(Value::String(text)) if !text.trim().is_empty() => {
                parts.push(json!({ "text": text }));
            }
            Some(Value::Array(items)) => {
                for item in items {
                    if item.get("text").and_then(Value::as_str).is_some() {
                        parts.push(item.clone());
                    }
                }
            }
            _ => {}
        }
    }
    (!parts.is_empty()).then(|| json!({ "parts": parts }))
}

/// Translate an OpenAI-style `tool_choice` into Gemini's
/// `toolConfig.functionCallingConfig`. `auto`/absent → no config (model
/// decides); `none` → NONE; `required`/`any` → ANY; a specific
/// `{type:"function", function:{name}}` → ANY constrained to that name.
fn build_tool_config(tool_choice: Option<&Value>) -> Option<Value> {
    let choice = tool_choice?;
    match choice {
        Value::String(mode) => match mode.to_ascii_lowercase().as_str() {
            "none" => Some(json!({ "functionCallingConfig": { "mode": "NONE" } })),
            "required" | "any" => Some(json!({ "functionCallingConfig": { "mode": "ANY" } })),
            _ => None, // "auto" or unknown → let the model decide
        },
        Value::Object(_) => {
            if choice.get("type").and_then(Value::as_str) == Some("function") {
                if let Some(name) = choice
                    .get("function")
                    .and_then(|f| f.get("name"))
                    .and_then(Value::as_str)
                    .filter(|n| !n.trim().is_empty())
                {
                    return Some(json!({
                        "functionCallingConfig": {
                            "mode": "ANY",
                            "allowedFunctionNames": [name]
                        }
                    }));
                }
            }
            None
        }
        _ => None,
    }
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

    use super::{
        build_contents, build_system_instruction, build_tool_config, parse_json_string_value,
        sanitize_google_schema,
    };

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
    fn google_contents_skip_system_and_map_roles() {
        let messages = [
            json!({"role": "system", "content": "rules"}),
            json!({"role": "assistant", "content": "ok"}),
            json!({"role": "user", "content": [{"text": "hi"}]}),
        ];
        let contents = build_contents(&messages);

        // System is lifted to systemInstruction, not folded into contents.
        assert_eq!(contents.as_array().unwrap().len(), 2);
        assert_eq!(contents[0]["role"], "model");
        assert_eq!(contents[0]["parts"][0]["text"], "ok");
        assert_eq!(contents[1]["role"], "user");
        assert_eq!(contents[1]["parts"][0]["text"], "hi");

        let system = build_system_instruction(&messages).expect("system instruction");
        assert_eq!(system["parts"][0]["text"], "rules");
    }

    #[test]
    fn google_tool_config_maps_choice_modes() {
        assert!(build_tool_config(None).is_none());
        assert!(build_tool_config(Some(&json!("auto"))).is_none());
        assert_eq!(
            build_tool_config(Some(&json!("required")))
                .unwrap()
                .pointer("/functionCallingConfig/mode")
                .unwrap(),
            "ANY"
        );
        assert_eq!(
            build_tool_config(Some(&json!("none")))
                .unwrap()
                .pointer("/functionCallingConfig/mode")
                .unwrap(),
            "NONE"
        );
        let forced = build_tool_config(Some(&json!({
            "type": "function",
            "function": { "name": "echo_answer" }
        })))
        .unwrap();
        assert_eq!(forced["functionCallingConfig"]["mode"], "ANY");
        assert_eq!(
            forced["functionCallingConfig"]["allowedFunctionNames"][0],
            "echo_answer"
        );
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
        assert_eq!(contents[1]["role"], "user");
        assert_eq!(contents[1]["parts"][0]["functionResponse"]["name"], "echo");
        assert_eq!(
            contents[1]["parts"][0]["functionResponse"]["response"]["ok"],
            true
        );
    }

    #[test]
    fn google_function_output_media_adds_inline_data_sidecar() {
        let contents = build_contents(&[
            json!({
                "type": "function_call",
                "call_id": "call_media",
                "name": "command_run",
                "arguments": "{}"
            }),
            json!({
                "type": "function_call_output",
                "call_id": "call_media",
                "output": [
                    { "type": "input_text", "text": "read_media returned image" },
                    { "type": "input_image", "image_url": "data:image/png;base64,AAA" }
                ]
            }),
        ]);

        assert_eq!(contents[1]["role"], "user");
        assert_eq!(
            contents[1]["parts"][0]["functionResponse"]["response"]["output"],
            "read_media returned image"
        );
        assert_eq!(contents[2]["role"], "user");
        assert_eq!(
            contents[2]["parts"][1]["inlineData"]["mimeType"],
            "image/png"
        );
        assert_eq!(contents[2]["parts"][1]["inlineData"]["data"], "AAA");
    }

    #[test]
    fn google_function_outputs_merge_into_one_user_turn() {
        let contents = build_contents(&[
            json!({
                "type": "function_call",
                "call_id": "call_1",
                "name": "first",
                "arguments": "{}"
            }),
            json!({
                "type": "function_call_output",
                "call_id": "call_1",
                "output": "{\"ok\":1}"
            }),
            json!({
                "type": "function_call_output",
                "call_id": "call_missing",
                "output": "{\"ok\":2}"
            }),
        ]);

        assert_eq!(contents[1]["role"], "user");
        assert_eq!(contents[1]["parts"].as_array().unwrap().len(), 2);
        assert_eq!(contents[1]["parts"][0]["functionResponse"]["name"], "first");
        assert_eq!(
            contents[1]["parts"][1]["functionResponse"]["name"],
            "function"
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
