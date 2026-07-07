use std::collections::HashSet;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use super::cli::CliConfig;
use super::env::normalize_model;
use super::output::{
    aggregate_runtime_usage, emit_jsonl, turn_completed_event, write_last_message,
    write_turn_log_stderr,
};
use super::session::final_text_from_session_db;

const ROUTER_HEALTH_TIMEOUT: Duration = Duration::from_secs(20);
const ROUTER_HEALTH_POLL_INTERVAL: Duration = Duration::from_millis(200);

/// Thin-client turn: dispatch to the detached `tura_router` daemon (which owns
/// session_db and spawns the runtime worker), block for completion, then render
/// the final assistant message from the persisted session. The CLI never runs
/// runtime or opens a database in-process.
pub(crate) fn run_via_router(
    config: &CliConfig,
    session_id: &str,
    prompt: &str,
) -> Result<i32, String> {
    let addr = ensure_router_daemon()?;
    let stream = TcpStream::connect(&addr)
        .map_err(|err| format!("failed to connect to router daemon at {addr}: {err}"))?;
    stream.set_read_timeout(Some(Duration::from_secs(900))).ok();

    let mut payload = json!({
        "session_id": session_id,
        "directory": config.cwd.to_string_lossy(),
        "prompt": prompt,
    });
    if config.log || config.json {
        payload["return_log"] = json!(true);
    }
    if let Some(agent) = config.agent.as_deref() {
        payload["agent"] = json!(agent);
    }
    if let Some(model) = config.model.as_deref() {
        payload["model"] = json!(normalize_model(model));
    }
    if let Some(planning_mode) = config.planning_mode {
        payload["planning_mode_override"] = json!(planning_mode);
    }
    if config.no_op_manual {
        payload["no_op_manual"] = json!(true);
    }
    let worker_env = worker_env_from_current_process();
    if !worker_env.is_empty() {
        payload["worker_env"] = Value::Object(worker_env);
    }
    let request_id = format!("exec-{}", uuid::Uuid::new_v4());
    let request = json!({
        "request_id": request_id,
        "kind": "call",
        "method": "execution.enqueue_turn",
        "payload": { "turn_id": format!("turn-{}", uuid::Uuid::new_v4()), "session_id": session_id, "payload": payload },
    });

    let mut writer = stream
        .try_clone()
        .map_err(|err| format!("router socket clone failed: {err}"))?;
    writer
        .write_all(format!("{request}\n").as_bytes())
        .and_then(|_| writer.flush())
        .map_err(|err| format!("failed to send turn to router: {err}"))?;

    let response = read_router_response(stream, &request_id)?;
    if !response.get("ok").and_then(Value::as_bool).unwrap_or(false) {
        return Err(response
            .get("error")
            .and_then(Value::as_str)
            .unwrap_or("router turn failed")
            .to_string());
    }

    // The worker persisted the session to the single owner; render from there.
    let text =
        router_final_text(&response).unwrap_or_else(|| final_text_from_session_db(session_id));
    if let Some(path) = config.last_message_path.as_ref() {
        write_last_message(path, &text)?;
    }
    let session_log = router_session_log(&response);
    if config.log {
        if let Some(session_log) = session_log.as_ref() {
            write_turn_log_stderr(session_log, router_turn_started_at_ms(&response))?;
        }
    }
    if config.json {
        emit_jsonl(
            &json!({"type": "item.completed", "item": {"type": "assistant_message", "text": text}}),
        )?;
        let usage = session_log
            .as_deref()
            .map(aggregate_runtime_usage)
            .or_else(|| router_usage(&response))
            .unwrap_or_else(|| aggregate_runtime_usage(&[]));
        emit_jsonl(&turn_completed_event(
            config,
            session_id,
            usage,
            "completed",
            None,
        ))?;
    } else {
        println!("{text}");
    }
    Ok(0)
}

fn read_router_response(stream: TcpStream, request_id: &str) -> Result<Value, String> {
    let mut reader = BufReader::new(stream);
    let mut emitted_cli_items = HashSet::new();
    loop {
        let mut line = String::new();
        let bytes = reader
            .read_line(&mut line)
            .map_err(|err| format!("failed to read router response: {err}"))?;
        if bytes == 0 {
            return Err("router closed before final response".to_string());
        }
        let value: Value = serde_json::from_str(line.trim())
            .map_err(|err| format!("invalid router response: {err}"))?;
        let matches_request = value
            .get("request_id")
            .and_then(Value::as_str)
            .is_some_and(|candidate| candidate == request_id);
        if !matches_request {
            continue;
        }
        if value.get("kind").and_then(Value::as_str) == Some("gateway.callback") {
            emit_router_callback_cli_events(&value, &mut emitted_cli_items)?;
            continue;
        }
        if value.get("ok").and_then(Value::as_bool).is_some() {
            return Ok(value);
        }
    }
}

fn emit_router_callback_cli_events(
    value: &Value,
    emitted_cli_items: &mut HashSet<String>,
) -> Result<(), String> {
    for event in router_callback_cli_events(value, emitted_cli_items) {
        emit_jsonl(&event)?;
    }
    Ok(())
}

fn router_callback_cli_events(
    value: &Value,
    emitted_cli_items: &mut HashSet<String>,
) -> Vec<Value> {
    let method = value
        .get("method")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let payload = value.get("payload").unwrap_or(&Value::Null);
    let body = payload.get("body").unwrap_or(payload);
    if body.get("item").is_some()
        && matches!(
            body.get("type").and_then(Value::as_str),
            Some("item.started" | "item.completed")
        )
    {
        return router_callback_direct_item_event(body, emitted_cli_items)
            .into_iter()
            .collect();
    }
    if method != "session.agent_message" {
        return Vec::new();
    }
    command_update_cli_events(body, emitted_cli_items)
}

fn router_callback_direct_item_event(
    event: &Value,
    emitted_cli_items: &mut HashSet<String>,
) -> Option<Value> {
    let item_type = event.pointer("/item/type").and_then(Value::as_str)?;
    if !matches!(item_type, "command_execution" | "file_change") {
        return None;
    }
    let event_type = event.get("type").and_then(Value::as_str)?;
    let id = event
        .pointer("/item/id")
        .and_then(Value::as_str)
        .unwrap_or("");
    let key = format!("{event_type}:{item_type}:{id}");
    if !emitted_cli_items.insert(key) {
        return None;
    }
    let mut event = event.clone();
    if let Some(item) = event.get_mut("item").and_then(Value::as_object_mut) {
        item.entry("status".to_string()).or_insert_with(|| {
            json!(if event_type == "item.started" {
                "in_progress"
            } else {
                "completed"
            })
        });
    }
    Some(event)
}

fn command_update_cli_events(body: &Value, emitted_cli_items: &mut HashSet<String>) -> Vec<Value> {
    body.get("command_updates")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|update| command_update_cli_event(update, emitted_cli_items))
        .collect()
}

fn command_update_cli_event(
    update: &Value,
    emitted_cli_items: &mut HashSet<String>,
) -> Option<Value> {
    let status = update.get("status").and_then(Value::as_str)?;
    let (event_type, item_status, phase) = match status {
        "ready" | "running" | "in_progress" => ("item.started", "in_progress", "started"),
        "completed" => ("item.completed", "completed", "completed"),
        "failed" | "error" => ("item.completed", "failed", "completed"),
        _ => return None,
    };
    let command_type = command_update_command_type(update);
    if command_type.as_deref() == Some("task_status") {
        return None;
    }
    let item_type = if command_type.as_deref() == Some("apply_patch") {
        "file_change"
    } else {
        "command_execution"
    };
    let item_id = command_update_id(update);
    let key = format!("{phase}:{item_id}");
    if !emitted_cli_items.insert(key) {
        return None;
    }

    let mut item = serde_json::Map::new();
    item.insert("id".to_string(), Value::String(item_id));
    item.insert("type".to_string(), Value::String(item_type.to_string()));
    item.insert("status".to_string(), Value::String(item_status.to_string()));
    if let Some(command_type) = command_type.as_deref() {
        item.insert(
            "command".to_string(),
            Value::String(command_type.to_string()),
        );
        item.insert(
            "command_type".to_string(),
            Value::String(command_type.to_string()),
        );
    }
    if let Some(command_line) = command_update_command_line(update) {
        item.insert(
            "command_line".to_string(),
            Value::String(command_line.clone()),
        );
        item.insert("display_command".to_string(), Value::String(command_line));
    }
    if let Some(step) = update
        .pointer("/command/step")
        .or_else(|| update.pointer("/result/step"))
    {
        item.insert("step".to_string(), step.clone());
    }
    if let Some(provider_tool_call_id) = update.get("providerToolCallID").or_else(|| {
        update
            .get("provider_tool_call_id")
            .filter(|value| !value.is_null())
    }) {
        item.insert(
            "provider_tool_call_id".to_string(),
            provider_tool_call_id.clone(),
        );
    }
    if let Some(command_index) = update
        .get("commandIndex")
        .or_else(|| update.get("command_index"))
    {
        item.insert("command_index".to_string(), command_index.clone());
    }
    if event_type == "item.completed" {
        item.insert(
            "aggregated_output".to_string(),
            Value::String(command_update_aggregated_output(update)),
        );
        if let Some(exit_code) = update.pointer("/result/exit_code") {
            item.insert("exit_code".to_string(), exit_code.clone());
        }
        if item_type == "file_change" {
            if let Some(changes) = command_update_changes(update) {
                item.insert("changes".to_string(), changes);
            }
        }
    }

    Some(serde_json::json!({
        "type": event_type,
        "item": Value::Object(item),
    }))
}

fn command_update_id(update: &Value) -> String {
    update
        .get("commandID")
        .or_else(|| update.get("command_id"))
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| {
            let run_id = update
                .get("commandRunID")
                .or_else(|| update.get("command_run_id"))
                .and_then(Value::as_str)
                .unwrap_or("command_run");
            let index = update
                .get("commandIndex")
                .or_else(|| update.get("command_index"))
                .and_then(Value::as_u64)
                .unwrap_or(0);
            format!("{run_id}:{index}")
        })
}

fn command_update_command_type(update: &Value) -> Option<String> {
    [
        update.pointer("/result/command_type"),
        update.pointer("/result/command"),
        update.pointer("/command/command_type"),
        update.pointer("/command/command"),
        update.pointer("/result/command/command_type"),
        update.pointer("/result/command/command"),
    ]
    .into_iter()
    .flatten()
    .find_map(|value| value.as_str().map(str::to_string))
}

fn command_update_command_line(update: &Value) -> Option<String> {
    [
        update.pointer("/command/command_line"),
        update.pointer("/result/command_line"),
        update.pointer("/result/command/command_line"),
        update.pointer("/command_line"),
    ]
    .into_iter()
    .flatten()
    .find_map(|value| value.as_str().map(str::to_string))
}

fn command_update_aggregated_output(update: &Value) -> String {
    [
        update.pointer("/result/stdout"),
        update.pointer("/result/output"),
        update.pointer("/result/error"),
        update.pointer("/output"),
        update.pointer("/error"),
    ]
    .into_iter()
    .flatten()
    .find_map(|value| match value {
        Value::String(text) => Some(text.to_string()),
        Value::Null => None,
        other => serde_json::to_string(other).ok(),
    })
    .unwrap_or_default()
}

fn command_update_changes(update: &Value) -> Option<Value> {
    update
        .pointer("/result/changes")
        .or_else(|| update.pointer("/result/output/changes"))
        .cloned()
}

/// Probe the per-home router daemon; start it detached if none is reachable.
/// Returns the socket address. The CLI keeps its turn request socket open until
/// completion; if that socket closes early, router cancels the active turn and
/// aborts unfinished router-owned command_run work for the connection.
fn ensure_router_daemon() -> Result<String, String> {
    if let Some(addr) = reachable_router_addr() {
        return Ok(addr);
    }
    let bin = resolve_router_binary()
        .ok_or_else(|| "tura_router binary not found for router daemon".to_string())?;
    let mut command = std::process::Command::new(&bin);
    command
        .arg("serve-socket")
        .stdin(Stdio::null())
        .stdout(Stdio::null());
    command.env_remove("TURA_CLI_LIVE_JSONL");
    command.env_remove("TURA_CLI_PROGRESS");
    configure_router_stderr(&mut command);
    tura_path::process_hardening::hide_child_console_window_and_detach(&mut command);
    command
        .spawn()
        .map_err(|err| format!("failed to start router daemon: {err}"))?;
    let started = Instant::now();
    while started.elapsed() < ROUTER_HEALTH_TIMEOUT {
        if let Some(addr) = reachable_router_addr() {
            return Ok(addr);
        }
        std::thread::sleep(ROUTER_HEALTH_POLL_INTERVAL);
    }
    if let Some(error) = session_log::service::unreachable_owner_lock_message() {
        return Err(format!(
            "router daemon did not become healthy within 20 seconds: {error}"
        ));
    }
    Err("router daemon did not become healthy within 20 seconds".to_string())
}

fn router_final_text(response: &Value) -> Option<String> {
    response
        .pointer("/payload/result/result/final_text")
        .or_else(|| response.pointer("/payload/result/final_text"))
        .or_else(|| response.pointer("/payload/final_text"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(str::to_string)
}

fn router_session_log(response: &Value) -> Option<Vec<String>> {
    response
        .pointer("/payload/result/result/session_log")
        .or_else(|| response.pointer("/payload/result/session_log"))
        .or_else(|| response.pointer("/payload/session_log"))
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(str::to_string))
                .collect::<Vec<_>>()
        })
        .filter(|items| !items.is_empty())
}

fn router_turn_started_at_ms(response: &Value) -> Option<i64> {
    response
        .pointer("/payload/result/result/turn_started_at_ms")
        .or_else(|| response.pointer("/payload/result/turn_started_at_ms"))
        .or_else(|| response.pointer("/payload/turn_started_at_ms"))
        .and_then(Value::as_i64)
}

fn router_usage(response: &Value) -> Option<Value> {
    [
        response.pointer("/payload/result/result/usage"),
        response.pointer("/payload/result/usage"),
        response.pointer("/payload/usage"),
        response.pointer("/usage"),
    ]
    .into_iter()
    .flatten()
    .find(|value| value.is_object())
    .cloned()
}

fn worker_env_from_current_process() -> serde_json::Map<String, Value> {
    const KEYS: &[&str] = &[
        "TURA_SESSION_REASONING_EFFORT",
        "TURA_SESSION_ACCELERATION_ENABLED",
        "TURA_SESSION_MAX_TOKENS",
        "TURA_GOAL_MODE",
        "TURA_NO_OP_MANUAL",
        "TURA_FRONTEND_SOURCE",
        "TURA_SESSION_LANGUAGE",
        "TURA_SESSION_USER_NAME",
        "TURA_COMMAND_RUN_STALL_CHECK_SECS",
        "TURA_COMMAND_RUN_STALL_IDENTICAL_CHECKS",
        "TURA_COMMAND_RUN_SHELL",
        "TURA_COMMAND_RUN_SANDBOX",
        "TURA_COMMAND_RUN_STRICT_JSON",
        "TURA_COMMAND_RUN_DISABLE_STRICT_JSON",
        "TURA_RUNTIME_ERRORS_FATAL",
        "TURA_RUNTIME_WORKER_STDERR_LOG",
        "TURA_DEBUG_RUNTIME",
        "TURA_PROFILE_TURN_TIMINGS",
        "TURA_PROFILE_TIMINGS",
        "TURA_PROFILE_TURN_TIMING_BYTES",
        "TURA_PROFILE_TIMING_BYTES",
        "TURA_PROVIDER_CONFIG",
        "TURA_ENV_PATH",
        "TURA_PROJECT_ROOT",
        "LOG_PATH",
        "SESSION_LOG_DB_ROOT",
        "TURA_HOME",
        "TURA_DB_ROOT",
    ];
    KEYS.iter()
        .filter_map(|key| {
            std::env::var(key)
                .ok()
                .filter(|value| !value.is_empty())
                .map(|value| ((*key).to_string(), Value::String(value)))
        })
        .collect()
}

fn configure_router_stderr(command: &mut std::process::Command) {
    let Some(path) = router_stderr_log_path() else {
        command.stderr(Stdio::null());
        return;
    };
    if let Some(parent) = path.parent() {
        if fs::create_dir_all(parent).is_err() {
            command.stderr(Stdio::null());
            return;
        }
    }
    match fs::OpenOptions::new().create(true).append(true).open(path) {
        Ok(file) => {
            command.stderr(file);
        }
        Err(_) => {
            command.stderr(Stdio::null());
        }
    }
}

fn router_stderr_log_path() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("TURA_ROUTER_STDERR_LOG") {
        return Some(PathBuf::from(path));
    }
    std::env::var_os("TURA_DEBUG_RUNTIME")?;
    Some(session_log::path::default_db_dir().join("router-daemon.stderr.log"))
}

fn router_addr_path() -> PathBuf {
    session_log::path::default_db_dir().join("router.addr")
}

/// Read the published router endpoint and confirm it is actually connectable.
fn reachable_router_addr() -> Option<String> {
    let raw = fs::read_to_string(router_addr_path()).ok()?;
    let endpoint: Value = serde_json::from_str(raw.trim()).ok()?;
    let version = endpoint
        .get("version")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if !version.is_empty() && version != tura_path::instance_version() {
        return None;
    }
    let addr = endpoint.get("addr").and_then(Value::as_str)?.to_string();
    let socket: std::net::SocketAddr = addr.parse().ok()?;
    if router_health_ok(socket) {
        Some(addr)
    } else {
        let _ = fs::remove_file(router_addr_path());
        None
    }
}

fn router_health_ok(socket: std::net::SocketAddr) -> bool {
    let mut stream = match std::net::TcpStream::connect_timeout(&socket, Duration::from_secs(2)) {
        Ok(stream) => stream,
        Err(_) => return false,
    };
    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(2)));
    let request = json!({
        "request_id": format!("exec-health-{}", uuid::Uuid::new_v4()),
        "kind": "health_check",
        "method": "health_check",
        "payload": {},
        "deadline_ms": 5000,
    });
    if stream
        .write_all(format!("{request}\n").as_bytes())
        .and_then(|_| stream.flush())
        .is_err()
    {
        return false;
    }
    let mut line = String::new();
    if BufReader::new(stream).read_line(&mut line).is_err() {
        return false;
    }
    serde_json::from_str::<Value>(line.trim())
        .ok()
        .and_then(|response| response.get("ok").and_then(Value::as_bool))
        .unwrap_or(false)
}

fn resolve_router_binary() -> Option<PathBuf> {
    let executable = if cfg!(windows) {
        "tura_router.exe"
    } else {
        "tura_router"
    };
    let mut candidates = Vec::new();
    let root = tura_path::canonical_root();
    if let Ok(current_exe) = std::env::current_exe() {
        if current_exe
            .parent()
            .and_then(std::path::Path::file_name)
            .and_then(|name| name.to_str())
            != Some("deps")
        {
            candidates.push(current_exe.with_file_name(executable));
        }
    }
    let profile = if tura_path::build_kind() == "release" {
        "release"
    } else {
        "debug"
    };
    candidates.push(root.join("target").join(profile).join(executable));
    candidates.into_iter().find(|path| path.exists())
}

#[cfg(test)]
mod tests {
    use super::{
        read_router_response, router_callback_cli_events, router_final_text, router_health_ok,
        router_session_log, router_stderr_log_path, router_turn_started_at_ms, router_usage,
        worker_env_from_current_process,
    };
    use serde_json::json;
    use std::collections::HashSet;
    use std::ffi::OsString;
    use std::io::{BufRead, BufReader, Write};
    use std::net::TcpListener;
    use std::sync::Mutex;
    use std::thread;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn restore_env(key: &str, value: Option<OsString>) {
        if let Some(value) = value {
            std::env::set_var(key, value);
        } else {
            std::env::remove_var(key);
        }
    }

    #[test]
    fn router_final_text_reads_nested_result_shapes_in_priority_order() {
        assert_eq!(
            router_final_text(&json!({
                "payload": {
                    "result": {
                        "result": {"final_text": " first "},
                        "final_text": "second"
                    },
                    "final_text": "third"
                }
            }))
            .as_deref(),
            Some("first")
        );
        assert_eq!(
            router_final_text(&json!({
                "payload": {"result": {"final_text": " second "}}
            }))
            .as_deref(),
            Some("second")
        );
        assert_eq!(
            router_final_text(&json!({
                "payload": {"final_text": " third "}
            }))
            .as_deref(),
            Some("third")
        );
        assert_eq!(
            router_final_text(&json!({"payload": {"final_text": "  "}})),
            None
        );
    }

    #[test]
    fn router_log_helpers_read_nested_runtime_worker_log_payload() {
        let response = json!({
            "payload": {
                "result": {
                    "result": {
                        "session_log": ["{\"role\":\"user\",\"content\":\"hi\"}"],
                        "turn_started_at_ms": 1234,
                        "usage": {"total_tokens": 7}
                    }
                }
            }

        });

        let log = router_session_log(&response).expect("session log");
        assert_eq!(log.len(), 1);
        assert_eq!(router_turn_started_at_ms(&response), Some(1234));
        assert_eq!(router_usage(&response), Some(json!({"total_tokens": 7})));
    }

    #[test]
    fn worker_env_from_current_process_keeps_only_documented_nonempty_keys() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|err| err.into_inner());
        let previous_model = std::env::var_os("TURA_SESSION_MAX_TOKENS");
        let previous_goal = std::env::var_os("TURA_GOAL_MODE");
        let previous_no_op = std::env::var_os("TURA_NO_OP_MANUAL");
        let previous_empty = std::env::var_os("TURA_SESSION_LANGUAGE");
        let previous_other = std::env::var_os("TURA_UNRELATED_ENV_FOR_TEST");
        let legacy_timeout_key = ["TURA_WORKER", "INVOKE_TIMEOUT_SECS"].join("_");
        let previous_timeout = std::env::var_os(&legacy_timeout_key);

        std::env::set_var("TURA_SESSION_MAX_TOKENS", "4096");
        std::env::set_var("TURA_GOAL_MODE", "1");
        std::env::set_var("TURA_NO_OP_MANUAL", "1");
        std::env::set_var("TURA_SESSION_LANGUAGE", "");
        std::env::set_var("TURA_UNRELATED_ENV_FOR_TEST", "must-not-forward");
        std::env::set_var(&legacy_timeout_key, "1");

        let env = worker_env_from_current_process();

        assert_eq!(env.get("TURA_SESSION_MAX_TOKENS"), Some(&json!("4096")));
        assert_eq!(env.get("TURA_GOAL_MODE"), Some(&json!("1")));
        assert_eq!(env.get("TURA_NO_OP_MANUAL"), Some(&json!("1")));
        assert!(!env.contains_key("TURA_SESSION_LANGUAGE"));
        assert!(!env.contains_key("TURA_UNRELATED_ENV_FOR_TEST"));
        assert!(
            !env.keys().any(|key| key.contains("INVOKE_TIMEOUT")),
            "worker env must not forward a session-wide invoke deadline"
        );
        assert!(!env.contains_key("TURA_COMMAND_RUN_SANDBOX"));

        restore_env("TURA_SESSION_MAX_TOKENS", previous_model);
        restore_env("TURA_GOAL_MODE", previous_goal);
        restore_env("TURA_NO_OP_MANUAL", previous_no_op);
        restore_env("TURA_SESSION_LANGUAGE", previous_empty);
        restore_env("TURA_UNRELATED_ENV_FOR_TEST", previous_other);
        restore_env(&legacy_timeout_key, previous_timeout);
    }

    #[test]
    fn router_stderr_log_path_prefers_explicit_path_and_debug_default() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|err| err.into_inner());
        let previous_log = std::env::var_os("TURA_ROUTER_STDERR_LOG");
        let previous_debug = std::env::var_os("TURA_DEBUG_RUNTIME");

        std::env::remove_var("TURA_ROUTER_STDERR_LOG");
        std::env::remove_var("TURA_DEBUG_RUNTIME");
        assert_eq!(router_stderr_log_path(), None);

        std::env::set_var("TURA_DEBUG_RUNTIME", "1");
        let debug_path = router_stderr_log_path().expect("debug path");
        assert!(debug_path.ends_with("router-daemon.stderr.log"));

        let explicit = std::env::temp_dir().join("explicit-router.stderr.log");
        std::env::set_var("TURA_ROUTER_STDERR_LOG", &explicit);
        assert_eq!(router_stderr_log_path(), Some(explicit));

        restore_env("TURA_ROUTER_STDERR_LOG", previous_log);
        restore_env("TURA_DEBUG_RUNTIME", previous_debug);
    }

    #[test]
    fn router_health_probe_requires_json_health_response() -> anyhow::Result<()> {
        let listener = TcpListener::bind(("127.0.0.1", 0))?;
        let addr = listener.local_addr()?;
        let server = thread::spawn(move || -> anyhow::Result<()> {
            let (mut stream, _) = listener.accept()?;
            let mut line = String::new();
            BufReader::new(stream.try_clone()?).read_line(&mut line)?;
            let request: serde_json::Value = serde_json::from_str(line.trim())?;
            assert_eq!(request["method"], "health_check");
            stream.write_all(b"{\"ok\":true,\"payload\":{\"status\":\"ok\"}}\n")?;
            stream.flush()?;
            Ok(())
        });

        assert!(router_health_ok(addr));
        server
            .join()
            .map_err(|_| anyhow::anyhow!("health probe server panicked"))??;
        Ok(())
    }

    #[test]
    fn read_router_response_skips_matching_notifications_until_final_response() -> anyhow::Result<()>
    {
        let listener = TcpListener::bind(("127.0.0.1", 0))?;
        let addr = listener.local_addr()?;
        let server = thread::spawn(move || -> anyhow::Result<()> {
            let (mut stream, _) = listener.accept()?;
            stream.write_all(
                br#"{"request_id":"request-1","kind":"gateway.callback","method":"session.agent_stream","payload":{"type":"turn.started"}}"#,
            )?;
            stream.write_all(b"\n")?;
            stream.write_all(
                br#"{"request_id":"request-1","ok":true,"payload":{"status":"finished"}}"#,
            )?;
            stream.write_all(b"\n")?;
            stream.flush()?;
            Ok(())
        });

        let stream = std::net::TcpStream::connect(addr)?;
        let response = read_router_response(stream, "request-1").map_err(anyhow::Error::msg)?;
        assert_eq!(response["ok"], true);
        assert_eq!(response["payload"]["status"], "finished");
        server
            .join()
            .map_err(|_| anyhow::anyhow!("response server panicked"))??;
        Ok(())
    }

    #[test]
    fn router_callback_cli_events_emit_command_updates_once() {
        let mut emitted = HashSet::new();
        let notification = json!({
            "request_id": "request-1",
            "kind": "gateway.callback",
            "method": "session.agent_message",
            "payload": {
                "session_id": "session-1",
                "body": {
                    "command_updates": [
                        {
                            "commandID": "cmd-1",
                            "commandRunID": "run-1",
                            "providerToolCallID": "call-1",
                            "commandIndex": 0,
                            "status": "ready",
                            "command": {
                                "command_type": "shell_command",
                                "command_line": "Get-Content -Raw src/app.txt",
                                "step": 1
                            },
                            "createdAt": 1,
                            "updatedAt": 1
                        },
                        {
                            "commandID": "cmd-1",
                            "commandRunID": "run-1",
                            "providerToolCallID": "call-1",
                            "commandIndex": 0,
                            "status": "completed",
                            "command": {
                                "command_type": "shell_command",
                                "command_line": "Get-Content -Raw src/app.txt",
                                "step": 1
                            },
                            "result": {
                                "command_type": "shell_command",
                                "success": true,
                                "exit_code": 0,
                                "output": "broken-by-agent\n"
                            },
                            "createdAt": 1,
                            "updatedAt": 2
                        }
                    ]
                }
            }
        });

        let events = router_callback_cli_events(&notification, &mut emitted);

        assert_eq!(events.len(), 2);
        assert_eq!(events[0]["type"], "item.started");
        assert_eq!(events[0]["item"]["type"], "command_execution");
        assert_eq!(events[0]["item"]["status"], "in_progress");
        assert_eq!(events[0]["item"]["command"], "shell_command");
        assert_eq!(events[0]["item"]["command_type"], "shell_command");
        assert_eq!(
            events[0]["item"]["command_line"],
            "Get-Content -Raw src/app.txt"
        );
        assert_eq!(events[0]["item"]["provider_tool_call_id"], "call-1");
        assert_eq!(events[0]["item"]["command_index"], 0);
        assert_eq!(events[0]["item"]["step"], 1);
        assert_eq!(events[1]["type"], "item.completed");
        assert_eq!(events[1]["item"]["type"], "command_execution");
        assert_eq!(events[1]["item"]["status"], "completed");
        assert_eq!(events[1]["item"]["command"], "shell_command");
        assert_eq!(events[1]["item"]["command_type"], "shell_command");
        assert_eq!(
            events[1]["item"]["command_line"],
            "Get-Content -Raw src/app.txt"
        );
        assert_eq!(events[1]["item"]["aggregated_output"], "broken-by-agent\n");
        assert_eq!(events[1]["item"]["exit_code"], 0);

        let duplicate = router_callback_cli_events(&notification, &mut emitted);
        assert!(duplicate.is_empty());
    }

    #[test]
    fn router_callback_cli_events_emit_apply_patch_as_file_change() {
        let mut emitted = HashSet::new();
        let notification = json!({
            "kind": "gateway.callback",
            "method": "session.agent_message",
            "payload": {
                "body": {
                    "command_updates": [{
                        "commandID": "patch-1",
                        "status": "completed",
                        "command": {
                            "command_type": "apply_patch",
                            "command_line": "*** Begin Patch"
                        },
                        "result": {
                            "command_type": "apply_patch",
                            "success": true,
                            "changes": [{ "path": "src/app.txt", "kind": "update" }]
                        }
                    }]
                }
            }
        });

        let events = router_callback_cli_events(&notification, &mut emitted);

        assert_eq!(events.len(), 1);
        assert_eq!(events[0]["type"], "item.completed");
        assert_eq!(events[0]["item"]["type"], "file_change");
        assert_eq!(events[0]["item"]["command"], "apply_patch");
        assert_eq!(events[0]["item"]["command_line"], "*** Begin Patch");
        assert_eq!(events[0]["item"]["changes"][0]["path"], "src/app.txt");
    }
}
