use crate::prompt_style::{compact_context, task_status, user_new_command, PromptBuilder};
use crate::state_machine::session_management::{
    PlanStatus, SessionManagement, StartCondition, TaskStep,
};

pub(crate) fn messages_for_turn(
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
    if super::constants::gateway_callbacks_disabled() {
        return Vec::new();
    }
    let endpoint = format!(
        "{}/session/{}/user-commands",
        gateway_base_url(),
        url_escape(session_id)
    );
    let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    else {
        return Vec::new();
    };
    runtime
        .block_on(async move {
            let response = reqwest::Client::new()
                .get(endpoint)
                .timeout(std::time::Duration::from_secs(2))
                .send()
                .await
                .ok()?;
            if !response.status().is_success() {
                return None;
            }
            let value = response.json::<serde_json::Value>().await.ok()?;
            Some(
                value
                    .get("commands")
                    .and_then(serde_json::Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(|item| item.as_str())
                    .map(str::trim)
                    .filter(|item| !item.is_empty())
                    .map(ToString::to_string)
                    .collect::<Vec<_>>(),
            )
        })
        .unwrap_or_default()
}

fn gateway_base_url() -> String {
    std::env::var("TURA_GATEWAY_URL")
        .or_else(|_| std::env::var("GATEWAY_BASE_URL"))
        .unwrap_or_else(|_| {
            let port = std::env::var("TURA_GATEWAY_PORT")
                .ok()
                .and_then(|value| value.parse::<u16>().ok())
                .unwrap_or(4096);
            format!("http://127.0.0.1:{port}")
        })
        .trim_end_matches('/')
        .to_string()
}

fn url_escape(value: &str) -> String {
    value
        .bytes()
        .flat_map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                vec![byte as char]
            }
            _ => format!("%{byte:02X}").chars().collect(),
        })
        .collect()
}

/// Inject the short task_status reminder when the model keeps doing workspace
/// work (command_run turns) without ever writing or settling the task state.
/// Used by the runtime loop after N consecutive no-write command_run turns.
pub(crate) fn push_task_status_nudge(messages: &mut Vec<serde_json::Value>) {
    messages.push(serde_json::json!({
        "role": "system",
        "content": task_status::TASK_STATUS,
    }));
}

pub(crate) fn push_no_tool_task_status_retry_message(
    messages: &mut Vec<serde_json::Value>,
    session: &SessionManagement,
) {
    let content = PromptBuilder::new()
        .part(task_status::planning_objective_context(&planning_objective_block(
            session,
        )))
        .part("If more command_run calls are required to complete the task, call command_run with task_status status doing. If user feedback, missing information, permissions, credentials, or keys are required, call command_run with task_status status question. If the task is complete and verified, call command_run with task_status status done. Every task_status status change must also have a normal assistant-channel reply.")
        .render();
    messages.push(serde_json::json!({
        "role": "system",
        "content": content,
    }));
}

pub(crate) fn planning_objective_block(session: &SessionManagement) -> String {
    let overall = session.current_objective.trim();
    let Some((_index, task)) = current_planning_task(session) else {
        return format!("[current objective]:\n{overall}");
    };
    format!(
        "[current objective]:\n{}\n\n{}",
        overall,
        planning_current_task_text(task)
    )
}

pub(crate) fn planning_current_task_text(task: &TaskStep) -> String {
    task.task_summary.trim().to_string()
}

fn current_planning_task(session: &SessionManagement) -> Option<(usize, &TaskStep)> {
    session
        .task_plan
        .detailed_tasks
        .iter()
        .enumerate()
        .find(|(_, task)| {
            task.status == PlanStatus::Doing
                || (task.status == PlanStatus::Todo
                    && task.start_condition == StartCondition::UserAction)
        })
}

#[cfg(test)]
mod tests {
    use super::{
        messages_for_turn, planning_objective_block, push_no_tool_task_status_retry_message,
    };
    use crate::state_machine::session_management::{
        PlanStatus, SessionInput, SessionManagement, StartCondition, TaskStep,
    };
    use chrono::Utc;
    use std::path::PathBuf;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn test_session(user_input: &str) -> SessionManagement {
        let now = Utc::now();
        SessionManagement::new(
            "sess-prompt-messages".to_string(),
            "prompt messages".to_string(),
            PathBuf::from("C:/workspace"),
            false,
            "coding".to_string(),
            SessionInput {
                user_input: user_input.to_string(),
                file_input: Vec::new(),
                agent: None,
                runtime_context: None,
                planning_mode_override: None,
            },
            user_input.to_string(),
            now,
        )
    }

    #[test]
    fn planning_objective_block_without_task_only_includes_overall_objective() {
        let mut session = test_session("ship the feature");
        session.current_objective = "ship the feature".to_string();

        let content = planning_objective_block(&session);

        assert_eq!(content, "[current objective]:\nship the feature");
        assert!(!content.contains("[current task]:"));
    }

    #[test]
    fn planning_objective_block_with_task_includes_overall_and_current_task() {
        let mut session = test_session("ship the feature");
        session.current_objective = "ship the feature".to_string();
        session.task_plan.detailed_tasks.push(TaskStep {
            task_id: "task-1".to_string(),
            step: 1,
            task_summary: "Patch parser".to_string(),
            step_deliverable_description: "Parser accepts fixture flags.".to_string(),
            status: PlanStatus::Doing,
            start_condition: StartCondition::UserAction,
            ..TaskStep::default()
        });

        let content = planning_objective_block(&session);

        assert_eq!(
            content,
            "[current objective]:\nship the feature\n\nPatch parser"
        );
        assert!(!content.contains("[current task]:"));
        assert!(!content.contains("Deliverable:"));
    }

    #[test]
    fn no_tool_retry_injects_objective_context_without_original_user_task() {
        let now = Utc::now();
        let mut session = SessionManagement::new(
            "sess-no-tool-retry".to_string(),
            "no tool retry".to_string(),
            PathBuf::from("C:/workspace"),
            false,
            "coding".to_string(),
            SessionInput {
                user_input: "ORIGINAL HUGE PROMPT".repeat(10),
                file_input: Vec::new(),
                agent: None,
                runtime_context: None,
                planning_mode_override: None,
            },
            "ORIGINAL HUGE PROMPT".to_string(),
            now,
        );
        session.current_objective = "STATE MACHINE OBJECTIVE".to_string();

        let mut messages = Vec::new();
        push_no_tool_task_status_retry_message(&mut messages, &session);
        let content = messages[0]["content"]
            .as_str()
            .expect("prompt message content should be a string");

        assert!(content.contains("Continue working toward the active thread goal."));
        assert!(content.contains("[current objective]:\nSTATE MACHINE OBJECTIVE"));
        assert!(content.contains("task_status status question"));
        assert!(content.contains("task_status status done"));
        assert!(content.contains("task_status status doing"));
        assert!(!content.contains("original_user_task:"));
    }

    #[test]
    fn no_tool_retry_injects_current_task_when_present() {
        let now = Utc::now();
        let mut session = SessionManagement::new(
            "sess-no-tool-task".to_string(),
            "no tool task".to_string(),
            PathBuf::from("C:/workspace"),
            false,
            "coding".to_string(),
            SessionInput {
                user_input: "fix the task".to_string(),
                file_input: vec![],
                agent: None,
                runtime_context: None,
                planning_mode_override: None,
            },
            "fix the task".to_string(),
            now,
        );
        session.current_objective = "fix the task".to_string();
        session.task_plan.detailed_tasks.push(TaskStep {
            task_id: "task-1".to_string(),
            step: 1,
            task_summary: "Patch parser".to_string(),
            status: PlanStatus::Doing,
            start_condition: StartCondition::UserAction,
            ..TaskStep::default()
        });

        let mut messages = Vec::new();
        push_no_tool_task_status_retry_message(&mut messages, &session);
        let content = messages[0]["content"]
            .as_str()
            .expect("prompt message content should be a string");

        assert!(content.contains("[current objective]:\nfix the task\n\nPatch parser"));
        assert!(!content.contains("original_user_task:"));
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
                planning_mode_override: None,
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
                planning_mode_override: None,
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
                planning_mode_override: None,
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
                planning_mode_override: None,
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
