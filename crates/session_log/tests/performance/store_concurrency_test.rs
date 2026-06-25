use session_log::{ListSessionsRequest, SessionLogStore, UpsertSessionRequest};
use std::path::Path;
use std::process::{Child, Command, ExitStatus};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());
const TEST_TIMEOUT: Duration = Duration::from_secs(90);
const CHILD_TIMEOUT: Duration = Duration::from_secs(30);

struct EnvRestore {
    keys: Vec<(&'static str, Option<std::ffi::OsString>)>,
}

impl EnvRestore {
    fn capture(keys: &[&'static str]) -> Self {
        Self {
            keys: keys
                .iter()
                .map(|key| (*key, std::env::var_os(key)))
                .collect(),
        }
    }
}

impl Drop for EnvRestore {
    fn drop(&mut self) {
        for (key, value) in &self.keys {
            match value {
                Some(value) => std::env::set_var(key, value),
                None => std::env::remove_var(key),
            }
        }
    }
}

struct DirectDbGuard {
    _serial: std::sync::MutexGuard<'static, ()>,
    _env: EnvRestore,
    root: tempfile::TempDir,
    workspaces: tempfile::TempDir,
}

impl DirectDbGuard {
    fn new() -> Self {
        let serial = SERIAL.lock().unwrap_or_else(|error| error.into_inner());
        let env = EnvRestore::capture(&["SESSION_LOG_DB_ROOT", "TURA_DB_ROOT"]);
        let root = tempfile::tempdir().expect("tempdir");
        let workspaces = tempfile::tempdir().expect("workspace tempdir");
        std::env::set_var("SESSION_LOG_DB_ROOT", root.path());
        std::env::remove_var("TURA_DB_ROOT");
        Self {
            _serial: serial,
            _env: env,
            root,
            workspaces,
        }
    }

    fn root(&self) -> &Path {
        self.root.path()
    }

    fn workspace(&self, name: &str) -> String {
        let path = self.workspaces.path().join(name);
        std::fs::create_dir_all(&path).expect("workspace dir");
        path.to_string_lossy().replace('\\', "/")
    }
}

#[test]
fn concurrent_upserts_keep_pagination_consistent() {
    let test_started = Instant::now();
    let db = DirectDbGuard::new();
    let store = SessionLogStore::open_default().expect("store");
    let nonce = uuid::Uuid::new_v4().to_string();
    let workspace = db.workspace(&format!("stress-{nonce}"));
    let started = Instant::now();
    let worker_count = 16;
    let per_worker = 50;

    let mut workers = Vec::new();
    for worker in 0..worker_count {
        let store = store.clone();
        let workspace = workspace.clone();
        let nonce = nonce.clone();
        workers.push(std::thread::spawn(move || {
            for index in 0..per_worker {
                let sequence = worker * per_worker + index;
                let session_id = format!("stress-{nonce}-{sequence}");
                store
                    .upsert_session(upsert(&session_id, &workspace, sequence as i64))
                    .expect("upsert");
            }
        }));
    }

    for worker in workers {
        join_thread_with_timeout(
            worker,
            remaining_timeout(test_started, TEST_TIMEOUT, "concurrent upsert pressure"),
            "concurrent upsert worker",
        );
    }

    let (page, sessions) = store
        .list_sessions(ListSessionsRequest {
            workspace,
            page: 0,
            page_size: 50,
        })
        .expect("sessions");

    assert_eq!(page.total, worker_count * per_worker);
    assert_eq!(page.page_size, 50);
    assert_eq!(sessions.len(), 50);
    assert!(
        started.elapsed() < Duration::from_secs(60),
        "concurrent upsert smoke test took {:?}",
        started.elapsed()
    );
}

#[test]
fn cross_process_writers_share_one_queued_local_database() {
    let test_started = Instant::now();
    let db = DirectDbGuard::new();
    let nonce = uuid::Uuid::new_v4().to_string();
    let workspace = db.workspace(&format!("cross-process-{nonce}"));
    let worker_count = 12;
    let current_exe = std::env::current_exe().expect("current test exe");

    let mut children = Vec::new();
    for worker in 0..worker_count {
        let session_id = format!("cross-process-{nonce}-{worker}");
        children.push(
            Command::new(&current_exe)
                .args(["--exact", "cross_process_session_log_helper", "--nocapture"])
                .env("SESSION_LOG_CROSS_PROCESS_MODE", "upsert")
                .env("SESSION_LOG_CROSS_PROCESS_SESSION_ID", session_id)
                .env("SESSION_LOG_CROSS_PROCESS_WORKSPACE", &workspace)
                .env("SESSION_LOG_DB_ROOT", db.root())
                .spawn()
                .expect("spawn helper"),
        );
    }

    for mut child in children {
        let status = wait_child_with_timeout(
            &mut child,
            remaining_timeout(
                test_started,
                TEST_TIMEOUT,
                "cross-process session_log pressure",
            )
            .min(CHILD_TIMEOUT),
            "cross-process upsert helper",
        );
        assert!(status.success(), "helper exited with {status}");
    }

    let mut verify = Command::new(&current_exe)
        .args(["--exact", "cross_process_session_log_helper", "--nocapture"])
        .env("SESSION_LOG_CROSS_PROCESS_MODE", "verify")
        .env("SESSION_LOG_CROSS_PROCESS_WORKSPACE", &workspace)
        .env(
            "SESSION_LOG_CROSS_PROCESS_EXPECTED",
            worker_count.to_string(),
        )
        .env("SESSION_LOG_DB_ROOT", db.root())
        .spawn()
        .expect("spawn verify helper");
    let status = wait_child_with_timeout(
        &mut verify,
        remaining_timeout(
            test_started,
            TEST_TIMEOUT,
            "cross-process session_log pressure",
        )
        .min(CHILD_TIMEOUT),
        "cross-process verify helper",
    );
    assert!(status.success(), "verify helper exited with {status}");
}

#[test]
fn cross_process_session_log_helper() {
    let Ok(mode) = std::env::var("SESSION_LOG_CROSS_PROCESS_MODE") else {
        return;
    };
    let store = SessionLogStore::open_default().expect("store");
    let workspace = std::env::var("SESSION_LOG_CROSS_PROCESS_WORKSPACE").expect("workspace");

    if mode == "upsert" {
        let session_id = std::env::var("SESSION_LOG_CROSS_PROCESS_SESSION_ID").expect("session id");
        store
            .upsert_session(upsert(&session_id, &workspace, 1))
            .expect("upsert");
        return;
    }

    assert_eq!(mode, "verify");
    let expected = std::env::var("SESSION_LOG_CROSS_PROCESS_EXPECTED")
        .expect("expected")
        .parse::<u64>()
        .expect("expected number");
    let (page, sessions) = store
        .list_sessions(ListSessionsRequest {
            workspace,
            page: 0,
            page_size: 100,
        })
        .expect("sessions");
    assert_eq!(page.total, expected);
    assert_eq!(sessions.len() as u64, expected);
}

fn upsert(session_id: &str, workspace: &str, sequence: i64) -> UpsertSessionRequest {
    UpsertSessionRequest {
        session: serde_json::json!({
            "id": session_id,
            "name": format!("Stress {sequence}"),
            "directory": workspace,
            "created_at": sequence,
            "updated_at": sequence,
            "status": "idle",
            "management": {
                "session_id": session_id,
                "session_name": format!("Stress {sequence}"),
                "state": "created"
            }
        }),
        parent_id: None,
        messages: vec![serde_json::json!({
            "id": format!("m-{sequence}"),
            "role": "assistant",
            "created_at": sequence,
            "updated_at": sequence
        })],
        todos: vec![],
    }
}

fn join_thread_with_timeout<T>(handle: JoinHandle<T>, timeout: Duration, label: &str) -> T
where
    T: Send + 'static,
{
    let started = Instant::now();
    while !handle.is_finished() {
        if started.elapsed() >= timeout {
            panic!("{label} timed out after {timeout:?}");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    handle.join().unwrap_or_else(|_| panic!("{label} panicked"))
}

fn wait_child_with_timeout(child: &mut Child, timeout: Duration, label: &str) -> ExitStatus {
    let started = Instant::now();
    loop {
        if let Some(status) = child.try_wait().expect("poll child process") {
            return status;
        }
        if started.elapsed() >= timeout {
            let _ = child.kill();
            let _ = child.wait();
            panic!("{label} timed out after {timeout:?}");
        }
        std::thread::sleep(Duration::from_millis(25));
    }
}

fn remaining_timeout(started: Instant, timeout: Duration, label: &str) -> Duration {
    timeout
        .checked_sub(started.elapsed())
        .unwrap_or_else(|| panic!("{label} exceeded total timeout {timeout:?}"))
}
