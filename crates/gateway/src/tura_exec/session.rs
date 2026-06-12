use std::io::{self, Write};
use std::path::PathBuf;
use std::process::Stdio;
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use super::output::emit_jsonl;

/// Ensure the per-home `tura_session_db` owner is reachable, starting it
/// (detached, so it outlives this one-shot front) when none is running. The CLI
/// is a client of the single embedded SQLite owner.
pub(crate) fn ensure_session_db_owner() {
    if session_log::ipc::service_is_running() {
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
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            const DETACHED_PROCESS: u32 = 0x0000_0008;
            command.creation_flags(CREATE_NO_WINDOW | DETACHED_PROCESS);
        }
    }
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
        if session_log::ipc::service_is_running() {
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

/// Extract the latest assistant message text for a session from the single
/// session_db owner (the worker has already persisted it).
pub(crate) fn final_text_from_session_db(session_id: &str) -> String {
    use session_log::{ListSessionRecordsRequest, SessionLogCommand, SessionLogResponse};
    let response = session_log::ipc::call_service(&SessionLogCommand::ListSessionRecords(
        ListSessionRecordsRequest {
            session_id: session_id.to_string(),
            page: 0,
            page_size: 500,
        },
    ));
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
    let Some(session) = runtime::session_log_client::SessionLogClient::discover()
        .map_err(|err| format!("failed to inspect session state: {err}"))?
        .get_session(session_id.to_string())
        .map_err(|err| format!("failed to inspect session state: {err}"))?
    else {
        return Ok(());
    };
    if !session_is_busy(&session) {
        return Ok(());
    }
    if json_output {
        emit_jsonl(&json!({
            "type": "session.locked",
            "thread_id": session_id,
            "status": session.status,
            "state": session.state,
            "message": "session is already running; append the prompt through the gateway user-commands endpoint"
        }))?;
        io::stdout()
            .flush()
            .map_err(|err| format!("failed to flush stdout: {err}"))?;
    }
    Err(busy_session_message(session_id))
}

fn session_is_busy(session: &runtime::session_log_client::SessionSnapshot) -> bool {
    fn busy_text(value: Option<&String>) -> bool {
        value
            .map(|value| value.trim())
            .is_some_and(|value| matches!(value, "busy" | "running"))
    }
    busy_text(session.status.as_ref()) || busy_text(session.state.as_ref())
}

fn busy_session_message(session_id: &str) -> String {
    let gateway = gateway_base_url_for_hint();
    let escaped = session_id.replace('\'', "''");
    format!(
        "session `{session_id}` is already running.\n\
         `tura exec --session-id {session_id}` will not start a second runtime for the same session.\n\
         To add guidance to the running session, send it through the gateway user-command queue:\n\
         PowerShell:\n\
           Invoke-RestMethod -Method Post -Uri '{gateway}/session/{escaped}/user-commands' -ContentType 'application/json' -Body '{{\"command\":\"your additional instruction\"}}'\n\
         curl:\n\
           curl -X POST '{gateway}/session/{escaped}/user-commands' -H 'Content-Type: application/json' -d '{{\"command\":\"your additional instruction\"}}'"
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
    fn busy_session_detection_accepts_gateway_and_runtime_statuses() {
        let mut session = test_snapshot();
        session.status = Some("busy".to_string());
        assert!(session_is_busy(&session));

        session.status = Some("idle".to_string());
        session.state = Some("running".to_string());
        assert!(session_is_busy(&session));

        session.state = Some("completed".to_string());
        assert!(!session_is_busy(&session));
    }

    #[test]
    fn busy_session_message_points_to_gateway_user_command_queue() {
        let message = busy_session_message("session-123");

        assert!(message.contains("session `session-123` is already running"));
        assert!(message.contains("/session/session-123/user-commands"));
        assert!(message.contains("Invoke-RestMethod"));
        assert!(message.contains("curl -X POST"));
    }

    fn test_snapshot() -> runtime::session_log_client::SessionSnapshot {
        runtime::session_log_client::SessionSnapshot {
            session_id: "session-123".to_string(),
            workspace: "C:/workspace".to_string(),
            name: None,
            parent_id: None,
            created_at: 0,
            updated_at: 0,
            state: None,
            status: None,
            message_count: 0,
            task_management: serde_json::json!({}),
            management: serde_json::json!({}),
            session: serde_json::json!({}),
            todos: Vec::new(),
        }
    }
}
