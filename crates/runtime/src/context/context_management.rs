use chrono::Utc;
use tracing::info;

use crate::state_machine::runtime_management::RuntimeManagement;
use crate::state_machine::session_management::SessionManagement;

use super::{types::ContextState, ContextualUserFragment, WorkspaceSnapshot};

const CONTEXT_OUTPUT_MAX_TOKENS: usize = 2_500;
const COMMAND_RUN_RESULT_OUTPUT_MAX_TOKENS: usize = 2_500;
const APPROX_CHARS_PER_TOKEN: usize = 4;
#[derive(Debug, Clone)]
pub struct ContextInput {
    pub session: SessionManagement,
    pub runtime: RuntimeManagement,
    pub additional_messages: Vec<serde_json::Value>,
}

#[derive(Debug, Clone)]
pub struct ContextOutput {
    pub session: SessionManagement,
    pub messages: Vec<serde_json::Value>,
    pub context_state: ContextState,
}

pub fn messages_with_runtime_context(
    session: &SessionManagement,
    messages: &[serde_json::Value],
    provider_name: Option<&str>,
    model_name: Option<&str>,
    is_first_llm_call: bool,
) -> Vec<serde_json::Value> {
    let mut output = Vec::with_capacity(messages.len() + 1);
    output.extend(messages.iter().cloned());
    let _ = (session, provider_name, model_name, is_first_llm_call);
    output
}

pub fn build_context(input: ContextInput) -> Result<ContextOutput, String> {
    let mut messages = build_messages_from_session_with_options(&input.session);

    let mut context_state = ContextState {
        session_id: input.session.session_id.clone(),
        messages: Vec::new(),
        tool_results: Vec::new(),
        last_tool_call_response: None,
        reasoning_history: Vec::new(),
    };

    if messages.is_empty() {
        if let Some(reasoning) = &input.runtime.reasoning {
            if !reasoning.is_empty() {
                context_state.reasoning_history.push(reasoning.clone());
                messages.push(serde_json::json!({
                    "role": "system",
                    "type": "reasoning",
                    "content": reasoning,
                }));
            }
        }

        if !input.runtime.text.is_empty() {
            messages.push(serde_json::json!({
                "role": "assistant",
                "content": input.runtime.text,
            }));
        }
    } else if let Some(reasoning) = &input.runtime.reasoning {
        if !reasoning.is_empty() {
            context_state.reasoning_history.push(reasoning.clone());
        }
    }

    for tool_call in &input.runtime.tool_call {
        context_state.tool_results.push(serde_json::json!({
            "tool_name": tool_call.tool_called_name,
            "input": tool_call.tool_called_input,
            "summary": tool_call.agent_reported_summary,
            "success": tool_call.tool_reported_success,
        }));
    }

    if input.session.use_last_tool_call_response {
        if let Some(last_tool_call_response) = last_tool_call_response_from_session(&input.session)
        {
            context_state.last_tool_call_response = Some(last_tool_call_response.clone());
        }
    }

    for msg in &input.additional_messages {
        messages.push(msg.clone());
    }

    context_state.messages = messages.clone();

    info!(
        session_id = %input.session.session_id,
        message_count = messages.len(),
        tool_result_count = context_state.tool_results.len(),
        "context built"
    );

    Ok(ContextOutput {
        session: input.session,
        messages,
        context_state,
    })
}

pub fn accumulate_tool_result(
    session: &mut SessionManagement,
    tool_name: &str,
    tool_input: serde_json::Value,
    tool_output: serde_json::Value,
    tool_success: bool,
    tool_error: Option<String>,
) -> Result<(), String> {
    accumulate_tool_result_with_feedback(
        session,
        tool_name,
        tool_input,
        tool_output,
        tool_success,
        tool_error,
        None,
        None,
    )
}

#[expect(
    clippy::too_many_arguments,
    reason = "runtime event ingestion keeps the persisted tool-result contract explicit"
)]
pub fn accumulate_tool_result_with_feedback(
    session: &mut SessionManagement,
    tool_name: &str,
    tool_input: serde_json::Value,
    tool_output: serde_json::Value,
    tool_success: bool,
    tool_error: Option<String>,
    _command_feedback: Option<serde_json::Value>,
    _legacy_last_tool_call_summary: Option<String>,
) -> Result<(), String> {
    accumulate_tool_result_with_provider_metadata(
        session,
        tool_name,
        tool_input,
        tool_output,
        tool_success,
        tool_error,
        None,
    )
}

pub fn accumulate_tool_result_with_provider_metadata(
    session: &mut SessionManagement,
    tool_name: &str,
    tool_input: serde_json::Value,
    tool_output: serde_json::Value,
    tool_success: bool,
    tool_error: Option<String>,
    provider_metadata: Option<serde_json::Value>,
) -> Result<(), String> {
    let now = Utc::now();
    let sequence = session
        .session_log
        .iter()
        .filter_map(|entry| serde_json::from_str::<serde_json::Value>(entry).ok())
        .filter(|value| value.get("type").and_then(|kind| kind.as_str()) == Some("tool_result"))
        .count()
        + 1;
    let mut tool_result_json = serde_json::json!({
        "type": "tool_result",
        "tool_name": tool_name,
        "input": strip_tool_reporting_fields(tool_input),
        "output": tool_output,
        "success": tool_success,
        "error": tool_error,
        "sequence": sequence,
        "timestamp": now.to_rfc3339(),
    });
    if let Some(provider_metadata) = provider_metadata {
        tool_result_json["provider_metadata"] = provider_metadata;
    }
    tool_result_json["context_cache"] = tool_result_context_cache(&tool_result_json);
    tool_result_json["context_message"] = immutable_tool_result_context_message(&tool_result_json);
    tool_result_json["context_messages"] =
        serde_json::Value::Array(immutable_tool_result_context_messages(&tool_result_json));

    session.push_log(
        serde_json::to_string(&tool_result_json)
            .unwrap_or_else(|_| format!("tool_result: {}", tool_name)),
        now,
    );

    Ok(())
}

pub fn accumulate_message(
    session: &mut SessionManagement,
    role: &str,
    content: serde_json::Value,
) -> Result<(), String> {
    let now = Utc::now();

    let message_json = serde_json::json!({
        "role": role,
        "content": content,
    });

    session.push_log(
        serde_json::to_string(&message_json).unwrap_or_else(|_| format!("message: {}", role)),
        now,
    );

    Ok(())
}

const USER_MEDIA_START: &str = "[MEDIA:";
const USER_MEDIA_END: &str = ":MEDIA]";

pub fn user_input_content_value(input: &str) -> serde_json::Value {
    let mut parts = Vec::new();
    let mut cursor = 0usize;
    let mut saw_image = false;

    while let Some(relative_start) = input[cursor..].find(USER_MEDIA_START) {
        let start = cursor + relative_start;
        let data_start = start + USER_MEDIA_START.len();
        let Some(relative_end) = input[data_start..].find(USER_MEDIA_END) else {
            break;
        };
        let end = data_start + relative_end;
        let marker_end = end + USER_MEDIA_END.len();
        let media_url = input[data_start..end].trim();

        if media_url.starts_with("data:image/") {
            push_input_text_part(&mut parts, &input[cursor..start]);
            parts.push(serde_json::json!({
                "type": "input_image",
                "image_url": media_url,
            }));
            saw_image = true;
        } else {
            push_input_text_part(&mut parts, &input[cursor..marker_end]);
        }

        cursor = marker_end;
    }

    if !saw_image {
        return serde_json::Value::String(input.to_string());
    }

    push_input_text_part(&mut parts, &input[cursor..]);
    serde_json::Value::Array(parts)
}

pub fn user_input_content_matches(content: &serde_json::Value, input: &str) -> bool {
    content
        .as_str()
        .is_some_and(|text| text.trim() == input.trim())
        || *content == user_input_content_value(input)
}

fn push_input_text_part(parts: &mut Vec<serde_json::Value>, text: &str) {
    if !text.is_empty() {
        parts.push(serde_json::json!({
            "type": "input_text",
            "text": text,
        }));
    }
}

pub fn compact_session_context(
    session: &mut SessionManagement,
    compact_text: &str,
) -> Result<(), String> {
    let now = Utc::now();
    let compact_text = truncate_text_to_token_budget(compact_text.trim(), 20_000);
    let workspace_snapshot = WorkspaceSnapshot::from_cwd(&session.session_directory)
        .map(|snapshot| snapshot.render())
        .unwrap_or_else(|| "<WORKSPACE_SNAPSHOT>\n\n</WORKSPACE_SNAPSHOT>".to_string());
    let environment_context = environment_context_message(&session.session_directory);
    session.push_log(
        serde_json::json!({
            "type": "context_compaction",
            "content": compact_text,
            "workspace_snapshot": workspace_snapshot,
            "environment_context": environment_context,
            "task_management": session.task_management_json(),
            "timestamp": now.to_rfc3339(),
        })
        .to_string(),
        now,
    );
    Ok(())
}

pub fn build_messages_from_session(session: &SessionManagement) -> Vec<serde_json::Value> {
    build_messages_from_session_with_options(session)
}

fn build_messages_from_session_with_options(session: &SessionManagement) -> Vec<serde_json::Value> {
    let mut messages = Vec::new();
    let mut saw_context_compaction = false;
    for value in session
        .session_log
        .iter()
        .filter_map(|entry| serde_json::from_str::<serde_json::Value>(entry).ok())
    {
        if value.get("type").and_then(|kind| kind.as_str()) == Some("context_compaction") {
            saw_context_compaction = true;
            messages.clear();
            messages.extend(context_compaction_messages(&value));
            continue;
        }
        messages.extend(immutable_context_messages_from_log_entry(value));
    }

    let raw_initial_user_input = &session.input.user_input;
    let initial_user_input = raw_initial_user_input.trim();
    if !saw_context_compaction
        && !initial_user_input.is_empty()
        && !messages.iter().any(|message| {
            message.get("role").and_then(|role| role.as_str()) == Some("user")
                && message.get("content").is_some_and(|content| {
                    user_input_content_matches(content, raw_initial_user_input)
                })
        })
    {
        messages.insert(
            0,
            serde_json::json!({
                "role": "user",
                "content": user_input_content_value(initial_user_input),
            }),
        );
    }

    messages
}

fn context_compaction_messages(value: &serde_json::Value) -> Vec<serde_json::Value> {
    let mut messages = Vec::new();
    if let Some(content) = value.get("content").and_then(serde_json::Value::as_str) {
        messages.push(serde_json::json!({
            "role": "user",
            "content": content,
        }));
    }
    if let Some(snapshot) = value
        .get("workspace_snapshot")
        .and_then(serde_json::Value::as_str)
    {
        messages.push(serde_json::json!({
            "role": "user",
            "content": snapshot,
        }));
    }
    if let Some(environment) = value
        .get("environment_context")
        .and_then(serde_json::Value::as_str)
    {
        messages.push(serde_json::json!({
            "role": "user",
            "content": environment,
        }));
    }
    if let Some(task_management) = value.get("task_management") {
        messages.push(serde_json::json!({
            "role": "user",
            "content": format!(
                "TASK_MANAGEMENT_STATE:\n{}",
                serde_json::to_string(task_management).unwrap_or_default()
            ),
        }));
    }
    messages
}

fn immutable_context_messages_from_log_entry(value: serde_json::Value) -> Vec<serde_json::Value> {
    let Some(obj) = value.as_object() else {
        return Vec::new();
    };
    if let Some(role) = obj.get("role").and_then(|role| role.as_str()) {
        if role == "user" || role == "system" || role == "assistant" {
            return obj
                .get("content")
                .map(|content| {
                    vec![serde_json::json!({
                    "role": role,
                    "content": content,
                    })]
                })
                .unwrap_or_default();
        }
    }

    if obj.get("type").and_then(|kind| kind.as_str()) != Some("tool_result") {
        return Vec::new();
    }

    if let Some(messages) = obj
        .get("context_messages")
        .and_then(|messages| messages.as_array())
    {
        return messages.clone();
    }

    immutable_tool_result_context_messages(&value)
}

fn strip_command_run_context_noise(value: serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(map) => serde_json::Value::Object(
            map.into_iter()
                .filter(|(key, _)| {
                    !matches!(
                        key.as_str(),
                        "step_summary"
                            | "last_tool_call_status"
                            | "last_tool_call_summary"
                            | "summary"
                            | "description"
                            | "interface"
                            | "used_prompt"
                            | "notes"
                            | "receipt"
                            | "should_register_tool"
                    )
                })
                .map(|(key, value)| (key, strip_command_run_context_noise(value)))
                .collect(),
        ),
        serde_json::Value::Array(items) => serde_json::Value::Array(
            items
                .into_iter()
                .map(strip_command_run_context_noise)
                .collect(),
        ),
        other => other,
    }
}

fn last_tool_call_response_from_session(session: &SessionManagement) -> Option<serde_json::Value> {
    session
        .session_log
        .iter()
        .rev()
        .filter_map(|entry| serde_json::from_str::<serde_json::Value>(entry).ok())
        .find(|value| value.get("type").and_then(|kind| kind.as_str()) == Some("tool_result"))
        .map(|value| {
            serde_json::json!({
                "tool_name": value.get("tool_name").cloned().unwrap_or(serde_json::Value::Null),
                "input": compact_json_for_context(value.get("input").cloned().unwrap_or(serde_json::Value::Null)),
                "output": cached_context_output_for_tool_result(&value),
                "success": value.get("success").cloned().unwrap_or(serde_json::Value::Bool(true)),
                "error": cached_context_error_for_tool_result(&value),
                "timestamp": value.get("timestamp").cloned().unwrap_or(serde_json::Value::Null),
            })
        })
}

fn tool_result_context_cache(value: &serde_json::Value) -> serde_json::Value {
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
        "input": compact_json_for_context(value.get("input").cloned().unwrap_or(serde_json::Value::Null)),
        "output": output.clone(),
        "success": value.get("success").cloned().unwrap_or(serde_json::Value::Bool(true)),
        "error": error.clone(),
    });
    serde_json::json!({
        "version": 1,
        "cache_id": stable_context_cache_id(&cache_id_input),
        "output": output,
        "error": error,
    })
}

fn immutable_tool_result_context_message(value: &serde_json::Value) -> serde_json::Value {
    if value.get("tool_name").and_then(|name| name.as_str()) == Some("command_run") {
        return command_run_function_output_context_message(value);
    }
    serde_json::json!({
        "role": "user",
        "content": compact_json_to_string(&serde_json::json!([immutable_tool_result_context_item(value)])),
    })
}

fn immutable_tool_result_context_messages(value: &serde_json::Value) -> Vec<serde_json::Value> {
    if value.get("tool_name").and_then(|name| name.as_str()) == Some("command_run") {
        return command_run_responses_api_context_items(value);
    }
    vec![immutable_tool_result_context_message(value)]
}

fn command_run_responses_api_context_items(value: &serde_json::Value) -> Vec<serde_json::Value> {
    let call_id = command_run_context_call_id(value);
    let arguments = serde_json::to_string(value.get("input").unwrap_or(&serde_json::Value::Null))
        .unwrap_or_else(|_| "{}".to_string());
    let mut function_call = serde_json::json!({
        "type": "function_call",
        "name": "command_run",
        "arguments": arguments,
        "call_id": call_id,
        "status": "completed",
    });
    if let Some(provider_metadata) = value.get("provider_metadata") {
        function_call["provider_metadata"] = provider_metadata.clone();
    }
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

fn command_run_function_output_for_context(value: &serde_json::Value) -> String {
    command_run_current_style_output_for_context(value)
}

fn command_run_function_output_payload_for_context(value: &serde_json::Value) -> serde_json::Value {
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

fn command_run_media_content_items_for_context(
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

fn strip_read_media_payload_data(value: &mut serde_json::Value) {
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
    let mut count = 0;
    collect_command_run_media_audio_preview_count(value.get("output").unwrap_or(value), &mut count);
    count
}

fn collect_command_run_media_audio_preview_count(value: &serde_json::Value, count: &mut usize) {
    match value {
        serde_json::Value::Object(object) => {
            if object.get("type").and_then(serde_json::Value::as_str) == Some("audio_url") {
                *count += 1;
            }
            for child in object.values() {
                collect_command_run_media_audio_preview_count(child, count);
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                collect_command_run_media_audio_preview_count(item, count);
            }
        }
        _ => {}
    }
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

fn command_run_current_style_output_string(value: &serde_json::Value) -> Option<String> {
    let output = value.get("output").unwrap_or(&serde_json::Value::Null);
    let input_commands = value
        .get("input")
        .and_then(|input| input.get("commands"))
        .and_then(|commands| commands.as_array());
    let results = flattened_command_run_results(output)
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
        "input": compact_json_for_context(value.get("input").cloned().unwrap_or(serde_json::Value::Null)),
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

fn command_run_summary_for_context(value: &serde_json::Value) -> serde_json::Value {
    let output = value
        .get("output")
        .cloned()
        .unwrap_or(serde_json::Value::Null);
    let input_commands = value
        .get("input")
        .and_then(|input| input.get("commands"))
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

fn flattened_command_run_results(output: &serde_json::Value) -> Vec<&serde_json::Value> {
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

fn command_run_display_command(command: &str, command_line: &str) -> String {
    if command_line.trim().is_empty() {
        return command.to_string();
    }
    if normalized_command_run_subcommand(command) == "apply_patch"
        && command_line.trim_start().starts_with("*** Begin Patch")
    {
        return format!("apply_patch <<'PATCH'\n{}\nPATCH", command_line.trim_end());
    }
    if let Some(cli) = structured_command_line_as_cli(command, command_line) {
        return cli;
    }
    if is_structured_code_read_command(command) {
        return format!("{command} {}", command_line.trim());
    }
    command_line.trim().to_string()
}

fn command_run_llm_streams(command: &str, stdout: &str) -> (String, String) {
    if let Some(streams) = verify_stdout_as_cli_streams(stdout) {
        return streams;
    }
    structured_stdout_as_cli_streams(command, stdout)
        .unwrap_or_else(|| (stdout.trim_end().to_string(), String::new()))
}

fn verify_stdout_as_cli_streams(stdout: &str) -> Option<(String, String)> {
    let value = serde_json::from_str::<serde_json::Value>(stdout).ok()?;
    let returncodes = value
        .get("returncodes")
        .and_then(serde_json::Value::as_object)?;
    let stdout_map = value.get("stdout").and_then(serde_json::Value::as_object);
    let stderr_map = value.get("stderr").and_then(serde_json::Value::as_object);
    if stdout_map.is_none() && stderr_map.is_none() {
        return None;
    }

    let ok = value
        .get("ok")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or_else(|| {
            returncodes
                .values()
                .all(|code| code.as_i64().unwrap_or(1) == 0)
        });
    let mut names = returncodes.keys().cloned().collect::<Vec<_>>();
    names.sort();

    let mut output_lines = vec![format!("verify.ps1 ok: {ok}")];
    output_lines.push(format!(
        "returncodes: {}",
        names
            .iter()
            .map(|name| format!(
                "{}={}",
                name,
                returncodes
                    .get(name)
                    .and_then(serde_json::Value::as_i64)
                    .unwrap_or(0)
            ))
            .collect::<Vec<_>>()
            .join(", ")
    ));

    if ok {
        return Some((output_lines.join("\n"), String::new()));
    }

    let mut failure_blocks = Vec::new();
    for name in names {
        let code = returncodes
            .get(&name)
            .and_then(serde_json::Value::as_i64)
            .unwrap_or(0);
        if code == 0 {
            output_lines.push(format!("{name}: passed"));
            continue;
        }
        for (label, map) in [("stdout", stdout_map), ("stderr", stderr_map)] {
            let Some(text) = map
                .and_then(|map| map.get(&name))
                .and_then(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|text| !text.is_empty())
            else {
                continue;
            };
            failure_blocks.push(format!(
                "{name} {label}:\n{}",
                formatted_truncate_text(text, COMMAND_RUN_RESULT_OUTPUT_MAX_TOKENS)
            ));
        }
    }

    Some((output_lines.join("\n"), failure_blocks.join("\n\n")))
}

fn structured_command_line_as_cli(command: &str, command_line: &str) -> Option<String> {
    let trimmed = command_line.trim();
    if !(trimmed.starts_with('{') || trimmed.starts_with('[')) {
        return None;
    }
    let value = serde_json::from_str::<serde_json::Value>(trimmed).ok()?;
    let item = match value {
        serde_json::Value::Array(items) => items.into_iter().next()?,
        other => other,
    };
    let command = normalized_command_run_subcommand(command);
    let path = json_string_field(&item, &["path", "file_path", "filePath"]);
    match command.as_str() {
        "read_line" | "cat" => {
            let path = path?;
            let start =
                json_usize_field(&item, "start_line").or_else(|| json_usize_field(&item, "line"));
            let end = json_usize_field(&item, "end_line").or(start);
            match (start, end) {
                (Some(start), Some(end)) if start != 1 || end != usize::MAX => Some(format!(
                    "sed -n '{}{}p' {}",
                    start,
                    if start == end {
                        String::new()
                    } else {
                        format!(",{end}")
                    },
                    shell_quote(&path)
                )),
                _ => Some(format!("cat {}", shell_quote(&path))),
            }
        }
        "read_block" | "sed" => {
            let path = path?;
            let start = json_usize_field(&item, "start_line")
                .or_else(|| json_usize_field(&item, "line"))
                .unwrap_or(1);
            let end = json_usize_field(&item, "end_line").unwrap_or(start);
            Some(format!(
                "sed -n '{}{}p' {}",
                start,
                if start == end {
                    String::new()
                } else {
                    format!(",{end}")
                },
                shell_quote(&path)
            ))
        }
        "rg" | "grep" => {
            let query = json_string_field(&item, &["query", "pattern"]).unwrap_or_default();
            let directory =
                json_string_field(&item, &["directory", "path"]).unwrap_or_else(|| ".".to_string());
            let mut parts = vec![if command == "grep" {
                "grep".to_string()
            } else {
                "rg".to_string()
            }];
            if command == "rg" {
                parts.push("-n".to_string());
            } else {
                parts.push("-R".to_string());
            }
            if !json_bool_field(&item, "case_sensitive").unwrap_or(false) {
                parts.push("-i".to_string());
            }
            if command == "rg" && !json_bool_field(&item, "use_regex").unwrap_or(false) {
                parts.push("--fixed-strings".to_string());
            }
            if let Some(glob) = json_string_field(&item, &["file_glob", "glob"]) {
                if command == "rg" {
                    parts.push("-g".to_string());
                    parts.push(shell_quote(&glob));
                } else {
                    parts.push("--include".to_string());
                    parts.push(shell_quote(&glob));
                }
            }
            parts.push(shell_quote(&query));
            parts.push(shell_quote(&directory));
            Some(parts.join(" "))
        }
        "glob" | "find" => {
            let directory =
                json_string_field(&item, &["directory", "path"]).unwrap_or_else(|| ".".to_string());
            let pattern = json_string_field(&item, &["pattern", "glob"])
                .unwrap_or_else(|| "**/*".to_string());
            let file_type = if json_bool_field(&item, "include_directories").unwrap_or(false) {
                String::new()
            } else {
                " -type f".to_string()
            };
            Some(format!(
                "find {}{} -path {}",
                shell_quote(&directory),
                file_type,
                shell_quote(&pattern)
            ))
        }
        "write_file" => path.map(|path| format!("cat > {}", shell_quote(&path))),
        _ => None,
    }
}

fn is_structured_code_read_command(command: &str) -> bool {
    matches!(
        command,
        "cat"
            | "sed"
            | "read_line"
            | "read_block"
            | "rg"
            | "grep"
            | "find"
            | "glob"
            | "lsp_outline"
            | "lsp_definition"
            | "lsp_references"
            | "get_file_outline"
            | "find_definition"
            | "find_references"
    )
}

fn structured_stdout_as_cli_streams(command: &str, stdout: &str) -> Option<(String, String)> {
    let value = serde_json::from_str::<serde_json::Value>(stdout).ok()?;
    let results = value
        .get("results")
        .and_then(|results| results.as_array())?;
    let command = normalized_command_run_subcommand(command);
    let mut blocks = Vec::new();
    let mut stderr = command_run_structured_diagnostics(&value);
    for result in results {
        stderr.extend(command_run_result_diagnostics(result));
        match command.as_str() {
            "read_line" | "read_block" | "cat" | "sed" => {
                if let Some(content) = result.get("content").and_then(serde_json::Value::as_str) {
                    blocks.push(content.trim_end().to_string());
                }
            }
            "rg" | "grep" | "find_definition" | "find_references" => {
                if let Some(matches) = result.get("matches").and_then(serde_json::Value::as_array) {
                    let lines = matches
                        .iter()
                        .filter_map(command_run_match_as_cli_line)
                        .collect::<Vec<_>>();
                    if !lines.is_empty() {
                        blocks.push(lines.join("\n"));
                    }
                }
            }
            "glob" | "find" => {
                if let Some(paths) = result
                    .get("matched_paths")
                    .and_then(serde_json::Value::as_array)
                {
                    let lines = paths
                        .iter()
                        .filter_map(|path| path.as_str().map(str::to_string))
                        .collect::<Vec<_>>();
                    if !lines.is_empty() {
                        blocks.push(lines.join("\n"));
                    }
                }
            }
            "get_file_outline" => {
                if let Some(outline) = result.get("outline").and_then(serde_json::Value::as_array) {
                    let path = result
                        .get("path")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or_default();
                    let lines = outline
                        .iter()
                        .filter_map(|item| command_run_outline_as_cli_line(path, item))
                        .collect::<Vec<_>>();
                    if !lines.is_empty() {
                        blocks.push(lines.join("\n"));
                    }
                }
            }
            "apply_patch" | "apply_diff" | "write_file" | "delete_file" => {
                if let Some(summary) = result
                    .get("summary_markdown")
                    .and_then(serde_json::Value::as_str)
                {
                    blocks.push(summary.trim_end().to_string());
                } else if let Some(line) = command_run_mutation_result_as_cli_line(result) {
                    blocks.push(line);
                }
            }
            _ => {
                if let Some(summary) = result
                    .get("summary_markdown")
                    .and_then(serde_json::Value::as_str)
                {
                    blocks.push(summary.trim_end().to_string());
                } else if let Some(content) =
                    result.get("content").and_then(serde_json::Value::as_str)
                {
                    blocks.push(content.trim_end().to_string());
                }
            }
        }
    }
    if blocks.is_empty() && stderr.is_empty() {
        return None;
    }
    Some((blocks.join("\n\n"), stderr.join("\n")))
}

fn normalized_command_run_subcommand(command: &str) -> String {
    let command = command
        .trim()
        .rsplit([':', '/'])
        .next()
        .unwrap_or(command)
        .trim()
        .to_ascii_lowercase()
        .replace('-', "_");
    match command.as_str() {
        "type" | "get_content" => "read_line".to_string(),
        "cat" => "cat".to_string(),
        "sed" => "sed".to_string(),
        "read_line" => "read_line".to_string(),
        "read_block" => "read_block".to_string(),
        "ripgrep" => "rg".to_string(),
        "grep" => "grep".to_string(),
        "rg" => "rg".to_string(),
        "find" => "find".to_string(),
        "glob" => "glob".to_string(),
        "lsp_outline" | "outline" | "symbols" => "get_file_outline".to_string(),
        "lsp_definition" | "definition" => "find_definition".to_string(),
        "lsp_references" | "references" => "find_references".to_string(),
        "patch" | "applypatch" => "apply_patch".to_string(),
        other => other.to_string(),
    }
}

fn command_run_match_as_cli_line(value: &serde_json::Value) -> Option<String> {
    let path = value.get("path").and_then(serde_json::Value::as_str)?;
    let content = value
        .get("content")
        .or_else(|| value.get("line"))
        .or_else(|| value.get("text"))
        .and_then(serde_json::Value::as_str);
    let line = value
        .get("line")
        .or_else(|| value.get("line_number"))
        .and_then(serde_json::Value::as_u64);
    match (line, content) {
        (Some(line), Some(content)) => Some(format!("{path}:{line}:{}", content.trim_end())),
        (_, Some(content)) => Some(format!("{path}:{}", content.trim_end())),
        _ => Some(path.to_string()),
    }
}

fn command_run_outline_as_cli_line(path: &str, value: &serde_json::Value) -> Option<String> {
    let name = value.get("name").and_then(serde_json::Value::as_str)?;
    let kind = value
        .get("kind")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("symbol");
    let line = value
        .get("line")
        .or_else(|| value.get("line_number"))
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    if line > 0 {
        Some(format!("{path}:{line}:{kind} {name}"))
    } else {
        Some(format!("{path}:{kind} {name}"))
    }
}

fn command_run_mutation_result_as_cli_line(value: &serde_json::Value) -> Option<String> {
    let path = value
        .get("path")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    if let Some(error) = value
        .get("error")
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.is_empty())
    {
        return Some(if path.is_empty() {
            error.to_string()
        } else {
            format!("{path}: {error}")
        });
    }
    if value.get("applied").and_then(serde_json::Value::as_bool) == Some(true) {
        return Some(if path.is_empty() {
            "Applied patch.".to_string()
        } else {
            format!("{path}: applied")
        });
    }
    if value.get("success").and_then(serde_json::Value::as_bool) == Some(true) {
        return Some(if path.is_empty() {
            "Wrote file.".to_string()
        } else {
            format!("{path}: wrote file")
        });
    }
    if value.get("deleted").and_then(serde_json::Value::as_bool) == Some(true) {
        return Some(if path.is_empty() {
            "Deleted file.".to_string()
        } else {
            format!("{path}: deleted")
        });
    }
    None
}

fn command_run_structured_diagnostics(value: &serde_json::Value) -> Vec<String> {
    ["errors", "warnings"]
        .into_iter()
        .filter_map(|field| value.get(field).and_then(serde_json::Value::as_array))
        .flat_map(|items| items.iter().filter_map(command_run_diagnostic_line))
        .collect()
}

fn command_run_result_diagnostics(value: &serde_json::Value) -> Vec<String> {
    let mut lines = command_run_structured_diagnostics(value);
    if let Some(error) = value
        .get("error")
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.is_empty())
    {
        lines.push(error.to_string());
    }
    lines
}

fn command_run_diagnostic_line(value: &serde_json::Value) -> Option<String> {
    if let Some(text) = value.as_str() {
        return Some(text.to_string());
    }
    let message = value.get("message").and_then(serde_json::Value::as_str)?;
    let path = value
        .get("path")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    let code = value
        .get("code")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    Some(match (path.is_empty(), code.is_empty()) {
        (true, true) => message.to_string(),
        (false, true) => format!("{path}: {message}"),
        (true, false) => format!("{code}: {message}"),
        (false, false) => format!("{path}: {code}: {message}"),
    })
}

fn json_string_field(value: &serde_json::Value, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(serde_json::Value::as_str))
        .map(str::to_string)
}

fn json_bool_field(value: &serde_json::Value, key: &str) -> Option<bool> {
    value.get(key).and_then(serde_json::Value::as_bool)
}

fn json_usize_field(value: &serde_json::Value, key: &str) -> Option<usize> {
    value
        .get(key)
        .and_then(serde_json::Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())
}

fn shell_quote(value: &str) -> String {
    if value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '/' | '\\' | '_' | '-' | ':'))
    {
        return value.to_string();
    }
    format!("'{}'", value.replace('\'', "'\\''"))
}

fn strip_tool_reporting_fields(value: serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(map) => serde_json::Value::Object(
            map.into_iter()
                .filter(|(key, _)| {
                    key != "step_summary"
                        && key != "last_tool_call_status"
                        && key != "last_tool_call_summary"
                        && key != "summary"
                        && key != "description"
                })
                .map(|(key, value)| (key, strip_tool_reporting_fields(value)))
                .collect(),
        ),
        serde_json::Value::Array(items) => {
            serde_json::Value::Array(items.into_iter().map(strip_tool_reporting_fields).collect())
        }
        other => other,
    }
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

fn truncate_text_to_token_budget(text: &str, max_tokens: usize) -> String {
    let max_chars = max_tokens.saturating_mul(APPROX_CHARS_PER_TOKEN);
    if text.len() <= max_chars {
        return text.to_string();
    }
    let mut out = text.chars().take(max_chars).collect::<String>();
    out.push_str("\n\n[context checkpoint truncated to about 20,000 tokens]");
    out
}

fn environment_context_message(cwd: &std::path::Path) -> String {
    format!(
        "<environment_context>\n  <cwd>{}</cwd>\n  <shell>{}</shell>\n  <current_date>{}</current_date>\n  <timezone>{}</timezone>\n</environment_context>",
        cwd.display(),
        context_shell_name(),
        chrono::Local::now().format("%Y-%m-%d"),
        std::env::var("TZ").unwrap_or_else(|_| "Europe/Paris".to_string())
    )
}

fn context_shell_name() -> &'static str {
    match std::env::var("TURA_COMMAND_RUN_SHELL")
        .ok()
        .map(|value| value.trim().to_ascii_lowercase())
        .as_deref()
    {
        Some("bash") => "bash",
        Some("shell") | Some("shell_command") | Some("shll") | Some("shall") => {
            if cfg!(windows) {
                "powershell"
            } else {
                "bash"
            }
        }
        _ if cfg!(windows) => "powershell",
        _ => "bash",
    }
}

fn context_output_byte_budget() -> usize {
    CONTEXT_OUTPUT_MAX_TOKENS * APPROX_CHARS_PER_TOKEN
}

fn formatted_truncate_text(content: &str, max_tokens: usize) -> String {
    if content.len() <= max_tokens * APPROX_CHARS_PER_TOKEN {
        return content.to_string();
    }
    let total_lines = content.lines().count();
    let truncated = truncate_middle_with_token_budget(content, max_tokens);
    format!("Total output lines: {total_lines}\n\n{truncated}")
}

fn command_run_truncate_text(
    content: &str,
    max_tokens: usize,
    command_line: Option<&str>,
) -> String {
    let effective_max_tokens = command_run_effective_max_tokens(max_tokens, command_line);
    if content.len() <= effective_max_tokens * APPROX_CHARS_PER_TOKEN {
        return content.to_string();
    }
    truncate_marker_sections_for_command_run(content, effective_max_tokens, command_line)
        .or_else(|| {
            truncate_query_sections_for_command_run(content, effective_max_tokens, command_line)
        })
        .or_else(|| truncate_ripgrep_file_sections_for_command_run(content, effective_max_tokens))
        .unwrap_or_else(|| formatted_truncate_text(content, effective_max_tokens))
}

fn command_run_effective_max_tokens(max_tokens: usize, command_line: Option<&str>) -> usize {
    let Some(command_line) = command_line else {
        return max_tokens;
    };
    if extract_read_targets(command_line).len() == 1 {
        max_tokens.saturating_mul(3)
    } else {
        max_tokens
    }
}

fn truncate_marker_sections_for_command_run(
    content: &str,
    max_tokens: usize,
    command_line: Option<&str>,
) -> Option<String> {
    let mut preamble = String::new();
    let mut sections = Vec::<String>::new();
    let mut current = String::new();
    let mut bare_file_marker_index = 0usize;
    let mut saw_bare_file_marker = false;
    let read_targets = command_line.map(extract_read_targets).unwrap_or_default();

    for chunk in content.split_inclusive('\n') {
        if is_command_run_section_marker(chunk) {
            if chunk.trim_end_matches(['\r', '\n']) == "---FILE---" {
                saw_bare_file_marker = true;
            }
            let chunk = rewrite_bare_file_marker(chunk, &read_targets, &mut bare_file_marker_index);
            if current.is_empty() {
                current.push_str(&chunk);
            } else {
                sections.push(std::mem::take(&mut current));
                current.push_str(&chunk);
            }
            continue;
        }

        if current.is_empty() {
            preamble.push_str(chunk);
        } else {
            current.push_str(chunk);
        }
    }

    if !current.is_empty() {
        sections.push(current);
    }

    if saw_bare_file_marker && !read_targets.is_empty() {
        split_first_bare_file_section(&mut preamble, &mut sections, &read_targets[0]);
    }

    if sections.is_empty() {
        return None;
    }

    let mut output = String::new();
    if !preamble.is_empty() {
        output.push_str(&formatted_truncate_text(&preamble, max_tokens));
        if !output.ends_with('\n') {
            output.push('\n');
        }
    }

    for section in sections {
        output.push_str(&formatted_truncate_text(&section, max_tokens));
        if !output.ends_with('\n') {
            output.push('\n');
        }
    }

    Some(output)
}

fn split_first_bare_file_section(
    preamble: &mut String,
    sections: &mut Vec<String>,
    first_path: &str,
) {
    let Some(output_marker) = preamble.rfind("Output:\n") else {
        return;
    };
    let file_body_start = output_marker + "Output:\n".len();
    if file_body_start >= preamble.len() {
        return;
    }
    let header = preamble[..file_body_start].to_string();
    let first_body = preamble[file_body_start..].to_string();
    *preamble = header;
    sections.insert(0, format!("---FILE--- {first_path}\n{first_body}"));
}

fn rewrite_bare_file_marker(
    line: &str,
    read_targets: &[String],
    bare_file_marker_index: &mut usize,
) -> String {
    let trimmed = line.trim_end_matches(['\r', '\n']);
    if trimmed != "---FILE---" {
        return line.to_string();
    }
    let target_index = bare_file_marker_index.saturating_add(1);
    *bare_file_marker_index = bare_file_marker_index.saturating_add(1);
    let Some(path) = read_targets.get(target_index) else {
        return line.to_string();
    };
    let newline = if line.ends_with("\r\n") {
        "\r\n"
    } else if line.ends_with('\n') {
        "\n"
    } else {
        ""
    };
    format!("---FILE--- {path}{newline}")
}

fn is_command_run_section_marker(line: &str) -> bool {
    let trimmed = line.trim_end_matches(['\r', '\n']);
    if let Some(path) = trimmed.strip_prefix("---FILE--- ") {
        return !path.trim().is_empty();
    }
    if trimmed == "---FILE---" {
        return true;
    }
    if !trimmed.starts_with("---") {
        return false;
    }
    let Some(rest) = trimmed.strip_prefix("---") else {
        return false;
    };
    let Some(label_end) = rest.find("---") else {
        return false;
    };
    if label_end == 0 {
        return false;
    }
    let label = &rest[..label_end];
    if !label
        .chars()
        .all(|ch| ch.is_ascii_uppercase() || ch.is_ascii_digit() || ch == '_' || ch == ' ')
    {
        return false;
    }
    rest[label_end + 3..]
        .chars()
        .next()
        .is_some_and(char::is_whitespace)
}

fn extract_read_targets(command_line: &str) -> Vec<String> {
    let mut targets = Vec::new();
    let command_text = shell_command_text_for_read_targets(command_line)
        .unwrap_or_else(|| command_line.trim().to_string());
    let tokens = shell_like_tokens(&command_text);
    let mut index = 0usize;
    while index < tokens.len() {
        let token = tokens[index].to_ascii_lowercase();
        if (token == "get-content" || token == "gc" || token == "cat" || token == "type")
            && index + 1 < tokens.len()
        {
            let mut next = index + 1;
            while next < tokens.len() && tokens[next].starts_with('-') {
                next += 1;
            }
            if let Some(path) = tokens
                .get(next)
                .and_then(|value| normalize_read_target(value))
            {
                if !targets.iter().any(|existing| existing == &path) {
                    targets.push(path);
                }
            }
            index = next;
        }
        index += 1;
    }
    targets
}

fn shell_command_text_for_read_targets(command_line: &str) -> Option<String> {
    fn parse_candidate(candidate: &str, depth: usize) -> Option<String> {
        if depth > 3 {
            return None;
        }
        let value = serde_json::from_str::<serde_json::Value>(candidate).ok()?;
        match value {
            serde_json::Value::String(inner) => {
                parse_candidate(inner.trim(), depth + 1).or_else(|| Some(inner.trim().to_string()))
            }
            serde_json::Value::Object(object) => object
                .get("command")
                .or_else(|| object.get("cmd"))
                .or_else(|| object.get("command_line"))
                .and_then(serde_json::Value::as_str)
                .map(|value| value.trim().to_string()),
            _ => None,
        }
    }

    let trimmed = command_line.trim();
    parse_candidate(trimmed, 0).or_else(|| {
        if trimmed.contains("\\\"") {
            parse_candidate(&trimmed.replace("\\\"", "\""), 0)
        } else {
            None
        }
    })
}

fn normalize_read_target(value: &str) -> Option<String> {
    let trimmed = value
        .trim()
        .trim_matches(';')
        .trim_matches(',')
        .trim_matches('"')
        .trim_matches('\'');
    if trimmed.is_empty() || trimmed.starts_with('$') || trimmed.starts_with('|') {
        return None;
    }
    if !(trimmed.contains('/') || trimmed.contains('\\') || trimmed.contains('.')) {
        return None;
    }
    Some(trimmed.replace('\\', "/"))
}

fn shell_like_tokens(value: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut quote = None::<char>;
    let mut escaped = false;
    for ch in value.chars() {
        if escaped {
            current.push(ch);
            escaped = false;
            continue;
        }
        if ch == '\\' && quote.is_some() {
            escaped = true;
            continue;
        }
        if let Some(active) = quote {
            if ch == active {
                quote = None;
            } else {
                current.push(ch);
            }
            continue;
        }
        if ch == '"' || ch == '\'' {
            quote = Some(ch);
        } else if ch.is_whitespace() || ch == ';' {
            if !current.is_empty() {
                tokens.push(std::mem::take(&mut current));
            }
        } else {
            current.push(ch);
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    tokens
}

fn truncate_query_sections_for_command_run(
    content: &str,
    max_tokens: usize,
    command_line: Option<&str>,
) -> Option<String> {
    let terms = extract_query_terms(command_line?);
    if terms.len() < 2 {
        return None;
    }

    let mut preamble = String::new();
    let mut sections = terms
        .iter()
        .map(|term| {
            (
                term.to_string(),
                format!("---QUERY--- {term}\n"),
                term.to_ascii_lowercase(),
            )
        })
        .collect::<Vec<_>>();

    for line in content.split_inclusive('\n') {
        let lower = line.to_ascii_lowercase();
        if let Some((_, section, _)) = sections
            .iter_mut()
            .find(|(_, _, term)| lower.contains(term.as_str()))
        {
            section.push_str(line);
        } else {
            preamble.push_str(line);
        }
    }

    if sections
        .iter()
        .all(|(_, section, _)| section.lines().count() <= 1)
    {
        return None;
    }

    let mut output = String::new();
    if !preamble.trim().is_empty() {
        output.push_str(&formatted_truncate_text(&preamble, max_tokens));
        if !output.ends_with('\n') {
            output.push('\n');
        }
    }
    for (_, section, _) in sections {
        output.push_str(&formatted_truncate_text(&section, max_tokens));
        if !output.ends_with('\n') {
            output.push('\n');
        }
    }
    Some(output)
}

fn extract_query_terms(command_line: &str) -> Vec<String> {
    let lower = command_line.to_ascii_lowercase();
    if !(lower.contains("rg ") || lower.contains("ripgrep") || lower.contains("select-string")) {
        return Vec::new();
    }

    let mut terms = Vec::new();
    for quoted in quoted_fragments(command_line) {
        let candidates = if quoted.contains('|') {
            quoted.split('|').collect::<Vec<_>>()
        } else if should_split_space_separated_query(&quoted) {
            quoted.split_whitespace().collect::<Vec<_>>()
        } else {
            vec![quoted.as_str()]
        };
        for candidate in candidates {
            let term = normalize_query_term(candidate);
            if is_query_term(&term) && !terms.iter().any(|existing| existing == &term) {
                terms.push(term);
            }
        }
    }
    terms
}

fn should_split_space_separated_query(value: &str) -> bool {
    let parts = value.split_whitespace().collect::<Vec<_>>();
    parts.len() >= 2
        && parts
            .iter()
            .all(|part| is_query_term(&normalize_query_term(part)))
}

fn quoted_fragments(value: &str) -> Vec<String> {
    let mut fragments = Vec::new();
    let mut current = String::new();
    let mut quote = None::<char>;
    let mut escaped = false;
    for ch in value.chars() {
        if escaped {
            current.push(ch);
            escaped = false;
            continue;
        }
        if ch == '\\' {
            if quote.is_some() {
                escaped = true;
            }
            continue;
        }
        if let Some(active) = quote {
            if ch == active {
                fragments.push(std::mem::take(&mut current));
                quote = None;
            } else {
                current.push(ch);
            }
        } else if ch == '"' || ch == '\'' {
            quote = Some(ch);
        }
    }
    fragments
}

fn normalize_query_term(value: &str) -> String {
    value
        .trim()
        .trim_matches('(')
        .trim_matches(')')
        .replace("\\b", "")
        .replace("\\s+", " ")
        .replace(".*", "")
        .trim()
        .to_string()
}

fn is_query_term(value: &str) -> bool {
    let len = value.chars().count();
    (1..=80).contains(&len)
        && value
            .chars()
            .any(|ch| ch.is_ascii_alphanumeric() || ch == '_')
        && !value.contains("**/")
        && !value.contains("*.")
}

fn truncate_ripgrep_file_sections_for_command_run(
    content: &str,
    max_tokens: usize,
) -> Option<String> {
    let mut preamble = String::new();
    let mut sections = Vec::<(String, String)>::new();

    for line in content.split_inclusive('\n') {
        if let Some(path) = ripgrep_result_path(line) {
            if let Some((_, section)) = sections.iter_mut().find(|(existing, _)| existing == &path)
            {
                section.push_str(line);
            } else {
                sections.push((path.clone(), format!("---MATCHES--- {path}\n{line}")));
            }
        } else {
            preamble.push_str(line);
        }
    }

    if sections.len() < 2 {
        return None;
    }

    let mut output = String::new();
    if !preamble.trim().is_empty() {
        output.push_str(&formatted_truncate_text(&preamble, max_tokens));
        if !output.ends_with('\n') {
            output.push('\n');
        }
    }
    for (_, section) in sections {
        output.push_str(&formatted_truncate_text(&section, max_tokens));
        if !output.ends_with('\n') {
            output.push('\n');
        }
    }
    Some(output)
}

fn ripgrep_result_path(line: &str) -> Option<String> {
    let trimmed = line.trim_end_matches(['\r', '\n']);
    let (path, rest) = trimmed.split_once(':')?;
    if path.is_empty() || !path.contains('.') {
        return None;
    }
    let line_number = rest.split_once(':').map(|(line, _)| line).unwrap_or(rest);
    if line_number.is_empty() || !line_number.chars().all(|ch| ch.is_ascii_digit()) {
        return None;
    }
    Some(path.replace('\\', "/"))
}

fn truncate_middle_with_token_budget(content: &str, max_tokens: usize) -> String {
    let max_chars = max_tokens.saturating_mul(APPROX_CHARS_PER_TOKEN);
    if content.len() <= max_chars {
        return content.to_string();
    }
    if max_chars == 0 {
        return format!("…{} tokens truncated…", approx_token_count(content.len()));
    }

    let marker_budget = 32usize;
    let visible_budget = max_chars.saturating_sub(marker_budget).max(2);
    let head_budget = visible_budget / 2;
    let tail_budget = visible_budget.saturating_sub(head_budget);
    let head_end = byte_floor_char_boundary(content, head_budget);
    let tail_start = byte_ceil_char_boundary(content, content.len().saturating_sub(tail_budget));
    let removed = tail_start.saturating_sub(head_end);
    let removed_tokens = approx_token_count(removed);
    format!(
        "{}…{} tokens truncated…{}",
        &content[..head_end],
        removed_tokens,
        &content[tail_start..]
    )
}

fn approx_token_count(byte_count: usize) -> usize {
    byte_count.div_ceil(APPROX_CHARS_PER_TOKEN)
}

fn byte_floor_char_boundary(text: &str, target: usize) -> usize {
    if target >= text.len() {
        return text.len();
    }
    let mut index = target;
    while index > 0 && !text.is_char_boundary(index) {
        index -= 1;
    }
    index
}

fn byte_ceil_char_boundary(text: &str, target: usize) -> usize {
    if target >= text.len() {
        return text.len();
    }
    let mut index = target;
    while index < text.len() && !text.is_char_boundary(index) {
        index += 1;
    }
    index
}

#[cfg(test)]
mod tests {
    use super::{
        accumulate_message, accumulate_tool_result, accumulate_tool_result_with_feedback,
        build_context, build_messages_from_session, command_run_function_output_for_context,
        command_run_function_output_payload_for_context, command_run_summary_for_context,
        command_run_truncate_text, compact_session_context, messages_with_runtime_context,
        ContextInput,
    };
    use crate::state_machine::agent_management::{ProviderConfig, ToolChoice};
    use crate::state_machine::runtime_management::{RuntimeManagement, RuntimeProviderConfig};
    use crate::state_machine::session_management::{
        PlanStatus, PollInterval, SessionInput, SessionManagement, StartCondition, TaskStep,
    };
    use chrono::Utc;
    use serde_json::json;
    use std::path::PathBuf;

    fn session() -> SessionManagement {
        let now = Utc::now();
        SessionManagement::new(
            "sess-test".to_string(),
            "test".to_string(),
            PathBuf::from("C:/workspace"),
            false,
            "coding".to_string(),
            SessionInput {
                user_input: "inspect".to_string(),
                file_input: vec![],
                agent: None,
                runtime_context: None,
            },
            "inspect".to_string(),
            now,
        )
    }

    fn runtime(session: &SessionManagement) -> RuntimeManagement {
        let now = Utc::now();
        let provider_name = crate::agent_router::coding_agent_provider_name();
        RuntimeManagement::new(
            "runtime-test".to_string(),
            session.session_id.clone(),
            "agent-test".to_string(),
            RuntimeProviderConfig {
                base: ProviderConfig {
                    tura_llm_name: provider_name.clone(),
                    stream: false,
                    temperature: 0.0,
                    max_tokens: 0,
                    tool_choice: ToolChoice::Auto,
                    time_out_ms: 120_000,
                },
                thinking: false,
                provider_name: provider_name.clone(),
                model_name: String::new(),
                provider_url_name: String::new(),
                llm_provider_name: provider_name,
            },
            now,
        )
    }

    #[test]
    fn compact_session_context_replaces_prior_tool_context_but_keeps_later_results() {
        let root = tempfile::TempDir::new().expect("tempdir");
        std::fs::create_dir_all(root.path().join("src")).expect("src dir");
        std::fs::write(root.path().join("src").join("lib.rs"), "fn main() {}\n").expect("fixture");
        let mut session = session();
        session.session_directory = root.path().to_path_buf();

        accumulate_tool_result(
            &mut session,
            "command_run",
            json!({
                "commands": [
                    { "command_type": "shell_command", "command_line": "echo old" }
                ]
            }),
            json!({
                "results": [
                    { "step": 1, "command_type": "shell_command", "success": true, "output": "old-tool-secret" }
                ]
            }),
            true,
            None,
        )
        .expect("old tool result");
        compact_session_context(
            &mut session,
            "Checkpoint: prior tool history is no longer needed. Continue with src/lib.rs.",
        )
        .expect("compact should write");
        accumulate_tool_result(
            &mut session,
            "command_run",
            json!({
                "commands": [
                    { "command_type": "shell_command", "command_line": "echo new" }
                ]
            }),
            json!({
                "results": [
                    { "step": 1, "command_type": "shell_command", "success": true, "output": "new-output" }
                ]
            }),
            true,
            None,
        )
        .expect("new tool result");

        let output = build_context(ContextInput {
            runtime: runtime(&session),
            session,
            additional_messages: vec![],
        })
        .expect("context should build");
        let joined = output
            .messages
            .iter()
            .map(|message| message.to_string())
            .collect::<Vec<_>>()
            .join("\n");

        assert!(joined.contains("Checkpoint: prior tool history is no longer needed"));
        assert!(joined.contains("<WORKSPACE_SNAPSHOT>"));
        assert!(joined.contains("src/lib.rs"));
        assert!(joined.contains("new-output"));
        assert!(!joined.contains("old-tool-secret"));
    }

    #[test]
    fn compact_session_context_appends_task_management_state() {
        let mut session = session();
        session.task_plan.plan_summary = "Inspect workspace".to_string();
        session.task_plan.detailed_tasks.push(TaskStep {
            nonce_id: "nonce-compact".to_string(),
            step: 0,
            task_summary: "Inspect workspace".to_string(),
            step_deliverable_description: "Find relevant files".to_string(),
            status: PlanStatus::Doing,
            ..TaskStep::default()
        });

        compact_session_context(&mut session, "handoff summary").expect("compact should succeed");
        let messages = build_messages_from_session(&session);
        let last = messages
            .last()
            .and_then(|message| message.get("content"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();

        assert!(last.starts_with("TASK_MANAGEMENT_STATE:"));
        assert!(last.contains("\"nonce_id\":\"nonce-compact\""));
        assert!(last.contains("\"status\":\"doing\""));
        assert!(!last.contains("\"tasks\""));
    }

    #[test]
    fn compact_session_context_appends_multi_task_management_state() {
        let mut session = session();
        session.task_plan.plan_summary = "Release plan".to_string();
        session.task_plan.detailed_tasks.push(TaskStep {
            nonce_id: "inspect".to_string(),
            step: 0,
            task_summary: "Inspect release blockers".to_string(),
            step_deliverable_description: "List blocking files".to_string(),
            sub_session_id: "sub-inspect".to_string(),
            poll_interval: PollInterval {
                m: 15,
                d: 0,
                h: 1,
                s: 5,
            },
            start_condition: StartCondition::ScheduledTask,
            status: PlanStatus::Question,
            ..TaskStep::default()
        });
        session.task_plan.detailed_tasks.push(TaskStep {
            nonce_id: "verify".to_string(),
            step: 1,
            task_summary: "Verify release checklist".to_string(),
            step_deliverable_description: "Passing regression output".to_string(),
            sub_session_id: "sub-verify".to_string(),
            poll_interval: PollInterval {
                m: 0,
                d: 1,
                h: 2,
                s: 30,
            },
            start_condition: StartCondition::PollingTask,
            status: PlanStatus::Done,
            ..TaskStep::default()
        });

        compact_session_context(&mut session, "multi task handoff")
            .expect("compact should succeed");
        let messages = build_messages_from_session(&session);
        let last = messages
            .last()
            .and_then(|message| message.get("content"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        let json_text = last
            .strip_prefix("TASK_MANAGEMENT_STATE:\n")
            .expect("task-management tail should be present");
        let task_management: serde_json::Value =
            serde_json::from_str(json_text).expect("task-management tail should be valid JSON");
        let tasks = task_management["tasks"]
            .as_array()
            .expect("multi-task compact state should include tasks array");

        assert_eq!(task_management["plan_summary"], "Release plan");
        assert_eq!(tasks.len(), 2);
        assert_eq!(tasks[0]["nonce_id"], "inspect");
        assert_eq!(tasks[0]["step"], 0);
        assert_eq!(tasks[0]["task_summary"], "Inspect release blockers");
        assert_eq!(tasks[0]["delivery"], "List blocking files");
        assert_eq!(tasks[0]["sub_session_id"], "sub-inspect");
        assert_eq!(tasks[0]["poll_interval"]["m"], 15);
        assert_eq!(tasks[0]["poll_interval"]["h"], 1);
        assert_eq!(tasks[0]["poll_interval"]["s"], 5);
        assert_eq!(tasks[0]["start_condition"], "scheduled_task");
        assert_eq!(tasks[0]["status"], "question");
        assert_eq!(tasks[1]["nonce_id"], "verify");
        assert_eq!(tasks[1]["poll_interval"]["d"], 1);
        assert_eq!(tasks[1]["poll_interval"]["h"], 2);
        assert_eq!(tasks[1]["poll_interval"]["s"], 30);
        assert_eq!(tasks[1]["start_condition"], "polling_task");
        assert_eq!(tasks[1]["status"], "done");
    }

    #[test]
    fn command_run_single_file_read_gets_larger_truncation_budget() {
        let content = format!(
            "Exit code: 0\nWall time: 0.1 seconds\nOutput:\n{}",
            "single file line\n".repeat(1_500)
        );

        let single = command_run_truncate_text(
            &content,
            2_500,
            Some(r#"{"command":"Get-Content src/a.py","timeout_ms":10000}"#),
        );
        let batch = command_run_truncate_text(
            &content,
            2_500,
            Some(r#"{"command":"Get-Content src/a.py; Get-Content src/b.py"}"#),
        );

        assert!(!single.contains("tokens truncated"), "{single}");
        assert!(batch.contains("tokens truncated"), "{batch}");
    }

    #[test]
    fn build_context_includes_last_tool_call_response() {
        let mut session = session();
        accumulate_tool_result(
            &mut session,
            "grep",
            json!({ "pattern": "needle" }),
            json!({ "matches": ["src/lib.rs"] }),
            true,
            None,
        )
        .expect("tool result should be logged");

        let output = build_context(ContextInput {
            runtime: runtime(&session),
            session,
            additional_messages: vec![],
        })
        .expect("context should build");

        let last = output
            .context_state
            .last_tool_call_response
            .expect("last tool response should be captured");
        assert_eq!(last["tool_name"], "grep");
        assert_eq!(last["success"], true);
        assert!(output
            .messages
            .iter()
            .any(
                |message| message["content"].as_str().is_some_and(|content| {
                    content.starts_with('[') && content.contains("src/lib.rs")
                })
            ));
    }

    #[test]
    fn build_context_omits_last_tool_call_response_when_session_disables_it() {
        let mut session = session();
        session.use_last_tool_call_response = false;
        accumulate_tool_result(
            &mut session,
            "grep",
            json!({ "pattern": "needle" }),
            json!({ "matches": ["src/lib.rs"] }),
            true,
            None,
        )
        .expect("tool result should be logged");

        let output = build_context(ContextInput {
            runtime: runtime(&session),
            session,
            additional_messages: vec![],
        })
        .expect("context should build");

        assert!(output.context_state.last_tool_call_response.is_none());
        assert!(!output.messages.iter().any(|message| message["content"]
            .as_str()
            .is_some_and(|content| content.contains("last_tool_call_response"))));
        assert!(output.messages.iter().any(|message| message["content"]
            .as_str()
            .is_some_and(|content| content.starts_with('[') && content.contains("src/lib.rs"))));
    }

    #[test]
    fn runtime_context_messages_preserve_codex_current_context_without_extra_tail() {
        let mut session = session();
        session.input.runtime_context = Some(
            serde_json::json!({
                "browser_version": "TestBrowser/1.0",
                "reply_language": "zh (zh-Hans)"
            })
            .to_string(),
        );
        let messages = messages_with_runtime_context(
            &session,
            &[serde_json::json!({
                "role": "user",
                "content": "temporary user text",
            })],
            Some("flagship_thinking"),
            Some("openai/gpt-test"),
            true,
        );

        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0]["role"], "user");
        assert_eq!(messages[0]["content"], "temporary user text");
        assert!(!messages
            .iter()
            .any(|message| message["content"]
                .as_str()
                .is_some_and(|content| content.contains("Permanent runtime context")
                    || content.contains("Tool reporting requirement")
                    || content.contains("Dynamic runtime state"))));
    }

    #[test]
    fn build_context_includes_failed_tool_error_in_last_tool_call_response() {
        let mut session = session();
        accumulate_tool_result(
            &mut session,
            "shell",
            json!({ "command": "bad" }),
            serde_json::Value::Null,
            false,
            Some("script failed: missing command".to_string()),
        )
        .expect("tool result should be logged");

        let output = build_context(ContextInput {
            runtime: runtime(&session),
            session,
            additional_messages: vec![],
        })
        .expect("context should build");

        let last = output
            .context_state
            .last_tool_call_response
            .expect("last tool response should be captured");
        assert_eq!(last["success"], false);
        assert_eq!(last["error"], "script failed: missing command");
    }

    #[test]
    fn build_context_truncates_large_tool_response() {
        let mut session = session();
        accumulate_tool_result(
            &mut session,
            "read_line",
            json!({ "file": "src/lib.rs" }),
            json!({ "content": "x".repeat(50_000) }),
            true,
            None,
        )
        .expect("tool result should be logged");

        let output = build_context(ContextInput {
            runtime: runtime(&session),
            session,
            additional_messages: vec![],
        })
        .expect("context should build");

        let last = output
            .context_state
            .last_tool_call_response
            .expect("last tool response should be captured");
        let content = last["output"]
            .as_str()
            .expect("large output should become a formatted truncated string");
        assert!(content.contains("Total output lines:"));
        assert!(content.contains("tokens truncated"));
        assert!(!content.contains("maximum context depth reached"));
        assert!(content.len() < 41_000);
    }

    #[test]
    fn command_run_context_truncates_grouped_file_reads_per_file_section() {
        let grouped_output = format!(
            "Exit code: 0\nWall time: 0.1 seconds\nOutput:\n---FILE--- src/a.py\n{}\n---FILE--- src/b.py\n{}\n---FILE--- src/c.py\n{}\n",
            "a-head\n".repeat(4_000),
            "b-middle\n".repeat(4_000),
            "c-tail\n".repeat(4_000),
        );
        let value = json!({
            "tool_name": "command_run",
            "input": {
                "commands": [
                    {
                        "step": 1,
                        "command_type": "shell_command",
                        "command_line": "$files=@('src/a.py','src/b.py','src/c.py'); foreach ($f in $files) { Write-Output ('---FILE--- ' + $f); Get-Content $f }"
                    }
                ]
            },
            "output": {
                "results": [
                    {
                        "step": 1,
                        "command_type": "shell_command",
                        "success": true,
                        "output": grouped_output
                    }
                ]
            },
            "success": true
        });

        let content = command_run_function_output_for_context(&value);

        assert!(content.contains("---FILE--- src/a.py"));
        assert!(content.contains("---FILE--- src/b.py"));
        assert!(content.contains("---FILE--- src/c.py"));
        assert!(
            content.matches("tokens truncated").count() >= 3,
            "each large file section should be truncated independently: {content}"
        );
    }

    #[test]
    fn command_run_context_truncates_grouped_queries_per_condition() {
        let grouped_output = format!(
            "Exit code: 0\nWall time: 0.1 seconds\nOutput:\n{}\n{}\n{}\n",
            "src/a.py:1:alpha keyword hit\n".repeat(4_000),
            "src/b.py:2:beta keyword hit\n".repeat(4_000),
            "src/c.py:3:gamma keyword hit\n".repeat(4_000),
        );
        let value = json!({
            "tool_name": "command_run",
            "input": {
                "commands": [
                    {
                        "step": 1,
                        "command_type": "shell_command",
                        "command_line": "rg -n \"alpha|beta|gamma\" src"
                    }
                ]
            },
            "output": {
                "results": [
                    {
                        "step": 1,
                        "command_type": "shell_command",
                        "success": true,
                        "output": grouped_output
                    }
                ]
            },
            "success": true
        });

        let content = command_run_function_output_for_context(&value);

        assert!(content.contains("---QUERY--- alpha"));
        assert!(content.contains("---QUERY--- beta"));
        assert!(content.contains("---QUERY--- gamma"));
        assert!(
            content.matches("tokens truncated").count() >= 3,
            "each query condition should be truncated independently: {content}"
        );
    }

    #[test]
    fn command_run_context_keeps_all_markers_for_large_grouped_file_batch() {
        let files = (0..25)
            .map(|index| format!("src/retail_core/file_{index:02}.py"))
            .collect::<Vec<_>>();
        let mut grouped_output = String::from("Exit code: 0\nWall time: 0.6 seconds\nOutput:\n");
        for file in &files {
            grouped_output.push_str(&format!("---FILE--- {file}\n"));
            grouped_output.push_str(&format!("{file} important header\n"));
            grouped_output.push_str(&format!("{}\n", "body line\n".repeat(3_000)));
            grouped_output.push_str(&format!("{file} important tail\n"));
        }
        let command_line = format!(
            "$files=@({}); foreach ($f in $files) {{ Write-Output ('---FILE--- ' + $f); Get-Content $f }}",
            files
                .iter()
                .map(|file| format!("'{file}'"))
                .collect::<Vec<_>>()
                .join(",")
        );
        let value = json!({
            "tool_name": "command_run",
            "input": {
                "commands": [
                    {
                        "step": 1,
                        "command_type": "shell_command",
                        "command_line": command_line
                    }
                ]
            },
            "output": {
                "results": [
                    {
                        "step": 1,
                        "command_type": "shell_command",
                        "success": true,
                        "output": grouped_output
                    }
                ]
            },
            "success": true
        });

        let content = command_run_function_output_for_context(&value);

        for file in &files {
            assert!(
                content.contains(&format!("---FILE--- {file}")),
                "missing grouped file marker for {file}"
            );
            assert!(
                content.contains(&format!("{file} important header"))
                    || content.contains(&format!("{file} important tail")),
                "missing retained content for {file}"
            );
        }
        assert!(
            content.matches("tokens truncated").count() >= files.len(),
            "each oversized file section should be independently truncated"
        );
    }

    #[test]
    fn command_run_context_keeps_all_markers_for_large_multi_query_batch() {
        let terms = ["alpha", "beta", "gamma", "delta", "epsilon", "zeta"];
        let mut output = String::from("Exit code: 0\nWall time: 0.2 seconds\nOutput:\n");
        for term in terms {
            output.push_str(&format!("src/{term}.py:1:{term} match\n").repeat(3_000));
        }
        let value = json!({
            "tool_name": "command_run",
            "input": {
                "commands": [
                    {
                        "step": 1,
                        "command_type": "shell_command",
                        "command_line": "rg -n \"alpha|beta|gamma|delta|epsilon|zeta\" src"
                    }
                ]
            },
            "output": {
                "results": [
                    {
                        "step": 1,
                        "command_type": "shell_command",
                        "success": true,
                        "output": output
                    }
                ]
            },
            "success": true
        });

        let content = command_run_function_output_for_context(&value);

        for term in terms {
            assert!(
                content.contains(&format!("---QUERY--- {term}")),
                "missing query marker for {term}"
            );
            assert!(content.contains(&format!("{term} match")));
        }
        assert!(
            content.matches("tokens truncated").count() >= terms.len(),
            "each oversized query section should be independently truncated"
        );
    }

    #[test]
    fn command_run_context_splits_space_separated_query_terms() {
        let output = format!(
            "Exit code: 0\nWall time: 0.2 seconds\nOutput:\n{}{}{}{}{}{}\n",
            "src/a.py:1:a keyword\n".repeat(1_000),
            "src/b.py:1:b keyword\n".repeat(1_000),
            "src/c.py:1:c keyword\n".repeat(1_000),
            "src/e.py:1:e function\n".repeat(1_000),
            "src/d.py:1:d function\n".repeat(1_000),
            "src/f.py:1:f function\n".repeat(1_000),
        );
        let value = json!({
            "tool_name": "command_run",
            "input": {
                "commands": [
                    {
                        "step": 1,
                        "command_type": "shell_command",
                        "command_line": "rg -n \"a b c\" src; rg -n \"e d f\" src"
                    }
                ]
            },
            "output": {
                "results": [
                    {
                        "step": 1,
                        "command_type": "shell_command",
                        "success": true,
                        "output": output
                    }
                ]
            },
            "success": true
        });

        let content = command_run_function_output_for_context(&value);

        for term in ["a", "b", "c", "e", "d", "f"] {
            assert!(
                content.contains(&format!("---QUERY--- {term}")),
                "missing query marker for {term}: {content}"
            );
        }
    }

    #[test]
    fn command_run_context_labels_bare_file_markers_from_command_line_reads() {
        let grouped_output = format!(
            "Exit code: 0\nWall time: 0.1 seconds\nOutput:\n{}\n---FILE---\n{}\n---FILE---\n{}\n",
            "a content\n".repeat(3_000),
            "b content\n".repeat(3_000),
            "c content\n".repeat(3_000),
        );
        let value = json!({
            "tool_name": "command_run",
            "input": {
                "commands": [
                    {
                        "step": 1,
                        "command_type": "shell_command",
                        "command_line": "{\"command\":\"Get-Content src/a.py; Write-Host '---FILE---'; Get-Content src/b.py; Write-Host '---FILE---'; Get-Content src/c.py\",\"timeout_ms\":10000}"
                    }
                ]
            },
            "output": {
                "results": [
                    {
                        "step": 1,
                        "command_type": "shell_command",
                        "success": true,
                        "output": grouped_output
                    }
                ]
            },
            "success": true
        });

        let content = command_run_function_output_for_context(&value);

        assert!(content.contains("---FILE--- src/b.py"));
        assert!(content.contains("---FILE--- src/c.py"));
        assert!(
            content.matches("tokens truncated").count() >= 3,
            "bare marker sections should still be independently truncated: {content}"
        );
    }

    #[test]
    fn build_context_replays_dialog_entries_without_rewriting_history() {
        let mut session = session();
        for index in 0..4 {
            accumulate_message(&mut session, "user", json!(format!("user-{index}")))
                .expect("user message should be logged");
            accumulate_message(
                &mut session,
                "assistant",
                json!(format!("assistant-{index}")),
            )
            .expect("assistant message should be logged");
        }

        let output = build_context(ContextInput {
            runtime: runtime(&session),
            session,
            additional_messages: vec![],
        })
        .expect("context should build");
        let contents = output
            .messages
            .iter()
            .filter_map(|message| message.get("content"))
            .map(|content| {
                content
                    .as_str()
                    .map(str::to_string)
                    .unwrap_or_else(|| content.to_string())
            })
            .collect::<Vec<_>>();

        assert!(contents.iter().any(|content| content.contains("user-0")));
        assert!(contents
            .iter()
            .any(|content| content.contains("assistant-0")));
        assert!(contents
            .iter()
            .any(|content| content.contains("assistant-1")));
        assert!(contents.iter().any(|content| content.contains("user-3")));
        assert_eq!(
            output
                .messages
                .iter()
                .filter(|message| matches!(message["role"].as_str(), Some("user" | "assistant")))
                .count(),
            9
        );
    }

    #[test]
    fn build_context_keeps_initial_task_before_tool_results_for_cache_prefix() {
        let mut session = session();
        session.input.user_input = "fix the failing planner tests".to_string();
        accumulate_tool_result(
            &mut session,
            "command_run",
            json!({ "commands": [{ "step": 1, "command": "npm test" }] }),
            json!({
                "ok": false,
                "results": [{
                    "step": 1,
                    "command": "shell_command",
                    "success": false,
                    "output": "Exit code: 1\nOutput:\nplanner failed"
                }]
            }),
            false,
            Some("tests failed".to_string()),
        )
        .expect("tool result should be logged");

        let output = build_context(ContextInput {
            runtime: runtime(&session),
            session,
            additional_messages: vec![],
        })
        .expect("context should build");

        assert_eq!(output.messages[0]["role"], "user");
        assert_eq!(
            output.messages[0]["content"],
            "fix the failing planner tests"
        );
        assert_eq!(output.messages[1]["type"], "function_call");
        assert_eq!(output.messages[1]["name"], "command_run");
        assert_eq!(output.messages[2]["type"], "function_call_output");
        assert!(output.messages[2]["output"]
            .as_str()
            .is_some_and(|content| content.starts_with('{') && content.contains("\"results\"")));
    }

    #[test]
    fn command_run_tool_results_persist_exact_response_items_for_cache_prefix() {
        let mut session = session();
        accumulate_tool_result(
            &mut session,
            "command_run",
            json!({
                "commands": [{
                    "step": 1,
                    "command_type": "shell_command",
                    "command_line": "python -m pytest tests/test_orders.py"
                }]
            }),
            json!({
                "results": [{
                    "step": 1,
                    "command": "shell_command",
                    "success": false,
                    "output": "Exit code: 1\nOutput:\norders failed"
                }]
            }),
            false,
            Some("tests failed".to_string()),
        )
        .expect("tool result should be logged");

        let entry = session
            .session_log
            .iter()
            .filter_map(|entry| serde_json::from_str::<serde_json::Value>(entry).ok())
            .find(|value| value.get("type").and_then(|kind| kind.as_str()) == Some("tool_result"))
            .expect("tool_result should be persisted");
        let context_messages = entry["context_messages"]
            .as_array()
            .expect("context messages should be persisted");
        assert_eq!(context_messages.len(), 2);
        assert_eq!(context_messages[0]["type"], "function_call");
        assert_eq!(context_messages[0]["name"], "command_run");
        assert_eq!(context_messages[1]["type"], "function_call_output");
        assert_eq!(
            context_messages[0]["call_id"],
            context_messages[1]["call_id"]
        );

        let output = build_context(ContextInput {
            runtime: runtime(&session),
            session,
            additional_messages: vec![],
        })
        .expect("context should build");
        assert!(output
            .messages
            .windows(2)
            .any(|pair| pair[0] == context_messages[0] && pair[1] == context_messages[1]));
    }

    #[test]
    fn command_run_context_prefix_is_append_only_across_later_tool_results() {
        let mut session = session();
        session.input.user_input = "fix the checkout bug".to_string();
        let large_output = "Exit code: 0\nOutput:\n".to_string() + &"line\n".repeat(200);
        accumulate_tool_result(
            &mut session,
            "command_run",
            json!({
                "commands": [{
                    "step": 1,
                    "command_type": "shell_command",
                    "command_line": "Get-Content src/checkout.py"
                }]
            }),
            json!({
                "results": [{
                    "step": 1,
                    "command": "shell_command",
                    "success": true,
                    "output": large_output
                }]
            }),
            true,
            None,
        )
        .expect("first command_run result should be logged");

        let first_messages = build_context(ContextInput {
            runtime: runtime(&session),
            session: session.clone(),
            additional_messages: vec![],
        })
        .expect("first context should build")
        .messages;

        accumulate_tool_result(
            &mut session,
            "command_run",
            json!({
                "commands": [{
                    "step": 1,
                    "command_type": "apply_patch",
                    "command_line": "*** Begin Patch\n*** Update File: src/checkout.py\n@@\n-old\n+new\n*** End Patch"
                }]
            }),
            json!({
                "results": [{
                    "step": 1,
                    "command": "apply_patch",
                    "success": true,
                    "output": "Success. Updated the following files:\nM src/checkout.py"
                }]
            }),
            true,
            None,
        )
        .expect("second command_run result should be logged");

        let second_messages = build_context(ContextInput {
            runtime: runtime(&session),
            session,
            additional_messages: vec![],
        })
        .expect("second context should build")
        .messages;

        assert!(second_messages.len() > first_messages.len());
        assert_eq!(
            &second_messages[..first_messages.len()],
            first_messages.as_slice()
        );
    }

    #[test]
    fn command_run_function_output_backfills_current_json_text_with_command_type_key() {
        let value = json!({
            "tool_name": "command_run",
            "input": {
                "commands": [
                    {
                        "step": 1,
                        "command": "shell_command",
                        "command_line": "powershell -NoProfile -ExecutionPolicy Bypass -File tools/verify.ps1"
                    }
                ]
            },
            "output": {
                "results": [{
                    "step": 1,
                    "command": "shell_command",
                    "success": false,
                    "output": {
                        "metadata": { "exit_code": 1 },
                        "output": "Exit code: 1\nWall time: 2.7 seconds\nOutput:\nverify failed\n",
                        "stdout": "verify failed\n",
                        "stderr": ""
                    }
                }]
            }
        });

        let output = command_run_function_output_for_context(&value);
        assert_eq!(
            output,
            concat!(
                "{\n",
                "  \"results\": [\n",
                "    {\n",
                "      \"step\": 1,\n",
                "      \"command_type\": \"shell_command\",\n",
                "      \"success\": false,\n",
                "      \"output\": \"Exit code: 1\\nWall time: 2.7 seconds\\nOutput:\\nverify failed\\n\"\n",
                "    }\n",
                "  ]\n",
                "}"
            )
        );
        assert!(!output.contains("\"metadata\""));
        assert!(!output.contains("\"stdout\""));
        assert!(!output.contains("\"stderr\""));
        assert!(!output.contains("Total output lines"));
        assert!(!output.contains("\"command\":"));
    }

    #[test]
    fn read_media_command_run_context_returns_text_plus_input_images_without_base64_text_bloat() {
        let output = command_run_function_output_payload_for_context(&json!({
            "tool_name": "command_run",
            "input": {
                "commands": [
                    { "command_type": "read_media", "command_line": "{\"paths\":[\"sample.png\"]}" }
                ]
            },
            "output": {
                "results": [
                    {
                        "step": 1,
                        "command_type": "read_media",
                        "success": true,
                        "output": {
                            "media_results": [
                                {
                                    "path": "sample.png",
                                    "success": true,
                                    "media_type": "image",
                                    "extracted_text": "",
                                    "visual_preview_count": 1,
                                    "visual_previews": [
                                        {
                                            "type": "image_url",
                                            "image_url": { "url": "data:image/jpeg;base64,AAA" }
                                        }
                                    ]
                                }
                            ],
                            "summary_markdown": "- sample.png: image, 1 visual previews"
                        }
                    }
                ]
            }
        }));
        let items = output.as_array().expect("content items");
        assert_eq!(items[0]["type"], "input_text");
        assert_eq!(items[1]["type"], "input_image");
        assert_eq!(items[1]["image_url"], "data:image/jpeg;base64,AAA");
        let text = items[0]["text"].as_str().expect("text item");
        assert!(text.contains("\"visual_preview_count\": 1"));
        assert!(!text.contains("data:image/jpeg;base64,AAA"));
    }

    #[test]
    fn read_media_command_run_context_omits_audio_media_without_base64_text_bloat() {
        let output = command_run_function_output_payload_for_context(&json!({
            "tool_name": "command_run",
            "input": {
                "commands": [
                    { "command_type": "read_media", "command_line": "{\"paths\":[\"tone.wav\"]}" }
                ]
            },
            "output": {
                "results": [
                    {
                        "step": 1,
                        "command_type": "read_media",
                        "success": true,
                        "output": {
                            "media_results": [
                                {
                                    "path": "tone.wav",
                                    "success": true,
                                    "media_type": "audio",
                                    "extracted_text": "",
                                    "visual_preview_count": 0,
                                    "visual_previews": [],
                                    "audio_preview_count": 1,
                                    "audio_previews": [
                                        {
                                            "type": "audio_url",
                                            "audio_url": {
                                                "url": "data:audio/mpeg;base64,QUJD",
                                                "format": "mp3"
                                            }
                                        }
                                    ]
                                }
                            ],
                            "summary_markdown": "- tone.wav: audio, 0 visual previews, 1 audio previews"
                        }
                    }
                ]
            }
        }));
        let items = output.as_array().expect("content items");
        assert_eq!(items[0]["type"], "input_text");
        assert_eq!(items[1]["type"], "input_text");
        assert!(items[1]["text"]
            .as_str()
            .expect("audio placeholder")
            .contains("Audio media omitted"));
        let text = items[0]["text"].as_str().expect("text item");
        assert!(text.contains("\"audio_preview_count\": 1"));
        assert!(!text.contains("data:audio/mpeg;base64,QUJD"));
        assert_eq!(items.len(), 2);
    }

    #[test]
    fn read_media_command_run_context_returns_document_attachment_as_input_file_without_base64_text_bloat(
    ) {
        let output = command_run_function_output_payload_for_context(&json!({
            "tool_name": "command_run",
            "input": {
                "commands": [
                    { "command_type": "read_media", "command_line": "report.docx --max-files 1" }
                ]
            },
            "output": {
                "results": [
                    {
                        "step": 1,
                        "command_type": "read_media",
                        "success": true,
                        "output": {
                            "media_results": [
                                {
                                    "path": "report.docx",
                                    "success": true,
                                    "media_type": "document",
                                    "extracted_text": "",
                                    "visual_preview_count": 0,
                                    "visual_previews": [],
                                    "audio_preview_count": 0,
                                    "audio_previews": [],
                                    "file_attachment_count": 1,
                                    "file_attachments": [
                                        {
                                            "type": "file",
                                            "file_name": "report.docx",
                                            "mime_type": "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
                                            "data_base64": "QUJD"
                                        }
                                    ]
                                }
                            ]
                        }
                    }
                ]
            }
        }));
        let items = output.as_array().expect("content items");
        assert_eq!(items[0]["type"], "input_text");
        assert_eq!(items[1]["type"], "input_file");
        assert_eq!(items[1]["filename"], "report.docx");
        assert_eq!(
            items[1]["file_data"],
            "data:application/vnd.openxmlformats-officedocument.wordprocessingml.document;base64,QUJD"
        );
        let text = items[0]["text"].as_str().expect("text item");
        assert!(text.contains("\"file_attachment_count\": 1"));
        assert!(!text.contains("QUJD"));
    }

    #[test]
    fn read_media_image_context_persists_across_later_turns() {
        let mut session = session();
        accumulate_tool_result(
            &mut session,
            "command_run",
            json!({
                "commands": [
                    { "command_type": "read_media", "command_line": "{\"paths\":[\"sample.png\"]}" }
                ]
            }),
            json!({
                "results": [
                    {
                        "step": 1,
                        "command_type": "read_media",
                        "success": true,
                        "output": {
                            "media_results": [
                                {
                                    "path": "sample.png",
                                    "success": true,
                                    "media_type": "image",
                                    "extracted_text": "",
                                    "visual_preview_count": 1,
                                    "visual_previews": [
                                        {
                                            "type": "image_url",
                                            "image_url": { "url": "data:image/jpeg;base64,AAA" }
                                        }
                                    ]
                                }
                            ],
                            "summary_markdown": "- sample.png: image, 1 visual previews"
                        }
                    }
                ]
            }),
            true,
            None,
        )
        .expect("tool result should accumulate");
        accumulate_message(
            &mut session,
            "assistant",
            json!("The left panel is red and the right panel is blue."),
        )
        .expect("assistant message should accumulate");
        accumulate_message(
            &mut session,
            "user",
            json!("What was the color on the right side?"),
        )
        .expect("user message should accumulate");

        let output = build_context(ContextInput {
            runtime: runtime(&session),
            session,
            additional_messages: Vec::new(),
        })
        .expect("context should build");

        let serialized = serde_json::to_string(&output.messages).expect("context serializes");
        assert!(serialized.contains("\"type\":\"input_image\""));
        assert!(serialized.contains("data:image/jpeg;base64,AAA"));
    }

    #[test]
    fn build_context_never_compacts_older_command_run_results_by_count() {
        let mut session = session();
        for index in 0..16 {
            accumulate_tool_result(
                &mut session,
                "command_run",
                json!({
                    "commands": [{
                        "step": index + 1,
                        "command": "shell_command",
                        "command_line": format!("echo unique-output-{index}")
                    }]
                }),
                json!({
                    "results": [{
                        "step": index + 1,
                        "command": "shell_command",
                        "success": true,
                        "output": format!("Exit code: 0\nOutput:\nunique-output-{index}")
                    }]
                }),
                true,
                None,
            )
            .expect("command_run result should be logged");
        }

        let output = build_context(ContextInput {
            runtime: runtime(&session),
            session,
            additional_messages: vec![],
        })
        .expect("context should build");
        let joined = output
            .messages
            .iter()
            .filter_map(|message| {
                message
                    .get("output")
                    .or_else(|| message.get("content"))
                    .and_then(|value| value.as_str())
            })
            .collect::<Vec<_>>()
            .join("\n");

        assert!(joined.contains("unique-output-0"));
        assert!(joined.contains("unique-output-15"));
        assert!(!joined.contains("older command_run output compacted"));
    }

    #[test]
    fn build_context_keeps_prior_tool_result_immutable_even_when_later_evaluated_not_helpful() {
        let mut session = session();
        accumulate_tool_result(
            &mut session,
            "grep",
            json!({ "pattern": "needle", "secret_command": "do not retain" }),
            json!({ "matches": ["secret-output.rs"] }),
            true,
            None,
        )
        .expect("first tool result should be logged");
        accumulate_tool_result_with_feedback(
            &mut session,
            "read",
            json!({ "filePath": "README.md" }),
            json!({ "content": "latest output" }),
            true,
            None,
            Some(json!([{ "command": "grep", "evaluation": "completed_not_helpful" }])),
            None,
        )
        .expect("second tool result should be logged");

        let output = build_context(ContextInput {
            runtime: runtime(&session),
            session,
            additional_messages: vec![],
        })
        .expect("context should build");
        let joined = output
            .messages
            .iter()
            .map(|message| message.to_string())
            .collect::<Vec<_>>()
            .join("\n");

        assert!(joined.contains("grep"));
        assert!(joined.contains("secret-output.rs"));
        assert!(joined.contains("secret_command"));
    }

    #[test]
    fn build_context_omits_evaluation_messages_when_agent_disables_them() {
        let mut session = session();
        session.use_last_tool_call_response = false;
        accumulate_tool_result(
            &mut session,
            "grep",
            json!({ "pattern": "needle", "secret_command": "do not retain" }),
            json!({ "matches": ["secret-output.rs"] }),
            true,
            None,
        )
        .expect("first tool result should be logged");
        accumulate_tool_result_with_feedback(
            &mut session,
            "read",
            json!({ "filePath": "README.md" }),
            json!({ "content": "latest output" }),
            true,
            None,
            Some(json!([{ "command": "grep", "evaluation": "completed_not_helpful" }])),
            None,
        )
        .expect("second tool result should be logged");

        let output = build_context(ContextInput {
            runtime: runtime(&session),
            session,
            additional_messages: vec![],
        })
        .expect("context should build");
        let joined = output
            .messages
            .iter()
            .map(|message| message.to_string())
            .collect::<Vec<_>>()
            .join("\n");

        assert!(!joined.contains("Tool result enum evaluations"));
        assert!(joined.contains("latest output"));
        assert!(joined.contains("secret-output.rs"));
    }

    #[test]
    fn build_context_retains_unevaluated_tool_results_when_agent_disables_evaluations() {
        let mut session = session();
        session.use_last_tool_call_response = false;
        accumulate_tool_result(
            &mut session,
            "command_run",
            json!({
                "commands": [
                    { "step": 1, "command": "read_line", "command_line": "target_tasks.json" }
                ]
            }),
            json!({
                "ok": true,
                "results": [{
                    "mode": "batch",
                    "results": [{
                        "step": 1,
                        "command_name": "read_line",
                        "ok": true,
                        "response": {
                            "ok": true,
                            "stdout": "{\"content\":\"django__django-11049 target metadata\"}",
                            "stderr": ""
                        }
                    }]
                }]
            }),
            true,
            None,
        )
        .expect("first command_run result should be logged");
        accumulate_tool_result(
            &mut session,
            "command_run",
            json!({
                "commands": [
                    { "step": 1, "command": "glob", "command_line": "*" }
                ]
            }),
            json!({
                "ok": true,
                "results": [{
                    "mode": "batch",
                    "results": [{
                        "step": 1,
                        "command_name": "glob",
                        "ok": true,
                        "response": {
                            "ok": true,
                            "stdout": "{\"matched_paths\":[\"predictions.jsonl\"]}",
                            "stderr": ""
                        }
                    }]
                }]
            }),
            true,
            None,
        )
        .expect("second command_run result should be logged");

        let output = build_context(ContextInput {
            runtime: runtime(&session),
            session,
            additional_messages: vec![],
        })
        .expect("context should build");
        let joined = output
            .messages
            .iter()
            .map(|message| message.to_string())
            .collect::<Vec<_>>()
            .join("\n");

        assert!(joined.contains("django__django-11049 target metadata"));
        assert!(joined.contains("predictions.jsonl"));
    }

    #[test]
    fn build_context_keeps_existing_tool_context_messages_append_only() {
        let mut session = session();
        session.use_last_tool_call_response = false;
        accumulate_tool_result(
            &mut session,
            "grep",
            json!({ "pattern": "needle" }),
            json!({ "matches": ["src/lib.rs"] }),
            true,
            None,
        )
        .expect("first tool result should be logged");

        let first_output = build_context(ContextInput {
            runtime: runtime(&session),
            session: session.clone(),
            additional_messages: vec![],
        })
        .expect("first context should build");
        let first_tool_message = first_output
            .messages
            .iter()
            .find(|message| {
                message["content"].as_str().is_some_and(|content| {
                    content.starts_with('[') && content.contains("src/lib.rs")
                })
            })
            .cloned()
            .expect("first tool context message should exist");
        assert!(first_tool_message["content"]
            .as_str()
            .is_some_and(|content| content.contains("\"cache_id\"")));

        accumulate_tool_result(
            &mut session,
            "read",
            json!({ "file": "src/lib.rs" }),
            json!({ "content": "latest output" }),
            true,
            None,
        )
        .expect("second tool result should be logged");

        let second_output = build_context(ContextInput {
            runtime: runtime(&session),
            session,
            additional_messages: vec![],
        })
        .expect("second context should build");

        assert_eq!(second_output.messages[0]["role"], "user");
        assert_eq!(second_output.messages[1], first_tool_message);
        assert!(second_output
            .messages
            .iter()
            .any(|message| message["content"]
                .as_str()
                .is_some_and(|content| content.contains("latest output"))));
    }

    #[test]
    fn runtime_context_messages_do_not_inject_reporting_prompt_from_evaluation_flag() {
        let session = session();
        let enabled =
            messages_with_runtime_context(&session, &[], Some("fast"), Some("model"), false);
        let disabled = messages_with_runtime_context(
            &session,
            &[],
            Some("flagship_thinking"),
            Some("model"),
            false,
        );

        assert!(enabled.is_empty());
        assert!(disabled.is_empty());
    }

    #[test]
    fn build_context_does_not_rewrite_prior_tool_result_when_later_marked_helpful() {
        let mut session = session();
        accumulate_tool_result(
            &mut session,
            "grep",
            json!({ "pattern": "needle" }),
            json!({ "matches": ["src/lib.rs"] }),
            true,
            None,
        )
        .expect("first tool result should be logged");
        accumulate_tool_result_with_feedback(
            &mut session,
            "read",
            json!({ "filePath": "src/lib.rs" }),
            json!({ "content": "latest output" }),
            true,
            None,
            Some(json!([{ "command": "grep", "evaluation": "completed_helpful" }])),
            Some("grep was useful but this receipt should not be retained".to_string()),
        )
        .expect("second tool result should be logged");

        let output = build_context(ContextInput {
            runtime: runtime(&session),
            session,
            additional_messages: vec![],
        })
        .expect("context should build");
        let joined = output
            .messages
            .iter()
            .map(|message| message.to_string())
            .collect::<Vec<_>>()
            .join("\n");

        assert!(joined.contains("src/lib.rs"));
        assert!(!joined.contains("grep was useful but this receipt should not be retained"));
        assert!(joined.contains("pattern"));
    }

    #[test]
    fn build_context_keeps_mixed_evaluation_command_run_result_immutable() {
        let mut session = session();
        accumulate_tool_result(
            &mut session,
            "command_run",
            json!({
                "commands": [
                    { "command": "rg", "command_line": "rg needle" },
                    { "command": "cargo", "command_line": "cargo check" }
                ]
            }),
            json!({
                "service": "command_run",
                "mode": "batch",
                "results": [
                    { "tool_name": "rg", "stdout": "helpful-file.rs" },
                    { "tool_name": "cargo", "stdout": "not-helpful-build-output" }
                ]
            }),
            true,
            None,
        )
        .expect("command_run result should be logged");
        accumulate_tool_result_with_feedback(
            &mut session,
            "command_run",
            json!({
                "commands": [
                    { "command": "shell_command", "command_line": "Write-Output next" }
                ]
            }),
            json!({
                "service": "command_run",
                "mode": "batch",
                "results": [
                    { "tool_name": "next", "stdout": "latest output" }
                ]
            }),
            true,
            None,
            Some(json!([
                { "step": 2, "command": "rg", "evaluation": "completed_helpful" },
                { "step": 3, "command": "cargo", "evaluation": "completed_not_helpful" }
            ])),
            None,
        )
        .expect("second command_run result should be logged");

        let output = build_context(ContextInput {
            runtime: runtime(&session),
            session,
            additional_messages: vec![],
        })
        .expect("context should build");
        let joined = output
            .messages
            .iter()
            .map(|message| message.to_string())
            .collect::<Vec<_>>()
            .join("\n");

        assert!(joined.contains("helpful-file.rs"));
        assert!(joined.contains("not-helpful-build-output"));
    }

    #[test]
    fn build_context_flattens_nested_command_run_batch_results() {
        let mut session = session();
        session.use_last_tool_call_response = false;
        accumulate_tool_result(
            &mut session,
            "command_run",
            json!({
                "commands": [
                    { "step": 1, "command": "read_line", "command_line": "{\"path\":\"src/catalog.js\"}" },
                    { "step": 1, "command": "node", "command_line": "node test/order.test.js" }
                ]
            }),
            json!({
                "ok": false,
                "results": [{
                    "mode": "batch",
                    "ok": false,
                    "results": [
                        {
                            "step": 1,
                            "index": 0,
                            "ok": true,
                            "tool_name": "read_line",
                            "response": {
                                "ok": true,
                                "cwd": "C:/workspace",
                                "stdout": "{\"ok\":true,\"results\":[{\"content\":\"function normalizeSku(sku) { return String(sku) }\"}]}",
                                "stderr": ""
                            }
                        },
                        {
                            "step": 1,
                            "index": 1,
                            "ok": false,
                            "tool_name": "node",
                            "response": {
                                "ok": false,
                                "cwd": "C:/workspace",
                                "stdout": "",
                                "stderr": "' a100 ' !== 'A100'"
                            }
                        }
                    ]
                }]
            }),
            false,
            Some("node test failed".to_string()),
        )
        .expect("command_run result should be logged");

        let output = build_context(ContextInput {
            runtime: runtime(&session),
            session,
            additional_messages: vec![],
        })
        .expect("context should build");
        let joined = output
            .messages
            .iter()
            .map(|message| message.to_string())
            .collect::<Vec<_>>()
            .join("\n");

        assert!(joined.contains("normalizeSku"));
        assert!(joined.contains("' a100 ' !== 'A100'"));
        assert!(joined.contains("read_line"));
        assert!(joined.contains("node test/order.test.js"));
        assert!(!joined.contains("last_tool_call_response"));
    }

    #[test]
    fn command_run_code_reads_use_raw_transcript_for_llm_context() {
        let context = command_run_summary_for_context(&json!({
            "tool_name": "command_run",
            "input": {
                "commands": [
                    { "step": 1, "command": "cat", "command_line": "{\"path\":\"src/app.py\"}" }
                ]
            },
            "output": {
                "ok": true,
                "results": [{
                    "mode": "batch",
                    "ok": true,
                    "results": [{
                        "step": 1,
                        "index": 0,
                        "ok": true,
                        "tool_name": "cat",
                        "response": {
                            "ok": true,
                            "exit_code": 0,
                            "stdout": "{\"ok\":true,\"results\":[{\"path\":\"src/app.py\",\"start_line\":1,\"end_line\":2,\"content\":\"def normalize(value):\\n    return value.strip()\"}],\"errors\":[],\"warnings\":[]}",
                            "stderr": ""
                        }
                    }]
                }]
            }
        }));
        let text = context
            .as_str()
            .expect("LLM context should be raw transcript");

        assert!(text.contains("$ cat"));
        assert!(text.contains("src/app.py"));
        assert!(text.contains("Exit code: 0"));
        assert!(text.contains("def normalize(value):\n    return value.strip()"));
        assert!(!text.contains("\"results\""));
        assert!(!text.contains("\\n"));
    }

    #[test]
    fn command_run_rg_context_looks_like_ripgrep_output() {
        let context = command_run_summary_for_context(&json!({
            "tool_name": "command_run",
            "input": {
                "commands": [
                    {
                        "step": 1,
                        "command": "rg",
                        "command_line": "{\"query\":\"normalize\",\"directory\":\"src\",\"file_glob\":\"*.py\",\"use_regex\":false}"
                    }
                ]
            },
            "output": {
                "ok": true,
                "results": [{
                    "mode": "batch",
                    "ok": true,
                    "results": [{
                        "step": 1,
                        "index": 0,
                        "ok": true,
                        "tool_name": "rg",
                        "response": {
                            "ok": true,
                            "exit_code": 0,
                            "stdout": "{\"ok\":true,\"results\":[{\"query\":\"normalize\",\"directory\":\"src\",\"summary_markdown\":\"table omitted\",\"matches\":[{\"path\":\"src/app.py\",\"line\":7,\"content\":\"def normalize(value):\"}],\"matched_paths\":[\"src/app.py\"]}],\"errors\":[],\"warnings\":[]}",
                            "stderr": ""
                        }
                    }]
                }]
            }
        }));
        let text = context
            .as_str()
            .expect("LLM context should be a transcript");

        assert!(text.contains("$ rg -n -i --fixed-strings -g '*.py' normalize src"));
        assert!(text.contains("src/app.py:7:def normalize(value):"));
        assert!(!text.contains("summary_markdown"));
        assert!(!text.contains("matched_paths"));
        assert!(!text.contains("\"matches\""));
    }

    #[test]
    fn command_run_mutation_context_uses_cli_style_summary_without_rich_json() {
        let context = command_run_summary_for_context(&json!({
            "tool_name": "command_run",
            "input": {
                "commands": [
                    { "step": 1, "command": "apply_patch", "command_line": "*** Begin Patch\n*** Update File: src/app.py\n@@\n-old\n+new\n*** End Patch" }
                ]
            },
            "output": {
                "ok": true,
                "results": [{
                    "mode": "batch",
                    "ok": true,
                    "results": [{
                        "step": 1,
                        "index": 0,
                        "ok": true,
                        "tool_name": "apply_patch",
                        "response": {
                            "ok": true,
                            "exit_code": 0,
                            "stdout": "{\"ok\":true,\"results\":[{\"path\":\"src/app.py\",\"resolved_path\":\"C:/workspace/src/app.py\",\"operation\":\"update\",\"status\":\"applied\",\"applied\":true,\"syntax_ok\":true,\"diagnostics\":[],\"error\":null},{\"summary_markdown\":\"Success. Updated the following files:\\nM src/app.py\",\"changed_paths\":[\"M src/app.py\"],\"failed_paths\":[],\"syntax_error_paths\":[],\"partial\":false}],\"errors\":[],\"warnings\":[]}",
                            "stderr": ""
                        }
                    }]
                }]
            }
        }));
        let text = context
            .as_str()
            .expect("LLM context should be a transcript");

        assert!(text.contains("$ apply_patch <<'PATCH'"));
        assert!(text.contains("src/app.py: applied"));
        assert!(text.contains("Success. Updated the following files:\nM src/app.py"));
        assert!(!text.contains("resolved_path"));
        assert!(!text.contains("syntax_ok"));
        assert!(!text.contains("changed_paths"));
    }

    #[test]
    fn command_run_verify_output_keeps_failure_streams_only() {
        let context = command_run_summary_for_context(&json!({
            "tool_name": "command_run",
            "input": {
                "commands": [
                    { "step": 1, "command": "shell_command", "command_line": "powershell -NoProfile -ExecutionPolicy Bypass -File tools/verify.ps1" }
                ]
            },
            "output": {
                "ok": false,
                "results": [{
                    "step": 1,
                    "command": "shell_command",
                    "success": false,
                    "exit_code": 1,
                    "response": {
                        "success": false,
                        "exit_code": 1,
                        "stdout": serde_json::to_string(&json!({
                            "ok": false,
                            "returncodes": { "node": 1, "python": 0 },
                            "stdout": {
                                "node": "not ok 1 - generated view modules fail",
                                "python": "test_ok ... ok\nRan 29 tests in 0.01s\nOK"
                            },
                            "stderr": {
                                "node": "SyntaxError: Invalid regular expression",
                                "python": ""
                            }
                        }))
                            .expect("command-run fixture stdout JSON should serialize"),
                        "stderr": ""
                    }
                }]
            }
        }));
        let text = context
            .as_str()
            .expect("LLM context should be a transcript");

        assert!(text.contains("verify.ps1 ok: false"));
        assert!(text.contains("returncodes: node=1, python=0"));
        assert!(text.contains("node stdout:"));
        assert!(text.contains("SyntaxError: Invalid regular expression"));
        assert!(text.contains("python: passed"));
        assert!(!text.contains("test_ok ... ok"));
    }
}
