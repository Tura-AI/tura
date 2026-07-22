use std::io::{self, Write};
use std::path::PathBuf;
use std::process::Stdio;
use std::time::{Duration, Instant};

use lifecycle::{SessionCommand, SessionState, TaskPlan};
use serde_json::{json, Value};
use session_log_contract::{
    CreateSessionRequest, GetSessionRequest, SessionLogCommand, SessionLogResponse, SessionSnapshot,
};

use super::cli::CliConfig;
use super::output::emit_jsonl;

/// Ensure the per-home `tura_session_db` owner is reachable, starting it
/// (detached, so it outlives this one-shot front) when none is running. The CLI
/// is a client of the single embedded SQLite owner.
pub(crate) fn ensure_session_db_owner() {
    if session_log_contract::client::service_is_running() {
        return;
    }

    let Some(bin) = resolve_session_db_binary() else {
        // No service binary available (for example in a minimal dev tree):
        // allow the caller to continue instead of failing the turn before the
        // runtime has a chance to report a structured error.
        if std::env::var_os("TURA_DEBUG_RUNTIME").is_some() {
            eprintln!("[tura] ensure_session_db_owner: no tura_session_db binary found");
        }
        return;
    };
    let debug = std::env::var_os("TURA_DEBUG_RUNTIME").is_some();
    if debug {
        eprintln!("[tura] ensure_session_db_owner: starting {}", bin.display());
    }
    let mut command = std::process::Command::new(&bin);
    command.stdin(Stdio::null());
    if debug {
        let err_path = std::env::temp_dir().join("tura-session-db-spawn.err");
        if let Ok(file) = std::fs::File::create(&err_path) {
            command.stderr(file);
            eprintln!(
                "[tura] ensure_session_db_owner: child stderr -> {}",
                err_path.display()
            );
        }
        command.stdout(Stdio::null());
    } else {
        command.stdout(Stdio::null()).stderr(Stdio::null());
    }
    tura_path::process_hardening::hide_child_console_window_and_detach(&mut command);
    // Spawn detached: do not retain the child handle, so the owner keeps running
    // after this CLI exits and can be reused by the next front.
    match command.spawn() {
        Ok(_) => {}
        Err(error) => {
            if debug {
                eprintln!("[tura] ensure_session_db_owner: spawn failed: {error}");
            }
            return;
        }
    }

    let started = Instant::now();
    while started.elapsed() < Duration::from_secs(60) {
        if session_log_contract::client::service_is_running() {
            if debug {
                eprintln!(
                    "[tura] ensure_session_db_owner: reachable after {:?}",
                    started.elapsed()
                );
            }
            return;
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    if debug {
        eprintln!("[tura] ensure_session_db_owner: timed out after 60s waiting for service");
    }
}

pub(crate) fn ensure_cli_session(config: &CliConfig, session_id: &str) -> Result<(), String> {
    let response = session_log_contract::client::call_service(&SessionLogCommand::GetSession(
        GetSessionRequest {
            session_id: session_id.to_string(),
        },
    ))
    .map_err(|error| format!("failed to query CLI session `{session_id}`: {error}"))?;
    match response {
        SessionLogResponse::Session { session: Some(_) } => return Ok(()),
        SessionLogResponse::Session { session: None } => {}
        SessionLogResponse::Error { error } => {
            return Err(format!(
                "failed to query CLI session `{session_id}`: {error}"
            ));
        }
        other => {
            return Err(format!(
                "unexpected session_db response while querying CLI session `{session_id}`: {other:?}"
            ));
        }
    }

    let workspace = config.cwd.to_string_lossy().to_string();
    let created_at = chrono::Utc::now().timestamp_millis();
    let response = session_log_contract::client::call_service(&SessionLogCommand::CreateSession(
        Box::new(CreateSessionRequest {
            command_id: format!("create:{session_id}"),
            session_id: session_id.to_string(),
            creation_command: SessionCommand::CreateSession {
                task_plan: TaskPlan::default(),
            },
            copy_context: false,
            workspace: workspace.clone(),
            session_directory: workspace,
            name: "CLI session".to_string(),
            created_at,
            model: config.model.clone(),
            agent: config.agent.clone(),
            session_type: "coding".to_string(),
            kill_processes_on_start: false,
            validator_enabled: false,
            force_planning: config.planning_mode == Some(true),
            model_variant: None,
            model_acceleration_enabled: false,
            disable_permission_restrictions: false,
            use_last_tool_call_response: false,
            auto_session_name: true,
            initial_task_plan_patch: None,
        }),
    ))
    .map_err(|error| format!("failed to create CLI session `{session_id}`: {error}"))?;
    match response {
        SessionLogResponse::SessionCommandApplied { .. } => Ok(()),
        SessionLogResponse::Error { error } => Err(format!(
            "failed to create CLI session `{session_id}`: {error}"
        )),
        other => Err(format!(
            "unexpected session_db response while creating CLI session `{session_id}`: {other:?}"
        )),
    }
}

/// Extract the latest assistant message text for a session from the single
/// session_db owner (the worker has already persisted it).
pub(crate) fn final_text_from_session_db(session_id: &str) -> String {
    use session_log_contract::{ListSessionRecordsRequest, SessionLogCommand, SessionLogResponse};
    let response = session_log_contract::client::call_service(
        &SessionLogCommand::ListSessionRecords(ListSessionRecordsRequest {
            session_id: session_id.to_string(),
            page: 0,
            page_size: 500,
        }),
    );
    let records = match response {
        Ok(SessionLogResponse::Records { records, .. }) => records,
        _ => return String::new(),
    };
    records
        .iter()
        .filter(|record| record.role == "assistant")
        .max_by_key(|record| record.created_at)
        .map(|record| extract_record_text(&record.record))
        .unwrap_or_default()
}

/// Pull plain text out of a persisted message record (`parts[].text` of type
/// `text`, else a `content`/`text` string field).
fn extract_record_text(record: &Value) -> String {
    if let Some(parts) = record.get("parts").and_then(Value::as_array) {
        let text = parts
            .iter()
            .filter(|part| part.get("type").and_then(Value::as_str) == Some("text"))
            .filter_map(|part| part.get("text").and_then(Value::as_str))
            .collect::<Vec<_>>()
            .join("");
        if !text.trim().is_empty() {
            return text;
        }
    }
    record
        .get("content")
        .or_else(|| record.get("text"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string()
}

fn resolve_session_db_binary() -> Option<PathBuf> {
    let executable = if cfg!(windows) {
        "tura_session_db.exe"
    } else {
        "tura_session_db"
    };
    let mut candidates = Vec::new();
    if let Ok(current_exe) = std::env::current_exe() {
        candidates.push(current_exe.with_file_name(executable));
    }
    let root = tura_path::canonical_root();
    candidates.push(root.join("target").join("release").join(executable));
    candidates.push(root.join("target").join("debug").join(executable));
    candidates.into_iter().find(|path| path.exists())
}

pub(crate) fn reject_busy_session(session_id: &str, json_output: bool) -> Result<(), String> {
    let response = session_log_contract::client::call_service(&SessionLogCommand::GetSession(
        GetSessionRequest {
            session_id: session_id.to_string(),
        },
    ))
    .map_err(|error| format!("failed to inspect session state: {error}"))?;
    let Some(session) = (match response {
        SessionLogResponse::Session { session } => session.map(|session| *session),
        SessionLogResponse::Error { error } => {
            return Err(format!("failed to inspect session state: {error}"));
        }
        other => {
            return Err(format!(
                "failed to inspect session state: unexpected session service response {other:?}"
            ));
        }
    }) else {
        return Ok(());
    };
    let state = session_state(&session);
    if !state.is_recoverable_running() {
        return Ok(());
    }
    if json_output {
        emit_jsonl(&json!({
            "type": "session.locked",
            "thread_id": session_id,
            "status": state.ui_status(),
            "state": state,
            "message": "session is already running; append the prompt through the gateway prompt_async endpoint"
        }))?;
        io::stdout()
            .flush()
            .map_err(|err| format!("failed to flush stdout: {err}"))?;
    }
    Err(busy_session_message(session_id))
}

fn session_state(session: &SessionSnapshot) -> SessionState {
    session.lifecycle_projection.state
}

fn busy_session_message(session_id: &str) -> String {
    let gateway = gateway_base_url_for_hint();
    let escaped = session_id.replace('\'', "''");
    format!(
        "session `{session_id}` is already running.\n\
         `tura exec --session-id {session_id}` will not start a second runtime for the same session.\n\
         To add guidance to the running session, send it through the gateway prompt_async endpoint:\n\
         PowerShell:\n\
           Invoke-RestMethod -Method Post -Uri '{gateway}/session/{escaped}/prompt_async' -ContentType 'application/json' -Body '{{\"parts\":[{{\"type\":\"text\",\"text\":\"your additional instruction\"}}]}}'\n\
         curl:\n\
           curl -X POST '{gateway}/session/{escaped}/prompt_async' -H 'Content-Type: application/json' -d '{{\"parts\":[{{\"type\":\"text\",\"text\":\"your additional instruction\"}}]}}'"
    )
}

fn gateway_base_url_for_hint() -> String {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn busy_session_detection_uses_only_canonical_lifecycle_state() {
        let mut session = test_snapshot();
        session.lifecycle_projection = test_projection(SessionState::Running);
        assert_eq!(session_state(&session), SessionState::Running);
        assert!(session_state(&session).is_recoverable_running());

        session.lifecycle_projection = test_projection(SessionState::Paused);
        assert!(session_state(&session).is_recoverable_running());

        session.lifecycle_projection = test_projection(SessionState::Completed);
        assert!(!session_state(&session).is_recoverable_running());
    }

    #[test]
    fn busy_session_message_points_to_gateway_user_command_queue() {
        let message = busy_session_message("session-123");

        assert!(message.contains("session `session-123` is already running"));
        assert!(message.contains("/session/session-123/prompt_async"));
        assert!(message.contains("Invoke-RestMethod"));
        assert!(message.contains("curl -X POST"));
    }

    fn test_snapshot() -> SessionSnapshot {
        let timestamp = chrono::DateTime::<chrono::Utc>::UNIX_EPOCH;
        let projection = test_projection(SessionState::Created);
        let mut management = lifecycle::SessionManagement::new(
            "session-123".to_string(),
            "Session".to_string(),
            "C:/workspace".into(),
            false,
            Vec::<String>::new(),
            lifecycle::SessionInput {
                user_input: String::new(),
                file_input: Vec::new(),
                agent: None,
                runtime_context: None,
                planning_mode_override: None,
            },
            String::new(),
            timestamp,
        );
        management.replace_lifecycle_projection(projection.clone());
        SessionSnapshot {
            session_id: "session-123".to_string(),
            workspace: "C:/workspace".to_string(),
            name: Some(management.session_name.clone()),
            created_at: 0,
            updated_at: 0,
            last_user_message_at: None,
            message_count: 0,
            lifecycle_projection: projection,
            metadata: session_log_contract::SessionMetadata {
                session_directory: "C:/workspace".to_string(),
                model: None,
                agent: None,
                session_type: "coding".to_string(),
                kill_processes_on_start: false,
                validator_enabled: false,
                force_planning: false,
                model_variant: None,
                model_acceleration_enabled: false,
                disable_permission_restrictions: management.disable_permission_restrictions,
                use_last_tool_call_response: management.use_last_tool_call_response,
                auto_session_name: management.auto_session_name,
                context_tokens: management.context_tokens,
                runtime_usage: management.runtime_usage.clone(),
            },
            management,
            todos: Vec::new(),
        }
    }

    fn test_projection(state: SessionState) -> lifecycle::SessionProjection {
        let mut aggregate = lifecycle::SessionAggregate::new("session-123".to_string());
        aggregate.state = state;
        aggregate.query(lifecycle::SessionQuery::Lifecycle)
    }
}
