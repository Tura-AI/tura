use serde_json::Value;
mod command_run_arguments;
mod command_run_stream_events;
mod json_prefix;
mod media_content;
mod text_tool_calls;
mod xml;

pub use command_run_arguments::normalize_command_run_tool_input;
pub use command_run_stream_events::emit_command_run_stream_events_from_content;
pub use json_prefix::json_prefix;
pub use media_content::{
    anthropic_blocks_from_canonical, anthropic_tool_result_content_from_canonical,
    google_parts_from_canonical, openai_chat_content_from_canonical,
    openai_chat_media_content_from_canonical, openai_responses_content_from_canonical,
    text_from_content,
};
pub use text_tool_calls::{
    extract_xml_tool_calls, strip_text_tool_calls, strip_xml_tool_calls, text_tool_calls_value,
};
pub use xml::{parse_xml_parameter_value, xml_parameters, xml_unescape};

pub fn strip_json_fence(input: &str) -> String {
    let s = input.trim();
    if let Some(rest) = s.strip_prefix("```json") {
        return rest.trim().trim_end_matches("```").trim().to_string();
    }
    if let Some(rest) = s.strip_prefix("```") {
        return rest.trim().trim_end_matches("```").trim().to_string();
    }
    s.to_string()
}

pub fn force_strict_schema(value: &mut Value) {
    match value {
        Value::Object(map) => {
            if map.get("type").and_then(Value::as_str) == Some("object") {
                map.insert("additionalProperties".to_string(), Value::Bool(false));
                if let Some(Value::Object(props)) = map.get("properties") {
                    let required = props.keys().map(|k| Value::String(k.clone())).collect();
                    map.insert("required".to_string(), Value::Array(required));
                }
            }
            for (k, v) in map.iter_mut() {
                if k != "additionalProperties" {
                    force_strict_schema(v);
                }
            }
        }
        Value::Array(items) => {
            for item in items {
                force_strict_schema(item);
            }
        }
        _ => {}
    }
}

pub fn to_bedrock_tools(tools: &[Value]) -> Vec<Value> {
    tools
        .iter()
        .filter_map(|tool| {
            let Value::Object(obj) = tool else {
                return None;
            };
            if obj.get("type").and_then(Value::as_str) != Some("function") {
                return None;
            }
            let Value::Object(function) = obj.get("function")? else {
                return None;
            };
            let name = function.get("name")?.as_str()?.to_string();
            let description = function
                .get("description")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            let parameters = function
                .get("parameters")
                .cloned()
                .unwrap_or_else(|| serde_json::json!({"type":"object","properties":{}}));

            Some(serde_json::json!({
                "toolSpec": {
                    "name": name,
                    "description": description,
                    "inputSchema": { "json": parameters }
                }
            }))
        })
        .collect()
}

pub fn to_anthropic_tools(tools: &[Value]) -> Vec<Value> {
    tools
        .iter()
        .filter_map(|tool| {
            let Value::Object(obj) = tool else {
                return None;
            };
            if obj.get("type").and_then(Value::as_str) != Some("function") {
                if obj.get("name").and_then(Value::as_str).is_some()
                    && obj.get("input_schema").is_some()
                {
                    return Some(Value::Object(obj.clone()));
                }
                return None;
            }
            let Value::Object(function) = obj.get("function")? else {
                return None;
            };
            let name = function.get("name")?.as_str()?.to_string();
            let description = function
                .get("description")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            let input_schema = function
                .get("parameters")
                .cloned()
                .unwrap_or_else(|| serde_json::json!({"type":"object","properties":{}}));

            Some(serde_json::json!({
                "name": name,
                "description": description,
                "input_schema": input_schema,
            }))
        })
        .collect()
}

pub fn deep_merge_json(dst: &mut Value, src: Value) {
    match (dst, src) {
        (Value::Object(dst_map), Value::Object(src_map)) => {
            for (k, v) in src_map {
                deep_merge_json(dst_map.entry(k).or_insert(Value::Null), v);
            }
        }
        (dst_slot, src_value) => *dst_slot = src_value,
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{
        deep_merge_json, force_strict_schema, strip_json_fence, to_anthropic_tools,
        to_bedrock_tools,
    };

    #[test]
    fn strip_json_fence_removes_markdown_wrappers() {
        assert_eq!(
            strip_json_fence("```json\n{\"ok\":true}\n```"),
            "{\"ok\":true}"
        );
        assert_eq!(strip_json_fence(" plain "), "plain");
    }

    #[test]
    fn strict_schema_recurses_and_requires_all_properties() {
        let mut schema = json!({
            "type": "object",
            "properties": {
                "outer": {
                    "type": "object",
                    "properties": {"inner": {"type": "string"}}
                }
            }
        });

        force_strict_schema(&mut schema);

        assert_eq!(schema["additionalProperties"], false);
        assert_eq!(schema["required"], json!(["outer"]));
        assert_eq!(schema["properties"]["outer"]["additionalProperties"], false);
        assert_eq!(schema["properties"]["outer"]["required"], json!(["inner"]));
    }

    #[test]
    fn bedrock_tools_convert_openapi_function_tools() {
        let tools = to_bedrock_tools(&[json!({
            "type": "function",
            "function": {
                "name": "echo",
                "description": "Echo",
                "parameters": {"type": "object"}
            }
        })]);

        assert_eq!(tools[0]["toolSpec"]["name"], "echo");
        assert_eq!(
            tools[0]["toolSpec"]["inputSchema"]["json"]["type"],
            "object"
        );
    }

    #[test]
    fn anthropic_tools_convert_openapi_function_tools() {
        let tools = to_anthropic_tools(&[json!({
            "type": "function",
            "function": {
                "name": "echo",
                "description": "Echo",
                "parameters": {"type": "object", "properties": {"msg": {"type": "string"}}}
            }
        })]);

        assert_eq!(tools[0]["name"], "echo");
        assert_eq!(tools[0]["description"], "Echo");
        assert_eq!(tools[0]["input_schema"]["type"], "object");
        assert_eq!(
            tools[0]["input_schema"]["properties"]["msg"]["type"],
            "string"
        );
    }

    #[test]
    fn anthropic_tools_preserve_native_anthropic_tool_schema() {
        let tools = to_anthropic_tools(&[json!({
            "name": "Read",
            "description": "Read a file",
            "input_schema": {"type": "object", "properties": {"file_path": {"type": "string"}}},
            "cache_control": {"type": "ephemeral"},
            "eager_input_streaming": true
        })]);

        assert_eq!(tools[0]["name"], "Read");
        assert_eq!(tools[0]["input_schema"]["type"], "object");
        assert_eq!(tools[0]["cache_control"]["type"], "ephemeral");
        assert_eq!(tools[0]["eager_input_streaming"], true);
    }

    #[test]
    fn anthropic_tools_skip_non_function_entries() {
        let tools = to_anthropic_tools(&[json!({"type": "web_search"})]);
        assert!(tools.is_empty());
    }

    #[test]
    fn deep_merge_json_merges_objects_and_replaces_scalars() {
        let mut value = json!({"a": {"b": 1}, "x": 1});
        deep_merge_json(&mut value, json!({"a": {"c": 2}, "x": 3}));

        assert_eq!(value, json!({"a": {"b": 1, "c": 2}, "x": 3}));
    }
}
