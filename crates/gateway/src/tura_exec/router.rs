use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use super::cli::CliConfig;
use super::env::normalize_model;
use super::output::{emit_jsonl, write_last_message};
use super::session::final_text_from_session_db;

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
    if let Some(agent) = config.agent.as_deref() {
        payload["agent"] = json!(agent);
    }
    if let Some(model) = config.model.as_deref() {
        payload["model"] = json!(normalize_model(model));
    }
    if let Some(planning_mode) = config.planning_mode {
        payload["planning_mode_override"] = json!(planning_mode);
    }
    let worker_env = worker_env_from_current_process();
    if !worker_env.is_empty() {
        payload["worker_env"] = Value::Object(worker_env);
    }
    let request = json!({
        "request_id": format!("exec-{}", uuid::Uuid::new_v4()),
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

    let mut line = String::new();
    BufReader::new(stream)
        .read_line(&mut line)
        .map_err(|err| format!("failed to read router response: {err}"))?;
    let response: Value = serde_json::from_str(line.trim())
        .map_err(|err| format!("invalid router response: {err}"))?;
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
    if config.json {
        emit_jsonl(
            &json!({"type": "item.completed", "item": {"type": "assistant_message", "text": text}}),
        )?;
        emit_jsonl(&json!({"type": "turn.completed"}))?;
    } else {
        println!("{text}");
    }
    Ok(0)
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
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // DETACHED_PROCESS so the daemon outlives this CLI and the spawning shell
        // does not wait on it; CREATE_NO_WINDOW so no console flashes.
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        command.creation_flags(CREATE_NO_WINDOW | DETACHED_PROCESS);
    }
    command
        .spawn()
        .map_err(|err| format!("failed to start router daemon: {err}"))?;
    let started = Instant::now();
    while started.elapsed() < Duration::from_secs(90) {
        if let Some(addr) = reachable_router_addr() {
            return Ok(addr);
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    Err("router daemon did not become reachable".to_string())
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

fn worker_env_from_current_process() -> serde_json::Map<String, Value> {
    const KEYS: &[&str] = &[
        "TURA_SESSION_REASONING_EFFORT",
        "TURA_SESSION_ACCELERATION_ENABLED",
        "TURA_SESSION_MAX_TOKENS",
        "TURA_SESSION_LANGUAGE",
        "TURA_SESSION_USER_NAME",
        "TURA_COMMAND_RUN_STALL_CHECK_SECS",
        "TURA_COMMAND_RUN_STALL_IDENTICAL_CHECKS",
        "TURA_COMMAND_RUN_SHELL",
        "TURA_COMMAND_RUN_STRICT_JSON",
        "TURA_COMMAND_RUN_DISABLE_STRICT_JSON",
        "TURA_RUNTIME_ERRORS_FATAL",
        "TURA_WORKER_INVOKE_TIMEOUT_SECS",
        "TURA_RUNTIME_WORKER_STDERR_LOG",
        "TURA_DEBUG_RUNTIME",
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
    candidates.push(root.join("target").join("debug").join(executable));
    candidates.push(root.join("target").join("release").join(executable));
    candidates.into_iter().find(|path| path.exists())
}

#[cfg(test)]
mod tests {
    use super::{
        router_final_text, router_health_ok, router_stderr_log_path,
        worker_env_from_current_process,
    };
    use serde_json::json;
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
    fn worker_env_from_current_process_keeps_only_documented_nonempty_keys() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|err| err.into_inner());
        let previous_model = std::env::var_os("TURA_SESSION_MAX_TOKENS");
        let previous_empty = std::env::var_os("TURA_SESSION_LANGUAGE");
        let previous_other = std::env::var_os("TURA_UNRELATED_ENV_FOR_TEST");

        std::env::set_var("TURA_SESSION_MAX_TOKENS", "4096");
        std::env::set_var("TURA_SESSION_LANGUAGE", "");
        std::env::set_var("TURA_UNRELATED_ENV_FOR_TEST", "must-not-forward");

        let env = worker_env_from_current_process();

        assert_eq!(env.get("TURA_SESSION_MAX_TOKENS"), Some(&json!("4096")));
        assert!(!env.contains_key("TURA_SESSION_LANGUAGE"));
        assert!(!env.contains_key("TURA_UNRELATED_ENV_FOR_TEST"));

        restore_env("TURA_SESSION_MAX_TOKENS", previous_model);
        restore_env("TURA_SESSION_LANGUAGE", previous_empty);
        restore_env("TURA_UNRELATED_ENV_FOR_TEST", previous_other);
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
}
