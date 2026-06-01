use crate::prompt_style::{
    compact_context, task_continuity, task_status, user_new_command, PromptBuilder,
};
use crate::state_machine::session_management::SessionManagement;

pub(super) fn messages_for_turn(
    current_messages: &[serde_json::Value],
    session: &SessionManagement,
    original_user_task: &str,
) -> Vec<serde_json::Value> {
    let mut messages = current_messages.to_vec();
    if should_append_original_user_task(&messages, original_user_task) {
        messages.push(serde_json::json!({
            "role": "user",
            "content": original_user_task.trim(),
        }));
    }
    if let Some(content) = user_new_command_message(&session.session_id) {
        messages.push(serde_json::json!({
            "role": "system",
            "content": content,
        }));
    }
    if approximate_message_tokens(&messages) >= compact_context_token_threshold() {
        messages.push(serde_json::json!({
            "role": "user",
            "content": compact_context_required_message(),
        }));
    }
    messages
}

fn should_append_original_user_task(
    messages: &[serde_json::Value],
    original_user_task: &str,
) -> bool {
    let task = original_user_task.trim();
    if task.is_empty() {
        return false;
    }
    !messages.iter().any(|message| {
        message.get("role").and_then(serde_json::Value::as_str) == Some("user")
            && message
                .get("content")
                .and_then(serde_json::Value::as_str)
                .map(str::trim)
                == Some(task)
    })
}

fn approximate_message_tokens(messages: &[serde_json::Value]) -> usize {
    messages
        .iter()
        .map(|message| estimate_message_model_visible_chars(message) / 4)
        .sum()
}

fn estimate_message_model_visible_chars(message: &serde_json::Value) -> usize {
    let raw = serde_json::to_string(message).unwrap_or_default().len();
    let mut media_payload_chars = 0usize;
    let mut media_replacement_chars = 0usize;
    accumulate_media_payload_estimate_adjustment(
        message,
        &mut media_payload_chars,
        &mut media_replacement_chars,
    );
    raw.saturating_sub(media_payload_chars)
        .saturating_add(media_replacement_chars)
}

fn accumulate_media_payload_estimate_adjustment(
    value: &serde_json::Value,
    payload_chars: &mut usize,
    replacement_chars: &mut usize,
) {
    match value {
        serde_json::Value::Object(object) => {
            if object.get("type").and_then(serde_json::Value::as_str) == Some("input_image") {
                if let Some(image_url) = object.get("image_url").and_then(serde_json::Value::as_str)
                {
                    if let Some(payload) = parse_base64_media_data_url(image_url, "image/") {
                        *payload_chars = payload_chars.saturating_add(payload.len());
                        *replacement_chars = replacement_chars.saturating_add(4096);
                    }
                }
            }
            if object.get("type").and_then(serde_json::Value::as_str) == Some("input_audio") {
                if let Some(payload) = object
                    .get("input_audio")
                    .and_then(|input_audio| input_audio.get("data"))
                    .and_then(serde_json::Value::as_str)
                {
                    *payload_chars = payload_chars.saturating_add(payload.len());
                    *replacement_chars = replacement_chars.saturating_add(8192);
                }
                if let Some(audio_url) = object
                    .get("audio_url")
                    .or_else(|| object.get("input_audio"))
                    .and_then(|audio| audio.get("url"))
                    .and_then(serde_json::Value::as_str)
                {
                    if let Some(payload) = parse_base64_media_data_url(audio_url, "audio/") {
                        *payload_chars = payload_chars.saturating_add(payload.len());
                        *replacement_chars = replacement_chars.saturating_add(8192);
                    }
                }
            }
            for child in object.values() {
                accumulate_media_payload_estimate_adjustment(
                    child,
                    payload_chars,
                    replacement_chars,
                );
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                accumulate_media_payload_estimate_adjustment(
                    item,
                    payload_chars,
                    replacement_chars,
                );
            }
        }
        _ => {}
    }
}

fn parse_base64_media_data_url<'a>(url: &'a str, mime_prefix: &str) -> Option<&'a str> {
    if !url
        .get(.."data:".len())
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("data:"))
    {
        return None;
    }
    let comma_index = url.find(',')?;
    let metadata = &url[..comma_index];
    let payload = &url[comma_index + 1..];
    let metadata_without_scheme = &metadata["data:".len()..];
    let mut metadata_parts = metadata_without_scheme.split(';');
    let mime_type = metadata_parts.next().unwrap_or_default();
    let has_base64_marker = metadata_parts.any(|part| part.eq_ignore_ascii_case("base64"));
    if !mime_type
        .get(..mime_prefix.len())
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case(mime_prefix))
    {
        return None;
    }
    has_base64_marker.then_some(payload)
}

fn compact_context_token_threshold() -> usize {
    std::env::var("TURA_COMPACT_CONTEXT_TOKEN_THRESHOLD")
        .ok()
        .and_then(|value| value.trim().parse::<usize>().ok())
        .unwrap_or(220_000)
}

fn compact_context_required_message() -> String {
    PromptBuilder::new()
        .part(compact_context::COMPACT_CONTEXT_REQUIRED)
        .render()
}

pub(super) fn user_new_command_message(session_id: &str) -> Option<String> {
    let commands = fetch_user_commands(session_id);
    if commands.is_empty() {
        return None;
    }
    let commands = commands
        .iter()
        .enumerate()
        .map(|(index, command)| format!("{}. {}", index + 1, command))
        .collect::<Vec<_>>()
        .join("\n");
    Some(
        PromptBuilder::new()
            .part(user_new_command::USER_NEW_COMMAND)
            .section("user_new_commands", commands)
            .render(),
    )
}

pub(super) fn fetch_user_commands(session_id: &str) -> Vec<String> {
    let _ = session_id;
    Vec::new()
}

/// Inject the short task_status reminder when the model keeps doing workspace
/// work (command_run turns) without ever writing or settling the task state.
/// Used by the runtime loop after N consecutive no-write command_run turns.
pub(super) fn push_task_status_nudge(messages: &mut Vec<serde_json::Value>) {
    messages.push(serde_json::json!({
        "role": "system",
        "content": task_status::TASK_STATUS,
    }));
}

pub(super) fn push_task_continuity_message(
    messages: &mut Vec<serde_json::Value>,
    session: &SessionManagement,
    original_user_task: &str,
) {
    let continuity_task =
        compacted_continuity_task(session).unwrap_or_else(|| original_user_task.to_string());
    let builder = PromptBuilder::new()
        .part(task_continuity::TASK_CONTINUITY)
        .part(task_status::TASK_STATUS)
        .section("original_user_task", continuity_task);

    messages.push(serde_json::json!({
        "role": "system",
        "content": builder.render(),
    }));
}

fn compacted_continuity_task(session: &SessionManagement) -> Option<String> {
    session
        .session_log
        .iter()
        .rev()
        .filter_map(|entry| serde_json::from_str::<serde_json::Value>(entry).ok())
        .find(|value| {
            value.get("type").and_then(serde_json::Value::as_str) == Some("context_compaction")
        })
        .and_then(|value| {
            value
                .get("content")
                .and_then(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|text| !text.is_empty())
                .map(|text| {
                    format!(
                        "The original task has been compacted. Continue from this context checkpoint instead of reinserting the pre-compaction prompt:\n{text}"
                    )
                })
        })
}

#[cfg(test)]
mod tests {
    use super::{messages_for_turn, push_task_continuity_message};
    use crate::state_machine::session_management::{SessionInput, SessionManagement};
    use chrono::Utc;
    use std::path::PathBuf;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn task_continuity_uses_compacted_task_after_context_compaction() {
        let now = Utc::now();
        let mut session = SessionManagement::new(
            "sess-compact-continuity".to_string(),
            "compact continuity".to_string(),
            PathBuf::from("C:/workspace"),
            false,
            "coding".to_string(),
            SessionInput {
                user_input: "ORIGINAL HUGE PROMPT".repeat(10),
                file_input: Vec::new(),
                agent: None,
                runtime_context: None,
            },
            "ORIGINAL HUGE PROMPT".to_string(),
            now,
        );
        session.push_log(
            serde_json::json!({
                "type": "context_compaction",
                "content": "compact handoff",
            })
            .to_string(),
            now,
        );

        let mut messages = Vec::new();
        push_task_continuity_message(&mut messages, &session, &session.input.user_input);
        let content = messages[0]["content"]
            .as_str()
            .expect("prompt message content should be a string");
        assert!(content.contains("compact handoff"));
        assert!(!content.contains("ORIGINAL HUGE PROMPTORIGINAL HUGE PROMPT"));
    }

    #[test]
    fn task_continuity_injects_task_status_guidance_by_default() {
        let now = Utc::now();
        let session = SessionManagement::new(
            "sess-task-status-guidance".to_string(),
            "task status guidance".to_string(),
            PathBuf::from("C:/workspace"),
            false,
            "coding".to_string(),
            SessionInput {
                user_input: "fix the task".to_string(),
                file_input: vec![],
                agent: None,
                runtime_context: None,
            },
            "fix the task".to_string(),
            now,
        );

        let mut messages = Vec::new();
        push_task_continuity_message(&mut messages, &session, &session.input.user_input);
        let content = messages[0]["content"]
            .as_str()
            .expect("prompt message content should be a string");

        assert!(content.contains("task_status"));
        assert!(content.contains("settle the task state"));
        assert!(content.contains("`done`") && content.contains("`question`"));
    }

    #[test]
    fn messages_for_turn_injects_compact_context_prompt_above_threshold() {
        let _guard = ENV_LOCK.lock().expect("env lock");
        std::env::set_var("TURA_COMPACT_CONTEXT_TOKEN_THRESHOLD", "10");
        let now = Utc::now();
        let session = SessionManagement::new(
            "sess-compact-threshold".to_string(),
            "compact threshold".to_string(),
            PathBuf::from("C:/workspace"),
            false,
            "coding".to_string(),
            SessionInput {
                user_input: "fix the task".to_string(),
                file_input: vec![],
                agent: None,
                runtime_context: None,
            },
            "fix the task".to_string(),
            now,
        );

        let messages = messages_for_turn(
            &[serde_json::json!({
                "role": "user",
                "content": "x".repeat(100)
            })],
            &session,
            "fix the task",
        );
        let joined = messages
            .iter()
            .map(|message| message.to_string())
            .collect::<Vec<_>>()
            .join("\n");

        assert!(joined.contains("above about 220,000 tokens"));
        assert!(joined.contains("compact_context as the final command"));
        std::env::remove_var("TURA_COMPACT_CONTEXT_TOKEN_THRESHOLD");
    }

    #[test]
    fn image_data_urls_are_discounted_for_compact_threshold_estimation() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|err| err.into_inner());
        std::env::set_var("TURA_COMPACT_CONTEXT_TOKEN_THRESHOLD", "50000");
        let now = Utc::now();
        let session = SessionManagement::new(
            "sess-image-estimate".to_string(),
            "image estimate".to_string(),
            PathBuf::from("C:/workspace"),
            false,
            "coding".to_string(),
            SessionInput {
                user_input: "remember the image".to_string(),
                file_input: vec![],
                agent: None,
                runtime_context: None,
            },
            "remember the image".to_string(),
            now,
        );

        let messages = messages_for_turn(
            &[serde_json::json!({
                "type": "function_call_output",
                "call_id": "call_image",
                "output": [
                    {
                        "type": "input_image",
                        "image_url": format!("data:image/png;base64,{}", "A".repeat(240_000))
                    }
                ]
            })],
            &session,
            "remember the image",
        );
        let joined = messages
            .iter()
            .map(|message| message.to_string())
            .collect::<Vec<_>>()
            .join("\n");

        assert!(!joined.contains("above about 220,000 tokens"));
        std::env::remove_var("TURA_COMPACT_CONTEXT_TOKEN_THRESHOLD");
    }

    #[test]
    fn audio_payloads_are_discounted_for_compact_threshold_estimation() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|err| err.into_inner());
        std::env::set_var("TURA_COMPACT_CONTEXT_TOKEN_THRESHOLD", "50000");
        let now = Utc::now();
        let session = SessionManagement::new(
            "sess-audio-estimate".to_string(),
            "audio estimate".to_string(),
            PathBuf::from("C:/workspace"),
            false,
            "coding".to_string(),
            SessionInput {
                user_input: "remember the audio".to_string(),
                file_input: vec![],
                agent: None,
                runtime_context: None,
            },
            "remember the audio".to_string(),
            now,
        );

        let messages = messages_for_turn(
            &[serde_json::json!({
                "type": "function_call_output",
                "call_id": "call_audio",
                "output": [
                    {
                        "type": "input_audio",
                        "input_audio": {
                            "format": "mp3",
                            "data": "A".repeat(240_000)
                        }
                    }
                ]
            })],
            &session,
            "remember the audio",
        );
        let joined = messages
            .iter()
            .map(|message| message.to_string())
            .collect::<Vec<_>>()
            .join("\n");

        assert!(!joined.contains("above about 220,000 tokens"));
        std::env::remove_var("TURA_COMPACT_CONTEXT_TOKEN_THRESHOLD");
    }

    #[test]
    fn messages_for_turn_injects_compact_context_prompt_at_default_220k_threshold() {
        let now = Utc::now();
        let session = SessionManagement::new(
            "sess-compact-default-threshold".to_string(),
            "compact default threshold".to_string(),
            PathBuf::from("C:/workspace"),
            false,
            "coding".to_string(),
            SessionInput {
                user_input: "continue the long task".to_string(),
                file_input: vec![],
                agent: None,
                runtime_context: None,
            },
            "continue the long task".to_string(),
            now,
        );

        let messages = messages_for_turn(
            &[serde_json::json!({
                "role": "user",
                "content": "x".repeat(900_000)
            })],
            &session,
            "continue the long task",
        );
        let last = messages
            .last()
            .and_then(|message| message.get("content"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();

        assert!(last.contains("above about 220,000 tokens"));
        assert!(last.contains("compact_context as the final command"));
    }
}
