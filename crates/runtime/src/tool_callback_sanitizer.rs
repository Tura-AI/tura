use serde_json::Value;

const TOOL_CALLBACK_OUTPUT_MAX_CHARS: usize = 10_000;
const MEDIA_CHANNEL_REASON: &str = "media payload is sent through the provider media channel";

pub(crate) fn sanitize_tool_callback_output(value: &Value) -> Value {
    sanitize_value(value, None)
}

pub(crate) fn sanitize_tool_callback_result(value: &Value) -> Value {
    sanitize_value(value, None)
}

fn sanitize_value(value: &Value, key: Option<&str>) -> Value {
    match value {
        Value::String(text) if is_media_payload_string(key, text) => {
            redact_media_payload_string(key)
        }
        Value::String(text) if should_truncate_string(key, text) => {
            Value::String(truncate_middle(text, TOOL_CALLBACK_OUTPUT_MAX_CHARS))
        }
        Value::String(_) | Value::Null | Value::Bool(_) | Value::Number(_) => value.clone(),
        Value::Array(items) => {
            Value::Array(items.iter().map(|item| sanitize_value(item, key)).collect())
        }
        Value::Object(map) => sanitize_object(map),
    }
}

fn sanitize_object(map: &serde_json::Map<String, Value>) -> Value {
    let mut sanitized = serde_json::Map::new();
    let has_results = map.get("results").is_some();

    for (field, item) in map {
        let value = match field.as_str() {
            "visual_previews" => media_channel_summary(map, field, "visual_preview_count", item),
            "audio_previews" => media_channel_summary(map, field, "audio_preview_count", item),
            "file_attachments" => media_channel_summary(map, field, "file_attachment_count", item),
            "command_events" if has_results => serde_json::json!({
                "omitted_from_record": true,
                "count": array_len_value(item),
                "reason": "command event payloads are represented by the canonical results channel",
            }),
            _ => sanitize_value(item, Some(field.as_str())),
        };
        sanitized.insert(field.clone(), value);
    }

    Value::Object(sanitized)
}

fn media_channel_summary(
    map: &serde_json::Map<String, Value>,
    field: &str,
    count_field: &str,
    value: &Value,
) -> Value {
    serde_json::json!({
        "omitted_from_record": true,
        "media_channel": field,
        "count": map
            .get(count_field)
            .cloned()
            .unwrap_or_else(|| array_len_value(value)),
        "reason": MEDIA_CHANNEL_REASON,
    })
}

fn array_len_value(value: &Value) -> Value {
    value
        .as_array()
        .map(|items| Value::Number(items.len().into()))
        .unwrap_or(Value::Null)
}

fn should_truncate_string(key: Option<&str>, text: &str) -> bool {
    if text.len() <= TOOL_CALLBACK_OUTPUT_MAX_CHARS {
        return false;
    }
    !matches!(
        key,
        Some(
            "command"
                | "command_line"
                | "input"
                | "provider"
                | "runtime_id"
                | "session_id"
                | "id"
                | "call_id"
                | "tool"
                | "tool_name"
        )
    )
}

fn is_media_payload_string(key: Option<&str>, text: &str) -> bool {
    if matches!(key, Some("data_base64")) && !text.is_empty() {
        return true;
    }
    if matches!(
        key,
        Some("file_data" | "image_url" | "url" | "audio_url" | "data")
    ) {
        return contains_base64_data_url(text);
    }
    false
}

fn contains_base64_data_url(text: &str) -> bool {
    text.contains("data:") && text.contains(";base64,")
}

fn redact_media_payload_string(key: Option<&str>) -> Value {
    let label = match key {
        Some("data_base64") => "[redacted base64 media payload]",
        Some("file_data") => "[redacted media file data URL]",
        Some("audio_url") => "[redacted audio media data URL]",
        Some("image_url" | "url") => "[redacted image media data URL]",
        Some("data") => "[redacted media data URL]",
        _ => "[redacted media payload]",
    };
    Value::String(label.to_string())
}

fn truncate_middle(content: &str, max_chars: usize) -> String {
    let total_lines = content.lines().count();
    let keep_each_side = max_chars / 2;
    let mut head_end = 0usize;
    for (count, (index, ch)) in content.char_indices().enumerate() {
        if count >= keep_each_side {
            break;
        }
        head_end = index + ch.len_utf8();
    }
    let mut tail_start = content.len();
    for (count, (index, _)) in content.char_indices().rev().enumerate() {
        if count >= keep_each_side {
            break;
        }
        tail_start = index;
    }
    if head_end >= tail_start {
        return content.to_string();
    }
    let removed = tail_start.saturating_sub(head_end);
    format!(
        "Total output lines: {total_lines}\n\n{}...{removed} characters truncated...{}",
        &content[..head_end],
        &content[tail_start..]
    )
}

#[cfg(test)]
mod tests {
    use super::sanitize_tool_callback_output;
    use serde_json::json;

    #[test]
    fn sanitizer_truncates_large_output_strings_but_keeps_command_line() {
        let large = "line\n".repeat(4_000);
        let value = json!({
            "command_line": large,
            "output": large,
        });

        let sanitized = sanitize_tool_callback_output(&value);

        assert_eq!(sanitized["command_line"], value["command_line"]);
        let output = sanitized["output"].as_str().expect("output string");
        assert!(output.len() < large.len(), "{output}");
        assert!(output.contains("characters truncated"), "{output}");
    }

    #[test]
    fn sanitizer_redacts_media_payloads_to_single_channel_summaries() {
        let image_url = format!("data:image/jpeg;base64,{}", "A".repeat(20_000));
        let audio_url = format!("data:audio/mpeg;base64,{}", "B".repeat(20_000));
        let file_data = format!("data:application/pdf;base64,{}", "C".repeat(20_000));
        let data_base64 = "D".repeat(20_000);
        let value = json!({
            "output": "ordinary\n".repeat(4_000),
            "media_results": [{
                "visual_previews": [{
                    "type": "image_url",
                    "image_url": { "url": image_url }
                }],
                "audio_previews": [{
                    "type": "audio_url",
                    "audio_url": { "url": audio_url }
                }],
                "file_attachments": [{
                    "mime_type": "application/pdf",
                    "data_base64": data_base64
                }]
            }],
            "input_file": {
                "file_data": file_data
            }
        });

        let sanitized = sanitize_tool_callback_output(&value);

        assert_eq!(
            sanitized["media_results"][0]["visual_previews"]["media_channel"],
            "visual_previews"
        );
        assert_eq!(
            sanitized["media_results"][0]["audio_previews"]["media_channel"],
            "audio_previews"
        );
        assert_eq!(
            sanitized["media_results"][0]["file_attachments"]["media_channel"],
            "file_attachments"
        );
        assert_eq!(
            sanitized["input_file"]["file_data"],
            "[redacted media file data URL]"
        );
        let serialized = serde_json::to_string(&sanitized).expect("sanitized json");
        assert!(!serialized.contains("data:image/jpeg;base64"));
        assert!(!serialized.contains("data:audio/mpeg;base64"));
        assert!(!serialized.contains("data:application/pdf;base64"));
        assert!(!serialized.contains(&"D".repeat(1_000)));
        assert!(sanitized["output"]
            .as_str()
            .is_some_and(|output| output.contains("characters truncated")));
    }

    #[test]
    fn sanitizer_counts_command_events_only_once_when_results_are_present() {
        let value = json!({
            "results": [{
                "step": 1,
                "command_type": "read_media",
                "output": {
                    "visual_previews": [{
                        "type": "image_url",
                        "image_url": { "url": "data:image/png;base64,AAA" }
                    }]
                }
            }],
            "command_events": [{
                "result": {
                    "output": {
                        "visual_previews": [{
                            "type": "image_url",
                            "image_url": { "url": "data:image/png;base64,AAA" }
                        }]
                    }
                }
            }]
        });

        let sanitized = sanitize_tool_callback_output(&value);

        assert_eq!(sanitized["command_events"]["omitted_from_record"], true);
        assert_eq!(sanitized["command_events"]["count"], 1);
        let serialized = serde_json::to_string(&sanitized).expect("sanitized json");
        assert!(!serialized.contains("data:image/png;base64"));
    }
}
