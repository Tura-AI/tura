use serde_json::{Map, Value};

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

pub fn as_object_mut(value: &mut Value) -> &mut Map<String, Value> {
    if !value.is_object() {
        *value = Value::Object(Map::new());
    }
    value.as_object_mut().expect("object just initialized")
}
