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
    let results_snapshot = command_run_frontend_results_snapshot(state, metadata);
    if let Some(object) = state.as_object_mut() {
        if let Some(results) = results_snapshot {
            upsert_streamed_command_run_results(object, results);
        }
    }
    let commands = command_run_frontend_commands(state, metadata);
    if let Some(object) = state.as_object_mut() {
        object.insert("commands".to_string(), serde_json::Value::Array(commands));
    }
}

fn upsert_streamed_command_run_results(
    object: &mut serde_json::Map<String, serde_json::Value>,
    results: Vec<serde_json::Value>,
) {
    let results_value = serde_json::Value::Array(results);
    let stream = object
        .entry("streamed_command_run_result".to_string())
        .or_insert_with(|| serde_json::json!({}));
    if let Some(stream_object) = stream.as_object_mut() {
        stream_object.insert("results".to_string(), results_value.clone());
    }

    let output = object
        .entry("output".to_string())
        .or_insert_with(|| serde_json::json!({}));
    if let Some(output_object) = output.as_object_mut() {
        let output_stream = output_object
            .entry("streamed_command_run_result".to_string())
            .or_insert_with(|| serde_json::json!({}));
        if let Some(output_stream_object) = output_stream.as_object_mut() {
            output_stream_object.insert("results".to_string(), results_value);
        }
    }
}

fn command_run_frontend_commands(
    state: &serde_json::Value,
    metadata: Option<&serde_json::Value>,
) -> Vec<serde_json::Value> {
    let specs = primary_command_specs(state, metadata);
    let results = primary_command_results(state, metadata);
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

fn command_run_frontend_results_snapshot(
    state: &serde_json::Value,
    metadata: Option<&serde_json::Value>,
) -> Option<Vec<serde_json::Value>> {
    let specs = primary_command_specs(state, metadata);
    let results = primary_command_results(state, metadata);
    if specs.is_empty() && results.is_empty() {
        return None;
    }
    if specs.is_empty() {
        return Some(results);
    }

    let fallback_status = string_field_from_value(state, "status");
    let mut used_results = vec![false; results.len()];
    let mut snapshot = Vec::new();
    for (index, spec) in specs.iter().enumerate() {
        if is_task_status_command(spec) {
            continue;
        }
        let result_index =
            command_result_index_for_spec(index, spec, &specs, &results, &used_results);
        let result = result_index.map(|index| {
            used_results[index] = true;
            &results[index]
        });
        if result.is_some_and(is_task_status_command) {
            continue;
        }
        snapshot.push(command_result_snapshot_item(
            index,
            result,
            Some(spec),
            fallback_status.as_deref(),
        )?);
    }

    for (index, result) in results.iter().enumerate() {
        if used_results[index] || is_task_status_command(result) {
            continue;
        }
        snapshot.push(command_result_snapshot_item(
            index,
            Some(result),
            None,
            fallback_status.as_deref(),
        )?);
    }

    Some(snapshot)
}

fn command_result_index_for_spec(
    spec_index: usize,
    spec: &serde_json::Value,
    specs: &[serde_json::Value],
    results: &[serde_json::Value],
    used_results: &[bool],
) -> Option<usize> {
    if let Some(index) = results.iter().enumerate().find_map(|(index, result)| {
        if used_results[index] || is_task_status_command(result) {
            return None;
        }
        command_result_matches_spec(result, spec).then_some(index)
    }) {
        return Some(index);
    }

    let result = results.get(spec_index)?;
    if used_results[spec_index]
        || is_task_status_command(result)
        || command_line_from_value(result).is_some()
    {
        return None;
    }
    if results.len() == specs.len() || spec_index < results.len() {
        return Some(spec_index);
    }
    None
}

fn command_result_matches_spec(result: &serde_json::Value, spec: &serde_json::Value) -> bool {
    let Some(result_command) = command_line_from_value(result) else {
        return false;
    };
    command_line_from_value(spec).is_some_and(|spec_command| spec_command == result_command)
}

fn command_result_snapshot_item(
    index: usize,
    result: Option<&serde_json::Value>,
    spec: Option<&serde_json::Value>,
    fallback_status: Option<&str>,
) -> Option<serde_json::Value> {
    let command = command_line_from_result_or_spec(result, spec)?;
    let name = command_name_from_result_or_spec(result, spec)?;
    let step = command_step_from_result_or_spec(result, spec).unwrap_or(index + 1);
    let status = command_status_from_result_or_spec(result, spec, fallback_status)
        .unwrap_or_else(|| "pending".to_string());

    let mut item = result
        .and_then(record_like)
        .map(serde_json::Value::Object)
        .unwrap_or_else(|| serde_json::json!({}));
    if let Some(object) = item.as_object_mut() {
        object
            .entry("step".to_string())
            .or_insert_with(|| serde_json::json!(step));
        object
            .entry("command_type".to_string())
            .or_insert_with(|| serde_json::json!(name));
        object
            .entry("command_line".to_string())
            .or_insert_with(|| serde_json::json!(command));
        object.insert("status".to_string(), serde_json::json!(status));
        object
            .entry("success".to_string())
            .or_insert(serde_json::Value::Null);
    }
    Some(item)
}

fn primary_command_specs(
    state: &serde_json::Value,
    metadata: Option<&serde_json::Value>,
) -> Vec<serde_json::Value> {
    for specs in [
        command_specs_from_root(state),
        state
            .get("metadata")
            .map(command_specs_from_root)
            .unwrap_or_default(),
        metadata.map(command_specs_from_root).unwrap_or_default(),
    ] {
        if !specs.is_empty() {
            return specs;
        }
    }
    Vec::new()
}

fn command_specs_from_root(root: &serde_json::Value) -> Vec<serde_json::Value> {
    let Some(record) = record_like(root) else {
        return Vec::new();
    };
    for values in [
        object_field(&record, "input")
            .map(|input| array_field(&input, "commands"))
            .unwrap_or_default(),
        object_field(&record, "streamed_command_run_result")
            .map(|stream| array_field(&stream, "commands"))
            .unwrap_or_default(),
        object_field(&record, "output")
            .map(|output| {
                object_field(&output, "streamed_command_run_result")
                    .map(|stream| array_field(&stream, "commands"))
                    .unwrap_or_else(|| array_field(&output, "commands"))
            })
            .unwrap_or_default(),
        array_field(&record, "commands"),
    ] {
        if !values.is_empty() {
            return values;
        }
    }
    Vec::new()
}

fn primary_command_results(
    state: &serde_json::Value,
    metadata: Option<&serde_json::Value>,
) -> Vec<serde_json::Value> {
    for results in [
        command_results_from_root(state),
        state
            .get("metadata")
            .map(command_results_from_root)
            .unwrap_or_default(),
        metadata.map(command_results_from_root).unwrap_or_default(),
    ] {
        if !results.is_empty() {
            return results;
        }
    }
    Vec::new()
}

fn command_results_from_root(root: &serde_json::Value) -> Vec<serde_json::Value> {
    let Some(record) = record_like(root) else {
        return Vec::new();
    };
    for values in [
        object_field(&record, "streamed_command_run_result")
            .map(|stream| array_field(&stream, "results"))
            .unwrap_or_default(),
        object_field(&record, "output")
            .map(|output| {
                object_field(&output, "streamed_command_run_result")
                    .map(|stream| array_field(&stream, "results"))
                    .unwrap_or_else(|| array_field(&output, "results"))
            })
            .unwrap_or_default(),
    ] {
        if !values.is_empty() {
            return values;
        }
    }
    Vec::new()
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

#[cfg(test)]
mod tests {
    use super::normalize_command_run_frontend_state;
    use serde_json::json;

    #[test]
    fn command_run_frontend_state_keeps_running_commands_in_partial_updates() {
        let mut state = json!({
            "status": "running",
            "input": {
                "commands": [
                    {
                        "step": 1,
                        "command": "shell_command",
                        "command_line": "npm test -- --runInBand"
                    },
                    {
                        "step": 1,
                        "command": "shell_command",
                        "command_line": "npm run e2e"
                    }
                ]
            },
            "output": {
                "streamed_command_run_result": {
                    "results": [{
                        "step": 1,
                        "command_type": "shell_command",
                        "success": true,
                        "output": "unit tests passed"
                    }]
                }
            },
            "streamed_command_run_result": {
                "results": [{
                    "step": 1,
                    "command_type": "shell_command",
                    "success": true,
                    "output": "unit tests passed"
                }]
            }
        });

        normalize_command_run_frontend_state(&mut state, None);

        let results = state
            .pointer("/streamed_command_run_result/results")
            .and_then(serde_json::Value::as_array)
            .expect("normalized command_run results should be present");
        assert_eq!(results.len(), 2);
        assert_eq!(results[0]["command_line"], "npm test -- --runInBand");
        assert_eq!(results[0]["status"], "completed");
        assert_eq!(results[0]["success"], true);
        assert_eq!(results[1]["command_line"], "npm run e2e");
        assert_eq!(results[1]["status"], "running");
        assert!(results[1]["success"].is_null());

        let output_results = state
            .pointer("/output/streamed_command_run_result/results")
            .and_then(serde_json::Value::as_array)
            .expect("output stream results should mirror normalized results");
        assert_eq!(output_results, results);
    }

    #[test]
    fn command_run_frontend_state_preserves_command_order_for_identified_results() {
        let mut state = json!({
            "status": "running",
            "input": {
                "commands": [
                    {
                        "step": 1,
                        "command": "shell_command",
                        "command_line": "npm test -- --runInBand"
                    },
                    {
                        "step": 1,
                        "command": "shell_command",
                        "command_line": "npm run e2e"
                    }
                ]
            },
            "streamed_command_run_result": {
                "results": [{
                    "step": 1,
                    "command_type": "shell_command",
                    "command_line": "npm run e2e",
                    "success": true,
                    "output": "e2e passed"
                }]
            }
        });

        normalize_command_run_frontend_state(&mut state, None);

        let results = state
            .pointer("/streamed_command_run_result/results")
            .and_then(serde_json::Value::as_array)
            .expect("normalized command_run results should be present");
        assert_eq!(results.len(), 2);
        assert_eq!(results[0]["command_line"], "npm test -- --runInBand");
        assert_eq!(results[0]["status"], "running");
        assert!(results[0]["success"].is_null());
        assert_eq!(results[1]["command_line"], "npm run e2e");
        assert_eq!(results[1]["status"], "completed");
        assert_eq!(results[1]["success"], true);

        let commands = state["commands"]
            .as_array()
            .expect("commands should be normalized from the full snapshot");
        assert_eq!(commands.len(), 2);
        assert_eq!(commands[0]["command"], "npm test -- --runInBand");
        assert_eq!(commands[0]["status"], "running");
        assert_eq!(commands[1]["command"], "npm run e2e");
        assert_eq!(commands[1]["status"], "completed");
    }
}
