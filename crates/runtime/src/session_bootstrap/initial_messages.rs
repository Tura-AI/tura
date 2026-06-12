use crate::context::{
    accumulate_message, build_messages_from_session, user_input_content_matches,
    user_input_content_value, ContextualUserFragment, WorkspaceSnapshot,
};
use crate::state_machine::session_management::SessionManagement;

pub(crate) fn initial_messages_for_session(
    session: &mut SessionManagement,
) -> Result<Vec<serde_json::Value>, String> {
    let permissions_message = serde_json::json!({
        "role": "developer",
        "content": permissions_instructions(),
    });
    if session.session_current_turn == 0 && !session_has_initial_user_message(session) {
        let snapshot_message = serde_json::json!({
            "role": "user",
            "content": workspace_snapshot_message(&session.session_directory),
        });
        let environment_message = serde_json::json!({
            "role": "user",
            "content": environment_context_message(&session.session_directory),
        });
        let user_message = serde_json::json!({
            "role": "user",
            "content": user_input_content_value(&session.input.user_input),
        });

        for message in [
            &permissions_message,
            &snapshot_message,
            &environment_message,
            &user_message,
        ] {
            let role = message
                .get("role")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("system");
            let content = message
                .get("content")
                .cloned()
                .unwrap_or_else(|| serde_json::Value::String(String::new()));
            accumulate_message(session, role, content)?;
        }

        return Ok(vec![
            permissions_message,
            snapshot_message,
            environment_message,
            user_message,
        ]);
    }

    if !session_has_initial_user_message(session) {
        accumulate_message(
            session,
            "user",
            user_input_content_value(&session.input.user_input),
        )?;
    }

    let mut messages = vec![permissions_message];
    messages.extend(build_messages_from_session(session));
    Ok(messages)
}

fn permissions_instructions() -> &'static str {
    "<permissions instructions>\nFilesystem sandboxing defines which files can be read or written. `sandbox_mode` is `danger-full-access`: No filesystem sandboxing - all commands are permitted. Network access is enabled.\nApproval policy is currently never. Do not provide the `sandbox_permissions` for any reason, commands will be rejected.\n</permissions instructions>"
}

fn environment_context_message(cwd: &std::path::Path) -> String {
    format!(
        "<environment_context>\n  <cwd>{}</cwd>\n  <shell>{}</shell>\n  <current_date>{}</current_date>\n  <timezone>{}</timezone>\n  <system_language>{}</system_language>\n</environment_context>",
        cwd.display(),
        context_shell_name(),
        chrono::Local::now().format("%Y-%m-%d"),
        std::env::var("TZ").unwrap_or_else(|_| "Europe/Paris".to_string()),
        session_language()
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
