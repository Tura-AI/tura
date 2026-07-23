//! Business E2E: dirty session_db file queue data must never prevent the owner
//! from starting. Malformed items are quarantined and clean writes still work.

#[path = "../support/typed_session.rs"]
mod typed_session;

use anyhow::{anyhow, bail, Context, Result};
use lifecycle::TaskPlan;
use session_log::SessionLogStore;
use session_log_contract::client::{enqueue_command, open_session_feed_subscription};
use session_log_contract::{
    GetSessionRequest, SessionFeedEvent, SessionLogCommand, SessionLogResponse,
};
use std::{
    ffi::OsString,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{mpsc, Mutex},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

static SERIAL: Mutex<()> = Mutex::new(());

#[test]
fn session_db_start_quarantines_dirty_file_queue_items_and_accepts_clean_writes() -> Result<()> {
    let _serial = SERIAL.lock().unwrap_or_else(|error| error.into_inner());
    let repo = repo_root()?;
    ensure_session_db_binary(&repo)?;

    let root = temp_root("session-db-dirty-queue")?;
    let home = root.join("home");
    let workspace = root.join("workspace");
    std::fs::create_dir_all(&home)?;
    std::fs::create_dir_all(&workspace)?;
    let workspace_key = session_log::path::normalize_workspace(&workspace.to_string_lossy());
    let _env = EnvGuard::set(&[
        ("TURA_HOME", Some(home.as_path())),
        ("TURA_PROJECT_ROOT", Some(repo.as_path())),
        ("TURA_DB_ROOT", None),
        ("SESSION_LOG_DB_ROOT", None),
    ]);

    let store = SessionLogStore::open_default()?;
    let dirty_file = write_dirty_file_queue_item(&home)?;

    let mut service = SessionDbGuard::start(&repo, &home, &workspace)?;
    wait_until(
        Duration::from_secs(30),
        session_log_contract::client::service_is_running,
    )
    .context("session_db did not publish a reachable socket")?;
    call_service_with_retry(&SessionLogCommand::ListWorkspaces, Duration::from_secs(30))
        .context("session_db did not become ready for data-path reads")?;

    wait_until(Duration::from_secs(10), || !dirty_file.exists())
        .context("dirty file queue item stayed pending")?;
    assert_failed_file_queue_contains(&home, &dirty_file)?;

    typed_session::create_via_service(
        "clean-direct",
        &workspace_key,
        "Dirty Queue clean-direct",
        10,
        TaskPlan::default(),
    )?;
    let mut subscription = open_session_feed_subscription()?;
    let cancellation = subscription.cancellation_handle()?;
    let (feed_sender, feed_receiver) = mpsc::channel();
    let feed_reader = thread::spawn(move || {
        while let Ok(Some(entry)) = subscription.next_entry() {
            if feed_sender.send(entry).is_err() {
                return;
            }
        }
    });
    enqueue_command(&SessionLogCommand::CreateSession(Box::new(
        typed_session::create_request(
            "clean-file-queue",
            &workspace_key,
            "Dirty Queue clean-file-queue",
            20,
            TaskPlan::default(),
        ),
    )))?;
    wait_until(Duration::from_secs(10), || {
        session_visible("clean-file-queue").unwrap_or(false)
    })
    .context("clean file queue write was not drained")?;
    let queued_feed = feed_receiver
        .recv_timeout(Duration::from_secs(10))
        .context("queued create did not reach the online session feed")?;
    assert_eq!(queued_feed.session_id, "clean-file-queue");
    assert!(matches!(
        queued_feed.event,
        SessionFeedEvent::SessionSnapshotCreated { .. }
    ));
    assert!(
        feed_receiver
            .recv_timeout(Duration::from_millis(200))
            .is_err(),
        "queued create should publish exactly one feed entry"
    );
    cancellation.cancel()?;
    feed_reader
        .join()
        .map_err(|_| anyhow!("session feed reader panicked"))?;

    assert!(session_visible("clean-direct")?);
    assert!(store
        .get_session(GetSessionRequest {
            session_id: "clean-direct".to_string(),
        })?
        .is_some());

    let _ = call_service_with_retry(&SessionLogCommand::Shutdown, Duration::from_secs(10));
    service.wait(Duration::from_secs(10))?;
    Ok(())
}

fn write_dirty_file_queue_item(home: &Path) -> Result<PathBuf> {
    let pending = home
        .join("db")
        .join("session_log")
        .join("message_queue")
        .join("pending");
    std::fs::create_dir_all(&pending)?;
    let path = pending.join("00000000000000000001-1-00000000000000000001.json");
    std::fs::write(&path, "{not-json")?;
    Ok(path)
}

fn assert_failed_file_queue_contains(home: &Path, dirty_file: &Path) -> Result<()> {
    let failed = home
        .join("db")
        .join("session_log")
        .join("message_queue")
        .join("failed");
    let file_name = dirty_file
        .file_name()
        .ok_or_else(|| anyhow!("dirty file missing name"))?;
    let failed_json = failed.join(file_name);
    let failed_error = failed_json.with_extension("error.txt");
    assert!(
        failed_json.exists(),
        "dirty file queue item should be retained in failed"
    );
    assert!(
        failed_error.exists(),
        "dirty file queue item should have an error sidecar"
    );
    Ok(())
}

fn session_visible(session_id: &str) -> Result<bool> {
    match session_log_contract::client::call_service(&SessionLogCommand::GetSession(
        GetSessionRequest {
            session_id: session_id.to_string(),
        },
    ))? {
        SessionLogResponse::Session { session } => Ok(session.is_some()),
        SessionLogResponse::Error { error } => bail!("get session returned error: {error}"),
        other => bail!("unexpected get session response: {other:?}"),
    }
}

fn call_service_with_retry(
    command: &SessionLogCommand,
    timeout: Duration,
) -> Result<SessionLogResponse> {
    let started = Instant::now();
    let mut last_error = None;
    while started.elapsed() < timeout {
        match session_log_contract::client::call_service(command) {
            Ok(response) => return Ok(response),
            Err(error) => {
                last_error = Some(error);
                thread::sleep(Duration::from_millis(50));
            }
        }
    }
    Err(last_error.unwrap_or_else(|| anyhow!("session_db service call did not run")))
}

struct SessionDbGuard {
    child: Option<Child>,
}

impl SessionDbGuard {
    fn start(repo: &Path, home: &Path, workspace: &Path) -> Result<Self> {
        let stdout = process_log_file(home, "session-db-dirty-queue.stdout.log")?;
        let stderr = process_log_file(home, "session-db-dirty-queue.stderr.log")?;
        let child = Command::new(session_db_bin(repo))
            .current_dir(workspace)
            .env("TURA_HOME", home)
            .env("TURA_PROJECT_ROOT", repo)
            .stdin(Stdio::null())
            .stdout(Stdio::from(stdout))
            .stderr(Stdio::from(stderr))
            .spawn()
            .context("spawn tura_session_db")?;
        Ok(Self { child: Some(child) })
    }

    fn wait(&mut self, timeout: Duration) -> Result<()> {
        let Some(child) = self.child.as_mut() else {
            return Ok(());
        };
        let started = Instant::now();
        while started.elapsed() < timeout {
            if child.try_wait()?.is_some() {
                self.child.take();
                return Ok(());
            }
            thread::sleep(Duration::from_millis(50));
        }
        bail!("session_db did not exit within {}ms", timeout.as_millis())
    }
}

impl Drop for SessionDbGuard {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

fn wait_until(timeout: Duration, mut condition: impl FnMut() -> bool) -> Result<()> {
    let started = Instant::now();
    while started.elapsed() < timeout {
        if condition() {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(50));
    }
    bail!("timed out after {}ms", timeout.as_millis())
}

fn ensure_session_db_binary(repo: &Path) -> Result<()> {
    if session_db_bin(repo).exists() {
        return Ok(());
    }
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    let status = Command::new(cargo)
        .current_dir(repo)
        .args(["build", "-p", "session_log", "--bin", "tura_session_db"])
        .status()
        .context("build session_log::tura_session_db")?;
    if !status.success() {
        bail!("cargo build -p session_log --bin tura_session_db failed with {status}");
    }
    Ok(())
}

fn session_db_bin(repo: &Path) -> PathBuf {
    std::env::var_os("CARGO_BIN_EXE_tura_session_db")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            target_dir(repo)
                .join("debug")
                .join(exe_name("tura_session_db"))
        })
}

fn target_dir(repo: &Path) -> PathBuf {
    std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| repo.join("target"))
}

fn exe_name(binary: &str) -> String {
    if cfg!(windows) {
        format!("{binary}.exe")
    } else {
        binary.to_string()
    }
}

fn process_log_file(home: &Path, name: &str) -> Result<std::fs::File> {
    let dir = home.join("logs");
    std::fs::create_dir_all(&dir)?;
    std::fs::File::create(dir.join(name)).map_err(Into::into)
}

fn repo_root() -> Result<PathBuf> {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .ok_or_else(|| anyhow!("session_log crate is not under workspace/crates"))
}

fn temp_root(prefix: &str) -> Result<PathBuf> {
    let path = std::env::temp_dir().join(format!(
        "{prefix}-{}-{}",
        std::process::id(),
        unique_nonce()?
    ));
    std::fs::create_dir_all(&path)?;
    Ok(path)
}

fn unique_nonce() -> Result<u128> {
    Ok(SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("system clock before UNIX_EPOCH")?
        .as_nanos())
}

struct EnvGuard {
    previous: Vec<(&'static str, Option<OsString>)>,
}

impl EnvGuard {
    fn set(values: &[(&'static str, Option<&Path>)]) -> Self {
        let previous = values
            .iter()
            .map(|(key, _)| (*key, std::env::var_os(key)))
            .collect::<Vec<_>>();
        for (key, value) in values {
            match value {
                Some(value) => std::env::set_var(key, value),
                None => std::env::remove_var(key),
            }
        }
        Self { previous }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        for (key, value) in self.previous.drain(..).rev() {
            match value {
                Some(value) => std::env::set_var(key, value),
                None => std::env::remove_var(key),
            }
        }
    }
}
