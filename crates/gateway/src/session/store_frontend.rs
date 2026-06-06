use super::*;

fn frontend_safe_value(value: Option<serde_json::Value>) -> Option<serde_json::Value> {
    value.map(sanitize_frontend_value)
}

pub(super) fn frontend_safe_part_value(
    part: &MessagePart,
    value: Option<serde_json::Value>,
) -> Option<serde_json::Value> {
    if part.part_type == "tool" && part.tool.as_deref() == Some("runtime") {
        return value;
    }
    frontend_safe_value(value)
}

pub(super) fn normalize_tool_message_state(
    tool_name: &str,
    mut state: serde_json::Value,
    metadata: Option<serde_json::Value>,
) -> (serde_json::Value, Option<serde_json::Value>) {
    let Some(state_object) = state.as_object_mut() else {
        return (state, metadata);
    };
    if state_object
        .get("status")
        .and_then(serde_json::Value::as_str)
        != Some("running")
    {
        return (state, metadata);
    }

    let metadata_ref = metadata.as_ref().or_else(|| state_object.get("metadata"));
    let Some(metadata_object) = metadata_ref.and_then(serde_json::Value::as_object) else {
        return (state, metadata);
    };
    if metadata_object
        .get("kind")
        .and_then(serde_json::Value::as_str)
        != Some("mano_tool_call")
    {
        return (state, metadata);
    }
    if metadata_object
        .get("streaming_partial")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false)
    {
        return (state, metadata);
    }
    let Some(output) = metadata_object.get("output") else {
        return (state, metadata);
    };

    let ok = output
        .get("ok")
        .and_then(serde_json::Value::as_bool)
        .or_else(|| {
            metadata_object
                .get("success")
                .and_then(serde_json::Value::as_bool)
        })
        .unwrap_or(true);
    let output_text = tool_output_display_text(output, metadata_object.get("error"));
    let error_value = metadata_object
        .get("error")
        .cloned()
        .unwrap_or_else(|| serde_json::json!("Tool execution failed"));
    if ok {
        state_object.insert("status".to_string(), serde_json::json!("completed"));
        state_object.insert(
            "title".to_string(),
            serde_json::json!(format!("Called `{tool_name}`")),
        );
        state_object
            .entry("output".to_string())
            .or_insert(output_text);
    } else {
        state_object.insert("status".to_string(), serde_json::json!("error"));
        state_object.insert("error".to_string(), error_value);
    }
    if let Some(time) = state_object
        .get_mut("time")
        .and_then(serde_json::Value::as_object_mut)
    {
        time.entry("end".to_string())
            .or_insert_with(|| serde_json::json!(Utc::now().timestamp_millis()));
    }

    (state, metadata)
}

fn tool_output_display_text(
    output: &serde_json::Value,
    error: Option<&serde_json::Value>,
) -> serde_json::Value {
    if let Some(error) = error.and_then(serde_json::Value::as_str) {
        return serde_json::Value::String(error.to_string());
    }
    match serde_json::to_string(output) {
        Ok(text) => serde_json::Value::String(text),
        Err(_) => serde_json::Value::String(String::new()),
    }
}

fn sanitize_frontend_value(value: serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(object) => {
            let object = object
                .into_iter()
                .filter(|(key, _)| !matches!(key.as_str(), "new_learning" | "runtime_id"))
                .map(|(key, value)| (key, sanitize_frontend_value(value)))
                .collect();
            serde_json::Value::Object(object)
        }
        serde_json::Value::Array(items) => {
            serde_json::Value::Array(items.into_iter().map(sanitize_frontend_value).collect())
        }
        value => value,
    }
}
