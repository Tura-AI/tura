use std::sync::atomic::{AtomicBool, Ordering};
use std::{
    fs::OpenOptions, io::Write as StdWrite, path::Path, process::Stdio, sync::Arc, time::Duration,
};

use anyhow::{anyhow, Context, Result};
use serde_json::{json, Value};
use tokio::{
    io::{AsyncBufRead, AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    process::{Child, ChildStdin, ChildStdout, Command},
    sync::{Mutex, Notify},
    time::{timeout, Instant},
};
use tracing::{error, info, warn};

use crate::ipc;

use super::models::{CallContext, WorkerEnvelope};
use super::process_scope::{attach_child_scope, configure_scoped_spawn, WorkerProcessScope};

const WORKER_HEALTH_TIMEOUT: Duration = Duration::from_secs(10);

pub enum WorkerMode {
    Persistent,
    OneShot,
}

pub struct WorkerProcess {
    pub worker_id: String,
    pub service_name: String,
    pub mode: WorkerMode,
    pub executable_path: std::path::PathBuf,
    spawn_args: Vec<String>,
    spawn_env: Vec<(String, String)>,
    child: Mutex<Option<Child>>,
    process_scope: Mutex<Option<WorkerProcessScope>>,
    one_shot_process_scope: Mutex<Option<WorkerProcessScope>>,
    one_shot_cancelled: AtomicBool,
    one_shot_cancel: Notify,
    stdin: Mutex<Option<ChildStdin>>,
    stdout: Mutex<Option<BufReader<ChildStdout>>>,
    round_trip: Mutex<()>,
}

impl WorkerProcess {
    /// Start a worker from its executable, arguments, and env contract.
    pub async fn start_with(
        worker_id: String,
        service_name: String,
        executable_path: &Path,
        args: &[String],
        env: &[(String, String)],
    ) -> Result<Arc<Self>> {
        if one_shot_worker_mode(env) {
            return Ok(Arc::new(Self::one_shot(
                worker_id,
                service_name,
                executable_path,
                args,
                env,
            )));
        }
        match Self::spawn_persistent(&worker_id, &service_name, executable_path, args, env).await {
            Ok(worker) => Ok(Arc::new(worker)),
            Err(err) => {
                warn!(
                    service_name,
                    error = %err,
                    "persistent worker mode unavailable, falling back to one-shot mode"
                );
                Ok(Arc::new(Self::one_shot(
                    worker_id,
                    service_name,
                    executable_path,
                    args,
                    env,
                )))
            }
        }
    }

    fn one_shot(
        worker_id: String,
        service_name: String,
        executable_path: &Path,
        args: &[String],
        env: &[(String, String)],
    ) -> Self {
        Self {
            worker_id,
            service_name,
            mode: WorkerMode::OneShot,
            executable_path: executable_path.to_path_buf(),
            spawn_args: args.to_vec(),
            spawn_env: env.to_vec(),
            child: Mutex::new(None),
            process_scope: Mutex::new(None),
            one_shot_process_scope: Mutex::new(None),
            one_shot_cancelled: AtomicBool::new(false),
            one_shot_cancel: Notify::new(),
            stdin: Mutex::new(None),
            stdout: Mutex::new(None),
            round_trip: Mutex::new(()),
        }
    }

    async fn spawn_persistent(
        worker_id: &str,
        service_name: &str,
        executable_path: &Path,
        args: &[String],
        env: &[(String, String)],
    ) -> Result<Self> {
        let mut command = Command::new(executable_path);
        command
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped());
        command.env_remove("TURA_CLI_LIVE_JSONL");
        command.env_remove("TURA_CLI_PROGRESS");
        configure_worker_stderr(&mut command, worker_id, service_name, env);
        configure_scoped_spawn(&mut command);
        for (key, value) in env {
            command.env(key, value);
        }
        if debug_enabled(env) {
            eprintln!(
                "router debug: spawning worker service={} executable={} args={:?}",
                service_name,
                executable_path.display(),
                args
            );
        }
        let mut child = command.spawn().with_context(|| {
            format!(
                "failed to spawn worker executable: {}",
                executable_path.display()
            )
        })?;
        let process_scope = attach_child_scope(&child)
            .inspect_err(|error| {
                warn!(
                    worker_id,
                    service_name,
                    error = %error,
                    "failed to attach worker process scope; direct child cleanup remains active"
                );
            })
            .ok()
            .flatten();

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| anyhow!("worker stdin missing"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| anyhow!("worker stdout missing"))?;
        let mut reader = BufReader::new(stdout);

        let mut stdin_for_probe = stdin;
        let probe_result = async {
            let health_req = WorkerEnvelope {
                kind: "health_check".to_string(),
                payload: json!({}),
            };
            let payload = format!("{}\n", serde_json::to_string(&health_req)?);
            stdin_for_probe.write_all(payload.as_bytes()).await?;
            stdin_for_probe.flush().await?;

            let line = read_worker_json_response_line(
                &mut reader,
                Some(WORKER_HEALTH_TIMEOUT),
                "health check",
                None,
                None,
            )
            .await?;
            if line.trim().is_empty() {
                return Err(anyhow!("worker health check returned empty response"));
            }

            let parsed: Value = serde_json::from_str(line.trim())?;
            if parsed.get("ok").is_none() {
                warn!(
                    worker_id = worker_id,
                    service_name = service_name,
                    response = %line.trim(),
                    "worker health check returned no ok flag"
                );
                return Err(anyhow!("worker health check failed"));
            }

            // Version handshake (codex-style): refuse a worker built from a
            // different version than this router. A worker that publishes no
            // version (older build) is tolerated during the transition.
            if let Some(worker_version) = parsed.get("version").and_then(Value::as_str) {
                let expected = tura_path::instance_version();
                if worker_version != expected {
                    warn!(
                        worker_id,
                        service_name, worker_version, %expected, "worker version mismatch; refusing"
                    );
                    return Err(anyhow!(
                        "runtime worker version {worker_version} does not match router {expected}; \
                         refusing to dispatch to a different build"
                    ));
                }
            }

            Ok::<(), anyhow::Error>(())
        }
        .await;
        if let Err(error) = probe_result {
            if debug_enabled(env) {
                eprintln!(
                    "router debug: worker health failed service={service_name} error={error}"
                );
            }
            if let Some(scope) = process_scope.as_ref() {
                scope.terminate();
            }
            let _ = child.kill().await;
            let _ = child.wait().await;
            return Err(error);
        }
        if debug_enabled(env) {
            eprintln!(
                "router debug: worker health ok service={service_name} worker_id={worker_id}"
            );
        }

        info!(worker_id, service_name, "persistent worker started");

        Ok(Self {
            worker_id: worker_id.to_string(),
            service_name: service_name.to_string(),
            mode: WorkerMode::Persistent,
            executable_path: executable_path.to_path_buf(),
            spawn_args: args.to_vec(),
            spawn_env: env.to_vec(),
            child: Mutex::new(Some(child)),
            process_scope: Mutex::new(process_scope),
            one_shot_process_scope: Mutex::new(None),
            one_shot_cancelled: AtomicBool::new(false),
            one_shot_cancel: Notify::new(),
            stdin: Mutex::new(Some(stdin_for_probe)),
            stdout: Mutex::new(Some(reader)),
            round_trip: Mutex::new(()),
        })
    }

    pub async fn is_alive(&self) -> bool {
        match self.mode {
            WorkerMode::OneShot => true,
            WorkerMode::Persistent => {
                let mut guard = self.child.lock().await;
                if let Some(child) = guard.as_mut() {
                    match child.try_wait() {
                        Ok(Some(status)) => {
                            warn!(
                                worker_id = self.worker_id,
                                ?status,
                                "worker exited unexpectedly"
                            );
                            false
                        }
                        Ok(None) => true,
                        Err(_) => false,
                    }
                } else {
                    false
                }
            }
        }
    }

    #[allow(dead_code)]
    pub async fn invoke(&self, ctx: CallContext) -> Result<Value> {
        self.invoke_with_notifications(ctx, None).await
    }

    pub async fn invoke_with_notifications(
        &self,
        ctx: CallContext,
        notifications: Option<ipc::IpcNotificationSender>,
    ) -> Result<Value> {
        match self.mode {
            WorkerMode::Persistent => self.invoke_persistent(ctx, notifications).await,
            WorkerMode::OneShot => self.invoke_one_shot(ctx, notifications).await,
        }
    }

    pub async fn stop(&self) {
        if matches!(self.mode, WorkerMode::Persistent) {
            let mut child = self.child.lock().await;
            if let Some(mut child) = child.take() {
                if let Some(scope) = self.process_scope.lock().await.as_ref() {
                    scope.terminate();
                }
                let _ = child.kill().await;
                let _ = child.wait().await;
            }
            self.process_scope.lock().await.take();
        } else {
            self.one_shot_cancelled.store(true, Ordering::SeqCst);
            self.one_shot_cancel.notify_waiters();
            if let Some(scope) = self.one_shot_process_scope.lock().await.as_ref() {
                scope.terminate();
            }
        }
        self.stdin.lock().await.take();
        self.stdout.lock().await.take();
    }

    async fn invoke_persistent(
        &self,
        ctx: CallContext,
        notifications: Option<ipc::IpcNotificationSender>,
    ) -> Result<Value> {
        let _round_trip = self.round_trip.lock().await;
        let envelope = WorkerEnvelope {
            kind: "call".to_string(),
            payload: json!({
                "input": {
                    "request_id": ctx.request_id,
                    "method": ctx.method,
                    "path": ctx.path,
                    "input": ctx.input
                }
            }),
        };
        let line = format!("{}\n", serde_json::to_string(&envelope)?);

        {
            let mut stdin_guard = self.stdin.lock().await;
            let stdin = stdin_guard
                .as_mut()
                .ok_or_else(|| anyhow!("persistent worker stdin unavailable"))?;
            if let Err(error) = stdin.write_all(line.as_bytes()).await {
                drop(stdin_guard);
                self.stop().await;
                return Err(anyhow!(
                    "persistent worker write failed; worker stopped: {error}"
                ));
            }
            if let Err(error) = stdin.flush().await {
                drop(stdin_guard);
                self.stop().await;
                return Err(anyhow!(
                    "persistent worker flush failed; worker stopped: {error}"
                ));
            }
        }
        if process_debug_enabled() {
            eprintln!(
                "router debug: worker request sent service={} worker_id={}",
                self.service_name, self.worker_id
            );
        }

        let response_line = {
            let mut stdout_guard = self.stdout.lock().await;
            let stdout = stdout_guard
                .as_mut()
                .ok_or_else(|| anyhow!("persistent worker stdout unavailable"))?;
            read_worker_json_response_line(
                stdout,
                None,
                "invocation",
                Some(ctx.request_id.as_str()),
                notifications.as_ref(),
            )
            .await
        };
        let response_line = match response_line {
            Ok(line) => line,
            Err(error) => {
                warn!(
                    worker_id = self.worker_id,
                    service_name = self.service_name,
                    error = %error,
                    "persistent worker invocation failed; stopping worker before reuse"
                );
                self.stop().await;
                return Err(anyhow!(
                    "persistent worker invocation failed; worker stopped: {error}"
                ));
            }
        };
        if process_debug_enabled() {
            eprintln!(
                "router debug: worker response received service={} worker_id={} bytes={}",
                self.service_name,
                self.worker_id,
                response_line.len()
            );
        }

        if response_line.trim().is_empty() {
            warn!(
                worker_id = self.worker_id,
                service_name = self.service_name,
                "persistent worker returned empty response"
            );
            self.stop().await;
            return Err(anyhow!("worker returned empty response"));
        }

        match serde_json::from_str(response_line.trim()) {
            Ok(v) => Ok(v),
            Err(err) => {
                error!(
                    worker_id = self.worker_id,
                    service_name = self.service_name,
                    response = %response_line.trim(),
                    error = %err,
                    "persistent worker returned invalid json"
                );
                self.stop().await;
                Err(anyhow!("worker returned invalid response"))
            }
        }
    }

    async fn invoke_one_shot(
        &self,
        ctx: CallContext,
        notifications: Option<ipc::IpcNotificationSender>,
    ) -> Result<Value> {
        self.one_shot_cancelled.store(false, Ordering::SeqCst);
        let mut command = Command::new(&self.executable_path);
        command
            .args(&self.spawn_args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        configure_scoped_spawn(&mut command);
        for (key, value) in &self.spawn_env {
            command.env(key, value);
        }
        let child = command.spawn().with_context(|| {
            format!(
                "failed to spawn one-shot executable: {}",
                self.executable_path.display()
            )
        })?;
        let process_scope = attach_child_scope(&child).inspect_err(|error| {
            warn!(
                worker_id = self.worker_id,
                service_name = self.service_name,
                error = %error,
                "failed to attach one-shot worker process scope; direct child cleanup remains active"
            );
        }).ok().flatten();

        let input = one_shot_input_bytes(&ctx, &self.spawn_env)?;
        let mut child = child;
        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(&input).await?;
            stdin.write_all(b"\n").await?;
            stdin.flush().await?;
        }
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| anyhow!("one-shot worker stdout missing"))?;
        let mut stderr = child
            .stderr
            .take()
            .ok_or_else(|| anyhow!("one-shot worker stderr missing"))?;
        let request_id = ctx.request_id.clone();
        let stdout_notifications = notifications.clone();
        let stdout_task = tokio::spawn(async move {
            read_one_shot_worker_stdout(stdout, request_id, stdout_notifications).await
        });
        let stderr_task = tokio::spawn(async move {
            let mut bytes = Vec::new();
            stderr.read_to_end(&mut bytes).await.map(|_| bytes)
        });

        {
            let mut active_scope = self.one_shot_process_scope.lock().await;
            *active_scope = process_scope;
        }
        let wait_result = if self.one_shot_cancelled.load(Ordering::SeqCst) {
            terminate_one_shot_child(
                &mut child,
                self.one_shot_process_scope.lock().await.as_ref(),
            )
            .await;
            Err(anyhow!("one-shot worker cancelled"))
        } else {
            tokio::select! {
                status = child.wait() => status.map_err(Into::into),
                _ = self.one_shot_cancel.notified() => {
                    terminate_one_shot_child(
                        &mut child,
                        self.one_shot_process_scope.lock().await.as_ref(),
                    ).await;
                    Err(anyhow!("one-shot worker cancelled"))
                }
            }
        };
        self.one_shot_process_scope.lock().await.take();
        self.one_shot_cancelled.store(false, Ordering::SeqCst);
        let status = wait_result?;
        let (stdout, parsed_stdout) = stdout_task
            .await
            .map_err(|err| anyhow!("failed to join one-shot stdout reader: {err}"))??;
        let stderr = String::from_utf8_lossy(
            &stderr_task
                .await
                .map_err(|err| anyhow!("failed to join one-shot stderr reader: {err}"))??,
        )
        .to_string();
        append_one_shot_worker_stderr_log(
            &self.worker_id,
            &self.service_name,
            &self.spawn_env,
            &stderr,
        );

        if !stdout.trim().is_empty() {
            info!(
                worker_id = self.worker_id,
                service_name = self.service_name,
                stdout = %stdout.trim(),
                "one-shot worker stdout"
            );
        }

        if !stderr.trim().is_empty() {
            warn!(
                worker_id = self.worker_id,
                service_name = self.service_name,
                stderr = %stderr.trim(),
                "one-shot worker stderr"
            );
        }

        if !status.success() {
            warn!(
                worker_id = self.worker_id,
                service_name = self.service_name,
                exit_code = status.code().unwrap_or(-1),
                "one-shot worker exited with failure"
            );
            return Err(anyhow!("worker execution failed"));
        }

        match parsed_stdout.or_else(|| {
            parse_one_shot_worker_stdout(&stdout, one_shot_envelope_protocol(&self.spawn_env)).ok()
        }) {
            Some(v) => Ok(v),
            None => {
                error!(
                    worker_id = self.worker_id,
                    service_name = self.service_name,
                    stdout = %stdout.trim(),
                    "one-shot worker returned invalid json"
                );
                Err(anyhow!("worker returned invalid response"))
            }
        }
    }
}

fn append_one_shot_worker_stderr_log(
    worker_id: &str,
    service_name: &str,
    env: &[(String, String)],
    stderr: &str,
) {
    if stderr.is_empty() {
        return;
    }
    let Some(path) = worker_stderr_log_path(worker_id, service_name, env) else {
        return;
    };
    if let Some(parent) = path.parent() {
        if let Err(error) = std::fs::create_dir_all(parent) {
            warn!(
                path = %path.display(),
                error = %error,
                "failed to create one-shot worker stderr log directory"
            );
            return;
        }
    }
    match OpenOptions::new().create(true).append(true).open(&path) {
        Ok(mut file) => {
            if let Err(error) = file.write_all(stderr.as_bytes()) {
                warn!(
                    path = %path.display(),
                    error = %error,
                    "failed to append one-shot worker stderr log"
                );
            }
        }
        Err(error) => warn!(
            path = %path.display(),
            error = %error,
            "failed to open one-shot worker stderr log"
        ),
    }
}

fn one_shot_input_bytes(ctx: &CallContext, env: &[(String, String)]) -> Result<Vec<u8>> {
    if one_shot_envelope_protocol(env) {
        let envelope = WorkerEnvelope {
            kind: "call".to_string(),
            payload: json!({
                "input": {
                    "request_id": ctx.request_id,
                    "method": ctx.method,
                    "path": ctx.path,
                    "input": ctx.input
                }
            }),
        };
        serde_json::to_vec(&envelope).map_err(Into::into)
    } else {
        serde_json::to_vec(&ctx.input).map_err(Into::into)
    }
}

fn parse_one_shot_worker_stdout(stdout: &str, allow_protocol_lines: bool) -> Result<Value> {
    if !allow_protocol_lines {
        return serde_json::from_str::<Value>(stdout.trim()).map_err(Into::into);
    }
    stdout
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .find_map(|line| {
            let value = serde_json::from_str::<Value>(line).ok()?;
            (value.get("kind").and_then(Value::as_str) != Some("gateway.callback")).then_some(value)
        })
        .ok_or_else(|| anyhow!("one-shot worker returned no json response"))
}

async fn read_one_shot_worker_stdout(
    stdout: ChildStdout,
    request_id: String,
    notifications: Option<ipc::IpcNotificationSender>,
) -> Result<(String, Option<Value>)> {
    let mut reader = BufReader::new(stdout);
    let mut raw = String::new();
    let mut parsed_response = None;
    loop {
        let mut line = String::new();
        let bytes_read = reader.read_line(&mut line).await?;
        if bytes_read == 0 {
            break;
        }
        raw.push_str(&line);
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        match serde_json::from_str::<Value>(trimmed) {
            Ok(value) => {
                if emit_worker_notification(
                    &value,
                    Some(request_id.as_str()),
                    notifications.as_ref(),
                ) {
                    continue;
                }
                if parsed_response.is_none() {
                    parsed_response = Some(value);
                } else {
                    warn!(
                        request_id,
                        line = %trimmed,
                        "ignoring extra worker JSON response line"
                    );
                }
            }
            Err(_) => {
                warn!(
                    request_id,
                    line = %trimmed,
                    "skipping non-protocol one-shot worker stdout line"
                );
            }
        }
    }
    Ok((raw, parsed_response))
}

async fn terminate_one_shot_child(child: &mut Child, scope: Option<&WorkerProcessScope>) {
    if let Some(scope) = scope {
        scope.terminate();
    }
    let _ = child.start_kill();
    let _ = child.wait().await;
}

fn one_shot_worker_mode(env: &[(String, String)]) -> bool {
    env_value(env, "TURA_WORKER_MODE").is_some_and(|value| {
        matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "one-shot" | "oneshot"
        )
    })
}

fn one_shot_envelope_protocol(env: &[(String, String)]) -> bool {
    env_value(env, "TURA_WORKER_ONESHOT_PROTOCOL").is_some_and(|value| {
        matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "envelope" | "worker-envelope" | "line-envelope"
        )
    })
}

async fn read_worker_json_response_line<R>(
    reader: &mut R,
    duration: Option<Duration>,
    operation: &str,
    request_id: Option<&str>,
    notifications: Option<&ipc::IpcNotificationSender>,
) -> Result<String>
where
    R: AsyncBufRead + Unpin,
{
    let deadline = duration.map(|duration| Instant::now() + duration);
    let mut skipped = 0usize;
    loop {
        let mut line = String::new();
        let bytes_read = if let Some(deadline) = deadline {
            let now = Instant::now();
            if now >= deadline {
                let duration = duration.expect("deadline exists only when duration exists");
                return Err(anyhow!(
                    "worker {operation} timed out after {}s",
                    duration.as_secs()
                ));
            }
            let duration = duration.expect("deadline exists only when duration exists");
            timeout(deadline - now, reader.read_line(&mut line))
                .await
                .map_err(|_| {
                    anyhow!("worker {operation} timed out after {}s", duration.as_secs())
                })??
        } else {
            reader.read_line(&mut line).await?
        };
        if bytes_read == 0 {
            return Err(anyhow!("worker {operation} closed stdout before response"));
        }
        if line.trim().is_empty() {
            skipped = skipped.saturating_add(1);
            if skipped >= 16 {
                return Err(anyhow!(
                    "worker {operation} produced too many non-protocol stdout lines"
                ));
            }
            continue;
        }
        let trimmed = line.trim();
        if let Ok(value) = serde_json::from_str::<Value>(trimmed) {
            if emit_worker_notification(&value, request_id, notifications) {
                continue;
            }
            return Ok(line);
        }
        if trimmed.starts_with('{') || trimmed.starts_with('[') {
            return Ok(line);
        }
        skipped = skipped.saturating_add(1);
        warn!(
            operation,
            skipped,
            line = %line.trim(),
            "skipping non-protocol worker stdout line"
        );
        if skipped >= 16 {
            return Err(anyhow!(
                "worker {operation} produced too many non-protocol stdout lines"
            ));
        }
    }
}

fn emit_worker_notification(
    value: &Value,
    request_id: Option<&str>,
    notifications: Option<&ipc::IpcNotificationSender>,
) -> bool {
    let Some(kind) = value.get("kind").and_then(Value::as_str) else {
        return false;
    };
    if kind != "gateway.callback" {
        return false;
    }
    let method = value
        .get("method")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let payload = value.get("payload").cloned().unwrap_or(Value::Null);
    let Some(request_id) = request_id.filter(|value| !value.trim().is_empty()) else {
        warn!(
            method,
            "dropping worker gateway callback without request id"
        );
        return true;
    };
    let Some(notifications) = notifications else {
        warn!(
            request_id,
            method, "dropping worker gateway callback without notification sink"
        );
        return true;
    };
    if let Err(error) = notifications.send(ipc::IpcNotification::new(
        request_id.to_string(),
        kind.to_string(),
        method.to_string(),
        payload,
    )) {
        warn!(
            request_id,
            method,
            error = %error,
            "failed to forward worker gateway callback"
        );
    }
    true
}

fn configure_worker_stderr(
    command: &mut Command,
    worker_id: &str,
    service_name: &str,
    env: &[(String, String)],
) {
    let Some(path) = worker_stderr_log_path(worker_id, service_name, env) else {
        command.stderr(Stdio::null());
        return;
    };
    if let Some(parent) = path.parent() {
        if let Err(error) = std::fs::create_dir_all(parent) {
            warn!(
                path = %path.display(),
                error = %error,
                "failed to create worker stderr log directory"
            );
        }
    }
    match OpenOptions::new().create(true).append(true).open(&path) {
        Ok(file) => {
            command.stderr(Stdio::from(file));
        }
        Err(error) => {
            warn!(
                path = %path.display(),
                error = %error,
                "failed to open worker stderr log"
            );
            command.stderr(Stdio::null());
        }
    }
}

fn worker_stderr_log_path(
    worker_id: &str,
    service_name: &str,
    env: &[(String, String)],
) -> Option<std::path::PathBuf> {
    if let Some(path) = env_value(env, "TURA_RUNTIME_WORKER_STDERR_LOG")
        .map(std::path::PathBuf::from)
        .or_else(|| std::env::var_os("TURA_RUNTIME_WORKER_STDERR_LOG").map(Into::into))
    {
        return Some(path);
    }
    let debug_enabled = env_value(env, "TURA_DEBUG_RUNTIME").is_some_and(env_flag)
        || std::env::var("TURA_DEBUG_RUNTIME")
            .ok()
            .is_some_and(|value| env_flag(&value));
    if !debug_enabled {
        return None;
    }
    let name = format!(
        "{}-{}.stderr.log",
        sanitize_log_component(service_name),
        sanitize_log_component(worker_id)
    );
    Some(session_log::path::default_db_dir().join(name))
}

fn env_value<'a>(env: &'a [(String, String)], key: &str) -> Option<&'a str> {
    env.iter()
        .find(|(candidate, _)| candidate == key)
        .map(|(_, value)| value.as_str())
}

fn env_flag(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    )
}

fn sanitize_log_component(value: &str) -> String {
    let mut sanitized = String::with_capacity(value.len());
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.') {
            sanitized.push(ch);
        } else {
            sanitized.push('_');
        }
    }
    if sanitized.is_empty() {
        "worker".to_string()
    } else {
        sanitized
    }
}

fn debug_enabled(env: &[(String, String)]) -> bool {
    env_value(env, "TURA_DEBUG_RUNTIME").is_some_and(env_flag) || process_debug_enabled()
}

fn process_debug_enabled() -> bool {
    std::env::var("TURA_DEBUG_RUNTIME")
        .ok()
        .is_some_and(|value| env_flag(&value))
}

#[cfg(test)]
mod tests {
    use super::super::models::CallContext;
    use super::{
        env_flag, env_value, one_shot_input_bytes, one_shot_worker_mode,
        parse_one_shot_worker_stdout, sanitize_log_component, worker_stderr_log_path, WorkerMode,
        WorkerProcess,
    };
    use serde_json::json;
    use std::path::PathBuf;
    use std::sync::atomic::AtomicBool;
    use tokio::sync::Notify;
    use tokio::{io::AsyncWriteExt, sync::Mutex};

    #[tokio::test]
    async fn missing_persistent_worker_falls_back_to_one_shot_and_preserves_spec() {
        let executable = PathBuf::from("definitely-missing-runtime-worker-for-test");
        let args = vec!["--serve".to_string(), "--jsonl".to_string()];
        let env = vec![("TURA_DEBUG_RUNTIME".to_string(), "0".to_string())];

        let worker = WorkerProcess::start_with(
            "worker-id".to_string(),
            "runtime".to_string(),
            &executable,
            &args,
            &env,
        )
        .await
        .expect("missing persistent worker should fall back to one-shot mode");

        assert!(matches!(worker.mode, WorkerMode::OneShot));
        assert_eq!(worker.worker_id, "worker-id");
        assert_eq!(worker.service_name, "runtime");
        assert_eq!(worker.executable_path, executable);
        assert_eq!(worker.spawn_args, args);
        assert_eq!(worker.spawn_env, env);
        assert!(worker.is_alive().await);
    }

    #[tokio::test]
    async fn explicit_one_shot_worker_mode_skips_persistent_health_probe() {
        let executable = PathBuf::from("definitely-missing-explicit-one-shot-worker-for-test");
        let env = vec![("TURA_WORKER_MODE".to_string(), "one-shot".to_string())];

        let worker = WorkerProcess::start_with(
            "worker-id".to_string(),
            "runtime_worker".to_string(),
            &executable,
            &[],
            &env,
        )
        .await
        .expect("explicit one-shot mode should not spawn during ensure");

        assert!(matches!(worker.mode, WorkerMode::OneShot));
        assert_eq!(worker.executable_path, executable);
        assert_eq!(worker.spawn_env, env);
    }

    #[test]
    fn worker_stderr_log_path_prefers_explicit_env_path() {
        let explicit = PathBuf::from("target/test-worker.stderr.log");
        let env = vec![(
            "TURA_RUNTIME_WORKER_STDERR_LOG".to_string(),
            explicit.display().to_string(),
        )];

        assert_eq!(
            worker_stderr_log_path("worker", "runtime", &env),
            Some(explicit)
        );
    }

    #[test]
    fn worker_stderr_log_path_sanitizes_debug_default_filename() {
        let env = vec![("TURA_DEBUG_RUNTIME".to_string(), "true".to_string())];
        let path = worker_stderr_log_path("worker/one", "runtime service", &env)
            .expect("debug stderr path should be created");

        assert!(
            path.ends_with("runtime_service-worker_one.stderr.log"),
            "unexpected debug stderr log path: {}",
            path.display()
        );
    }

    #[test]
    fn env_helpers_parse_flags_and_exact_keys() {
        let env = vec![
            ("TURA_DEBUG_RUNTIME".to_string(), "yes".to_string()),
            ("OTHER".to_string(), "1".to_string()),
        ];

        assert_eq!(env_value(&env, "TURA_DEBUG_RUNTIME"), Some("yes"));
        assert_eq!(env_value(&env, "MISSING"), None);
        assert!(env_flag("ON"));
        assert!(env_flag(" true "));
        assert!(!env_flag("disabled"));
    }

    #[test]
    fn one_shot_mode_and_envelope_protocol_are_env_driven() {
        let env = vec![
            ("TURA_WORKER_MODE".to_string(), "oneshot".to_string()),
            (
                "TURA_WORKER_ONESHOT_PROTOCOL".to_string(),
                "envelope".to_string(),
            ),
        ];
        let ctx = CallContext {
            request_id: "request-1".to_string(),
            method: "POST".to_string(),
            path: "/runtime_worker/session".to_string(),
            input: json!({ "session_id": "session", "prompt": "hello" }),
        };

        assert!(one_shot_worker_mode(&env));
        let bytes = one_shot_input_bytes(&ctx, &env).expect("input bytes");
        let value: serde_json::Value =
            serde_json::from_slice(&bytes).expect("envelope should be json");
        assert_eq!(value["kind"], "call");
        assert_eq!(value["payload"]["input"]["request_id"], "request-1");
        assert_eq!(value["payload"]["input"]["input"]["prompt"], "hello");
    }

    #[test]
    fn one_shot_envelope_stdout_parser_ignores_noise_lines() {
        let parsed =
            parse_one_shot_worker_stdout("debug before json\n{\"ok\":true,\"value\":42}\n", true)
                .expect("json line should be parsed");

        assert_eq!(parsed["ok"], true);
        assert_eq!(parsed["value"], 42);
    }

    #[test]
    fn log_component_sanitization_keeps_stable_ascii_names() {
        assert_eq!(
            sanitize_log_component("runtime/service 1"),
            "runtime_service_1"
        );
        assert_eq!(sanitize_log_component(""), "worker");
    }

    #[tokio::test]
    async fn one_shot_invoke_reports_spawn_failure_with_executable_path() {
        let executable = PathBuf::from("definitely-missing-one-shot-worker-for-test");
        let worker = WorkerProcess {
            worker_id: "worker-one-shot".to_string(),
            service_name: "runtime".to_string(),
            mode: WorkerMode::OneShot,
            executable_path: executable.clone(),
            spawn_args: vec!["--serve".to_string()],
            spawn_env: Vec::new(),
            child: Mutex::new(None),
            process_scope: Mutex::new(None),
            one_shot_process_scope: Mutex::new(None),
            one_shot_cancelled: AtomicBool::new(false),
            one_shot_cancel: Notify::new(),
            stdin: Mutex::new(None),
            stdout: Mutex::new(None),
            round_trip: Mutex::new(()),
        };

        let error = worker
            .invoke(CallContext {
                request_id: "request-one-shot".to_string(),
                method: "run".to_string(),
                path: "/runtime".to_string(),
                input: json!({ "prompt": "hello" }),
            })
            .await
            .expect_err("missing one-shot executable should fail");

        let text = error.to_string();
        assert!(
            text.contains("failed to spawn one-shot executable"),
            "spawn failure should include operation context: {text}"
        );
        assert!(
            text.contains(executable.to_string_lossy().as_ref()),
            "spawn failure should include executable path: {text}"
        );
    }

    #[tokio::test]
    async fn worker_json_response_reader_skips_bounded_stdout_noise() {
        let (mut writer, reader) = tokio::io::duplex(256);
        let mut reader = tokio::io::BufReader::new(reader);
        writer
            .write_all(b"library debug log\n{\"ok\":true,\"result\":42}\n")
            .await
            .expect("write mock worker stdout");
        drop(writer);

        let line = super::read_worker_json_response_line(&mut reader, None, "test", None, None)
            .await
            .expect("json line should be found without an invocation deadline");

        assert_eq!(line.trim(), r#"{"ok":true,"result":42}"#);
    }

    #[tokio::test]
    async fn worker_json_response_reader_can_use_startup_health_deadline() {
        let (mut writer, reader) = tokio::io::duplex(256);
        let mut reader = tokio::io::BufReader::new(reader);
        writer
            .write_all(b"library debug log\n{\"ok\":true,\"result\":42}\n")
            .await
            .expect("write mock worker stdout");
        drop(writer);

        let line = super::read_worker_json_response_line(
            &mut reader,
            Some(std::time::Duration::from_secs(1)),
            "test",
            None,
            None,
        )
        .await
        .expect("json line should be found after noise");

        assert_eq!(line.trim(), r#"{"ok":true,"result":42}"#);
    }

    #[tokio::test]
    async fn worker_json_response_reader_forwards_gateway_callback_before_response() {
        let (mut writer, reader) = tokio::io::duplex(1024);
        let mut reader = tokio::io::BufReader::new(reader);
        writer
            .write_all(
                br#"{"kind":"gateway.callback","method":"session.agent_stream","payload":{"session_id":"s1","body":{"delta":"hi"}}}
{"ok":true,"result":42}
"#,
            )
            .await
            .expect("write mock worker stdout");
        drop(writer);
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();

        let line = super::read_worker_json_response_line(
            &mut reader,
            Some(std::time::Duration::from_secs(1)),
            "test",
            Some("request-1"),
            Some(&tx),
        )
        .await
        .expect("json line should be found after callback");

        assert_eq!(line.trim(), r#"{"ok":true,"result":42}"#);
        let notification = rx.try_recv().expect("callback should be forwarded");
        assert_eq!(notification.request_id, "request-1");
        assert_eq!(notification.kind, "gateway.callback");
        assert_eq!(notification.method, "session.agent_stream");
        assert_eq!(notification.payload["session_id"], "s1");
    }

    #[tokio::test]
    async fn worker_json_response_reader_skips_blank_stdout_lines() {
        let (mut writer, reader) = tokio::io::duplex(256);
        let mut reader = tokio::io::BufReader::new(reader);
        writer
            .write_all(b"\n\r\n{\"ok\":true,\"result\":\"after-blank\"}\n")
            .await
            .expect("write mock worker stdout");
        drop(writer);

        let line = super::read_worker_json_response_line(
            &mut reader,
            Some(std::time::Duration::from_secs(1)),
            "test",
            None,
            None,
        )
        .await
        .expect("json line should be found after blank lines");

        assert_eq!(line.trim(), r#"{"ok":true,"result":"after-blank"}"#);
    }

    #[tokio::test]
    async fn worker_json_response_reader_reports_eof_before_response() {
        let (_writer, reader) = tokio::io::duplex(256);
        let mut reader = tokio::io::BufReader::new(reader);
        drop(_writer);

        let error = super::read_worker_json_response_line(
            &mut reader,
            Some(std::time::Duration::from_secs(1)),
            "test",
            None,
            None,
        )
        .await
        .expect_err("eof before json should reject the worker protocol");

        assert!(
            error.to_string().contains("closed stdout before response"),
            "unexpected error: {error:#}"
        );
    }

    #[tokio::test]
    async fn worker_json_response_reader_rejects_too_much_stdout_noise() {
        let (mut writer, reader) = tokio::io::duplex(512);
        let mut reader = tokio::io::BufReader::new(reader);
        for index in 0..17 {
            writer
                .write_all(format!("noise-{index}\n").as_bytes())
                .await
                .expect("write mock noise");
        }
        drop(writer);

        let error = super::read_worker_json_response_line(
            &mut reader,
            Some(std::time::Duration::from_secs(1)),
            "test",
            None,
            None,
        )
        .await
        .expect_err("excess stdout noise should reject the worker protocol");

        assert!(
            error
                .to_string()
                .contains("too many non-protocol stdout lines"),
            "unexpected error: {error:#}"
        );
    }
}
