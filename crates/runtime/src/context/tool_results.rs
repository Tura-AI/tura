use super::command_run_streams::{command_run_display_command, command_run_llm_streams};
use super::text_truncate::command_run_truncate_text;
use super::token_budget::{
    context_output_byte_budget, formatted_truncate_text, APPROX_CHARS_PER_TOKEN,
    COMMAND_RUN_RESULT_OUTPUT_MAX_TOKENS, CONTEXT_OUTPUT_MAX_TOKENS,
};
use crate::state_machine::session_management::SessionManagement;

use super::media::command_run_media_content_items_for_context;

pub(super) fn strip_context_reporting_fields(value: serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(map) => serde_json::Value::Object(
            map.into_iter()
                .filter(|(key, _)| !is_context_reporting_field(key))
                .map(|(key, value)| (key, strip_context_reporting_fields(value)))
                .collect(),
        ),
        serde_json::Value::Array(items) => serde_json::Value::Array(
            items
                .into_iter()
                .map(strip_context_reporting_fields)
                .collect(),
        ),
        other => other,
    }
}

fn is_context_reporting_field(key: &str) -> bool {
    matches!(
        key,
        "task_detail"
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
        return command_run_responses_api_context_items(value);
    }
    vec![immutable_tool_result_context_message(value)]
}

fn command_run_responses_api_context_items(value: &serde_json::Value) -> Vec<serde_json::Value> {
    let call_id = command_run_context_call_id(value);
    let input = strip_context_reporting_fields(
        value
            .get("input")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
    );
    let arguments = serde_json::to_string(&input).unwrap_or_else(|_| "{}".to_string());
    let function_call = serde_json::json!({
        "type": "function_call",
        "name": "command_run",
        "arguments": arguments,
        "call_id": call_id,
        "status": "completed",
    });
    vec![
        function_call,
        serde_json::json!({
            "type": "function_call_output",
            "call_id": call_id,
            "output": command_run_function_output_payload_for_context(value),
        }),
    ]
}

fn command_run_function_output_context_message(value: &serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "role": "user",
        "content": command_run_function_output_for_context(value),
    })
}

fn command_run_context_call_id(value: &serde_json::Value) -> String {
    let cache_id = value
        .get("context_cache")
        .and_then(|cache| cache.get("cache_id"))
        .and_then(|cache_id| cache_id.as_str())
        .unwrap_or("command_run");
    let suffix = cache_id
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .take(24)
        .collect::<String>();
    format!("call_{suffix}")
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

#[derive(serde::Serialize)]
struct CommandRunContextOutput {
    results: Vec<CommandRunContextItem>,
}

#[derive(serde::Serialize)]
struct CommandRunContextItem {
    step: serde_json::Value,
    command_type: serde_json::Value,
    success: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    output: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

pub(super) fn command_run_current_style_output_string(value: &serde_json::Value) -> Option<String> {
    let output = strip_context_reporting_fields(
        value
            .get("output")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
    );
    let input = strip_context_reporting_fields(
        value
            .get("input")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
    );
    let input_commands = input
        .get("commands")
        .and_then(|commands| commands.as_array());
    let results = flattened_command_run_results(&output)
        .into_iter()
        .enumerate()
        .map(|(index, result)| {
            let input = input_commands.and_then(|commands| commands.get(index));
            let command_type = result
                .get("command_type")
                .or_else(|| result.get("command"))
                .or_else(|| result.get("command_name"))
                .or_else(|| result.get("tool_name"))
                .or_else(|| input.and_then(|input| input.get("command_type")))
                .or_else(|| input.and_then(|input| input.get("command")))
                .cloned()
                .unwrap_or(serde_json::Value::Null);
            let output = result
                .get("output")
                .map(|output| command_run_model_output_value(output, input))
                .unwrap_or_else(|| {
                    serde_json::Value::String(command_run_result_transcript(result, input))
                });
            let error = result
                .get("error")
                .and_then(serde_json::Value::as_str)
                .map(|error| formatted_truncate_text(error, COMMAND_RUN_RESULT_OUTPUT_MAX_TOKENS));
            CommandRunContextItem {
                step: result
                    .get("step")
                    .cloned()
                    .unwrap_or(serde_json::Value::Null),
                command_type,
                success: result
                    .get("success")
                    .cloned()
                    .unwrap_or(serde_json::Value::Null),
                output: Some(compact_command_run_result_output(output, input)),
                error,
            }
        })
        .collect::<Vec<_>>();
    if results.is_empty() {
        return None;
    }
    serde_json::to_string_pretty(&CommandRunContextOutput { results }).ok()
}

fn compact_command_run_result_output(
    value: serde_json::Value,
    input: Option<&serde_json::Value>,
) -> serde_json::Value {
    match value {
        serde_json::Value::String(text) => serde_json::Value::String(command_run_truncate_text(
            &text,
            COMMAND_RUN_RESULT_OUTPUT_MAX_TOKENS,
            command_line_from_input(input),
        )),
        other => {
            let serialized =
                serde_json::to_string_pretty(&other).unwrap_or_else(|_| other.to_string());
            if serialized.len() <= COMMAND_RUN_RESULT_OUTPUT_MAX_TOKENS * APPROX_CHARS_PER_TOKEN {
                other
            } else {
                serde_json::Value::String(command_run_truncate_text(
                    &serialized,
                    COMMAND_RUN_RESULT_OUTPUT_MAX_TOKENS,
                    command_line_from_input(input),
                ))
            }
        }
    }
}

fn command_run_model_output_value(
    value: &serde_json::Value,
    input: Option<&serde_json::Value>,
) -> serde_json::Value {
    if let Some(text) = value.get("output").and_then(serde_json::Value::as_str) {
        return serde_json::Value::String(command_run_truncate_text(
            text,
            COMMAND_RUN_RESULT_OUTPUT_MAX_TOKENS,
            command_line_from_input(input),
        ));
    }
    compact_command_run_result_output(value.clone(), input)
}

fn command_line_from_input(input: Option<&serde_json::Value>) -> Option<&str> {
    input
        .and_then(|input| input.get("command_line"))
        .and_then(serde_json::Value::as_str)
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
    let output = strip_context_reporting_fields(
        value
            .get("output")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
    );
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

    let mut transcript = Vec::new();
    for (index, result) in flattened.into_iter().enumerate() {
        let input = input_commands.and_then(|commands| commands.get(index));
        transcript.push(command_run_result_transcript(result, input));
    }
    serde_json::Value::String(transcript.join("\n\n"))
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

fn command_run_result_transcript(
    result: &serde_json::Value,
    input_command: Option<&serde_json::Value>,
) -> String {
    let Some(response) = result.get("response") else {
        return compact_json_to_string(&strip_command_run_context_noise(result.clone()));
    };
    let command = input_command
        .and_then(|input| input.get("command_type"))
        .or_else(|| input_command.and_then(|input| input.get("command")))
        .or_else(|| result.get("command_type"))
        .or_else(|| result.get("command_name"))
        .or_else(|| result.get("tool_name"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or("command");
    let command_line = input_command
        .and_then(|input| input.get("command_line"))
        .or_else(|| result.get("command_code"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    let ok = result
        .get("ok")
        .or_else(|| response.get("ok"))
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(true);
    let exit_code = response
        .get("exit_code")
        .and_then(serde_json::Value::as_i64)
        .unwrap_or(if ok { 0 } else { 1 });
    let stdout = response
        .get("stdout")
        .or_else(|| result.get("stdout"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    let stderr = response
        .get("stderr")
        .or_else(|| result.get("stderr"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();

    let display_command = command_run_display_command(command, command_line);
    let mut text = format!("$ {display_command}\nExit code: {exit_code}");
    let (stdout, structured_stderr) = command_run_llm_streams(command, stdout);
    if !stdout.trim().is_empty() {
        text.push_str("\nOutput:\n");
        text.push_str(stdout.trim_end());
    }
    let stderr = [stderr.trim_end(), structured_stderr.trim_end()]
        .into_iter()
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>()
        .join("\n");
    if !stderr.trim().is_empty() {
        text.push_str("\nStderr:\n");
        text.push_str(stderr.trim_end());
    }
    text
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
        CONTEXT_OUTPUT_MAX_TOKENS,
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
        command_run_function_output_for_context, command_run_responses_api_context_items,
        command_run_summary_for_context, tool_result_context_cache,
    };
    use serde_json::json;

    #[test]
    fn command_run_batch_result_becomes_transcript() {
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
                        "response": {
                            "ok": true,
                            "exit_code": 0,
                            "stdout": "ok\n",
                            "stderr": ""
                        }
                    }]
                }]
            }
        }));

        let text = context.as_str().expect("command_run context is text");
        assert!(text.contains("$ echo ok"));
        assert!(text.contains("Exit code: 0"));
        assert!(text.contains("ok"));
    }

    #[test]
    fn command_run_context_uses_stable_cache_call_id() {
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
                    "response": {
                        "ok": true,
                        "exit_code": 0,
                        "stdout": "ok\n",
                        "stderr": ""
                    }
                }]
            }
        });
        let messages = command_run_responses_api_context_items(&value);

        assert_eq!(messages[0]["call_id"], "call_abc123stable");
        assert_eq!(messages[1]["call_id"], "call_abc123stable");
        assert!(messages[0].get("provider_metadata").is_none());
        value["context_cache"] = tool_result_context_cache(&value);
        assert!(command_run_function_output_for_context(&json!({
            "tool_name": "command_run",
            "output": {
                "results": [{
                    "step": 1,
                    "success": true,
                    "response": {
                        "ok": true,
                        "exit_code": 0,
                        "stdout": "ok\n",
                        "stderr": ""
                    }
                }]
            }
        }))
        .contains("Exit code: 0"));
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
                    "response": {
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

        let mut with_cache = base.clone();
        with_cache["context_cache"] = base_cache;
        let context = serde_json::to_string(&command_run_responses_api_context_items(&with_cache))
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
            "call-provider-a",
        ] {
            assert!(
                !context.contains(forbidden),
                "context should not contain volatile field/value {forbidden}: {context}"
            );
        }
    }
}
