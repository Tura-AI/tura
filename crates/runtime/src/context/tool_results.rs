use super::char_budget::{
    context_output_byte_budget, formatted_truncate_text, truncate_middle_with_char_budget,
    COMMAND_RUN_RESULT_OUTPUT_MAX_CHARS, CONTEXT_OUTPUT_MAX_CHARS,
};
use crate::state_machine::session_management::SessionManagement;

use super::media::{command_run_media_content_items_for_context, strip_read_media_payload_data};

pub(super) fn strip_context_reporting_fields(value: serde_json::Value) -> serde_json::Value {
    strip_context_reporting_fields_inner(value, false)
}

fn strip_context_reporting_fields_inner(
    value: serde_json::Value,
    task_status_context: bool,
) -> serde_json::Value {
    match value {
        serde_json::Value::Object(map) => {
            let command_is_task_status = object_is_task_status_command(&map);
            let preserve_task_status_fields = task_status_context || command_is_task_status;
            serde_json::Value::Object(
                map.into_iter()
                    .filter(|(key, _)| {
                        !is_context_reporting_field_for_context(key, preserve_task_status_fields)
                    })
                    .map(|(key, value)| {
                        let child_task_status_context =
                            preserve_task_status_fields || key == "task_status";
                        (
                            key,
                            strip_context_reporting_fields_inner(value, child_task_status_context),
                        )
                    })
                    .collect(),
            )
        }
        serde_json::Value::Array(items) => serde_json::Value::Array(
            items
                .into_iter()
                .map(|item| strip_context_reporting_fields_inner(item, task_status_context))
                .collect(),
        ),
        other => other,
    }
}

fn object_is_task_status_command(map: &serde_json::Map<String, serde_json::Value>) -> bool {
    map.get("command_type")
        .or_else(|| map.get("command"))
        .or_else(|| map.get("command_name"))
        .or_else(|| map.get("tool_name"))
        .and_then(serde_json::Value::as_str)
        .map(|value| value.trim().to_ascii_lowercase().replace('-', "_"))
        .as_deref()
        == Some("task_status")
}

fn is_context_reporting_field_for_context(key: &str, task_status_context: bool) -> bool {
    if task_status_context && is_task_status_context_field(key) {
        return false;
    }
    is_context_reporting_field(key)
}

fn is_task_status_context_field(key: &str) -> bool {
    matches!(
        key,
        "command" | "task_group" | "task_type" | "status" | "compact_context"
    )
}

fn is_context_reporting_field(key: &str) -> bool {
    matches!(
        key,
        "task_group"
            | "step_summary"
            | "last_tool_call_status"
            | "last_tool_call_summary"
            | "summary"
            | "description"
            | "interface"
            | "used_prompt"
            | "notes"
            | "receipt"
            | "should_register_tool"
            | "command_id"
            | "command_run_id"
            | "provider_tool_call_id"
            | "command_index"
            | "result_index"
            | "command"
            | "command_updates"
            | "messageID"
            | "partID"
            | "runtimeID"
            | "commandRunID"
            | "commandID"
            | "providerToolCallID"
            | "commandIndex"
            | "eventSeq"
            | "createdAt"
            | "updatedAt"
            | "runtime_id"
            | "created_at"
            | "updated_at"
            | "timestamp"
    )
}

fn strip_command_run_context_noise(value: serde_json::Value) -> serde_json::Value {
    strip_context_reporting_fields(value)
}

pub(super) fn last_tool_call_response_from_session(
    session: &SessionManagement,
) -> Option<serde_json::Value> {
    session
        .session_log
        .iter()
        .rev()
        .filter_map(|entry| serde_json::from_str::<serde_json::Value>(entry).ok())
        .find(|value| value.get("type").and_then(|kind| kind.as_str()) == Some("tool_result"))
        .map(|value| {
            serde_json::json!({
                "tool_name": value.get("tool_name").cloned().unwrap_or(serde_json::Value::Null),
                "input": compact_json_for_context(strip_context_reporting_fields(value.get("input").cloned().unwrap_or(serde_json::Value::Null))),
                "output": cached_context_output_for_tool_result(&value),
                "success": value.get("success").cloned().unwrap_or(serde_json::Value::Bool(true)),
                "error": cached_context_error_for_tool_result(&value),
            })
        })
}

pub(super) fn tool_result_context_cache(value: &serde_json::Value) -> serde_json::Value {
    let output = compact_json_for_context(context_output_for_tool_result(value));
    let error = compact_json_for_context(
        value
            .get("error")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
    );
    let cache_id_input = serde_json::json!({
        "version": 1,
        "sequence": value.get("sequence").cloned().unwrap_or(serde_json::Value::Null),
        "tool_name": value.get("tool_name").cloned().unwrap_or(serde_json::Value::Null),
        "input": compact_json_for_context(strip_context_reporting_fields(value.get("input").cloned().unwrap_or(serde_json::Value::Null))),
        "output": output,
        "success": value.get("success").cloned().unwrap_or(serde_json::Value::Bool(true)),
        "error": error,
    });
    serde_json::json!({
        "version": 1,
        "cache_id": stable_context_cache_id(&cache_id_input),
        "output": output,
        "error": error,
    })
}

pub(super) fn immutable_tool_result_context_message(
    value: &serde_json::Value,
) -> serde_json::Value {
    if value.get("tool_name").and_then(|name| name.as_str()) == Some("command_run") {
        return command_run_function_output_context_message(value);
    }
    serde_json::json!({
        "role": "user",
        "content": compact_json_to_string(&serde_json::json!([immutable_tool_result_context_item(value)])),
    })
}

pub(super) fn immutable_tool_result_context_messages(
    value: &serde_json::Value,
) -> Vec<serde_json::Value> {
    if value.get("tool_name").and_then(|name| name.as_str()) == Some("command_run") {
        return command_run_provider_context_items(value);
    }
    vec![immutable_tool_result_context_message(value)]
}

pub(super) fn command_run_cached_context_messages_are_valid(
    messages: &[serde_json::Value],
) -> bool {
    let mut seen_calls = std::collections::HashSet::new();
    for message in messages {
        match message.get("type").and_then(serde_json::Value::as_str) {
            Some("function_call") => {
                if message.get("name").and_then(serde_json::Value::as_str) == Some("command_run") {
                    let Some(call_id) = message
                        .get("call_id")
                        .and_then(serde_json::Value::as_str)
                        .filter(|value| !value.trim().is_empty())
                    else {
                        return false;
                    };
                    let Some(arguments) = message
                        .get("arguments")
                        .and_then(serde_json::Value::as_str)
                        .and_then(|arguments| {
                            serde_json::from_str::<serde_json::Value>(arguments).ok()
                        })
                    else {
                        return false;
                    };
                    if !(arguments.get("commands").is_some() || arguments.get("steps").is_some()) {
                        return false;
                    }
                    seen_calls.insert(call_id.to_string());
                }
            }
            Some("function_call_output") => {
                let Some(call_id) = message
                    .get("call_id")
                    .and_then(serde_json::Value::as_str)
                    .filter(|value| !value.trim().is_empty())
                else {
                    return false;
                };
                if !seen_calls.contains(call_id) {
                    return false;
                }
                if command_run_function_output_contains_command_identity(message) {
                    return false;
                }
            }
            _ => {}
        }
    }
    true
}

fn command_run_function_output_contains_command_identity(message: &serde_json::Value) -> bool {
    let Some(output) = message.get("output") else {
        return false;
    };
    match output {
        serde_json::Value::String(text) => serde_json::from_str::<serde_json::Value>(text)
            .ok()
            .is_some_and(|value| value_contains_command_identity(&value)),
        serde_json::Value::Array(items) => items.iter().any(|item| {
            item.get("text")
                .and_then(serde_json::Value::as_str)
                .and_then(|text| serde_json::from_str::<serde_json::Value>(text).ok())
                .is_some_and(|value| value_contains_command_identity(&value))
        }),
        other => value_contains_command_identity(other),
    }
}

fn value_contains_command_identity(value: &serde_json::Value) -> bool {
    match value {
        serde_json::Value::Object(map) => map.iter().any(|(field, value)| {
            matches!(field.as_str(), "step" | "command_type" | "command_line")
                || value_contains_command_identity(value)
        }),
        serde_json::Value::Array(items) => items.iter().any(value_contains_command_identity),
        _ => false,
    }
}

fn command_run_provider_context_items(value: &serde_json::Value) -> Vec<serde_json::Value> {
    let Some(call_id) = command_run_provider_call_id(value) else {
        return vec![command_run_function_output_context_message(value)];
    };
    vec![
        serde_json::json!({
            "type": "function_call",
            "call_id": call_id,
            "name": "command_run",
            "arguments": command_run_function_arguments_for_context(value),
        }),
        serde_json::json!({
            "type": "function_call_output",
            "call_id": call_id,
            "output": command_run_function_output_payload_for_context(value),
        }),
    ]
}

fn command_run_function_output_context_message(value: &serde_json::Value) -> serde_json::Value {
    if let Some(content) = command_run_media_content_items_for_context(value) {
        return serde_json::json!({
            "role": "user",
            "content": content,
        });
    }
    serde_json::json!({
        "role": "user",
        "content": command_run_function_output_for_context(value),
    })
}

fn command_run_provider_call_id(value: &serde_json::Value) -> Option<String> {
    let metadata = value.get("provider_metadata")?;
    metadata
        .get("call_id")
        .or_else(|| metadata.get("id"))
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn command_run_function_arguments_for_context(value: &serde_json::Value) -> String {
    value
        .get("input")
        .cloned()
        .map(strip_tool_reporting_fields)
        .and_then(|input| serde_json::to_string(&input).ok())
        .unwrap_or_else(|| "{}".to_string())
}

pub(super) fn command_run_function_output_for_context(value: &serde_json::Value) -> String {
    command_run_current_style_output_for_context(value)
}

pub(super) fn command_run_function_output_payload_for_context(
    value: &serde_json::Value,
) -> serde_json::Value {
    if let Some(content) = command_run_media_content_items_for_context(value) {
        return serde_json::Value::Array(content);
    }
    serde_json::Value::String(command_run_function_output_for_context(value))
}

fn command_run_current_style_output_for_context(value: &serde_json::Value) -> String {
    command_run_current_style_output_string(value).unwrap_or_else(|| {
        let output = value.get("output").unwrap_or(&serde_json::Value::Null);
        let output = strip_command_run_context_noise(output.clone());
        serde_json::to_string_pretty(&output).unwrap_or_else(|_| output.to_string())
    })
}

pub(super) fn command_run_current_style_output_string(value: &serde_json::Value) -> Option<String> {
    let mut output = strip_context_reporting_fields(
        value
            .get("output")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
    );
    strip_read_media_payload_data(&mut output);
    let input = strip_context_reporting_fields(
        value
            .get("input")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
    );
    let input_commands = input
        .get("commands")
        .and_then(|commands| commands.as_array());
    let results = flattened_command_run_results(&output);
    if results.is_empty() {
        return None;
    }
    let results = results
        .into_iter()
        .enumerate()
        .map(|(index, result)| {
            command_run_context_result(
                result,
                input_commands.and_then(|commands| commands.get(index)),
            )
        })
        .collect::<Vec<_>>();
    let output = serde_json::json!({ "results": results });
    serde_json::to_string_pretty(&output).ok()
}

fn immutable_tool_result_context_item(value: &serde_json::Value) -> serde_json::Value {
    let item = serde_json::json!({
        "type": "tool_result",
        "cache_id": value
            .get("context_cache")
            .and_then(|cache| cache.get("cache_id"))
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        "tool_name": value.get("tool_name").cloned().unwrap_or(serde_json::Value::Null),
        "input": compact_json_for_context(strip_context_reporting_fields(value.get("input").cloned().unwrap_or(serde_json::Value::Null))),
        "output": cached_context_output_for_tool_result(value),
        "success": value.get("success").cloned().unwrap_or(serde_json::Value::Bool(true)),
        "error": cached_context_error_for_tool_result(value),
    });
    item
}

fn stable_context_cache_id(value: &serde_json::Value) -> String {
    let serialized = serde_json::to_string(value).unwrap_or_else(|_| value.to_string());
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in serialized.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}

fn cached_context_output_for_tool_result(value: &serde_json::Value) -> serde_json::Value {
    value
        .get("context_cache")
        .and_then(|cache| cache.get("output"))
        .cloned()
        .unwrap_or_else(|| compact_json_for_context(context_output_for_tool_result(value)))
}

fn cached_context_error_for_tool_result(value: &serde_json::Value) -> serde_json::Value {
    value
        .get("context_cache")
        .and_then(|cache| cache.get("error"))
        .cloned()
        .unwrap_or_else(|| {
            compact_json_for_context(
                value
                    .get("error")
                    .cloned()
                    .unwrap_or(serde_json::Value::Null),
            )
        })
}

fn context_output_for_tool_result(value: &serde_json::Value) -> serde_json::Value {
    if value.get("tool_name").and_then(|name| name.as_str()) == Some("command_run") {
        return command_run_summary_for_context(value);
    }
    strip_command_run_context_noise(
        value
            .get("output")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
    )
}

pub(super) fn command_run_summary_for_context(value: &serde_json::Value) -> serde_json::Value {
    let mut output = strip_context_reporting_fields(
        value
            .get("output")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
    );
    strip_read_media_payload_data(&mut output);
    let input = strip_context_reporting_fields(
        value
            .get("input")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
    );
    let input_commands = input
        .get("commands")
        .and_then(|commands| commands.as_array());
    let flattened = flattened_command_run_results(&output);
    if flattened.is_empty() {
        return strip_command_run_context_noise(output);
    }

    let results = flattened
        .into_iter()
        .enumerate()
        .map(|(index, result)| {
            command_run_context_result(
                result,
                input_commands.and_then(|commands| commands.get(index)),
            )
        })
        .collect::<Vec<_>>();
    serde_json::json!({ "results": results })
}

pub(super) fn flattened_command_run_results(output: &serde_json::Value) -> Vec<&serde_json::Value> {
    let Some(results) = output.get("results").and_then(|results| results.as_array()) else {
        return Vec::new();
    };
    let mut flattened = Vec::new();
    for result in results {
        if result.get("mode").and_then(|mode| mode.as_str()) == Some("batch") {
            if let Some(batch_results) =
                result.get("results").and_then(|results| results.as_array())
            {
                flattened.extend(batch_results);
                continue;
            }
        }
        flattened.push(result);
    }
    flattened
}

fn command_run_context_result(
    result: &serde_json::Value,
    input_command: Option<&serde_json::Value>,
) -> serde_json::Value {
    let mut item = serde_json::Map::new();
    let command_type = result
        .get("command_type")
        .or_else(|| result.get("command"))
        .or_else(|| result.get("command_name"))
        .or_else(|| result.get("tool_name"))
        .or_else(|| input_command.and_then(|input| input.get("command_type")))
        .or_else(|| input_command.and_then(|input| input.get("command")))
        .cloned()
        .unwrap_or(serde_json::Value::Null);
    let command_type_name = command_type.as_str().map(ToString::to_string);
    item.insert(
        "success".to_string(),
        result
            .get("success")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
    );
    if let Some(output) = result.get("output") {
        let mut output = strip_command_run_context_noise(output.clone());
        if command_type_name.as_deref() == Some("apply_patch") {
            output = summarize_apply_patch_output_for_context(output);
        }
        item.insert(
            "output".to_string(),
            compact_command_run_context_value(output),
        );
    }
    if let Some(error) = result.get("error") {
        item.insert(
            "error".to_string(),
            compact_command_run_context_value(strip_command_run_context_noise(error.clone())),
        );
    }
    serde_json::Value::Object(item)
}

fn summarize_apply_patch_output_for_context(value: serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(map) if looks_like_patch_change(&map) => {
            summarize_patch_change_object(map)
        }
        serde_json::Value::Object(map) => serde_json::Value::Object(
            map.into_iter()
                .map(|(key, value)| {
                    let value = match key.as_str() {
                        "changes" => summarize_patch_changes_value(value),
                        "failed_change" => summarize_patch_change_value(value),
                        _ => summarize_apply_patch_output_for_context(value),
                    };
                    (key, value)
                })
                .collect(),
        ),
        serde_json::Value::Array(items) => serde_json::Value::Array(
            items
                .into_iter()
                .map(summarize_apply_patch_output_for_context)
                .collect(),
        ),
        other => other,
    }
}

fn summarize_patch_changes_value(value: serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Array(items) => serde_json::Value::Array(
            items
                .into_iter()
                .map(summarize_patch_change_value)
                .collect(),
        ),
        other => summarize_patch_change_value(other),
    }
}

fn summarize_patch_change_value(value: serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(map) => summarize_patch_change_object(map),
        other => serde_json::json!({
            "omitted_from_context": true,
            "value_type": json_value_type(&other),
        }),
    }
}

fn summarize_patch_change_object(
    map: serde_json::Map<String, serde_json::Value>,
) -> serde_json::Value {
    let hunk_count = map
        .get("hunks")
        .and_then(serde_json::Value::as_array)
        .map(|items| items.len());
    let line_count = map.get("hunks").map(count_patch_hunk_lines).unwrap_or(0);
    let mut summary = serde_json::Map::new();
    for key in ["kind", "path"] {
        if let Some(value) = map.get(key).cloned() {
            summary.insert(key.to_string(), value);
        }
    }
    if let Some(count) = hunk_count {
        summary.insert("hunk_count".to_string(), serde_json::json!(count));
    }
    if line_count > 0 {
        summary.insert("line_count".to_string(), serde_json::json!(line_count));
    }
    summary.insert(
        "hunks_omitted_from_context".to_string(),
        serde_json::Value::Bool(true),
    );
    serde_json::Value::Object(summary)
}

fn looks_like_patch_change(map: &serde_json::Map<String, serde_json::Value>) -> bool {
    map.contains_key("hunks") && (map.contains_key("path") || map.contains_key("kind"))
}

fn count_patch_hunk_lines(value: &serde_json::Value) -> usize {
    match value {
        serde_json::Value::String(_) => 1,
        serde_json::Value::Array(items) => items.iter().map(count_patch_hunk_lines).sum(),
        serde_json::Value::Object(map) => map.values().map(count_patch_hunk_lines).sum(),
        _ => 0,
    }
}

fn json_value_type(value: &serde_json::Value) -> &'static str {
    match value {
        serde_json::Value::Null => "null",
        serde_json::Value::Bool(_) => "bool",
        serde_json::Value::Number(_) => "number",
        serde_json::Value::String(_) => "string",
        serde_json::Value::Array(_) => "array",
        serde_json::Value::Object(_) => "object",
    }
}

fn compact_command_run_context_value(value: serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(map) => serde_json::Value::Object(
            map.into_iter()
                .map(|(key, value)| {
                    let value = if matches!(key.as_str(), "stdout" | "stderr" | "output") {
                        compact_command_run_context_stream_value(value)
                    } else {
                        compact_json_for_context(value)
                    };
                    (key, value)
                })
                .collect(),
        ),
        other => compact_json_for_context(other),
    }
}

fn compact_command_run_context_stream_value(value: serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::String(text) => {
            if text.contains("Total output lines:") {
                if text.len() <= COMMAND_RUN_RESULT_OUTPUT_MAX_CHARS {
                    return serde_json::Value::String(text);
                }
                return serde_json::Value::String(truncate_middle_with_char_budget(
                    &text,
                    COMMAND_RUN_RESULT_OUTPUT_MAX_CHARS,
                ));
            }
            serde_json::Value::String(formatted_truncate_text(
                &text,
                COMMAND_RUN_RESULT_OUTPUT_MAX_CHARS,
            ))
        }
        other => compact_json_for_context(other),
    }
}

pub(super) fn strip_tool_reporting_fields(value: serde_json::Value) -> serde_json::Value {
    strip_context_reporting_fields(value)
}

fn compact_json_for_context(value: serde_json::Value) -> serde_json::Value {
    let serialized = serde_json::to_string_pretty(&value).unwrap_or_else(|_| value.to_string());
    if serialized.len() <= context_output_byte_budget() {
        return value;
    }
    serde_json::Value::String(formatted_truncate_text(
        &serialized,
        CONTEXT_OUTPUT_MAX_CHARS,
    ))
}

fn compact_json_to_string(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(text) => text.clone(),
        other => serde_json::to_string_pretty(other).unwrap_or_else(|_| other.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        command_run_cached_context_messages_are_valid, command_run_function_output_for_context,
        command_run_summary_for_context, immutable_tool_result_context_messages,
        tool_result_context_cache,
    };
    use serde_json::{json, Value};

    fn parse_command_run_context(text: &str) -> Value {
        serde_json::from_str(text).expect("command_run context should be structured JSON")
    }

    #[test]
    fn command_run_batch_result_preserves_structured_streams() {
        let context = command_run_summary_for_context(&json!({
            "tool_name": "command_run",
            "input": {
                "commands": [
                    { "step": 1, "command_type": "shell_command", "command_line": "echo ok" }
                ]
            },
            "output": {
                "results": [{
                    "mode": "batch",
                    "results": [{
                        "step": 1,
                        "success": true,
                        "output": {
                            "ok": true,
                            "exit_code": 0,
                            "stdout": "ok\n",
                            "stderr": ""
                        }
                    }]
                }]
            }
        }));

        assert_eq!(context["results"][0]["output"]["exit_code"], 0);
        assert_eq!(context["results"][0]["output"]["stdout"], "ok\n");
        assert_eq!(context["results"][0]["output"]["stderr"], "");
    }

    #[test]
    fn command_run_function_output_is_structured_json_projection() {
        let text = command_run_function_output_for_context(&json!({
            "tool_name": "command_run",
            "input": {
                "commands": [
                    { "step": 1, "command_type": "shell_command", "command_line": "echo ok" }
                ]
            },
            "output": {
                "command_events": [{ "status": "ready", "command_line": "echo ok" }],
                "results": [{
                    "step": 1,
                    "command_type": "shell_command",
                    "success": true,
                    "output": {
                        "ok": true,
                        "exit_code": 0,
                        "stdout": "ok\n",
                        "stderr": ""
                    }
                }]
            }
        }));

        let context = parse_command_run_context(&text);
        assert!(
            text.trim_start().starts_with('{'),
            "expected JSON projection: {text}"
        );
        assert_eq!(context["results"][0]["output"]["stdout"], "ok\n");
        assert_eq!(context["results"][0]["output"]["stderr"], "");
        assert!(
            !text.contains("ready"),
            "ready event leaked into model context: {text}"
        );
        assert!(context["results"][0].get("step").is_none());
        assert!(context["results"][0].get("command_type").is_none());
        assert!(context["results"][0].get("command_line").is_none());
    }

    #[test]
    fn command_run_single_task_status_is_replayed_in_backfill() {
        let messages = immutable_tool_result_context_messages(&json!({
            "tool_name": "command_run",
            "provider_metadata": { "id": "call_task_status_only" },
            "input": {
                "commands": [{
                    "step": 1,
                    "command": "task_status",
                    "task_group": "runtime backfill",
                    "status": "doing"
                }]
            },
            "output": {
                "results": [{
                    "step": 1,
                    "command_type": "task_status",
                    "success": true,
                    "output": {
                        "task_status": {
                            "task_group": "runtime backfill",
                            "status": "doing",
                            "task_type": ["debug"]
                        }
                    }
                }]
            }
        }));

        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0]["type"], "function_call");
        assert_eq!(messages[0]["name"], "command_run");
        assert_eq!(messages[0]["call_id"], "call_task_status_only");
        let arguments: Value =
            serde_json::from_str(messages[0]["arguments"].as_str().expect("arguments string"))
                .expect("arguments JSON");
        assert_eq!(arguments["commands"][0]["command"], "task_status");
        assert_eq!(arguments["commands"][0]["task_group"], "runtime backfill");

        assert_eq!(messages[1]["type"], "function_call_output");
        assert_eq!(messages[1]["call_id"], "call_task_status_only");
        let output = messages[1]["output"].as_str().expect("output JSON string");
        let output = parse_command_run_context(output);
        assert_eq!(output["results"].as_array().expect("results").len(), 1);
        assert!(output["results"][0].get("step").is_none());
        assert!(output["results"][0].get("command_type").is_none());
        assert!(output["results"][0].get("command_line").is_none());
        assert_eq!(
            output["results"][0]["output"]["task_status"]["task_group"],
            "runtime backfill"
        );
        assert_eq!(
            output["results"][0]["output"]["task_status"]["status"],
            "doing"
        );
        assert_eq!(
            output["results"][0]["output"]["task_status"]["task_type"],
            json!(["debug"])
        );
    }

    #[test]
    fn command_run_refill_records_match_value_and_raw_byte_fixture() {
        let messages = immutable_tool_result_context_messages(&json!({
            "tool_name": "command_run",
            "provider_metadata": { "id": "call_phase0_refill" },
            "input": {
                "commands": [{
                    "step": 1,
                    "command": "shell_command",
                    "command_line": "printf phase0"
                }]
            },
            "output": {
                "results": [{
                    "step": 1,
                    "command_type": "shell_command",
                    "success": true,
                    "output": {
                        "exit_code": 0,
                        "stdout": "phase0",
                        "stderr": ""
                    }
                }]
            }
        }));
        let actual_raw = serde_json::to_vec(&messages).expect("serialize refill records");
        let expected_fixture =
            include_bytes!("../../tests/fixtures/llm_boundary/command_run_refill_records.json");
        let expected_raw = expected_fixture
            .strip_suffix(b"\n")
            .expect("refill fixture must end with one LF framing byte");
        assert_ne!(
            expected_raw.last(),
            Some(&b'\r'),
            "refill fixture must use LF framing, not CRLF"
        );
        let expected: Value =
            serde_json::from_slice(expected_raw).expect("valid refill fixture JSON");

        assert_eq!(
            Value::Array(messages),
            expected,
            "refill record value changed"
        );
        assert_eq!(actual_raw, expected_raw, "refill record raw bytes changed");
    }

    #[test]
    fn command_run_shell_failure_keeps_actionable_error_text() {
        let text = command_run_function_output_for_context(&json!({
            "tool_name": "command_run",
            "input": {
                "commands": [
                    { "step": 1, "command_type": "shell_command", "command_line": "cargo test -p runtime nope" }
                ]
            },
            "output": {
                "results": [{
                    "step": 1,
                    "command_type": "shell_command",
                    "success": false,
                    "output": {
                        "ok": false,
                        "exit_code": 101,
                        "stdout": "running 1 test\n",
                        "stderr": "error[E0425]: cannot find value `projection` in this scope\n"
                    }
                }]
            }
        }));

        let context = parse_command_run_context(&text);
        assert_eq!(context["results"][0]["success"], false);
        assert!(context["results"][0].get("command_line").is_none());
        assert_eq!(context["results"][0]["output"]["exit_code"], 101);
        assert_eq!(
            context["results"][0]["output"]["stdout"],
            "running 1 test\n"
        );
        assert_eq!(
            context["results"][0]["output"]["stderr"],
            "error[E0425]: cannot find value `projection` in this scope\n"
        );
    }

    #[test]
    fn command_run_shell_result_uses_flat_structured_output() {
        let text = command_run_function_output_for_context(&json!({
            "tool_name": "command_run",
            "input": {
                "commands": [
                    { "step": 1, "command_type": "shell_command", "command_line": "echo ok" }
                ]
            },
            "output": {
                "results": [{
                    "step": 1,
                    "command_type": "shell_command",
                    "success": true,
                    "output": {
                        "exit_code": 0,
                        "stdout": "ok\n",
                        "stderr": ""
                    }
                }]
            }
        }));

        let context = parse_command_run_context(&text);
        assert!(context["results"][0].get("command_line").is_none());
        assert_eq!(context["results"][0]["output"]["exit_code"], 0);
        assert_eq!(context["results"][0]["output"]["stdout"], "ok\n");
        assert_eq!(context["results"][0]["output"]["stderr"], "");
        assert!(context["results"][0]["output"].get("output").is_none());
        assert!(context["results"][0]["output"].get("cli_output").is_none());
    }

    #[test]
    fn command_run_apply_patch_success_keeps_structured_changes() {
        let text = command_run_function_output_for_context(&json!({
            "tool_name": "command_run",
            "input": {
                "commands": [{
                    "step": 1,
                    "command_type": "apply_patch",
                    "command_line": "patch body omitted for renderer test"
                }]
            },
            "output": {
                "results": [{
                    "step": 1,
                    "command_type": "apply_patch",
                    "success": true,
                    "output": {
                        "ok": true,
                        "exit_code": 0,
                        "stdout": "Success. Updated files.",
                        "stderr": "",
                        "changes": [{
                            "kind": "update",
                            "path": "app.txt",
                            "hunks": [["-old", "+new"]]
                        }]
                    }
                }]
            }
        }));

        let context = parse_command_run_context(&text);
        assert!(context["results"][0].get("step").is_none());
        assert!(context["results"][0].get("command_type").is_none());
        assert!(context["results"][0].get("command_line").is_none());
        assert_eq!(
            context["results"][0]["output"]["stdout"],
            "Success. Updated files."
        );
        assert_eq!(context["results"][0]["output"]["stderr"], "");
        assert_eq!(
            context["results"][0]["output"]["changes"][0]["path"],
            "app.txt"
        );
        assert_eq!(
            context["results"][0]["output"]["changes"][0]["hunk_count"],
            1
        );
        assert_eq!(
            context["results"][0]["output"]["changes"][0]["line_count"],
            2
        );
        assert!(context["results"][0]["output"]["changes"][0]
            .get("hunks")
            .is_none());
    }

    #[test]
    fn command_run_apply_patch_failure_keeps_structured_failure_context() {
        let text = command_run_function_output_for_context(&json!({
            "tool_name": "command_run",
            "input": {
                "commands": [{
                    "step": 1,
                    "command_type": "apply_patch",
                    "command_line": "patch body omitted for renderer test"
                }]
            },
            "output": {
                "results": [{
                    "step": 1,
                    "command_type": "apply_patch",
                    "success": false,
                    "output": {
                        "ok": false,
                        "exit_code": 1,
                        "stdout": "",
                        "stderr": "ContextMismatch: app.txt",
                        "output": {
                            "error_type": "ContextMismatch",
                            "message": "hunk context did not match",
                            "failed_change": {
                                "kind": "update",
                                "path": "app.txt",
                                "hunks": [[" old", "-missing", "+new"]]
                            }
                        }
                    }
                }]
            }
        }));

        let context = parse_command_run_context(&text);
        assert_eq!(context["results"][0]["success"], false);
        assert!(context["results"][0].get("command_line").is_none());
        assert_eq!(
            context["results"][0]["output"]["stderr"],
            "ContextMismatch: app.txt"
        );
        assert_eq!(
            context["results"][0]["output"]["output"]["failed_change"]["path"],
            "app.txt"
        );
        assert_eq!(
            context["results"][0]["output"]["output"]["failed_change"]["hunk_count"],
            1
        );
        assert_eq!(
            context["results"][0]["output"]["output"]["failed_change"]["line_count"],
            3
        );
        assert!(context["results"][0]["output"]["output"]["failed_change"]
            .get("hunks")
            .is_none());
    }

    #[test]
    fn command_run_search_output_keeps_path_line_matches() {
        let text = command_run_function_output_for_context(&json!({
            "tool_name": "command_run",
            "input": {
                "commands": [{
                    "step": 1,
                    "command_type": "rg",
                    "command_line": "{\"pattern\":\"needle\",\"path\":\"src\"}"
                }]
            },
            "output": {
                "results": [{
                    "step": 1,
                    "command_type": "rg",
                    "success": true,
                    "output": {
                        "ok": true,
                        "exit_code": 0,
                        "stdout": "{\"results\":[{\"matches\":[{\"path\":\"src/lib.rs\",\"line_number\":12,\"content\":\"let needle = true;\"}]}]}",
                        "stderr": ""
                    }
                }]
            }
        }));

        let context = parse_command_run_context(&text);
        assert!(context["results"][0].get("command_line").is_none());
        assert_eq!(
            context["results"][0]["output"]["stdout"],
            "{\"results\":[{\"matches\":[{\"path\":\"src/lib.rs\",\"line_number\":12,\"content\":\"let needle = true;\"}]}]}"
        );
    }

    #[test]
    fn command_run_large_output_stays_structured_with_single_total_output_header() {
        let long_output = (0..1200)
            .map(|index| format!("line {index}"))
            .collect::<Vec<_>>()
            .join("\n");
        let value = json!({
            "tool_name": "command_run",
            "input": {
                "commands": [{ "step": 1, "command_type": "shell_command", "command_line": "long-output" }]
            },
            "output": {
                "results": [{
                    "step": 1,
                    "command_type": "shell_command",
                    "success": true,
                    "output": {
                        "ok": true,
                        "exit_code": 0,
                        "stdout": long_output,
                        "stderr": ""
                    }
                }]
            }
        });

        let text = command_run_function_output_for_context(&value);
        let rendered_again = command_run_function_output_for_context(&json!({
            "tool_name": "command_run",
            "input": value["input"].clone(),
            "output": {
                "results": [{
                    "step": 1,
                    "command_type": "shell_command",
                    "success": true,
                    "output": {
                        "ok": true,
                        "exit_code": 0,
                        "stdout": text,
                        "stderr": ""
                    }
                }]
            }
        }));

        assert_eq!(text.matches("Total output lines:").count(), 1, "{text}");
        assert_eq!(
            rendered_again.matches("Total output lines:").count(),
            1,
            "{rendered_again}"
        );
        assert_eq!(
            parse_command_run_context(&text)["results"][0]["output"]["stderr"],
            ""
        );
    }

    #[test]
    fn command_run_context_replays_provider_tool_call_pair() {
        let mut value = json!({
            "tool_name": "command_run",
            "provider_metadata": { "id": "call_provider_123" },
            "context_cache": { "cache_id": "abc123stable" },
            "input": {
                "commands": [
                    { "step": 1, "command_type": "shell_command", "command_line": "echo ok" }
                ]
            },
            "output": {
                "results": [{
                    "step": 1,
                    "success": true,
                    "output": {
                        "ok": true,
                        "exit_code": 0,
                        "stdout": "ok\n",
                        "stderr": ""
                    }
                }]
            }
        });
        let messages = immutable_tool_result_context_messages(&value);

        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0]["type"], "function_call");
        assert_eq!(messages[0]["call_id"], "call_provider_123");
        assert_eq!(messages[0]["name"], "command_run");
        assert_eq!(messages[1]["type"], "function_call_output");
        assert_eq!(messages[1]["call_id"], "call_provider_123");
        let arguments: Value =
            serde_json::from_str(messages[0]["arguments"].as_str().expect("arguments string"))
                .expect("arguments JSON");
        assert_eq!(arguments["commands"][0]["command_line"], "echo ok");
        let output = messages[1]["output"].as_str().expect("output JSON string");
        let output = parse_command_run_context(output);
        assert!(output["results"][0].get("step").is_none());
        assert!(output["results"][0].get("command_type").is_none());
        assert!(output["results"][0].get("command_line").is_none());
        value["context_cache"] = tool_result_context_cache(&value);
        assert!(command_run_function_output_for_context(&json!({
            "tool_name": "command_run",
            "output": {
                "results": [{
                    "step": 1,
                    "success": true,
                    "output": {
                        "ok": true,
                        "exit_code": 0,
                        "stdout": "ok\n",
                        "stderr": ""
                    }
                }]
            }
        }))
        .contains("\"stdout\": \"ok\\n\""));
    }

    #[test]
    fn command_run_provider_replay_uses_paired_call_for_all_command_types() {
        for command_type in [
            "apply_patch",
            "shell_command",
            "bash",
            "zsh",
            "generate_media",
            "web_discover",
            "planning",
            "task_status",
            "read_media",
        ] {
            let value = json!({
                "tool_name": "command_run",
                "provider_metadata": { "id": format!("call_{command_type}") },
                "input": {
                    "commands": [{
                        "step": 1,
                        "command_type": command_type,
                        "command_line": format!("input for {command_type}")
                    }]
                },
                "output": {
                    "results": [{
                        "step": 1,
                        "command_type": command_type,
                        "success": true,
                        "output": {
                            "ok": true,
                            "exit_code": 0,
                            "stdout": format!("output for {command_type}\n"),
                            "stderr": ""
                        }
                    }]
                }
            });
            let messages = immutable_tool_result_context_messages(&value);
            assert_eq!(messages.len(), 2, "{command_type}");
            assert_eq!(messages[0]["type"], "function_call", "{command_type}");
            assert_eq!(messages[0]["name"], "command_run", "{command_type}");
            assert_eq!(messages[0]["call_id"], format!("call_{command_type}"));
            let arguments: Value =
                serde_json::from_str(messages[0]["arguments"].as_str().expect("arguments string"))
                    .expect("arguments JSON");
            assert_eq!(arguments["commands"][0]["command_type"], command_type);
            assert_eq!(
                arguments["commands"][0]["command_line"],
                format!("input for {command_type}")
            );

            assert_eq!(
                messages[1]["type"], "function_call_output",
                "{command_type}"
            );
            assert_eq!(messages[1]["call_id"], format!("call_{command_type}"));
            let output = messages[1]["output"]
                .as_str()
                .or_else(|| {
                    messages[1]["output"]
                        .as_array()
                        .and_then(|items| items.first())
                        .and_then(|item| item.get("text"))
                        .and_then(Value::as_str)
                })
                .expect("output JSON string or media text item");
            let output = parse_command_run_context(output);
            assert!(output["results"][0].get("step").is_none(), "{command_type}");
            assert!(
                output["results"][0].get("command_type").is_none(),
                "{command_type}"
            );
            assert!(
                output["results"][0].get("command_line").is_none(),
                "{command_type}"
            );
        }
    }

    #[test]
    fn command_run_read_media_replay_uses_paired_call_with_media_content() {
        let value = json!({
            "tool_name": "command_run",
            "provider_metadata": { "id": "call_read_media" },
            "input": {
                "commands": [{
                    "step": 1,
                    "command_type": "read_media",
                    "command_line": "read_media image.png"
                }]
            },
            "output": {
                "results": [{
                    "step": 1,
                    "command_type": "read_media",
                    "success": true,
                    "output": {
                        "summary": "image preview",
                        "visual_preview_count": 1,
                        "visual_previews": [{
                            "type": "image_url",
                            "image_url": { "url": "data:image/png;base64,AAA" }
                        }]
                    }
                }]
            }
        });
        let messages = immutable_tool_result_context_messages(&value);
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0]["type"], "function_call");
        assert_eq!(messages[0]["name"], "command_run");
        let arguments: Value =
            serde_json::from_str(messages[0]["arguments"].as_str().expect("arguments string"))
                .expect("arguments JSON");
        assert_eq!(
            arguments["commands"][0]["command_line"],
            "read_media image.png"
        );
        let output = messages[1]["output"]
            .as_array()
            .expect("media output array");
        assert_eq!(output[0]["type"], "input_text");
        assert!(output.iter().any(|item| item["type"] == "input_image"));
        assert!(messages[0].to_string().contains("read_media image.png"));
    }

    #[test]
    fn command_run_cached_context_messages_require_paired_call_with_arguments() {
        let old_messages = vec![
            json!({
                "type": "function_call",
                "call_id": "call_old",
                "name": "command_run",
                "arguments": "{\"commands\":[{\"command_type\":\"shell_command\",\"command_line\":\"echo old\"}]}"
            }),
            json!({
                "type": "function_call_output",
                "call_id": "call_old",
                "output": "{\"results\":[{\"command_type\":\"shell_command\",\"command_line\":\"echo old\"}]}"
            }),
        ];
        assert!(!command_run_cached_context_messages_are_valid(
            &old_messages
        ));
        let old_type_only_messages = vec![
            json!({
                "type": "function_call",
                "call_id": "call_old_type",
                "name": "command_run",
                "arguments": "{\"commands\":[{\"command_type\":\"shell_command\"}]}"
            }),
            json!({
                "type": "function_call_output",
                "call_id": "call_old_type",
                "output": "{\"results\":[{\"command_type\":\"shell_command\",\"command_line\":\"echo old\"}]}"
            }),
        ];
        assert!(!command_run_cached_context_messages_are_valid(
            &old_type_only_messages
        ));
        let orphan_output_messages = vec![json!({
            "type": "function_call_output",
            "call_id": "call_orphan",
            "output": "{\"results\":[{\"command_type\":\"shell_command\",\"command_line\":\"echo old\"}]}"
        })];
        assert!(!command_run_cached_context_messages_are_valid(
            &orphan_output_messages
        ));
        let old_empty_anchor_messages = vec![
            json!({
                "type": "function_call",
                "call_id": "call_old_empty",
                "name": "command_run",
                "arguments": "{}"
            }),
            json!({
                "type": "function_call_output",
                "call_id": "call_old_empty",
                "output": "{\"results\":[{\"command_type\":\"shell_command\",\"command_line\":\"echo old\"}]}"
            }),
        ];
        assert!(!command_run_cached_context_messages_are_valid(
            &old_empty_anchor_messages
        ));
        for (field, value) in [
            ("command", "shell_command"),
            ("command_name", "shell_command"),
        ] {
            let old_alias_messages = vec![
                json!({
                    "type": "function_call",
                    "call_id": format!("call_old_{field}"),
                    "name": "command_run",
                    "arguments": serde_json::json!({
                        "commands": [{ field: value }]
                    }).to_string()
                }),
                json!({
                    "type": "function_call_output",
                    "call_id": format!("call_old_{field}"),
                    "output": "{\"results\":[{\"command_type\":\"shell_command\",\"command_line\":\"echo old\"}]}"
                }),
            ];
            assert!(
                !command_run_cached_context_messages_are_valid(&old_alias_messages),
                "cached context with {field} duplicated in output must be rebuilt"
            );
        }

        let new_messages = immutable_tool_result_context_messages(&json!({
            "tool_name": "command_run",
            "provider_metadata": { "id": "call_new" },
            "input": {
                "commands": [
                    { "step": 1, "command_type": "shell_command", "command_line": "echo new" }
                ]
            },
            "output": {
                "results": [{
                    "step": 1,
                    "command_type": "shell_command",
                    "success": true,
                    "output": {
                        "ok": true,
                        "exit_code": 0,
                        "stdout": "new\n",
                        "stderr": ""
                    }
                }]
            }
        }));
        assert!(command_run_cached_context_messages_are_valid(&new_messages));
        assert_eq!(new_messages.len(), 2);
        assert_eq!(new_messages[0]["type"], "function_call");
        assert_eq!(new_messages[1]["type"], "function_call_output");
    }

    #[test]
    fn command_run_context_without_provider_metadata_uses_plain_user_context() {
        let messages = immutable_tool_result_context_messages(&json!({
            "tool_name": "command_run",
            "input": {
                "commands": [
                    { "step": 1, "command_type": "shell_command", "command_line": "echo ok" }
                ]
            },
            "output": {
                "results": [{
                    "step": 1,
                    "success": true,
                    "output": {
                        "ok": true,
                        "exit_code": 0,
                        "stdout": "ok\n",
                        "stderr": ""
                    }
                }]
            }
        }));

        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0]["role"], "user");
        assert!(messages[0].get("type").is_none());
        assert!(messages[0]["content"]
            .as_str()
            .is_some_and(|content| content.contains("\"stdout\": \"ok\\n\"")));
    }

    #[test]
    fn command_run_context_cache_ignores_runtime_reporting_fields() {
        let base = json!({
            "type": "tool_result",
            "tool_name": "command_run",
            "sequence": 7,
            "input": {
                "commands": [{
                    "step": 1,
                    "command_type": "shell_command",
                    "command_line": "echo ok",
                    "command_id": "runtime-a:call-a:0",
                    "command_run_id": "runtime-a",
                    "provider_tool_call_id": "call-a",
                    "command_index": 0,
                    "createdAt": 1,
                    "updatedAt": 2
                }]
            },
            "output": {
                "command_updates": [{
                    "messageID": "message-a",
                    "partID": "part-a",
                    "runtimeID": "runtime-a",
                    "commandRunID": "runtime-a",
                    "commandID": "runtime-a:call-a:0",
                    "providerToolCallID": "call-a",
                    "commandIndex": 0,
                    "eventSeq": 20,
                    "createdAt": 1,
                    "updatedAt": 2,
                    "command": {
                        "command_id": "runtime-a:call-a:0",
                        "command_run_id": "runtime-a",
                        "provider_tool_call_id": "call-a",
                        "command_index": 0,
                        "command_type": "shell_command",
                        "command_line": "echo ok"
                    }
                }],
                "results": [{
                    "step": 1,
                    "success": true,
                    "command_id": "runtime-a:call-a:0",
                    "command_run_id": "runtime-a",
                    "provider_tool_call_id": "call-a",
                    "command_index": 0,
                    "result_index": 0,
                    "runtime_id": "runtime-a",
                    "timestamp": "2026-06-20T00:00:00Z",
                    "command": {
                        "command_id": "runtime-a:call-a:0",
                        "command_run_id": "runtime-a",
                        "provider_tool_call_id": "call-a",
                        "command_index": 0,
                        "command_type": "shell_command",
                        "command_line": "echo ok"
                    },
                    "output": {
                        "ok": true,
                        "exit_code": 0,
                        "stdout": "ok\n",
                        "stderr": ""
                    }
                }]
            },
            "success": true,
            "error": null,
            "runtime_id": "runtime-a",
            "provider_metadata": { "id": "call-provider-a" },
            "timestamp": "2026-06-20T00:00:00Z"
        });
        let mut variant = base.clone();
        variant["input"]["commands"][0]["command_id"] = json!("runtime-b:call-b:0");
        variant["input"]["commands"][0]["command_run_id"] = json!("runtime-b");
        variant["input"]["commands"][0]["provider_tool_call_id"] = json!("call-b");
        variant["input"]["commands"][0]["updatedAt"] = json!(99);
        variant["output"]["command_updates"][0]["messageID"] = json!("message-b");
        variant["output"]["command_updates"][0]["runtimeID"] = json!("runtime-b");
        variant["output"]["command_updates"][0]["commandID"] = json!("runtime-b:call-b:0");
        variant["output"]["command_updates"][0]["providerToolCallID"] = json!("call-b");
        variant["output"]["command_updates"][0]["updatedAt"] = json!(99);
        variant["output"]["results"][0]["command_id"] = json!("runtime-b:call-b:0");
        variant["output"]["results"][0]["command_run_id"] = json!("runtime-b");
        variant["output"]["results"][0]["provider_tool_call_id"] = json!("call-b");
        variant["output"]["results"][0]["runtime_id"] = json!("runtime-b");
        variant["output"]["results"][0]["timestamp"] = json!("2026-06-21T00:00:00Z");
        variant["runtime_id"] = json!("runtime-b");
        variant["provider_metadata"] = json!({ "id": "call-provider-b" });
        variant["timestamp"] = json!("2026-06-21T00:00:00Z");

        let base_cache = tool_result_context_cache(&base);
        let variant_cache = tool_result_context_cache(&variant);
        assert_eq!(base_cache["cache_id"], variant_cache["cache_id"]);

        let mut with_cache = base;
        with_cache["context_cache"] = base_cache;
        let context = serde_json::to_string(&immutable_tool_result_context_messages(&with_cache))
            .expect("context messages should serialize");
        for forbidden in [
            "command_id",
            "command_run_id",
            "provider_tool_call_id",
            "command_index",
            "result_index",
            "command_updates",
            "messageID",
            "partID",
            "runtimeID",
            "commandID",
            "providerToolCallID",
            "createdAt",
            "updatedAt",
            "runtime_id",
            "timestamp",
        ] {
            assert!(
                !context.contains(forbidden),
                "context should not contain volatile field/value {forbidden}: {context}"
            );
        }
    }
}
