use std::{path::Path, process::Stdio, sync::Arc, time::Duration};

use anyhow::{anyhow, Context, Result};
use serde_json::{json, Value};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    process::{Child, ChildStdin, ChildStdout, Command},
    sync::Mutex,
    time::timeout,
};
use tracing::{error, info, warn};

use super::models::{CallContext, WorkerEnvelope};

const WORKER_HEALTH_TIMEOUT: Duration = Duration::from_secs(10);
const DEFAULT_WORKER_INVOKE_TIMEOUT: Duration = Duration::from_secs(180);

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
    stdin: Mutex<Option<ChildStdin>>,
    stdout: Mutex<Option<BufReader<ChildStdout>>>,
}

impl WorkerProcess {
    /// 声明式启动：任意 worker（可执行 + 启动参数 + env 契约）。
    pub async fn start_with(
        worker_id: String,
        service_name: String,
        executable_path: &Path,
        args: &[String],
        env: &[(String, String)],
    ) -> Result<Arc<Self>> {
        match Self::spawn_persistent(&worker_id, &service_name, executable_path, args, env).await {
            Ok(worker) => Ok(Arc::new(worker)),
            Err(err) => {
                warn!(
                    service_name,
                    error = %err,
                    "persistent worker mode unavailable, falling back to one-shot mode"
                );
                Ok(Arc::new(Self {
                    worker_id,
                    service_name,
                    mode: WorkerMode::OneShot,
                    executable_path: executable_path.to_path_buf(),
                    spawn_args: args.to_vec(),
                    spawn_env: env.to_vec(),
                    child: Mutex::new(None),
                    stdin: Mutex::new(None),
                    stdout: Mutex::new(None),
                }))
            }
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
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        hide_child_window(&mut command);
        for (key, value) in env {
            command.env(key, value);
        }
        let mut child = command.spawn().with_context(|| {
            format!(
                "failed to spawn worker executable: {}",
                executable_path.display()
            )
        })?;

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| anyhow!("worker stdin missing"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| anyhow!("worker stdout missing"))?;
        let mut reader = BufReader::new(stdout);

        let health_req = WorkerEnvelope {
            kind: "health_check".to_string(),
            payload: json!({}),
        };
        let payload = format!("{}\n", serde_json::to_string(&health_req)?);
        let mut stdin_for_probe = stdin;
        stdin_for_probe.write_all(payload.as_bytes()).await?;
        stdin_for_probe.flush().await?;

        let mut line = String::new();
        timeout(WORKER_HEALTH_TIMEOUT, reader.read_line(&mut line))
            .await
            .map_err(|_| {
                anyhow!(
                    "worker health check timed out after {}s",
                    WORKER_HEALTH_TIMEOUT.as_secs()
                )
            })??;
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

        info!(worker_id, service_name, "persistent worker started");

        Ok(Self {
            worker_id: worker_id.to_string(),
            service_name: service_name.to_string(),
            mode: WorkerMode::Persistent,
            executable_path: executable_path.to_path_buf(),
            spawn_args: args.to_vec(),
            spawn_env: env.to_vec(),
            child: Mutex::new(Some(child)),
            stdin: Mutex::new(Some(stdin_for_probe)),
            stdout: Mutex::new(Some(reader)),
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

    pub async fn invoke(&self, ctx: CallContext) -> Result<Value> {
        match self.mode {
            WorkerMode::Persistent => self.invoke_persistent(ctx).await,
            WorkerMode::OneShot => self.invoke_one_shot(ctx).await,
        }
    }

    pub async fn stop(&self) {
        if matches!(self.mode, WorkerMode::Persistent) {
            let mut child = self.child.lock().await;
            if let Some(mut child) = child.take() {
                let _ = child.kill().await;
                let _ = child.wait().await;
            }
        }
        self.stdin.lock().await.take();
        self.stdout.lock().await.take();
    }

    async fn invoke_persistent(&self, ctx: CallContext) -> Result<Value> {
        let envelope = WorkerEnvelope {
            kind: "call".to_string(),
            payload: json!({
                "input": {
                    "method": ctx.method,
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
            stdin.write_all(line.as_bytes()).await?;
            stdin.flush().await?;
        }

        let response_line = {
            let mut response_line = String::new();
            let mut stdout_guard = self.stdout.lock().await;
            let stdout = stdout_guard
                .as_mut()
                .ok_or_else(|| anyhow!("persistent worker stdout unavailable"))?;
            let invoke_timeout = worker_invoke_timeout();
            timeout(invoke_timeout, stdout.read_line(&mut response_line))
                .await
                .map_err(|_| {
                    anyhow!(
                        "worker invocation timed out after {}s",
                        invoke_timeout.as_secs()
                    )
                })??;
            response_line
        };

        if response_line.trim().is_empty() {
            warn!(
                worker_id = self.worker_id,
                service_name = self.service_name,
                "persistent worker returned empty response"
            );
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
                Err(anyhow!("worker returned invalid response"))
            }
        }
    }

    async fn invoke_one_shot(&self, ctx: CallContext) -> Result<Value> {
        let mut command = Command::new(&self.executable_path);
        command
            .args(&self.spawn_args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        hide_child_window(&mut command);
        for (key, value) in &self.spawn_env {
            command.env(key, value);
        }
        let mut child = command.spawn().with_context(|| {
            format!(
                "failed to spawn one-shot executable: {}",
                self.executable_path.display()
            )
        })?;

        let input = serde_json::to_vec(&ctx.input)?;
        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(&input).await?;
            stdin.flush().await?;
        }

        let invoke_timeout = worker_invoke_timeout();
        let out = timeout(invoke_timeout, child.wait_with_output())
            .await
            .map_err(|_| {
                anyhow!(
                    "one-shot worker timed out after {}s",
                    invoke_timeout.as_secs()
                )
            })??;
        let stdout = String::from_utf8_lossy(&out.stdout).to_string();
        let stderr = String::from_utf8_lossy(&out.stderr).to_string();

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

        if !out.status.success() {
            warn!(
                worker_id = self.worker_id,
                service_name = self.service_name,
                exit_code = out.status.code().unwrap_or(-1),
                "one-shot worker exited with failure"
            );
            return Err(anyhow!("worker execution failed"));
        }

        match serde_json::from_str::<Value>(&stdout) {
            Ok(v) => Ok(v),
            Err(err) => {
                error!(
                    worker_id = self.worker_id,
                    service_name = self.service_name,
                    error = %err,
                    stdout = %stdout.trim(),
                    "one-shot worker returned invalid json"
                );
                Err(anyhow!("worker returned invalid response"))
            }
        }
    }
}

fn worker_invoke_timeout() -> Duration {
    std::env::var("TURA_WORKER_INVOKE_TIMEOUT_SECS")
        .ok()
        .and_then(|value| value.trim().parse::<u64>().ok())
        .filter(|seconds| *seconds > 0)
        .map(Duration::from_secs)
        .unwrap_or(DEFAULT_WORKER_INVOKE_TIMEOUT)
}

fn hide_child_window(command: &mut Command) {
    #[cfg(windows)]
    {
        #[allow(unused_imports)]
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
}
