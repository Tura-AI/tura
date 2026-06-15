use super::*;

pub(crate) fn frontend_safe_value(value: Option<serde_json::Value>) -> Option<serde_json::Value> {
    value.map(sanitize_frontend_value)
}

pub(crate) fn frontend_safe_part_value(
    part: &MessagePart,
    value: Option<serde_json::Value>,
) -> Option<serde_json::Value> {
    if part.part_type == "tool" && part.tool.as_deref() == Some("runtime") {
        return value;
    }
    frontend_safe_value(value)
}

pub(crate) fn frontend_safe_part_state(
    part: &MessagePart,
    value: Option<serde_json::Value>,
) -> Option<serde_json::Value> {
    let mut value = frontend_safe_part_value(part, value)?;
    if part.part_type == "tool" && part.tool.as_deref() == Some("command_run") {
        normalize_command_run_frontend_state(&mut value, part.metadata.as_ref());
    }
    Some(value)
}

pub(super) fn normalize_tool_message_state(
    tool_name: &str,
    mut state: serde_json::Value,
    metadata: Option<serde_json::Value>,
) -> (serde_json::Value, Option<serde_json::Value>) {
    if tool_name == "command_run" {
        normalize_command_run_frontend_state(&mut state, metadata.as_ref());
    }

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

pub(crate) fn normalize_command_run_frontend_state(
    state: &mut serde_json::Value,
    metadata: Option<&serde_json::Value>,
) {
    let commands = command_run_frontend_commands(state, metadata);
    if let Some(object) = state.as_object_mut() {
        object.insert("commands".to_string(), serde_json::Value::Array(commands));
    }
}

fn command_run_frontend_commands(
    state: &serde_json::Value,
    metadata: Option<&serde_json::Value>,
) -> Vec<serde_json::Value> {
    let specs = command_specs(state, metadata);
    let results = command_results(state, metadata);
    let fallback_status = string_field_from_value(state, "status");
    let mut commands = Vec::new();
    let count = specs.len().max(results.len());
    for index in 0..count {
        let spec = specs.get(index);
        let result = results.get(index);
        if spec.is_some_and(is_task_status_command) || result.is_some_and(is_task_status_command) {
            continue;
        }
        let Some(command) = command_line_from_result_or_spec(result, spec) else {
            continue;
        };
        let Some(name) = command_name_from_result_or_spec(result, spec) else {
            continue;
        };
        let status = command_status_from_result_or_spec(result, spec, fallback_status.as_deref());
        commands.push(serde_json::json!({
            "command": command,
            "name": name,
            "step": command_step_from_result_or_spec(result, spec).unwrap_or(index + 1),
            "status": status,
        }));
    }
    commands
}

fn command_specs(
    state: &serde_json::Value,
    metadata: Option<&serde_json::Value>,
) -> Vec<serde_json::Value> {
    let mut values = Vec::new();
    for root in [Some(state), state.get("metadata"), metadata]
        .into_iter()
        .flatten()
    {
        collect_command_specs(root, &mut values);
    }
    values
}

fn collect_command_specs(root: &serde_json::Value, values: &mut Vec<serde_json::Value>) {
    let Some(record) = record_like(root) else {
        return;
    };
    values.extend(array_field(&record, "commands"));
    if let Some(input) = object_field(&record, "input") {
        values.extend(array_field(&input, "commands"));
    }
    if let Some(output) = object_field(&record, "output") {
        values.extend(array_field(&output, "commands"));
        if let Some(stream) = object_field(&output, "streamed_command_run_result") {
            values.extend(array_field(&stream, "commands"));
        }
    }
    if let Some(stream) = object_field(&record, "streamed_command_run_result") {
        values.extend(array_field(&stream, "commands"));
    }
}

fn command_results(
    state: &serde_json::Value,
    metadata: Option<&serde_json::Value>,
) -> Vec<serde_json::Value> {
    let mut values = Vec::new();
    for root in [Some(state), state.get("metadata"), metadata]
        .into_iter()
        .flatten()
    {
        collect_command_results(root, &mut values);
    }
    values
}

fn collect_command_results(root: &serde_json::Value, values: &mut Vec<serde_json::Value>) {
    let Some(record) = record_like(root) else {
        return;
    };
    if let Some(output) = object_field(&record, "output") {
        values.extend(array_field(&output, "results"));
        if let Some(stream) = object_field(&output, "streamed_command_run_result") {
            values.extend(array_field(&stream, "results"));
        }
    }
    if let Some(stream) = object_field(&record, "streamed_command_run_result") {
        values.extend(array_field(&stream, "results"));
    }
}

fn command_line_from_result_or_spec(
    result: Option<&serde_json::Value>,
    spec: Option<&serde_json::Value>,
) -> Option<String> {
    for value in [result, spec].into_iter().flatten() {
        if let Some(command) = command_line_from_value(value) {
            return Some(command);
        }
    }
    None
}

fn command_line_from_value(value: &serde_json::Value) -> Option<String> {
    let record = record_like(value)?;
    let nested = object_field(&record, "command");
    for source in [Some(&record), nested.as_ref()].into_iter().flatten() {
        if let Some(command) = canonical_command_field(source)
            .or_else(|| string_field(source, "command_line"))
            .or_else(|| string_field(source, "commandLine"))
            .or_else(|| command_field_with_type(source))
        {
            return Some(command.trim().to_string());
        }
    }
    None
}

fn command_name_from_result_or_spec(
    result: Option<&serde_json::Value>,
    spec: Option<&serde_json::Value>,
) -> Option<String> {
    for value in [result, spec].into_iter().flatten() {
        if let Some(name) = command_name_from_value(value) {
            return Some(name);
        }
    }
    None
}

fn command_name_from_value(value: &serde_json::Value) -> Option<String> {
    let record = record_like(value)?;
    let nested = object_field(&record, "command");
    for source in [Some(&record), nested.as_ref()].into_iter().flatten() {
        if let Some(name) =
            string_field(source, "name").or_else(|| command_type_from_record(source))
        {
            return Some(name);
        }
    }
    None
}

fn command_step_from_result_or_spec(
    result: Option<&serde_json::Value>,
    spec: Option<&serde_json::Value>,
) -> Option<usize> {
    for value in [result, spec].into_iter().flatten() {
        if let Some(step) = command_step_from_value(value) {
            return Some(step);
        }
    }
    None
}

fn command_step_from_value(value: &serde_json::Value) -> Option<usize> {
    let record = record_like(value)?;
    number_field(&record, "step").or_else(|| {
        object_field(&record, "command").and_then(|command| number_field(&command, "step"))
    })
}

fn command_status_from_result_or_spec(
    result: Option<&serde_json::Value>,
    spec: Option<&serde_json::Value>,
    fallback: Option<&str>,
) -> Option<String> {
    let Some(result) = result.and_then(record_like) else {
        if let Some(status) = spec
            .and_then(record_like)
            .and_then(|record| string_field(&record, "status"))
        {
            return Some(if status == "in_progress" {
                "running".to_string()
            } else {
                status
            });
        }
        return fallback.map(ToString::to_string);
    };
    if result
        .get("success")
        .and_then(serde_json::Value::as_bool)
        .is_some_and(|success| !success)
    {
        return Some("failed".to_string());
    }
    if let Some(status) = string_field(&result, "status") {
        return Some(if status == "in_progress" {
            "running".to_string()
        } else {
            status
        });
    }
    if result
        .get("success")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false)
    {
        return Some("completed".to_string());
    }
    fallback.map(ToString::to_string)
}

fn is_task_status_command(value: &serde_json::Value) -> bool {
    if task_status_payload(value) {
        return true;
    }
    let Some(record) = record_like(value) else {
        return false;
    };
    string_field(&record, "name")
        .or_else(|| command_type_from_record(&record))
        .is_some_and(|name| name.trim().eq_ignore_ascii_case("task_status"))
        || object_field(&record, "command").is_some_and(|command| {
            string_field(&command, "name")
                .or_else(|| command_type_from_record(&command))
                .is_some_and(|name| name.trim().eq_ignore_ascii_case("task_status"))
        })
}

fn task_status_payload(value: &serde_json::Value) -> bool {
    match value {
        serde_json::Value::Object(object) => {
            object.contains_key("task_status")
                || object.values().any(task_status_payload)
                || object
                    .get("command_type")
                    .and_then(serde_json::Value::as_str)
                    .is_some_and(|value| value.eq_ignore_ascii_case("task_status"))
        }
        serde_json::Value::Array(items) => items.iter().any(task_status_payload),
        serde_json::Value::String(text) => serde_json::from_str::<serde_json::Value>(text)
            .ok()
            .is_some_and(|value| task_status_payload(&value)),
        _ => false,
    }
}

fn command_type_from_record(record: &serde_json::Map<String, serde_json::Value>) -> Option<String> {
    string_field(record, "command_type")
        .or_else(|| string_field(record, "commandType"))
        .or_else(|| {
            let command = string_field(record, "command")?;
            (!command.contains(char::is_whitespace)).then_some(command)
        })
}

fn canonical_command_field(record: &serde_json::Map<String, serde_json::Value>) -> Option<String> {
    let command = string_field(record, "command")?;
    record.contains_key("name").then_some(command)
}

fn command_field_with_type(record: &serde_json::Map<String, serde_json::Value>) -> Option<String> {
    let command_type = command_type_from_record(record)?;
    let command = string_field(record, "command")?;
    (command != command_type).then_some(command)
}

fn string_field_from_value(value: &serde_json::Value, key: &str) -> Option<String> {
    record_like(value).and_then(|record| string_field(&record, key))
}

fn string_field(record: &serde_json::Map<String, serde_json::Value>, key: &str) -> Option<String> {
    record
        .get(key)
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn number_field(record: &serde_json::Map<String, serde_json::Value>, key: &str) -> Option<usize> {
    record
        .get(key)
        .and_then(serde_json::Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())
}

fn object_field(
    record: &serde_json::Map<String, serde_json::Value>,
    key: &str,
) -> Option<serde_json::Map<String, serde_json::Value>> {
    record.get(key).and_then(record_like)
}

fn array_field(
    record: &serde_json::Map<String, serde_json::Value>,
    key: &str,
) -> Vec<serde_json::Value> {
    match record.get(key) {
        Some(serde_json::Value::Array(items)) => items.clone(),
        Some(serde_json::Value::String(text)) => serde_json::from_str::<serde_json::Value>(text)
            .ok()
            .and_then(|value| value.as_array().cloned())
            .unwrap_or_default(),
        _ => Vec::new(),
    }
}

fn record_like(value: &serde_json::Value) -> Option<serde_json::Map<String, serde_json::Value>> {
    match value {
        serde_json::Value::Object(object) => Some(object.clone()),
        serde_json::Value::String(text) => serde_json::from_str::<serde_json::Value>(text)
            .ok()
            .and_then(|value| value.as_object().cloned()),
        _ => None,
    }
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
