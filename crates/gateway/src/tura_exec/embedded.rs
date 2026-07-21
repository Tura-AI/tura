use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use runtime_contract::{CallContext, RunAgentRequest, RuntimeWorkerResponse, WorkerEnvelope};
use session_log_contract::{
    ActivateRuntimeLeaseRequest, RegisterRuntimeRequest, RuntimeLeaseOutcome,
    RuntimeRegistrationOutcome, SessionLogCommand, SessionLogResponse,
};

use super::cli::CliConfig;
use super::output::{write_jsonl, write_last_message, write_turn_log_stderr};
use super::router::worker_env_from_current_process;

const RUNTIME_TIMEOUT: Duration = Duration::from_secs(900);
const RUNTIME_POLL_INTERVAL: Duration = Duration::from_millis(50);

pub(crate) fn run_via_runtime_worker(
    config: &CliConfig,
    session_id: &str,
    prompt: String,
) -> Result<i32, String> {
    let binary = resolve_runtime_binary().ok_or_else(|| {
        "runtime worker binary (tura_runtime) not found beside tura_exec or under target"
            .to_string()
    })?;
    let runtime_id = format!("runtime-{}", uuid::Uuid::new_v4());
    let lease_id = format!("lease-{}", uuid::Uuid::new_v4());
    register_and_activate_runtime(session_id, &runtime_id, &lease_id)?;
    let request = RunAgentRequest {
        runtime_id,
        lease_id,
        session_id: Some(session_id.to_string()),
        directory: Some(config.cwd.to_string_lossy().to_string()),
        model: config.model.clone(),
        agent: config.agent.clone(),
        prompt: Some(prompt),
        planning_mode_override: config.planning_mode,
        no_op_manual: config.no_op_manual,
        return_log: config.log || config.json,
        worker_env: worker_env_from_current_process()
            .into_iter()
            .filter_map(|(key, value)| value.as_str().map(|value| (key, value.to_string())))
            .collect(),
        ..RunAgentRequest::default()
    };
    let envelope = WorkerEnvelope::call(CallContext {
        request_id: format!("embedded-{}", uuid::Uuid::new_v4()),
        method: "POST".to_string(),
        path: format!("/runtime_worker/{session_id}"),
        input: serde_json::to_value(request)
            .map_err(|error| format!("failed to encode runtime worker request: {error}"))?,
    });

    let mut command = Command::new(&binary);
    command
        .current_dir(&config.cwd)
        .env("TURA_RUNTIME_ONESHOT", "1")
        .env("TURA_GATEWAY_CALLBACKS", "0")
        .env("TURA_RUNTIME_ERRORS_FATAL", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    tura_path::process_hardening::hide_child_console_window_and_create_group(&mut command);
    let mut child = command.spawn().map_err(|error| {
        format!(
            "failed to start runtime worker {}: {error}",
            binary.display()
        )
    })?;
    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| "runtime worker stdin is unavailable".to_string())?;
    let payload = serde_json::to_vec(&envelope)
        .map_err(|error| format!("failed to encode runtime worker envelope: {error}"))?;
    stdin
        .write_all(&payload)
        .and_then(|_| stdin.write_all(b"\n"))
        .and_then(|_| stdin.flush())
        .map_err(|error| format!("failed to write runtime worker request: {error}"))?;
    drop(stdin);

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "runtime worker stdout is unavailable".to_string())?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "runtime worker stderr is unavailable".to_string())?;
    let stdout_reader = std::thread::spawn(move || read_pipe(stdout));
    let stderr_reader = std::thread::spawn(move || read_pipe(stderr));
    let started = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if started.elapsed() < RUNTIME_TIMEOUT => {
                std::thread::sleep(RUNTIME_POLL_INTERVAL);
            }
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                let _ = stdout_reader.join();
                let stderr = join_pipe(stderr_reader, "stderr")?;
                return Err(format!(
                    "runtime worker timed out after {} seconds{}",
                    RUNTIME_TIMEOUT.as_secs(),
                    stderr_suffix(&stderr)
                ));
            }
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                let _ = stdout_reader.join();
                let _ = stderr_reader.join();
                return Err(format!("failed to poll runtime worker: {error}"));
            }
        }
    };
    let stdout = join_pipe(stdout_reader, "stdout")?;
    let stderr = join_pipe(stderr_reader, "stderr")?;
    if !status.success() {
        return Err(format!(
            "runtime worker exited with status {status}{}",
            stderr_suffix(&stderr)
        ));
    }
    let response = parse_worker_response(&stdout)?;
    if !response.ok {
        return Err(response
            .error
            .unwrap_or_else(|| "runtime worker failed without an error message".to_string()));
    }
    render_response(config, session_id, response)
}

fn register_and_activate_runtime(
    session_id: &str,
    runtime_id: &str,
    lease_id: &str,
) -> Result<(), String> {
    let registered = session_log_contract::client::call_service(
        &SessionLogCommand::RegisterRuntime(RegisterRuntimeRequest {
            runtime_id: runtime_id.to_string(),
            session_id: session_id.to_string(),
            fallback_from_id: None,
        }),
    )
    .map_err(|error| format!("failed to register embedded runtime `{runtime_id}`: {error}"))?;
    match registered {
        SessionLogResponse::RuntimeRegistered {
            result:
                RuntimeRegistrationOutcome::Registered { .. }
                | RuntimeRegistrationOutcome::AlreadyRegistered { .. },
        } => {}
        SessionLogResponse::RuntimeRegistered { result } => {
            return Err(format!(
                "session_db rejected embedded runtime `{runtime_id}` registration: {result:?}"
            ));
        }
        SessionLogResponse::Error { error } => {
            return Err(format!(
                "failed to register embedded runtime `{runtime_id}`: {error}"
            ));
        }
        other => {
            return Err(format!(
                "unexpected session_db response while registering embedded runtime `{runtime_id}`: {other:?}"
            ));
        }
    }

    let activated = session_log_contract::client::call_service(
        &SessionLogCommand::ActivateRuntimeLease(ActivateRuntimeLeaseRequest {
            runtime_id: runtime_id.to_string(),
            lease_id: lease_id.to_string(),
        }),
    )
    .map_err(|error| format!("failed to activate embedded runtime `{runtime_id}`: {error}"))?;
    match activated {
        SessionLogResponse::RuntimeLeaseActivated {
            result: RuntimeLeaseOutcome::Activated | RuntimeLeaseOutcome::AlreadyActive,
        } => Ok(()),
        SessionLogResponse::RuntimeLeaseActivated { result } => Err(format!(
            "session_db rejected embedded runtime `{runtime_id}` lease: {result:?}"
        )),
        SessionLogResponse::Error { error } => Err(format!(
            "failed to activate embedded runtime `{runtime_id}`: {error}"
        )),
        other => Err(format!(
            "unexpected session_db response while activating embedded runtime `{runtime_id}`: {other:?}"
        )),
    }
}

fn render_response(
    config: &CliConfig,
    session_id: &str,
    response: RuntimeWorkerResponse,
) -> Result<i32, String> {
    let text = response
        .final_text
        .filter(|text| !text.trim().is_empty())
        .ok_or_else(|| "runtime worker completed without a final assistant message".to_string())?;
    if let Some(path) = config.last_message_path.as_ref() {
        write_last_message(path, &text)?;
    }
    if config.log {
        write_turn_log_stderr(&response.session_log, response.turn_started_at_ms)?;
    }
    if config.json {
        write_jsonl(&response.session_log, session_id, config, false)?;
    } else {
        println!("{text}");
    }
    Ok(0)
}

fn resolve_runtime_binary() -> Option<PathBuf> {
    let file_name = if cfg!(windows) {
        "tura_runtime.exe"
    } else {
        "tura_runtime"
    };
    let mut candidates = Vec::new();
    if let Some(explicit) = std::env::var_os("TURA_RUNTIME_BIN") {
        candidates.push(PathBuf::from(explicit));
    }
    if let Some(directory) = std::env::var_os("TURA_RELEASE_BIN_DIR") {
        candidates.push(PathBuf::from(directory).join(file_name));
    }
    if let Ok(current_exe) = std::env::current_exe() {
        candidates.push(current_exe.with_file_name(file_name));
    }
    let root = tura_path::canonical_root();
    candidates.push(root.join("target").join("debug").join(file_name));
    candidates.push(root.join("target").join("release").join(file_name));
    candidates.into_iter().find(|path| path.is_file())
}

fn read_pipe(mut pipe: impl Read) -> std::io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    pipe.read_to_end(&mut bytes)?;
    Ok(bytes)
}

fn join_pipe(
    reader: std::thread::JoinHandle<std::io::Result<Vec<u8>>>,
    name: &str,
) -> Result<String, String> {
    let bytes = reader
        .join()
        .map_err(|_| format!("runtime worker {name} reader panicked"))?
        .map_err(|error| format!("failed to read runtime worker {name}: {error}"))?;
    String::from_utf8(bytes).map_err(|error| format!("runtime worker {name} is not UTF-8: {error}"))
}

fn parse_worker_response(stdout: &str) -> Result<RuntimeWorkerResponse, String> {
    stdout
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .find_map(|line| serde_json::from_str::<RuntimeWorkerResponse>(line).ok())
        .ok_or_else(|| "runtime worker returned no valid typed response".to_string())
}

fn stderr_suffix(stderr: &str) -> String {
    let stderr = stderr.trim();
    if stderr.is_empty() {
        String::new()
    } else {
        format!(": {stderr}")
    }
}
