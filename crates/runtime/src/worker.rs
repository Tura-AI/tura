//! Runtime worker entry: the body of the standalone `tura_runtime` binary.
//!
//! Refactor stage 3: the worker is its own binary (`crates/runtime` →
//! `tura_runtime`), no longer the gateway binary re-invoked by role. The router
//! spawns it and drives it over the line protocol: read `{ "kind", "payload" }`
//! per line, write one JSON reply per line. Router-managed runtime workers run
//! in one-shot mode so they exit after a single `call` envelope.
//!
//! Boundary: runtime stays a library; this entry only hosts the worker process,
//! activates the agent spec the router hands down, and runs one prompt. Session
//! state still flows through the session_db socket (never `open_default`).

use std::io::{BufRead, Write};
use std::path::PathBuf;

use runtime_contract::{WorkerEnvelope, WORKER_KIND_CALL, WORKER_KIND_HEALTH_CHECK};
use serde_json::{json, Value};
use sysinfo::{Pid, ProcessRefreshKind, RefreshKind, System};

use crate::mano::ManoProcessResult;
use crate::state_machine::session_management::SessionInput;
use lifecycle::SessionState;

/// Run the runtime worker loop: blocking read on stdin, write on stdout, until
/// the peer closes or one-shot mode completes a call.
pub fn run() -> std::io::Result<()> {
    start_router_parent_watchdog();
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    let mut line = String::new();
    let mut reader = stdin.lock();

    loop {
        line.clear();
        let read = reader.read_line(&mut line)?;
        if read == 0 {
            return Ok(());
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let parsed = serde_json::from_str::<WorkerEnvelope>(trimmed);
        let is_call = parsed
            .as_ref()
            .is_ok_and(|envelope| envelope.kind == WORKER_KIND_CALL);
        let response = match parsed {
            Ok(envelope) => handle_envelope(&envelope),
            Err(error) => json!({ "ok": false, "error": format!("invalid envelope: {error}") }),
        };

        let encoded = serde_json::to_string(&response)
            .unwrap_or_else(|error| format!("{{\"ok\":false,\"error\":\"{error}\"}}"));
        stdout.write_all(encoded.as_bytes())?;
        stdout.write_all(b"\n")?;
        stdout.flush()?;
        if runtime_oneshot_enabled() && is_call {
            return Ok(());
        }
    }
}

fn runtime_oneshot_enabled() -> bool {
    std::env::var("TURA_RUNTIME_ONESHOT")
        .ok()
        .is_some_and(|value| env_flag(&value))
}

fn start_router_parent_watchdog() {
    let Some(parent) = RouterParentProcess::from_env() else {
        return;
    };
    std::thread::spawn(move || loop {
        std::thread::sleep(std::time::Duration::from_millis(500));
        if !parent.is_alive() {
            std::process::exit(70);
        }
    });
}

#[derive(Debug, Clone, Copy)]
struct RouterParentProcess {
    pid: u32,
    start_time: Option<u64>,
}

impl RouterParentProcess {
    fn from_env() -> Option<Self> {
        let pid = std::env::var("TURA_ROUTER_PARENT_PID")
            .ok()
            .and_then(|value| value.trim().parse::<u32>().ok())
            .filter(|pid| *pid > 0)?;
        let start_time = std::env::var("TURA_ROUTER_PARENT_START_TIME")
            .ok()
            .and_then(|value| value.trim().parse::<u64>().ok());
        Some(Self { pid, start_time })
    }

    fn is_alive(&self) -> bool {
        let mut system = System::new_with_specifics(
            RefreshKind::new().with_processes(ProcessRefreshKind::new()),
        );
        system.refresh_processes();
        let Some(process) = system.process(Pid::from_u32(self.pid)) else {
            return false;
        };
        self.start_time
            .map(|expected| process.start_time() == expected)
            .unwrap_or(true)
    }
}

fn env_flag(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    )
}

fn no_op_manual_enabled_from_env() -> bool {
    std::env::var("TURA_NO_OP_MANUAL")
        .ok()
        .is_some_and(|value| env_flag(&value))
}

fn handle_envelope(envelope: &WorkerEnvelope) -> Value {
    match envelope.kind.as_str() {
        WORKER_KIND_HEALTH_CHECK => json!({
            "ok": true,
            "role": "runtime_worker",
            "version": tura_path::instance_version(),
        }),
        WORKER_KIND_CALL => handle_call(&envelope.payload),
        other => json!({ "ok": false, "error": format!("unsupported kind: {other}") }),
    }
}

/// Call payload shape: `{ "input": { "method", "input": <RuntimeWorkerCall> } }`.
/// This matches the router `invoke_persistent` envelope.
fn handle_call(payload: &Value) -> Value {
    let call = payload
        .get("input")
        .and_then(|value| value.get("input"))
        .cloned()
        .unwrap_or_else(|| payload.clone());

    let session_id = call
        .get("session_id")
        .and_then(Value::as_str)
        .map(str::to_string)
        .unwrap_or_else(|| format!("worker-{}", uuid::Uuid::new_v4()));
    let directory = call
        .get("directory")
        .and_then(Value::as_str)
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    let prompt = call
        .get("prompt")
        .or_else(|| call.get("message"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .unwrap_or_default();
    let agent = call
        .get("agent")
        .and_then(Value::as_str)
        .map(str::to_string);
    let runtime_context = call
        .get("runtime_context")
        .and_then(Value::as_str)
        .map(str::to_string);
    let no_op_manual = call
        .get("no_op_manual")
        .and_then(Value::as_bool)
        .unwrap_or_else(no_op_manual_enabled_from_env);
    let planning_mode_override = call.get("planning_mode_override").and_then(Value::as_bool);
    let return_log = call
        .get("return_log")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let agent_spec = call.get("agent_spec").cloned();

    if prompt.trim().is_empty() {
        return json!({ "ok": false, "session_id": session_id, "error": "empty prompt" });
    }

    let input = SessionInput {
        user_input: prompt,
        file_input: Vec::new(),
        agent,
        runtime_context,
        planning_mode_override,
    };

    if no_op_manual {
        std::env::set_var("TURA_NO_OP_MANUAL", "1");
    } else {
        std::env::remove_var("TURA_NO_OP_MANUAL");
    }

    if let Some(agent_spec) = agent_spec {
        std::env::set_var("TURA_ROUTER_AGENT_SPEC", agent_spec.to_string());
    } else {
        std::env::remove_var("TURA_ROUTER_AGENT_SPEC");
    }
    match crate::mano::process_from_gateway_session_in_directory(
        session_id.clone(),
        input,
        directory,
    ) {
        Ok(result) => response_from_mano_result(&session_id, result, return_log),
        Err(error) => json!({ "ok": false, "session_id": session_id, "error": error }),
    }
}

fn response_from_mano_result(
    session_id: &str,
    result: ManoProcessResult,
    return_log: bool,
) -> Value {
    let final_text = final_assistant_text(&result.session.session_log).unwrap_or_default();
    let message_count = result.session.session_log.len();
    let turn_started_at_ms = result.session.session_started_at.timestamp_millis();
    if result.session.state == SessionState::Failed {
        let error = result
            .final_error
            .filter(|error| !error.trim().is_empty())
            .or_else(|| {
                final_text
                    .trim()
                    .is_empty()
                    .then_some("runtime session failed without a final provider error".to_string())
            })
            .unwrap_or_else(|| final_text.clone());
        let mut response = json!({
            "ok": false,
            "session_id": session_id,
            "session_state": result.session.state,
            "message_count": message_count,
            "turn_started_at_ms": turn_started_at_ms,
            "final_text": final_text,
            "error": error,
        });
        if return_log {
            response["session_log"] = json!(result.session.session_log);
        }
        return response;
    }
    if final_text.trim().is_empty() {
        let mut response = json!({
            "ok": false,
            "session_id": session_id,
            "session_state": result.session.state,
            "message_count": message_count,
            "turn_started_at_ms": turn_started_at_ms,
            "error": "runtime completed without a final assistant message",
        });
        if return_log {
            response["session_log"] = json!(result.session.session_log);
        }
        return response;
    }
    let mut response = json!({
        "ok": true,
        "session_id": session_id,
        "session_state": result.session.state,
        "message_count": message_count,
        "turn_started_at_ms": turn_started_at_ms,
        "final_text": final_text,
    });
    if return_log {
        response["session_log"] = json!(result.session.session_log);
    }
    response
}

fn final_assistant_text(session_log: &[String]) -> Option<String> {
    session_log
        .iter()
        .rev()
        .filter_map(|entry| serde_json::from_str::<Value>(entry).ok())
        .find_map(|value| {
            if value.get("role").and_then(Value::as_str) != Some("assistant") {
                return None;
            }
            value
                .get("content")
                .and_then(Value::as_str)
                .map(clean_agent_message)
                .filter(|text| !text.trim().is_empty())
        })
}

fn clean_agent_message(text: &str) -> String {
    let trimmed = text.trim();
    if trimmed.is_empty() || looks_like_tool_payload(trimmed) {
        return String::new();
    }
    if let Some(index) = trimmed.find("{\"commands\"") {
        let (prefix, suffix) = trimmed.split_at(index);
        if looks_like_tool_payload(suffix) {
            return prefix.trim().to_string();
        }
    }
    trimmed.to_string()
}

fn looks_like_tool_payload(text: &str) -> bool {
    let trimmed = text.trim_start();
    trimmed.starts_with('{')
        && (trimmed.contains("\"commands\"")
            || trimmed.contains("\"task_group\"")
            || trimmed.contains("\"step_summary\"")
            || trimmed.contains("\"tool_calls\"")
            || trimmed.contains("\"reply_message\""))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state_machine::session_management::SessionManagement;
    use chrono::Utc;
    use std::path::PathBuf;

    #[test]
    fn health_check_reports_role_and_version() {
        let reply = handle_envelope(&WorkerEnvelope::health_check());
        assert_eq!(reply["ok"], true);
        assert_eq!(reply["role"], "runtime_worker");
        assert_eq!(reply["version"], tura_path::instance_version());
    }

    #[test]
    fn empty_prompt_is_rejected_without_running_a_session() {
        let reply = handle_call(&json!({ "input": { "input": { "session_id": "s1" } } }));
        assert_eq!(reply["ok"], false);
        assert_eq!(reply["session_id"], "s1");
        assert_eq!(reply["error"], "empty prompt");
    }

    #[test]
    fn unknown_kind_is_reported() {
        let reply = handle_envelope(&WorkerEnvelope {
            kind: "bogus".to_string(),
            payload: Value::Null,
        });
        assert_eq!(reply["ok"], false);
        assert!(reply["error"]
            .as_str()
            .expect("error should be a string")
            .contains("bogus"));
    }

    #[test]
    fn router_parent_process_from_env_uses_pid_and_start_time() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|error| error.into_inner());
        let previous_pid = std::env::var_os("TURA_ROUTER_PARENT_PID");
        let previous_start = std::env::var_os("TURA_ROUTER_PARENT_START_TIME");
        std::env::set_var("TURA_ROUTER_PARENT_PID", "1234");
        std::env::set_var("TURA_ROUTER_PARENT_START_TIME", "5678");

        let parent = RouterParentProcess::from_env().expect("parent env");
        assert_eq!(parent.pid, 1234);
        assert_eq!(parent.start_time, Some(5678));

        restore_env("TURA_ROUTER_PARENT_PID", previous_pid);
        restore_env("TURA_ROUTER_PARENT_START_TIME", previous_start);
    }

    #[test]
    fn current_router_parent_process_is_alive_with_current_start_time() {
        let pid = std::process::id();
        let mut system = System::new_all();
        system.refresh_processes();
        let start_time = system
            .process(Pid::from_u32(pid))
            .expect("current process should be visible")
            .start_time();

        let parent = RouterParentProcess {
            pid,
            start_time: Some(start_time),
        };
        assert!(parent.is_alive());
        let stale_parent = RouterParentProcess {
            pid,
            start_time: Some(start_time.saturating_sub(1)),
        };
        assert!(!stale_parent.is_alive());
    }

    #[test]
    fn final_assistant_text_ignores_tool_payloads() {
        let log = vec![
            json!({"role":"assistant","content":"{\"commands\":[]}"}).to_string(),
            json!({"role":"assistant","content":" done "}).to_string(),
        ];

        assert_eq!(final_assistant_text(&log), Some("done".to_string()));
    }

    #[test]
    fn failed_mano_result_preserves_provider_error_for_router_response() {
        let mut session = test_session("failed-provider-session");
        session
            .transition(SessionState::Running, Utc::now())
            .expect("running transition");
        session
            .transition(SessionState::Failed, Utc::now())
            .expect("failed transition");
        session
            .session_log
            .push(json!({"role":"assistant","content":"stale fallback"}).to_string());

        let reply = response_from_mano_result(
            "failed-provider-session",
            ManoProcessResult {
                session,
                agents: Vec::new(),
                final_error: Some(
                    "Provider runtime failed after 3 retries before completing the task: rate_limit_exceeded"
                        .to_string(),
                ),
            },
            false,
        );

        assert_eq!(reply["ok"], false);
        assert_eq!(reply["session_id"], "failed-provider-session");
        assert_eq!(reply["session_state"], "failed");
        assert!(reply["error"]
            .as_str()
            .is_some_and(|error| error.contains("rate_limit_exceeded")));
        assert_eq!(reply["final_text"], "stale fallback");
    }

    #[test]
    fn response_from_mano_result_includes_session_log_only_when_requested() {
        let mut session = test_session("log-response-session");
        session
            .transition(SessionState::Running, Utc::now())
            .expect("running transition");
        session
            .session_log
            .push(json!({"role":"assistant","content":"visible"}).to_string());
        session
            .transition(SessionState::Completed, Utc::now())
            .expect("completed transition");

        let without_log = response_from_mano_result(
            "log-response-session",
            ManoProcessResult {
                session: session.clone(),
                agents: Vec::new(),
                final_error: None,
            },
            false,
        );
        let with_log = response_from_mano_result(
            "log-response-session",
            ManoProcessResult {
                session,
                agents: Vec::new(),
                final_error: None,
            },
            true,
        );

        assert!(without_log.get("session_log").is_none());
        assert_eq!(with_log["session_log"].as_array().expect("log").len(), 1);
        assert!(with_log["turn_started_at_ms"].as_i64().is_some());
    }

    fn test_session(session_id: &str) -> SessionManagement {
        SessionManagement::new(
            session_id.to_string(),
            "worker response test".to_string(),
            PathBuf::from("C:/workspace/worker-response-test"),
            false,
            "coding".to_string(),
            SessionInput {
                user_input: "test prompt".to_string(),
                file_input: Vec::new(),
                agent: None,
                runtime_context: None,
                planning_mode_override: None,
            },
            "test prompt".to_string(),
            Utc::now(),
        )
    }

    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn restore_env(key: &str, previous: Option<std::ffi::OsString>) {
        if let Some(value) = previous {
            std::env::set_var(key, value);
        } else {
            std::env::remove_var(key);
        }
    }
}
