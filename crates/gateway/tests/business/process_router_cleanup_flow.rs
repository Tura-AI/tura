use anyhow::{anyhow, Context, Result};
use axum::extract::{Json, Path, Query};
use axum::http::HeaderMap;
use gateway::api::session::{
    abort_session, create_session, CreateSessionRequest, SessionDirectoryParams,
};
use std::path::{Path as FsPath, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

static ENV_LOCK: Mutex<()> = Mutex::const_new(());

#[tokio::test]
async fn gateway_abort_session_stops_router_worker_without_workspace_process_scan() -> Result<()> {
    let _guard = ENV_LOCK.lock().await;
    let root = tempfile::tempdir().context("test root")?;
    let home = root.path().join("home");
    std::fs::create_dir_all(&home)?;
    let workspace = tempfile::tempdir().context("session workspace")?;
    let other_workspace = tempfile::tempdir().context("other workspace")?;
    let _env = EnvGuard::new(&home, workspace.path());
    let _router_cleanup = RouterCleanupGuard;

    let Json(session) = create_session(
        HeaderMap::new(),
        Query(SessionDirectoryParams {
            directory: Some(workspace.path().to_string_lossy().to_string()),
        }),
        Some(Json(CreateSessionRequest {
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
            task_management: None,
        })),
    )
    .await;

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
        matches!(cleanup.status.as_str(), "idle" | "cancelling" | "error"),
        "router cleanup status should be explicit: {cleanup:#?}"
    );
    assert!(
        scoped_child.is_running()?,
        "abort must not kill arbitrary processes just because their cwd is the session workspace"
    );
    assert!(
        unrelated_child.is_running()?,
        "abort must not kill unrelated workspace processes"
    );

    let Json(after_abort) = gateway::api::session::get_session(Path(session.id)).await;
    assert_eq!(
        serde_json::to_value(after_abort.status)?,
        serde_json::json!("idle")
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
    let mut stale_child = ChildGuard::spawn(workspace.path(), "startup-cleanup-target")?;

    let Json(session) = create_session(
        HeaderMap::new(),
        Query(SessionDirectoryParams {
            directory: Some(workspace.path().to_string_lossy().to_string()),
        }),
        Some(Json(CreateSessionRequest {
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
        })),
    )
    .await;

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
