use crate::context::{
    accumulate_message, build_messages_from_session, user_input_content_matches,
    user_input_content_value, ContextualUserFragment, WorkspaceSnapshot, USER_AGENT_CONTEXT_ROLE,
};
use crate::prompt_style::context_blocks;
use lifecycle::SessionManagement;

const FRONTEND_MESSAGE_ID_ENV: &str = "TURA_FRONTEND_MESSAGE_ID";
const FRONTEND_PART_ID_ENV: &str = "TURA_FRONTEND_PART_ID";

pub(crate) fn initial_messages_for_session(
    session: &mut SessionManagement,
) -> Result<Vec<serde_json::Value>, String> {
    if session.session_current_turn == 0 && !session_has_initial_user_message(session) {
        let snapshot_message = serde_json::json!({
            "role": "developer",
            "content": workspace_snapshot_message(&session.session_directory),
        });
        let environment_message = serde_json::json!({
            "role": "developer",
            "content": environment_context_message(&session.session_directory),
        });
        let runtime_context_message = runtime_context_message(session);
        let user_message = serde_json::json!({
            "role": "user",
            "content": user_input_content_value(&session.input.user_input),
        });

        let mut initial_messages = vec![snapshot_message, environment_message];
        if let Some(message) = runtime_context_message {
            initial_messages.push(message);
        }
        initial_messages.push(user_message);

        for message in &initial_messages {
            let role = message
                .get("role")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("system");
            let content = message
                .get("content")
                .cloned()
                .unwrap_or_else(|| serde_json::Value::String(String::new()));
            if role == "user" {
                accumulate_initial_user_message(session)?;
            } else {
                accumulate_message(session, role, content)?;
            }
        }

        return Ok(initial_messages);
    }

    if let Some(message) = runtime_context_message(session) {
        let content = message
            .get("content")
            .cloned()
            .unwrap_or_else(|| serde_json::Value::String(String::new()));
        if !session_has_runtime_context_message(session, &content) {
            accumulate_message(session, USER_AGENT_CONTEXT_ROLE, content)?;
        }
    }

    if !session_has_initial_user_message(session) {
        accumulate_initial_user_message(session)?;
    }

    Ok(build_messages_from_session(session))
}

fn environment_context_message(cwd: &std::path::Path) -> String {
    let timezone = std::env::var("TZ").unwrap_or_else(|_| "Europe/Paris".to_string());
    let system_language = session_language();
    context_blocks::environment_context(
        cwd,
        context_shell_name(),
        chrono::Local::now().format("%Y-%m-%d"),
        &timezone,
        &system_language,
    )
}

fn session_language() -> String {
    std::env::var("TURA_SESSION_LANGUAGE")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "en".to_string())
}

fn context_shell_name() -> &'static str {
    match std::env::var("TURA_COMMAND_RUN_SHELL")
        .ok()
        .map(|value| value.trim().to_ascii_lowercase())
        .as_deref()
    {
        Some("bash") => "bash",
        Some("zsh") => "zsh",
        Some("shell") | Some("shell_command") | Some("shll") | Some("shall") => {
            if cfg!(windows) {
                "powershell"
            } else if cfg!(target_os = "macos") {
                "zsh"
            } else {
                "bash"
            }
        }
        _ if cfg!(windows) => "powershell",
        _ if cfg!(target_os = "macos") => "zsh",
        _ => "bash",
    }
}

fn workspace_snapshot_message(cwd: &std::path::Path) -> String {
    WorkspaceSnapshot::from_cwd(cwd)
        .map(|snapshot| snapshot.render())
        .unwrap_or_else(|| "<WORKSPACE_SNAPSHOT>\n\n</WORKSPACE_SNAPSHOT>".to_string())
}

fn runtime_context_message(session: &SessionManagement) -> Option<serde_json::Value> {
    let content = session
        .input
        .runtime_context
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())?;
    Some(serde_json::json!({
        "role": USER_AGENT_CONTEXT_ROLE,
        "content": content,
    }))
}

fn accumulate_initial_user_message(session: &mut SessionManagement) -> Result<(), String> {
    let now = chrono::Utc::now();
    let message = initial_user_log_message(session, now);
    session.push_log(
        serde_json::to_string(&message).unwrap_or_else(|_| "message: user".to_string()),
        now,
    );
    session.record_user_message_at(now);
    Ok(())
}

fn initial_user_log_message(
    session: &SessionManagement,
    now: chrono::DateTime<chrono::Utc>,
) -> serde_json::Value {
    let mut message = serde_json::json!({
        "role": "user",
        "content": user_input_content_value(&session.input.user_input),
        "created_at": now.timestamp_millis(),
        "updated_at": now.timestamp_millis(),
        "timestamp": now.to_rfc3339(),
    });
    if let Some(object) = message.as_object_mut() {
        if let Some(message_id) = frontend_env(FRONTEND_MESSAGE_ID_ENV) {
            object.insert("id".to_string(), serde_json::Value::String(message_id));
        }
        if let Some(part_id) = frontend_env(FRONTEND_PART_ID_ENV) {
            object.insert("part_id".to_string(), serde_json::Value::String(part_id));
        }
    }
    message
}

fn frontend_env(key: &str) -> Option<String> {
    std::env::var(key)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn session_has_initial_user_message(session: &SessionManagement) -> bool {
    let raw_input = &session.input.user_input;
    session.session_log.iter().any(|entry| {
        serde_json::from_str::<serde_json::Value>(entry)
            .ok()
            .is_some_and(|value| {
                value.get("role").and_then(serde_json::Value::as_str) == Some("user")
                    && value
                        .get("content")
                        .is_some_and(|content| user_input_content_matches(content, raw_input))
            })
    })
}

fn session_has_runtime_context_message(
    session: &SessionManagement,
    content: &serde_json::Value,
) -> bool {
    session.session_log.iter().any(|entry| {
        serde_json::from_str::<serde_json::Value>(entry)
            .ok()
            .is_some_and(|value| {
                value.get("role").and_then(serde_json::Value::as_str)
                    == Some(USER_AGENT_CONTEXT_ROLE)
                    && value.get("content") == Some(content)
            })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use lifecycle::SessionInput;
    use std::path::PathBuf;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    struct EnvRestore {
        keys: Vec<(&'static str, Option<std::ffi::OsString>)>,
    }

    impl EnvRestore {
        fn capture(keys: &[&'static str]) -> Self {
            Self {
                keys: keys
                    .iter()
                    .map(|key| (*key, std::env::var_os(key)))
                    .collect(),
            }
        }
    }

    impl Drop for EnvRestore {
        fn drop(&mut self) {
            for (key, value) in &self.keys {
                match value {
                    Some(value) => std::env::set_var(key, value),
                    None => std::env::remove_var(key),
                }
            }
        }
    }

    #[test]
    fn initial_user_log_preserves_frontend_ids_from_worker_env() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|error| error.into_inner());
        let _restore = EnvRestore::capture(&[FRONTEND_MESSAGE_ID_ENV, FRONTEND_PART_ID_ENV]);
        std::env::set_var(FRONTEND_MESSAGE_ID_ENV, "msg_tui_reopen");
        std::env::set_var(FRONTEND_PART_ID_ENV, "part_tui_reopen");

        let now = chrono::Utc::now();
        let mut session = SessionManagement::new(
            "session-reopen".to_string(),
            "reopen".to_string(),
            PathBuf::from("C:/workspace/reopen"),
            false,
            "coding".to_string(),
            SessionInput {
                user_input: "用户重开后仍然可见".to_string(),
                file_input: Vec::new(),
                agent: Some("coding".to_string()),
                runtime_context: None,
                planning_mode_override: None,
            },
            "用户重开后仍然可见".to_string(),
            now,
        );

        let provider_messages =
            initial_messages_for_session(&mut session).expect("initial messages should build");
        assert!(provider_messages.iter().any(|message| {
            message.get("role").and_then(serde_json::Value::as_str) == Some("user")
                && message.get("id").is_none()
                && message.get("part_id").is_none()
        }));

        let user_log = session
            .session_log
            .iter()
            .filter_map(|entry| serde_json::from_str::<serde_json::Value>(entry).ok())
            .find(|value| value.get("role").and_then(serde_json::Value::as_str) == Some("user"))
            .expect("user log record should exist");
        assert_eq!(
            user_log.get("id").and_then(serde_json::Value::as_str),
            Some("msg_tui_reopen")
        );
        assert_eq!(
            user_log.get("part_id").and_then(serde_json::Value::as_str),
            Some("part_tui_reopen")
        );
        let created_at = user_log
            .get("created_at")
            .and_then(serde_json::Value::as_i64)
            .expect("user log should persist created_at");
        let updated_at = user_log
            .get("updated_at")
            .and_then(serde_json::Value::as_i64)
            .expect("user log should persist updated_at");
        let timestamp = user_log
            .get("timestamp")
            .and_then(serde_json::Value::as_str)
            .expect("user log should persist timestamp");
        assert!(created_at >= now.timestamp_millis());
        assert!(updated_at >= created_at);
        assert!(!timestamp.trim().is_empty());
        assert!(user_log.get("content").is_some_and(|content| {
            user_input_content_matches(content, "用户重开后仍然可见")
        }));
    }

    #[test]
    fn initial_messages_inject_workspace_and_environment_as_developer() {
        let now = chrono::Utc::now();
        let mut session = SessionManagement::new(
            "session-developer-context".to_string(),
            "probe".to_string(),
            PathBuf::from("C:/workspace/no-permissions"),
            false,
            "coding".to_string(),
            SessionInput {
                user_input: "probe".to_string(),
                file_input: Vec::new(),
                agent: Some("coding".to_string()),
                runtime_context: None,
                planning_mode_override: None,
            },
            "probe".to_string(),
            now,
        );

        let provider_messages =
            initial_messages_for_session(&mut session).expect("initial messages should build");
        let developer_contexts = provider_messages
            .iter()
            .filter(|message| {
                message.get("role").and_then(serde_json::Value::as_str) == Some("developer")
            })
            .collect::<Vec<_>>();
        assert_eq!(developer_contexts.len(), 2, "{provider_messages:?}");
        assert!(developer_contexts.iter().any(|message| {
            message["content"]
                .as_str()
                .is_some_and(|content| content.contains("<WORKSPACE_SNAPSHOT>"))
        }));
        assert!(developer_contexts.iter().any(|message| {
            message["content"]
                .as_str()
                .is_some_and(|content| content.contains("<environment_context>"))
        }));

        let stored_developer_contexts = session
            .session_log
            .iter()
            .filter_map(|entry| serde_json::from_str::<serde_json::Value>(entry).ok())
            .filter(|value| {
                value.get("role").and_then(serde_json::Value::as_str) == Some("developer")
            })
            .collect::<Vec<_>>();
        assert_eq!(
            stored_developer_contexts.len(),
            2,
            "{:?}",
            session.session_log
        );
    }
}
