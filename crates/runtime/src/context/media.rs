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

pub(super) fn strip_read_media_payload_data(value: &mut serde_json::Value) {
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
    let mut seen = std::collections::HashSet::new();
    urls.retain(|url| seen.insert(url.clone()));
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
    let mut seen = std::collections::HashSet::new();
    inputs.retain(|input| {
        let key = serde_json::to_string(input).unwrap_or_else(|_| input.to_string());
        seen.insert(key)
    });
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
    let mut previews = Vec::new();
    collect_command_run_media_audio_previews(value.get("output").unwrap_or(value), &mut previews);
    let mut seen = std::collections::HashSet::new();
    previews
        .into_iter()
        .filter(|preview| seen.insert(preview.clone()))
        .count()
}

fn collect_command_run_media_audio_previews(value: &serde_json::Value, previews: &mut Vec<String>) {
    match value {
        serde_json::Value::Object(object) => {
            if object.get("type").and_then(serde_json::Value::as_str) == Some("audio_url") {
                previews.push(serde_json::to_string(value).unwrap_or_else(|_| value.to_string()));
            }
            for child in object.values() {
                collect_command_run_media_audio_previews(child, previews);
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                collect_command_run_media_audio_previews(item, previews);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::{
        command_run_media_audio_preview_count, command_run_media_content_items_for_context,
        command_run_media_image_urls, command_run_media_input_files,
    };
    use serde_json::json;

    #[test]
    fn non_read_media_command_run_does_not_create_media_context_items() {
        let value = json!({
            "tool_name": "command_run",
            "input": {
                "commands": [
                    {"step": 1, "command_type": "shell_command", "command_line": "echo ok"}
                ]
            },
            "output": {
                "results": [{
                    "step": 1,
                    "command_type": "shell_command",
                    "response": {"ok": true, "stdout": "ok\n", "stderr": ""}
                }]
            }
        });

        assert!(command_run_media_content_items_for_context(&value).is_none());
        assert!(command_run_media_image_urls(&value).is_empty());
        assert!(command_run_media_input_files(&value).is_empty());
        assert_eq!(command_run_media_audio_preview_count(&value), 0);
    }

    #[test]
    fn read_media_context_strips_large_payload_data_but_keeps_media_references() {
        let value = json!({
            "tool_name": "command_run",
            "input": {
                "commands": [
                    {"step": 1, "command_type": "read_media", "command_line": "read_media image.png"}
                ]
            },
            "output": {
                "results": [{
                    "step": 1,
                    "command_type": "read_media",
                    "output": {
                        "summary": "image and document previews",
                        "visual_preview_count": 2,
                        "visual_previews": [
                            {"type": "image_url", "image_url": {"url": "data:image/png;base64,AAA"}},
                            {"nested": {"type": "image_url", "image_url": {"url": "https://cdn.example.com/preview.jpg"}}}
                        ],
                        "audio_preview_count": 1,
                        "audio_previews": [
                            {"type": "audio_url", "audio_url": {"url": "data:audio/wav;base64,BBB"}}
                        ],
                        "file_attachment_count": 2,
                        "file_attachments": [
                            {
                                "type": "file",
                                "file_name": "report.pdf",
                                "mime_type": "application/pdf",
                                "data_base64": "PDFDATA"
                            },
                            {
                                "type": "file",
                                "file_name": "raw.bin",
                                "mime_type": "application/octet-stream",
                                "data_base64": "BINDATA"
                            }
                        ]
                    },
                    "response": {
                        "ok": true,
                        "stdout": "",
                        "stderr": "",
                        "output": {
                            "summary": "image and document previews",
                            "visual_preview_count": 2,
                            "visual_previews": [
                                {"type": "image_url", "image_url": {"url": "data:image/png;base64,AAA"}},
                                {"nested": {"type": "image_url", "image_url": {"url": "https://cdn.example.com/preview.jpg"}}}
                            ],
                            "audio_preview_count": 1,
                            "audio_previews": [
                                {"type": "audio_url", "audio_url": {"url": "data:audio/wav;base64,BBB"}}
                            ],
                            "file_attachment_count": 2,
                            "file_attachments": [
                                {
                                    "type": "file",
                                    "file_name": "report.pdf",
                                    "mime_type": "application/pdf",
                                    "data_base64": "PDFDATA"
                                },
                                {
                                    "type": "file",
                                    "file_name": "raw.bin",
                                    "mime_type": "application/octet-stream",
                                    "data_base64": "BINDATA"
                                }
                            ]
                        }
                    }
                }]
            }
        });

        let content = command_run_media_content_items_for_context(&value).expect("media content");

        assert_eq!(content[0]["type"], "input_text");
        let text = content[0]["text"].as_str().expect("text item");
        assert!(text.contains("omitted_from_text"));
        assert!(text.contains("\"count\": 2"));
        assert!(!text.contains("AAA"));
        assert!(!text.contains("BBB"));
        assert!(!text.contains("PDFDATA"));
        assert!(!text.contains("BINDATA"));

        assert!(content.iter().any(|item| {
            item["type"] == "input_image" && item["image_url"] == "data:image/png;base64,AAA"
        }));
        assert!(content.iter().any(|item| {
            item["type"] == "input_image"
                && item["image_url"] == "https://cdn.example.com/preview.jpg"
        }));
        assert!(content.iter().any(|item| {
            item["type"] == "input_text"
                && item["text"]
                    .as_str()
                    .is_some_and(|text| text.contains("Audio media omitted: 1"))
        }));
        assert!(content.iter().any(|item| {
            item["type"] == "input_file"
                && item["filename"] == "report.pdf"
                && item["file_data"] == "data:application/pdf;base64,PDFDATA"
        }));
        assert!(!content.iter().any(|item| item["filename"] == "raw.bin"));
    }

    #[test]
    fn read_media_context_limits_images_and_input_files() {
        let image_items = (0..30)
            .map(|index| {
                json!({
                    "type": "image_url",
                    "image_url": {"url": format!("https://cdn.example.com/{index}.jpg")}
                })
            })
            .collect::<Vec<_>>();
        let file_items = (0..12)
            .map(|index| {
                json!({
                    "type": "file",
                    "file_name": format!("file-{index}.txt"),
                    "mime_type": "text/plain",
                    "data_base64": format!("DATA{index}")
                })
            })
            .collect::<Vec<_>>();
        let value = json!({
            "output": {
                "results": [{
                    "step": 1,
                    "command_type": "read_media",
                    "response": {
                        "ok": true,
                        "output": {
                            "visual_previews": image_items,
                            "file_attachments": file_items
                        }
                    }
                }]
            }
        });

        let content = command_run_media_content_items_for_context(&value).expect("media content");
        let image_count = content
            .iter()
            .filter(|item| item["type"] == "input_image")
            .count();
        let file_count = content
            .iter()
            .filter(|item| item["type"] == "input_file")
            .count();

        assert_eq!(image_count, 24);
        assert_eq!(file_count, 8);
        assert!(content
            .iter()
            .any(|item| item["image_url"] == "https://cdn.example.com/23.jpg"));
        assert!(!content
            .iter()
            .any(|item| item["image_url"] == "https://cdn.example.com/24.jpg"));
        assert!(content.iter().any(|item| item["filename"] == "file-7.txt"));
        assert!(!content.iter().any(|item| item["filename"] == "file-8.txt"));
    }
}
