use serde_json::{json, Value};

pub fn text_from_content(content: Option<&Value>) -> Option<String> {
    match content? {
        Value::String(value) => Some(value.clone()),
        Value::Array(items) => {
            let text = items
                .iter()
                .filter_map(|item| {
                    item.get("text")
                        .and_then(Value::as_str)
                        .or_else(|| item.get("content").and_then(Value::as_str))
                })
                .collect::<Vec<_>>()
                .join("\n");
            (!text.trim().is_empty()).then_some(text)
        }
        other if other.is_null() => None,
        other => Some(other.to_string()),
    }
}

pub fn openai_chat_content_from_canonical(content: Option<&Value>) -> Option<Value> {
    match content? {
        Value::String(value) => Some(Value::String(value.clone())),
        Value::Array(items) => {
            let parts = openai_chat_parts_from_canonical_items(items, true);
            if !parts.is_empty() {
                Some(Value::Array(parts))
            } else {
                text_from_content(content).map(Value::String)
            }
        }
        other if other.is_null() => None,
        other => Some(Value::String(other.to_string())),
    }
}

pub fn openai_chat_media_content_from_canonical(content: Option<&Value>) -> Option<Value> {
    let Value::Array(items) = content? else {
        return None;
    };
    let parts = openai_chat_parts_from_canonical_items(items, false);
    (!parts.is_empty()).then_some(Value::Array(parts))
}

pub fn openai_responses_content_from_canonical(content: Option<&Value>) -> Option<Value> {
    match content? {
        Value::String(value) => Some(Value::String(value.clone())),
        Value::Array(items) => {
            let mut parts = Vec::new();
            for item in items {
                match item.get("type").and_then(Value::as_str) {
                    Some("input_text" | "input_image" | "input_file") => parts.push(item.clone()),
                    Some("text") => {
                        if let Some(text) = item
                            .get("text")
                            .and_then(Value::as_str)
                            .or_else(|| item.get("content").and_then(Value::as_str))
                            .filter(|text| !text.trim().is_empty())
                        {
                            parts.push(json!({ "type": "input_text", "text": text }));
                        }
                    }
                    Some("image_url") => {
                        if let Some(url) = canonical_image_url(item) {
                            parts.push(json!({ "type": "input_image", "image_url": url }));
                        }
                    }
                    _ => {}
                }
            }
            if !parts.is_empty() {
                Some(Value::Array(parts))
            } else {
                text_from_content(content).map(Value::String)
            }
        }
        other if other.is_null() => None,
        other => Some(Value::String(other.to_string())),
    }
}

pub fn google_parts_from_canonical(content: Option<&Value>) -> Option<Vec<Value>> {
    match content? {
        Value::String(value) => Some(vec![json!({ "text": value })]),
        Value::Array(items) => {
            let mut parts = Vec::new();
            for item in items {
                match item.get("type").and_then(Value::as_str) {
                    Some("input_text") | Some("text") => {
                        if let Some(text) = item
                            .get("text")
                            .and_then(Value::as_str)
                            .or_else(|| item.get("content").and_then(Value::as_str))
                            .filter(|text| !text.trim().is_empty())
                        {
                            parts.push(json!({ "text": text }));
                        }
                    }
                    Some("input_image") | Some("image_url") => {
                        if let Some(part) = google_inline_data_part(item) {
                            parts.push(part);
                        }
                    }
                    Some("input_file") => {
                        if let Some(filename) = item.get("filename").and_then(Value::as_str) {
                            parts.push(json!({
                                "text": format!("[File attachment omitted for this provider: {filename}]")
                            }));
                        }
                    }
                    _ => {}
                }
            }
            if !parts.is_empty() {
                Some(parts)
            } else {
                text_from_content(content).map(|text| vec![json!({ "text": text })])
            }
        }
        other if other.is_null() => None,
        other => Some(vec![json!({ "text": other.to_string() })]),
    }
}

pub fn anthropic_blocks_from_canonical(content: Option<&Value>) -> Option<Vec<Value>> {
    match content? {
        Value::String(value) => Some(vec![json!({ "type": "text", "text": value })]),
        Value::Array(items) => {
            let mut blocks = Vec::new();
            for item in items {
                match item.get("type").and_then(Value::as_str) {
                    Some("image" | "document" | "search_result" | "tool_reference") => {
                        blocks.push(item.clone());
                    }
                    Some("input_text") | Some("text") => {
                        if let Some(text) = item
                            .get("text")
                            .and_then(Value::as_str)
                            .or_else(|| item.get("content").and_then(Value::as_str))
                            .filter(|text| !text.trim().is_empty())
                        {
                            blocks.push(json!({ "type": "text", "text": text }));
                        }
                    }
                    Some("input_image") | Some("image_url") => {
                        if let Some(block) = anthropic_image_block(item) {
                            blocks.push(block);
                        }
                    }
                    Some("input_file") => {
                        if let Some(filename) = item.get("filename").and_then(Value::as_str) {
                            blocks.push(json!({
                                "type": "text",
                                "text": format!("[File attachment omitted for this provider: {filename}]")
                            }));
                        }
                    }
                    _ => {}
                }
            }
            if !blocks.is_empty() {
                Some(blocks)
            } else {
                text_from_content(content).map(|text| vec![json!({ "type": "text", "text": text })])
            }
        }
        other if other.is_null() => None,
        other => Some(vec![json!({ "type": "text", "text": other.to_string() })]),
    }
}

pub fn anthropic_tool_result_content_from_canonical(content: Option<&Value>) -> Value {
    match content {
        Some(Value::Array(_)) => anthropic_blocks_from_canonical(content)
            .map(Value::Array)
            .unwrap_or_else(|| Value::String(String::new())),
        _ => text_from_content(content)
            .map(Value::String)
            .unwrap_or_else(|| Value::String(String::new())),
    }
}

fn openai_chat_parts_from_canonical_items(items: &[Value], include_text: bool) -> Vec<Value> {
    let mut parts = Vec::new();
    for item in items {
        match item.get("type").and_then(Value::as_str) {
            Some("input_text") | Some("text") if include_text => {
                if let Some(text) = item
                    .get("text")
                    .and_then(Value::as_str)
                    .or_else(|| item.get("content").and_then(Value::as_str))
                    .filter(|text| !text.trim().is_empty())
                {
                    parts.push(json!({ "type": "text", "text": text }));
                }
            }
            Some("input_image") | Some("image_url") => {
                if let Some(url) = canonical_image_url(item) {
                    parts.push(json!({
                        "type": "image_url",
                        "image_url": { "url": url },
                    }));
                }
            }
            _ => {}
        }
    }
    parts
}

fn canonical_image_url(item: &Value) -> Option<String> {
    item.get("image_url")
        .and_then(|value| {
            value.as_str().map(ToString::to_string).or_else(|| {
                value
                    .get("url")
                    .and_then(Value::as_str)
                    .map(ToString::to_string)
            })
        })
        .filter(|url| !url.trim().is_empty())
}

fn anthropic_image_block(item: &Value) -> Option<Value> {
    let url = canonical_image_url(item)?;
    let (media_type, data) = parse_data_url(&url)?;
    if !media_type.starts_with("image/") || data.trim().is_empty() {
        return None;
    }
    Some(json!({
        "type": "image",
        "source": {
            "type": "base64",
            "media_type": media_type,
            "data": data,
        }
    }))
}

fn google_inline_data_part(item: &Value) -> Option<Value> {
    let url = canonical_image_url(item)?;
    let (mime_type, data) = parse_data_url(&url)?;
    if !mime_type.starts_with("image/") || data.trim().is_empty() {
        return None;
    }
    Some(json!({
        "inlineData": {
            "mimeType": mime_type,
            "data": data,
        }
    }))
}

fn parse_data_url(url: &str) -> Option<(String, String)> {
    let rest = url.strip_prefix("data:")?;
    let (metadata, data) = rest.split_once(',')?;
    let media_type = metadata
        .split(';')
        .next()
        .filter(|value| !value.trim().is_empty())?;
    Some((media_type.to_string(), data.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn openai_chat_content_keeps_input_images() {
        let content = json!([
            { "type": "input_text", "text": "see image" },
            { "type": "input_image", "image_url": "data:image/jpeg;base64,AAA" }
        ]);

        let converted =
            openai_chat_content_from_canonical(Some(&content)).expect("openai chat content");

        assert_eq!(converted[0]["type"], "text");
        assert_eq!(converted[1]["type"], "image_url");
        assert_eq!(
            converted[1]["image_url"]["url"],
            "data:image/jpeg;base64,AAA"
        );
    }

    #[test]
    fn anthropic_blocks_convert_input_images_to_base64_source() {
        let content = json!([
            { "type": "input_text", "text": "see image" },
            { "type": "input_image", "image_url": "data:image/png;base64,AAA" }
        ]);

        let blocks = anthropic_blocks_from_canonical(Some(&content)).expect("anthropic blocks");

        assert_eq!(blocks[0]["type"], "text");
        assert_eq!(blocks[1]["type"], "image");
        assert_eq!(blocks[1]["source"]["media_type"], "image/png");
        assert_eq!(blocks[1]["source"]["data"], "AAA");
    }

    #[test]
    fn anthropic_blocks_preserve_native_anthropic_media_blocks() {
        let content = json!([
            { "type": "text", "text": "see native image" },
            {
                "type": "image",
                "source": {
                    "type": "base64",
                    "media_type": "image/png",
                    "data": "AAA"
                }
            }
        ]);

        let blocks = anthropic_blocks_from_canonical(Some(&content)).expect("anthropic blocks");

        assert_eq!(blocks[0]["type"], "text");
        assert_eq!(blocks[1]["type"], "image");
        assert_eq!(blocks[1]["source"]["media_type"], "image/png");
        assert_eq!(blocks[1]["source"]["data"], "AAA");
    }

    #[test]
    fn openai_responses_content_keeps_canonical_media_blocks() {
        let content = json!([
            { "type": "input_text", "text": "see image" },
            { "type": "input_image", "image_url": "data:image/jpeg;base64,AAA" }
        ]);

        let converted = openai_responses_content_from_canonical(Some(&content))
            .expect("openai responses content");

        assert_eq!(converted[0]["type"], "input_text");
        assert_eq!(converted[1]["type"], "input_image");
        assert_eq!(converted[1]["image_url"], "data:image/jpeg;base64,AAA");
    }

    #[test]
    fn google_parts_convert_input_images_to_inline_data() {
        let content = json!([
            { "type": "input_text", "text": "see image" },
            { "type": "input_image", "image_url": "data:image/png;base64,AAA" }
        ]);

        let parts = google_parts_from_canonical(Some(&content)).expect("google parts");

        assert_eq!(parts[0]["text"], "see image");
        assert_eq!(parts[1]["inlineData"]["mimeType"], "image/png");
        assert_eq!(parts[1]["inlineData"]["data"], "AAA");
    }
}
