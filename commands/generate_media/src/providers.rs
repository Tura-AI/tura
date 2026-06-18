use super::config::{
    env_value, openai_auth_candidates, provider_endpoint, provider_key, provider_model, OpenAiAuth,
};
use super::files::{
    aspect_ratio, image_size_label, mime_type_for_format, openai_size, provider_dimensions,
    reference_data_url, reference_part_bytes,
};
use super::types::{GenerateMediaArgs, ImageBytes, ImageProvider, ProviderOutcome};
use base64::{engine::general_purpose, Engine as _};
use reqwest::blocking::{multipart, Client, RequestBuilder, Response};
use reqwest::StatusCode;
use serde_json::{json, Value};
use std::path::Path;

#[derive(Debug)]
struct HttpJsonError {
    message: String,
    status: Option<StatusCode>,
}

impl From<String> for HttpJsonError {
    fn from(message: String) -> Self {
        Self {
            message,
            status: None,
        }
    }
}

pub(super) fn render_prompt(args: &GenerateMediaArgs) -> String {
    match args.negative_prompt.as_deref() {
        Some(negative) => format!("{}\n\nNegative prompt: {negative}", args.prompt),
        None => args.prompt.clone(),
    }
}

pub(super) fn dry_run_payload(
    provider: ImageProvider,
    args: &GenerateMediaArgs,
    session_dir: &Path,
) -> Result<Value, String> {
    let model = provider_model(provider);
    Ok(match provider {
        ImageProvider::ChatGptImage2 => openai_json_payload(args, &model)?,
        ImageProvider::ReplicateZImageTurbo => replicate_payload(args)?,
        ImageProvider::Gemini31Flash => gemini_payload(args, session_dir)?,
        ImageProvider::Grok3 => grok_payload(args, session_dir)?,
    })
}

pub(super) fn call_provider(
    client: &Client,
    provider: ImageProvider,
    args: &GenerateMediaArgs,
    session_dir: &Path,
) -> Result<ProviderOutcome, String> {
    match provider {
        ImageProvider::ChatGptImage2 => call_openai(client, args, session_dir),
        ImageProvider::ReplicateZImageTurbo => call_replicate(client, args),
        ImageProvider::Gemini31Flash => call_gemini(client, args, session_dir),
        ImageProvider::Grok3 => call_grok(client, args, session_dir),
    }
}

fn call_openai(
    client: &Client,
    args: &GenerateMediaArgs,
    session_dir: &Path,
) -> Result<ProviderOutcome, String> {
    let auth_candidates = openai_auth_candidates()?;
    let model = provider_model(ImageProvider::ChatGptImage2);
    let edit = !args.references.is_empty();
    let endpoint = provider_endpoint(ImageProvider::ChatGptImage2, edit);
    let has_api_key_fallback = auth_candidates.iter().any(|auth| !auth.is_codex_oauth());
    let mut oauth_error = None;
    let mut value = None;

    for auth in &auth_candidates {
        let result = match auth {
            OpenAiAuth::CodexOAuth { .. } => send_codex_image_generation(
                client,
                auth,
                args,
                session_dir,
                &codex_responses_model(),
            ),
            OpenAiAuth::ApiKey(_) if edit => {
                send_openai_edit(client, &endpoint, auth, args, session_dir, &model)
            }
            OpenAiAuth::ApiKey(_) => send_openai_generation(client, &endpoint, auth, args, &model),
        };
        match result {
            Ok(reply) => {
                value = Some(reply);
                break;
            }
            Err(err)
                if auth.is_codex_oauth()
                    && has_api_key_fallback
                    && matches!(
                        err.status,
                        Some(StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN)
                    ) =>
            {
                oauth_error = Some(err.message);
            }
            Err(err) => {
                let suffix = oauth_error
                    .map(|previous| format!("; previous Codex OAuth attempt: {previous}"))
                    .unwrap_or_default();
                return Err(format!("{}{suffix}", err.message));
            }
        }
    }

    let value = value.ok_or_else(|| {
        oauth_error.unwrap_or_else(|| "OpenAI image generation failed before request".to_string())
    })?;
    let outcome_model = value
        .get("_model")
        .and_then(Value::as_str)
        .unwrap_or(&model)
        .to_string();
    let images = images_from_openai_like_response(&value, client, &args.output_format)?;
    Ok(ProviderOutcome {
        provider: ImageProvider::ChatGptImage2,
        model: outcome_model,
        images,
        raw: value,
    })
}

fn send_codex_image_generation(
    client: &Client,
    auth: &OpenAiAuth,
    args: &GenerateMediaArgs,
    session_dir: &Path,
    model: &str,
) -> Result<Value, HttpJsonError> {
    let request = apply_openai_auth(
        client
            .post(codex_responses_endpoint())
            .header(reqwest::header::ACCEPT, "text/event-stream")
            .json(&codex_responses_payload(args, session_dir, model)?),
        auth,
    );
    let response = request.send().map_err(|err| HttpJsonError {
        message: format!("Codex hosted image generation failed: {err}"),
        status: err.status(),
    })?;
    parse_codex_responses_response(response, args, model)
}

fn send_openai_generation(
    client: &Client,
    endpoint: &str,
    auth: &OpenAiAuth,
    args: &GenerateMediaArgs,
    model: &str,
) -> Result<Value, HttpJsonError> {
    let request = apply_openai_auth(
        client
            .post(endpoint)
            .json(&openai_json_payload(args, model)?),
        auth,
    );
    send_json_status(request, "OpenAI image generation failed")
}

fn send_openai_edit(
    client: &Client,
    endpoint: &str,
    auth: &OpenAiAuth,
    args: &GenerateMediaArgs,
    session_dir: &Path,
    model: &str,
) -> Result<Value, HttpJsonError> {
    let mut form = multipart::Form::new()
        .text("model", model.to_string())
        .text("prompt", render_prompt(args))
        .text("n", args.count.to_string())
        .text("size", openai_size(args)?)
        .text("output_format", args.output_format.clone());
    if args.quality != "auto" {
        form = form.text("quality", args.quality.clone());
    }
    for reference in &args.references {
        let (name, bytes) = reference_part_bytes(reference, session_dir)?;
        form = form.part("image", multipart::Part::bytes(bytes).file_name(name));
    }

    let request = apply_openai_auth(client.post(endpoint).multipart(form), auth);
    send_json_status(request, "OpenAI image edit failed")
}

fn apply_openai_auth(request: RequestBuilder, auth: &OpenAiAuth) -> RequestBuilder {
    let request = request.bearer_auth(auth.token());
    match auth {
        OpenAiAuth::CodexOAuth { account_id, .. } => {
            let request = request.header("originator", "codex_cli_rs");
            if let Some(account_id) = account_id {
                request.header("chatgpt-account-id", account_id)
            } else {
                request
            }
        }
        OpenAiAuth::ApiKey(_) => request,
    }
}

fn codex_responses_endpoint() -> String {
    env_value("TURA_GENERATE_MEDIA_CODEX_RESPONSES_ENDPOINT")
        .or_else(|| env_value("TURA_GENERATE_MEDIA_OPENAI_CODEX_ENDPOINT"))
        .or_else(|| env_value("TURA_IMAGE_GENERATE_CODEX_RESPONSES_ENDPOINT"))
        .or_else(|| env_value("TURA_IMAGE_GENERATE_OPENAI_CODEX_ENDPOINT"))
        .or_else(|| env_value("OPENAI_CODEX_ENDPOINT"))
        .map(|value| {
            if value.ends_with("/responses") {
                value
            } else {
                format!("{}/responses", value.trim_end_matches('/'))
            }
        })
        .unwrap_or_else(|| "https://chatgpt.com/backend-api/codex/responses".to_string())
}

fn codex_responses_model() -> String {
    env_value("TURA_GENERATE_MEDIA_CODEX_MODEL")
        .or_else(|| env_value("TURA_IMAGE_GENERATE_CODEX_MODEL"))
        .or_else(|| env_value("TURA_CODEX_MODEL"))
        .unwrap_or_else(|| "gpt-5.5".to_string())
}

fn call_replicate(client: &Client, args: &GenerateMediaArgs) -> Result<ProviderOutcome, String> {
    let key = provider_key(ImageProvider::ReplicateZImageTurbo)?;
    let model = provider_model(ImageProvider::ReplicateZImageTurbo);
    let endpoint = provider_endpoint(ImageProvider::ReplicateZImageTurbo, false);
    let value = client
        .post(endpoint)
        .bearer_auth(key)
        .header("Prefer", "wait")
        .json(&replicate_payload(args)?)
        .send()
        .and_then(|reply| reply.error_for_status())
        .map_err(|err| format!("Replicate Z-Image Turbo failed: {err}"))?
        .json::<Value>()
        .map_err(|err| format!("failed to parse Replicate response: {err}"))?;
    let output = value.get("output").unwrap_or(&value);
    let urls = match output {
        Value::String(url) => vec![url.clone()],
        Value::Array(items) => items
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_string)
            .collect::<Vec<_>>(),
        _ => Vec::new(),
    };
    if urls.is_empty() {
        return Err("Replicate response did not include image URL output".to_string());
    }
    let images = urls
        .iter()
        .map(|url| download_image(client, url, &args.output_format))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(ProviderOutcome {
        provider: ImageProvider::ReplicateZImageTurbo,
        model,
        images,
        raw: value,
    })
}

fn call_gemini(
    client: &Client,
    args: &GenerateMediaArgs,
    session_dir: &Path,
) -> Result<ProviderOutcome, String> {
    let key = provider_key(ImageProvider::Gemini31Flash)?;
    let model = provider_model(ImageProvider::Gemini31Flash);
    let value = client
        .post(provider_endpoint(ImageProvider::Gemini31Flash, false))
        .header("x-goog-api-key", key)
        .json(&gemini_payload(args, session_dir)?);
    let value = send_json(value, "Gemini image generation failed")?;
    let images = images_from_gemini_response(&value, &args.output_format)?;
    Ok(ProviderOutcome {
        provider: ImageProvider::Gemini31Flash,
        model,
        images,
        raw: value,
    })
}

fn call_grok(
    client: &Client,
    args: &GenerateMediaArgs,
    session_dir: &Path,
) -> Result<ProviderOutcome, String> {
    let key = provider_key(ImageProvider::Grok3)?;
    let model = provider_model(ImageProvider::Grok3);
    let edit = !args.references.is_empty();
    let value = client
        .post(provider_endpoint(ImageProvider::Grok3, edit))
        .bearer_auth(key)
        .json(&grok_payload(args, session_dir)?)
        .send()
        .and_then(|reply| reply.error_for_status())
        .map_err(|err| format!("xAI Grok image generation failed: {err}"))?
        .json::<Value>()
        .map_err(|err| format!("failed to parse xAI image response: {err}"))?;
    let images = images_from_openai_like_response(&value, client, &args.output_format)?;
    Ok(ProviderOutcome {
        provider: ImageProvider::Grok3,
        model,
        images,
        raw: value,
    })
}

fn openai_json_payload(args: &GenerateMediaArgs, model: &str) -> Result<Value, String> {
    let mut payload = json!({
        "model": model,
        "prompt": render_prompt(args),
        "n": args.count,
        "size": openai_size(args)?,
        "output_format": args.output_format,
    });
    if args.quality != "auto" {
        payload["quality"] = Value::String(args.quality.clone());
    }
    if let Some(extra) = args.extra_body.as_ref() {
        merge_extra(&mut payload, extra);
    }
    Ok(payload)
}

fn codex_responses_payload(
    args: &GenerateMediaArgs,
    session_dir: &Path,
    model: &str,
) -> Result<Value, String> {
    let mut content = vec![json!({
        "type": "input_text",
        "text": codex_image_prompt(args)?,
    })];
    for reference in &args.references {
        content.push(json!({
            "type": "input_image",
            "image_url": reference_data_url(reference, session_dir)?,
            "detail": "auto",
        }));
    }

    let mut payload = json!({
        "model": model,
        "instructions": "Use the hosted image_generation tool to create exactly one image from the user's request. Return no extra text.",
        "input": [{
            "type": "message",
            "role": "user",
            "content": content,
        }],
        "tools": [{
            "type": "image_generation",
            "output_format": args.output_format,
        }],
        "tool_choice": "auto",
        "parallel_tool_calls": false,
        "store": false,
        "stream": true,
    });
    if let Some(extra) = args.extra_body.as_ref() {
        merge_extra(&mut payload, extra);
    }
    Ok(payload)
}

fn codex_image_prompt(args: &GenerateMediaArgs) -> Result<String, String> {
    let mut prompt = render_prompt(args);
    prompt.push_str("\n\nGenerate exactly one image.");
    if args.width.is_some()
        || args.height.is_some()
        || args.size.is_some()
        || args.aspect_ratio.is_some()
    {
        prompt.push_str(&format!(
            "\nCanvas constraints: target aspect ratio {}; requested image size class {}.",
            aspect_ratio(args)?,
            image_size_label(args)?
        ));
    }
    if let Some(seed) = args.seed {
        prompt.push_str(&format!("\nSeed hint: {seed}."));
    }
    if args.quality != "auto" {
        prompt.push_str(&format!("\nQuality hint: {}.", args.quality));
    }
    Ok(prompt)
}

fn replicate_payload(args: &GenerateMediaArgs) -> Result<Value, String> {
    let dims = provider_dimensions(args, 1440)?;
    let mut input = json!({
        "prompt": render_prompt(args),
        "width": dims.width,
        "height": dims.height,
        "output_format": if args.output_format == "jpeg" { "jpg" } else { &args.output_format },
        "output_quality": quality_to_percent(&args.quality),
        "guidance_scale": 0,
    });
    if let Some(seed) = args.seed {
        input["seed"] = json!(seed);
    }
    let mut payload = json!({ "input": input });
    if let Some(extra) = args.extra_body.as_ref() {
        merge_extra(&mut payload, extra);
    }
    Ok(payload)
}

fn gemini_payload(args: &GenerateMediaArgs, session_dir: &Path) -> Result<Value, String> {
    let mut parts = vec![json!({ "text": gemini_prompt(args)? })];
    for reference in &args.references {
        let data_url = reference_data_url(reference, session_dir)?;
        let (mime_type, data) = split_data_url(&data_url)?;
        parts.push(json!({ "inline_data": { "mime_type": mime_type, "data": data } }));
    }
    let mut payload = json!({ "contents": [{ "parts": parts }] });
    if let Some(extra) = args.extra_body.as_ref() {
        merge_extra(&mut payload, extra);
    }
    Ok(payload)
}

fn gemini_prompt(args: &GenerateMediaArgs) -> Result<String, String> {
    let mut prompt = render_prompt(args);
    if args.width.is_some()
        || args.height.is_some()
        || args.size.is_some()
        || args.aspect_ratio.is_some()
    {
        prompt.push_str(&format!(
            "\n\nCanvas constraints: target aspect ratio {}; requested image size class {}.",
            aspect_ratio(args)?,
            image_size_label(args)?
        ));
    }
    Ok(prompt)
}

fn grok_payload(args: &GenerateMediaArgs, session_dir: &Path) -> Result<Value, String> {
    let model = provider_model(ImageProvider::Grok3);
    let mut payload = json!({
        "model": model,
        "prompt": render_prompt(args),
        "n": args.count,
        "response_format": "b64_json",
        "aspect_ratio": aspect_ratio(args)?,
        "resolution": if image_size_label(args)? == "2K" { "2k" } else { "1k" },
    });
    if !args.references.is_empty() {
        let refs = args
            .references
            .iter()
            .map(|reference| {
                reference_data_url(reference, session_dir).map(|url| json!({ "url": url }))
            })
            .collect::<Result<Vec<_>, _>>()?;
        if refs.len() == 1 {
            payload["image"] = refs[0].clone();
        } else {
            payload["images"] = Value::Array(refs);
        }
    }
    if let Some(extra) = args.extra_body.as_ref() {
        merge_extra(&mut payload, extra);
    }
    Ok(payload)
}

fn images_from_openai_like_response(
    value: &Value,
    client: &Client,
    output_format: &str,
) -> Result<Vec<ImageBytes>, String> {
    let items = value
        .get("data")
        .and_then(Value::as_array)
        .ok_or_else(|| "image response missing data array".to_string())?;
    let mut images = Vec::new();
    for item in items {
        if let Some(encoded) = item.get("b64_json").and_then(Value::as_str) {
            images.push(ImageBytes {
                bytes: general_purpose::STANDARD
                    .decode(encoded)
                    .map_err(|err| format!("invalid image base64: {err}"))?,
                mime_type: item
                    .get("mime_type")
                    .and_then(Value::as_str)
                    .map(str::to_string)
                    .unwrap_or_else(|| mime_type_for_format(output_format)),
                source_url: None,
            });
        } else if let Some(url) = item.get("url").and_then(Value::as_str) {
            images.push(download_image(client, url, output_format)?);
        }
    }
    if images.is_empty() {
        Err("image response did not include b64_json or url images".to_string())
    } else {
        Ok(images)
    }
}

fn parse_codex_responses_response(
    response: Response,
    args: &GenerateMediaArgs,
    model: &str,
) -> Result<Value, HttpJsonError> {
    let status = response.status();
    let text = response.text().map_err(|err| HttpJsonError {
        message: format!(
            "Codex hosted image generation failed: failed to read response body: {err}"
        ),
        status: Some(status),
    })?;
    if !status.is_success() {
        return Err(HttpJsonError {
            message: format!(
                "Codex hosted image generation failed: HTTP {status}: {}",
                truncate_body(&text)
            ),
            status: Some(status),
        });
    }

    let mime_type = mime_type_for_format(&args.output_format);
    let mut results = Vec::new();
    let raw = if let Ok(value) = serde_json::from_str::<Value>(&text) {
        collect_codex_image_results(&value, &mut results);
        value
    } else {
        let mut events = Vec::new();
        for line in text.lines() {
            let Some(data) = line.trim_start().strip_prefix("data:") else {
                continue;
            };
            let data = data.trim();
            if data.is_empty() || data == "[DONE]" {
                continue;
            }
            let event = serde_json::from_str::<Value>(data).map_err(|err| HttpJsonError {
                message: format!("Codex hosted image generation failed: invalid SSE JSON: {err}"),
                status: Some(status),
            })?;
            collect_codex_image_results(&event, &mut results);
            events.push(event);
        }
        json!({ "events": events })
    };

    if results.is_empty() {
        return Err(HttpJsonError {
            message: "Codex hosted image generation response did not include image_generation_call.result".to_string(),
            status: Some(status),
        });
    }

    let data = results
        .into_iter()
        .map(|encoded| json!({ "b64_json": encoded, "mime_type": mime_type }))
        .collect::<Vec<_>>();
    Ok(json!({
        "data": data,
        "_model": model,
        "codex_responses": raw,
    }))
}

fn collect_codex_image_results(value: &Value, results: &mut Vec<String>) {
    match value {
        Value::Object(map) => {
            if map.get("type").and_then(Value::as_str) == Some("image_generation_call") {
                if let Some(result) = map.get("result").and_then(Value::as_str) {
                    results.push(result.to_string());
                }
            }
            for nested in map.values() {
                collect_codex_image_results(nested, results);
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_codex_image_results(item, results);
            }
        }
        _ => {}
    }
}

fn images_from_gemini_response(
    value: &Value,
    output_format: &str,
) -> Result<Vec<ImageBytes>, String> {
    let mut images = Vec::new();
    let candidates = value
        .get("candidates")
        .and_then(Value::as_array)
        .ok_or_else(|| "Gemini response missing candidates".to_string())?;
    for candidate in candidates {
        let Some(parts) = candidate
            .get("content")
            .and_then(|content| content.get("parts"))
            .and_then(Value::as_array)
        else {
            continue;
        };
        for part in parts {
            let inline = part.get("inlineData").or_else(|| part.get("inline_data"));
            let Some(inline) = inline else { continue };
            let Some(data) = inline.get("data").and_then(Value::as_str) else {
                continue;
            };
            let mime_type = inline
                .get("mimeType")
                .or_else(|| inline.get("mime_type"))
                .and_then(Value::as_str)
                .map(str::to_string)
                .unwrap_or_else(|| mime_type_for_format(output_format));
            images.push(ImageBytes {
                bytes: general_purpose::STANDARD
                    .decode(data)
                    .map_err(|err| format!("invalid Gemini image base64: {err}"))?,
                mime_type,
                source_url: None,
            });
        }
    }
    if images.is_empty() {
        Err("Gemini response did not include inline image data".to_string())
    } else {
        Ok(images)
    }
}

fn download_image(client: &Client, url: &str, output_format: &str) -> Result<ImageBytes, String> {
    let response = client
        .get(url)
        .send()
        .and_then(|reply| reply.error_for_status())
        .map_err(|err| format!("failed to download generated image {url}: {err}"))?;
    let mime_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string)
        .unwrap_or_else(|| mime_type_for_format(output_format));
    let bytes = response
        .bytes()
        .map_err(|err| format!("failed to read generated image bytes: {err}"))?
        .to_vec();
    Ok(ImageBytes {
        bytes,
        mime_type,
        source_url: Some(url.to_string()),
    })
}

fn send_json(request: RequestBuilder, label: &str) -> Result<Value, String> {
    send_json_status(request, label).map_err(|err| err.message)
}

fn send_json_status(request: RequestBuilder, label: &str) -> Result<Value, HttpJsonError> {
    let response = request.send().map_err(|err| HttpJsonError {
        message: format!("{label}: {err}"),
        status: err.status(),
    })?;
    parse_json_response(response, label)
}

fn parse_json_response(response: Response, label: &str) -> Result<Value, HttpJsonError> {
    let status = response.status();
    let text = response.text().map_err(|err| HttpJsonError {
        message: format!("{label}: failed to read response body: {err}"),
        status: Some(status),
    })?;
    if !status.is_success() {
        return Err(HttpJsonError {
            message: format!("{label}: HTTP {status}: {}", truncate_body(&text)),
            status: Some(status),
        });
    }
    serde_json::from_str::<Value>(&text).map_err(|err| HttpJsonError {
        message: format!("{label}: invalid JSON: {err}"),
        status: Some(status),
    })
}

fn truncate_body(text: &str) -> String {
    const MAX: usize = 1200;
    if text.len() <= MAX {
        text.to_string()
    } else {
        format!("{}...", &text[..MAX])
    }
}

fn split_data_url(data_url: &str) -> Result<(String, String), String> {
    let Some(rest) = data_url.strip_prefix("data:") else {
        return Err("reference image must be local path, URL, or data URL".to_string());
    };
    let Some((metadata, data)) = rest.split_once(',') else {
        return Err("invalid data URL reference".to_string());
    };
    let mime_type = metadata
        .split(';')
        .next()
        .filter(|value| value.starts_with("image/"))
        .unwrap_or("image/png")
        .to_string();
    Ok((mime_type, data.to_string()))
}

fn quality_to_percent(value: &str) -> u8 {
    match value {
        "low" => 60,
        "medium" | "standard" | "auto" => 80,
        "high" | "hd" => 95,
        _ => 80,
    }
}

fn merge_extra(target: &mut Value, extra: &Value) {
    let (Some(target), Some(extra)) = (target.as_object_mut(), extra.as_object()) else {
        return;
    };
    for (key, value) in extra {
        target.insert(key.clone(), value.clone());
    }
}
