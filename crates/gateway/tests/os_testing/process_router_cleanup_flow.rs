use anyhow::{anyhow, Context, Result};
use axum::extract::{Json, Path};
use gateway::api::session::{abort_session, create_session_value};
use gateway::contracts::{CreateSessionRequest, SessionDirectoryParams};
use lifecycle::SessionState;
use session_log::SessionLogStore;
use session_log_contract::SessionLogCommand;
use std::path::{Path as FsPath, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

static ENV_LOCK: Mutex<()> = Mutex::const_new(());

struct ServiceThread {
    handle: Option<std::thread::JoinHandle<Result<()>>>,
}

impl ServiceThread {
    fn start() -> Result<Self> {
        let store = SessionLogStore::open_default().context("open cleanup-flow session DB")?;
        let handle = std::thread::spawn(move || session_log::ipc::serve_blocking(store));
        let started = Instant::now();
        while started.elapsed() < Duration::from_secs(10) {
            if session_log_contract::client::service_is_running() {
                return Ok(Self {
                    handle: Some(handle),
                });
            }
            if handle.is_finished() {
                let result = handle
                    .join()
                    .map_err(|_| anyhow!("cleanup-flow session DB thread panicked"))?;
                result.context("cleanup-flow session DB exited before becoming reachable")?;
                return Err(anyhow!(
                    "cleanup-flow session DB exited before becoming reachable"
                ));
            }
            thread::sleep(Duration::from_millis(25));
        }
        Err(anyhow!(
            "cleanup-flow session DB did not become reachable within 10 seconds"
        ))
    }
}

impl Drop for ServiceThread {
    fn drop(&mut self) {
        let _ = session_log_contract::client::call_service(&SessionLogCommand::Shutdown);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

#[tokio::test]
async fn gateway_abort_session_stops_router_worker_without_workspace_process_scan() -> Result<()> {
    let _guard = ENV_LOCK.lock().await;
    let root = tempfile::tempdir().context("test root")?;
    let home = root.path().join("home");
    std::fs::create_dir_all(&home)?;
    let workspace = tempfile::tempdir().context("session workspace")?;
    let other_workspace = tempfile::tempdir().context("other workspace")?;
    let _env = EnvGuard::new(&home, workspace.path());
    let _service = ServiceThread::start()?;
    let _router_cleanup = RouterCleanupGuard;

    let session = create_session_value(
        SessionDirectoryParams {
            directory: Some(workspace.path().to_string_lossy().to_string()),
        },
        CreateSessionRequest {
            directory: None,
            model: Some("cleanup-model".to_string()),
            agent: Some("cleanup-agent".to_string()),
            session_type: Some("coding".to_string()),
            kill_processes_on_start: Some(false),
            validator_enabled: Some(false),
            force_planning: Some(false),
            model_variant: None,
            model_acceleration_enabled: Some(false),
            disable_permission_restrictions: Some(false),
            auto_session_name: Some(false),
            task_management: Some(serde_json::json!({
                "plan_summary": "abort all work",
                "tasks": [
                    {
                        "task_id": "doing-task",
                        "task_summary": "currently running work",
                        "status": "doing",
                        "start_condition": "session_idle"
                    },
                    {
                        "task_id": "todo-task",
                        "task_summary": "queued work must pause",
                        "status": "todo",
                        "start_condition": "scheduled_task",
                        "start_at": chrono::Utc::now().to_rfc3339()
                    },
                    {
                        "task_id": "done-task",
                        "task_summary": "completed work stays completed",
                        "status": "done",
                        "start_condition": "session_idle"
                    }
                ]
            })),
        },
        None,
    )
    .await
    .map_err(anyhow::Error::msg)?;

    let Json(before_abort) = gateway::api::session::get_session(Path(session.id.clone())).await;
    let mut scoped_child = ChildGuard::spawn(workspace.path(), "abort-target")?;
    let mut unrelated_child = ChildGuard::spawn(other_workspace.path(), "abort-unrelated")?;

    let Json(response) = abort_session(Path(session.id.clone())).await;
    assert!(response.aborted);
    assert_eq!(response.sessions, vec![session.id.clone()]);
    let cleanup = response
        .cleanup
        .expect("abort should report router cleanup");
    assert_eq!(cleanup.session_id, session.id);
    assert!(
        matches!(cleanup.status.as_str(), "stopped" | "error"),
        "router cleanup status should report force-stop result: {cleanup:#?}"
    );
    assert!(
        scoped_child.is_running()?,
        "abort must not kill arbitrary processes just because their cwd is the session workspace"
    );
    assert!(
        unrelated_child.is_running()?,
        "abort must not kill unrelated workspace processes"
    );

    assert!(
        gateway::session::session_store()
            .session_lifecycle_projection(&session.id)
            .is_some_and(|projection| {
                projection.cancelled && projection.state == SessionState::Cancelled
            }),
        "abort must persist canonical cancellation while stopping the runtime worker"
    );

    let Json(after_abort) = gateway::api::session::get_session(Path(session.id)).await;
    assert_eq!(
        serde_json::to_value(after_abort.status)?,
        serde_json::json!("error"),
        "cancelled lifecycle must project to the API error status"
    );
    assert_eq!(
        after_abort.task_management, before_abort.task_management,
        "abort must not rewrite task management; task updates are separate user-visible actions"
    );

    scoped_child.kill_and_wait()?;
    unrelated_child.kill_and_wait()?;
    Ok(())
}

#[tokio::test]
async fn gateway_create_session_records_kill_processes_on_start_without_workspace_scan(
) -> Result<()> {
    let _guard = ENV_LOCK.lock().await;
    let root = tempfile::tempdir().context("test root")?;
    let home = root.path().join("home");
    std::fs::create_dir_all(&home)?;
    let workspace = tempfile::tempdir().context("session workspace")?;
    let _env = EnvGuard::new(&home, workspace.path());
    let _service = ServiceThread::start()?;
    let mut stale_child = ChildGuard::spawn(workspace.path(), "startup-cleanup-target")?;

    let session = create_session_value(
        SessionDirectoryParams {
            directory: Some(workspace.path().to_string_lossy().to_string()),
        },
        CreateSessionRequest {
            directory: None,
            model: Some("startup-cleanup-model".to_string()),
            agent: Some("startup-cleanup-agent".to_string()),
            session_type: Some("coding".to_string()),
            kill_processes_on_start: Some(true),
            validator_enabled: Some(false),
            force_planning: Some(false),
            model_variant: None,
            model_acceleration_enabled: Some(false),
            disable_permission_restrictions: Some(false),
            auto_session_name: Some(false),
            task_management: None,
        },
        None,
    )
    .await
    .map_err(anyhow::Error::msg)?;

    assert_eq!(
        session.directory.as_deref(),
        Some(workspace.path().to_string_lossy().as_ref())
    );
    assert!(session.kill_processes_on_start);
    assert!(
        stale_child.is_running()?,
        "kill_processes_on_start must not run workspace-wide process scanning"
    );

    stale_child.kill_and_wait()?;
    Ok(())
}

struct EnvGuard {
    previous: Vec<(&'static str, Option<std::ffi::OsString>)>,
}

impl EnvGuard {
    fn new(home: &FsPath, workspace: &FsPath) -> Self {
        let keys = [
            "TURA_HOME",
            "SESSION_LOG_DB_ROOT",
            "TURA_DB_ROOT",
            "TURA_PROJECT_ROOT",
            "TURA_CWD",
            "TURA_ROUTER_IDLE_SHUTDOWN_SECS",
        ];
        let previous = keys
            .iter()
            .map(|key| (*key, std::env::var_os(key)))
            .collect::<Vec<_>>();
        std::env::set_var("TURA_HOME", home);
        std::env::remove_var("SESSION_LOG_DB_ROOT");
        std::env::remove_var("TURA_DB_ROOT");
        std::env::set_var("TURA_PROJECT_ROOT", repo_root());
        std::env::set_var("TURA_CWD", workspace);
        std::env::set_var("TURA_ROUTER_IDLE_SHUTDOWN_SECS", "2");
        Self { previous }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        for (key, value) in self.previous.drain(..) {
            match value {
                Some(value) => std::env::set_var(key, value),
                None => std::env::remove_var(key),
            }
        }
    }
}

struct RouterCleanupGuard;

impl Drop for RouterCleanupGuard {
    fn drop(&mut self) {
        let _ = gateway::router_client::RouterClient::global().shutdown();
    }
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(FsPath::parent)
        .map(FsPath::to_path_buf)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")))
}

struct ChildGuard {
    child: Option<Child>,
}

impl ChildGuard {
    fn spawn(workspace: &FsPath, marker: &str) -> Result<Self> {
        let mut command = if cfg!(windows) {
            let mut command = Command::new("powershell");
            command.args([
                "-NoProfile",
                "-ExecutionPolicy",
                "Bypass",
                "-Command",
                &format!("Set-Content -Path {marker}.txt -Value $PID; Start-Sleep -Seconds 60"),
            ]);
            command
        } else {
            let mut command = Command::new("sh");
            command.args(["-c", &format!("echo $$ > {marker}.txt; sleep 60")]);
            command
        };
        let child = command
            .current_dir(workspace)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .with_context(|| format!("spawn child in {}", workspace.display()))?;
        wait_for_file(workspace.join(format!("{marker}.txt")).as_path())?;
        Ok(Self { child: Some(child) })
    }

    fn is_running(&mut self) -> Result<bool> {
        let child = self.child.as_mut().expect("child present");
        Ok(child.try_wait()?.is_none())
    }

    fn kill_and_wait(&mut self) -> Result<()> {
        if let Some(child) = self.child.as_mut() {
            if child.try_wait()?.is_none() {
                let _ = child.kill();
            }
            let _ = child.wait();
        }
        Ok(())
    }
}

impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.kill_and_wait();
    }
}

fn wait_for_file(path: &FsPath) -> Result<()> {
    wait_until(Duration::from_secs(8), || {
        if path.exists() {
            Ok(())
        } else {
            Err(anyhow!("{} not created yet", path.display()))
        }
    })
}

fn wait_until<T>(timeout: Duration, mut f: impl FnMut() -> Result<T>) -> Result<T> {
    let started = Instant::now();
    loop {
        match f() {
            Ok(value) => return Ok(value),
            Err(err) if started.elapsed() < timeout => {
                let _ = err;
                thread::sleep(Duration::from_millis(100));
            }
            Err(err) => return Err(err),
        }
    }
}
