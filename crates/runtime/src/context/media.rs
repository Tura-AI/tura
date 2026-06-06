use super::text_truncate::command_run_truncate_text;
use super::tool_results::{command_run_current_style_output_string, flattened_command_run_results};

pub(crate) fn command_run_media_content_items_for_context(
    value: &serde_json::Value,
) -> Option<Vec<serde_json::Value>> {
    let output = value.get("output").unwrap_or(&serde_json::Value::Null);
    let results = flattened_command_run_results(output);
    if !results.iter().any(|result| {
        result
            .get("command_type")
            .or_else(|| result.get("command"))
            .and_then(serde_json::Value::as_str)
            == Some("read_media")
    }) {
        return None;
    }

    let mut media_output = command_run_current_style_output_string_without_media_data(value)?;
    media_output = command_run_truncate_text(&media_output, 8_000, None);
    let mut content = vec![serde_json::json!({
        "type": "input_text",
        "text": media_output,
    })];
    for image_url in command_run_media_image_urls(value).into_iter().take(24) {
        content.push(serde_json::json!({
            "type": "input_image",
            "image_url": image_url,
        }));
    }
    let audio_preview_count = command_run_media_audio_preview_count(value);
    if audio_preview_count > 0 {
        content.push(serde_json::json!({
            "type": "input_text",
            "text": format!(
                "[Audio media omitted: {audio_preview_count} compressed audio preview(s) were produced by read_media, but the current Responses provider does not support audio input. Use the visual previews, metadata, and any extracted text instead.]"
            ),
        }));
    }
    for input_file in command_run_media_input_files(value).into_iter().take(8) {
        content.push(input_file);
    }
    Some(content)
}

fn command_run_current_style_output_string_without_media_data(
    value: &serde_json::Value,
) -> Option<String> {
    let mut value = value.clone();
    if let Some(output) = value.get_mut("output") {
        strip_read_media_payload_data(output);
    }
    command_run_current_style_output_string(&value)
}

fn strip_read_media_payload_data(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Object(object) => {
            if object.contains_key("visual_previews") {
                if let Some(count) = object.get("visual_preview_count").cloned() {
                    object.insert(
                        "visual_previews".to_string(),
                        serde_json::json!({ "omitted_from_text": true, "count": count }),
                    );
                } else {
                    object.remove("visual_previews");
                }
            }
            if object.contains_key("audio_previews") {
                if let Some(count) = object.get("audio_preview_count").cloned() {
                    object.insert(
                        "audio_previews".to_string(),
                        serde_json::json!({ "omitted_from_text": true, "count": count }),
                    );
                } else {
                    object.remove("audio_previews");
                }
            }
            if object.contains_key("file_attachments") {
                if let Some(count) = object.get("file_attachment_count").cloned() {
                    object.insert(
                        "file_attachments".to_string(),
                        serde_json::json!({ "omitted_from_text": true, "count": count }),
                    );
                } else {
                    object.remove("file_attachments");
                }
            }
            for child in object.values_mut() {
                strip_read_media_payload_data(child);
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                strip_read_media_payload_data(item);
            }
        }
        _ => {}
    }
}

fn command_run_media_image_urls(value: &serde_json::Value) -> Vec<String> {
    let mut urls = Vec::new();
    collect_command_run_media_image_urls(value.get("output").unwrap_or(value), &mut urls);
    urls
}

fn collect_command_run_media_image_urls(value: &serde_json::Value, urls: &mut Vec<String>) {
    match value {
        serde_json::Value::Object(object) => {
            if object.get("type").and_then(serde_json::Value::as_str) == Some("image_url") {
                if let Some(url) = object
                    .get("image_url")
                    .and_then(|image_url| image_url.get("url"))
                    .and_then(serde_json::Value::as_str)
                {
                    urls.push(url.to_string());
                }
            }
            for child in object.values() {
                collect_command_run_media_image_urls(child, urls);
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                collect_command_run_media_image_urls(item, urls);
            }
        }
        _ => {}
    }
}

fn command_run_media_input_files(value: &serde_json::Value) -> Vec<serde_json::Value> {
    let mut inputs = Vec::new();
    collect_command_run_media_input_files(value.get("output").unwrap_or(value), &mut inputs);
    inputs
}

fn collect_command_run_media_input_files(
    value: &serde_json::Value,
    inputs: &mut Vec<serde_json::Value>,
) {
    match value {
        serde_json::Value::Object(object) => {
            if object.get("type").and_then(serde_json::Value::as_str) == Some("file") {
                if let Some(data) = object
                    .get("data_base64")
                    .and_then(serde_json::Value::as_str)
                {
                    let file_name = object
                        .get("file_name")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or("attachment");
                    let mime_type = object
                        .get("mime_type")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or("application/octet-stream");
                    if mime_type == "application/octet-stream" {
                        return;
                    }
                    inputs.push(serde_json::json!({
                        "type": "input_file",
                        "filename": file_name,
                        "file_data": format!("data:{mime_type};base64,{data}"),
                    }));
                }
            }
            for child in object.values() {
                collect_command_run_media_input_files(child, inputs);
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                collect_command_run_media_input_files(item, inputs);
            }
        }
        _ => {}
    }
}

fn command_run_media_audio_preview_count(value: &serde_json::Value) -> usize {
    let mut count = 0;
    collect_command_run_media_audio_preview_count(value.get("output").unwrap_or(value), &mut count);
    count
}

fn collect_command_run_media_audio_preview_count(value: &serde_json::Value, count: &mut usize) {
    match value {
        serde_json::Value::Object(object) => {
            if object.get("type").and_then(serde_json::Value::as_str) == Some("audio_url") {
                *count += 1;
            }
            for child in object.values() {
                collect_command_run_media_audio_preview_count(child, count);
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                collect_command_run_media_audio_preview_count(item, count);
            }
        }
        _ => {}
    }
}
