use session_log::{ListSessionsRequest, SessionLogStore, UpsertSessionRequest};
use std::path::Path;
use std::process::{Child, Command, ExitStatus};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());
const TEST_TIMEOUT: Duration = Duration::from_secs(90);
const CHILD_TIMEOUT: Duration = Duration::from_secs(30);
const WORKSPACE_COUNT: usize = 10;
const TASKS_PER_WORKSPACE: usize = 20;
const RICH_RECORDS_PER_TASK: usize = 10;
const TOTAL_RICH_RECORDS: usize = WORKSPACE_COUNT * TASKS_PER_WORKSPACE * RICH_RECORDS_PER_TASK;

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
fn multi_workspace_rich_history_10_by_20_persists_2000_records() {
    let test_started = Instant::now();
    let db = DirectDbGuard::new();
    let store = SessionLogStore::open_default().expect("store");
    let nonce = uuid::Uuid::new_v4().to_string();
    let workspaces = (0..WORKSPACE_COUNT)
        .map(|index| db.workspace(&format!("rich-history-{nonce}-{index}")))
        .collect::<Vec<_>>();
    let started = Instant::now();
    let mut workers = Vec::with_capacity(WORKSPACE_COUNT * TASKS_PER_WORKSPACE);

    for (workspace_index, workspace) in workspaces.iter().enumerate() {
        for task_index in 0..TASKS_PER_WORKSPACE {
            let store = store.clone();
            let workspace = workspace.clone();
            let session_id = format!("rich-{nonce}-{workspace_index}-{task_index}");
            workers.push(std::thread::spawn(move || {
                store
                    .upsert_session(rich_upsert(
                        &session_id,
                        &workspace,
                        workspace_index,
                        task_index,
                    ))
                    .expect("rich upsert");
            }));
        }
    }

    for worker in workers {
        join_thread_with_timeout(
            worker,
            remaining_timeout(
                test_started,
                TEST_TIMEOUT,
                "multi-workspace rich history pressure",
            ),
            "multi-workspace rich history worker",
        );
    }

    let elapsed = started.elapsed();
    for (workspace_index, workspace) in workspaces.iter().enumerate() {
        let (page, sessions) = store
            .list_sessions(ListSessionsRequest {
                workspace: workspace.clone(),
                page: 0,
                page_size: 500,
            })
            .expect("workspace sessions");
        assert_eq!(
            page.total, TASKS_PER_WORKSPACE as u64,
            "workspace {workspace_index} should contain every task session"
        );
        assert_eq!(sessions.len(), TASKS_PER_WORKSPACE);
        assert!(
            sessions
                .iter()
                .all(|session| session.message_count == RICH_RECORDS_PER_TASK as u64),
            "workspace {workspace_index} sessions should each expose {RICH_RECORDS_PER_TASK} rich records"
        );
    }

    let summaries = store.list_workspaces().expect("workspace summaries");
    for workspace in &workspaces {
        let summary = summaries
            .iter()
            .find(|summary| summary.directory == *workspace)
            .unwrap_or_else(|| panic!("missing workspace summary for {workspace}"));
        assert_eq!(summary.session_count, TASKS_PER_WORKSPACE as u64);
    }

    eprintln!(
        "session_log_store_multi_workspace_rich_history summary: workspaces={WORKSPACE_COUNT} tasks_per_workspace={TASKS_PER_WORKSPACE} total_rich_records={TOTAL_RICH_RECORDS} elapsed_ms={}",
        elapsed.as_millis()
    );
    assert!(
        elapsed < Duration::from_secs(60),
        "multi-workspace rich history pressure took {elapsed:?}"
    );
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

fn rich_upsert(
    session_id: &str,
    workspace: &str,
    workspace_index: usize,
    task_index: usize,
) -> UpsertSessionRequest {
    let sequence = (workspace_index * TASKS_PER_WORKSPACE + task_index) as i64;
    UpsertSessionRequest {
        session: serde_json::json!({
            "id": session_id,
            "name": format!("Rich Workspace {workspace_index} Task {task_index}"),
            "directory": workspace,
            "created_at": sequence,
            "updated_at": 10_000 + sequence,
            "status": "idle",
            "management": {
                "session_id": session_id,
                "session_name": format!("Rich Workspace {workspace_index} Task {task_index}"),
                "state": "created",
                "task_plan": {
                    "plan_summary": "multi workspace rich history pressure",
                    "detailed_tasks": [{
                        "id": format!("task-{workspace_index}-{task_index}"),
                        "status": "done"
                    }]
                }
            }
        }),
        parent_id: None,
        messages: (0..RICH_RECORDS_PER_TASK)
            .map(|record_index| {
                let created = sequence * 100 + record_index as i64;
                serde_json::json!({
                    "id": format!("rich-message-{workspace_index}-{task_index}-{record_index}"),
                    "session_id": session_id,
                    "role": if record_index % 2 == 0 { "user" } else { "assistant" },
                    "created_at": created,
                    "updated_at": created,
                    "parts": [{
                        "type": "text",
                        "text": rich_text_payload(workspace_index, task_index, record_index),
                    }]
                })
            })
            .collect(),
        todos: vec![serde_json::json!({
            "id": format!("todo-{workspace_index}-{task_index}"),
            "content": "persist rich concurrent workspace history",
            "status": "done"
        })],
    }
}

fn rich_text_payload(workspace_index: usize, task_index: usize, record_index: usize) -> String {
    format!(
        "### Workspace {workspace_index} task {task_index} record {record_index}\n\n\
Rich text payload for session_db pressure with markdown, HTML, table rows, local links, and a code fence.\n\n\
| component | workspace | task | record |\n\
| --- | ---: | ---: | ---: |\n\
| session_db | {workspace_index} | {task_index} | {record_index} |\n\n\
```json\n{{\"workspace\":{workspace_index},\"task\":{task_index},\"record\":{record_index}}}\n```\n\n\
<b>bold marker</b> [workspace](file:///tmp/tura/workspace-{workspace_index}/task-{task_index})"
    )
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
