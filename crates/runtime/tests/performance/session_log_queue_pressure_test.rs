use anyhow::{anyhow, Context, Result};
use runtime::session_log_client::SessionLogClient;
use serde_json::{json, Value};
use session_log::SessionLogCommand;
use std::path::Path;
use std::time::{Duration, Instant};

static ENV_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

const WORKSPACE_COUNT: usize = 10;
const TASKS_PER_WORKSPACE: usize = 20;
const RUNTIME_COUNT: usize = WORKSPACE_COUNT * TASKS_PER_WORKSPACE;
const WRITES_PER_RUNTIME: usize = 9;
const EXPECTED_MESSAGES_PER_RUNTIME: usize = WRITES_PER_RUNTIME + 1;
const EXPECTED_TOTAL_RICH_RECORDS: usize = RUNTIME_COUNT * EXPECTED_MESSAGES_PER_RUNTIME;
const TEST_TIMEOUT: Duration = Duration::from_secs(180);
const DRAIN_TIMEOUT: Duration = Duration::from_secs(90);

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn runtime_session_log_file_queue_pressure_10_workspaces_20_tasks_2000_rich_records(
) -> Result<()> {
    tokio::time::timeout(TEST_TIMEOUT, session_log_queue_pressure_impl())
        .await
        .context("runtime session_log queue pressure exceeded total timeout")?
}

async fn session_log_queue_pressure_impl() -> Result<()> {
    let _guard = ENV_LOCK.lock().await;
    let root = tempfile::tempdir().context("temp runtime session_log queue pressure root")?;
    let home = root.path().join("home");
    std::fs::create_dir_all(&home).context("create pressure home")?;
    let workspaces = (0..WORKSPACE_COUNT)
        .map(|index| root.path().join(format!("workspace-{index}")))
        .collect::<Vec<_>>();
    for workspace in &workspaces {
        std::fs::create_dir_all(workspace).context("create pressure workspace")?;
    }
    let _env = EnvGuard::new(&home);
    let service = ServiceThread::start()?;
    let workspace_texts = workspaces
        .iter()
        .map(|workspace| workspace.to_string_lossy().to_string())
        .collect::<Vec<_>>();

    let enqueue_started = Instant::now();
    let mut tasks = Vec::with_capacity(RUNTIME_COUNT);
    for runtime_index in 0..RUNTIME_COUNT {
        let workspace_index = runtime_index / TASKS_PER_WORKSPACE;
        let workspace_text = workspace_texts[workspace_index].clone();
        tasks.push(tokio::task::spawn_blocking(move || {
            write_runtime_snapshots(runtime_index, workspace_index, &workspace_text)
        }));
    }

    let mut summaries = Vec::with_capacity(RUNTIME_COUNT);
    for task in tasks {
        summaries.push(
            task.await
                .context("runtime queue pressure writer panicked")??,
        );
    }
    let enqueue_elapsed = enqueue_started.elapsed();

    let client = SessionLogClient::discover()?;
    let drain_started = Instant::now();
    wait_until(DRAIN_TIMEOUT, || {
        pressure_sessions_visible(&client, &summaries, &home)
    })
    .await?;
    let drain_elapsed = drain_started.elapsed();

    for workspace_index in 0..WORKSPACE_COUNT {
        let workspace_key =
            session_log::path::normalize_workspace(&workspace_texts[workspace_index]);
        let (page, sessions) = client.list_sessions(workspace_key.clone(), 0, 500)?;
        assert_eq!(page.total, TASKS_PER_WORKSPACE as u64);
        assert_eq!(sessions.len(), TASKS_PER_WORKSPACE);
    }

    for summary in &summaries {
        let snapshot = client
            .get_session(summary.session_id.clone())?
            .ok_or_else(|| anyhow!("missing pressure session {}", summary.session_id))?;
        assert_eq!(snapshot.workspace, summary.workspace_key);
        assert_eq!(
            snapshot.message_count, EXPECTED_MESSAGES_PER_RUNTIME as u64,
            "summary session should include all runtime messages"
        );
        let (records_page, records) =
            client.list_session_records(summary.session_id.clone(), 0, 200)?;
        assert_eq!(records_page.total, EXPECTED_MESSAGES_PER_RUNTIME as u64);
        assert!(
            records
                .iter()
                .any(|record| record.message_id == summary.summary_message_id),
            "session {} should persist final summary message",
            summary.session_id
        );
    }

    eprintln!(
        "runtime_session_log_queue_pressure summary: workspaces={WORKSPACE_COUNT} tasks_per_workspace={TASKS_PER_WORKSPACE} runtimes={RUNTIME_COUNT} total_rich_records={EXPECTED_TOTAL_RICH_RECORDS} enqueue_ms={} drain_ms={}",
        enqueue_elapsed.as_millis(),
        drain_elapsed.as_millis()
    );

    drop(service);
    wait_until(Duration::from_secs(5), || {
        !session_log::ipc::service_is_running()
    })
    .await?;
    Ok(())
}

fn write_runtime_snapshots(
    runtime_index: usize,
    workspace_index: usize,
    workspace: &str,
) -> Result<RuntimeSummary> {
    let client = SessionLogClient::discover()?;
    let session_id = format!("queue-pressure-session-{runtime_index}");
    let runtime_id = format!("queue-pressure-runtime-{runtime_index}");
    let mut messages = Vec::with_capacity(EXPECTED_MESSAGES_PER_RUNTIME);

    for write in 0..WRITES_PER_RUNTIME {
        let message_id = format!("{runtime_id}-message-{write}");
        messages.push(message_payload(
            &session_id,
            &message_id,
            "assistant",
            &rich_text_payload(
                workspace_index,
                runtime_index,
                write,
                "runtime queued write",
            ),
            write as i64,
        ));
        client.upsert_session(
            session_payload(
                &session_id,
                workspace,
                &format!("Runtime Queue Pressure {runtime_index}"),
                "running",
                write as i64,
            ),
            None,
            messages.clone(),
            vec![json!({
                "id": format!("{runtime_id}-todo"),
                "status": "running",
                "write": write
            })],
        )?;
    }

    let summary_message_id = format!("{runtime_id}-summary");
    messages.push(message_payload(
        &session_id,
        &summary_message_id,
        "assistant",
        &rich_text_payload(
            workspace_index,
            runtime_index,
            WRITES_PER_RUNTIME,
            "runtime summary",
        ),
        10_000 + runtime_index as i64,
    ));
    client.upsert_session(
        session_payload(
            &session_id,
            workspace,
            &format!("Runtime Queue Pressure {runtime_index}"),
            "created",
            10_000 + runtime_index as i64,
        ),
        None,
        messages,
        vec![json!({
            "id": format!("{runtime_id}-summary-todo"),
            "status": "done",
            "summary_message_id": summary_message_id
        })],
    )?;

    Ok(RuntimeSummary {
        session_id,
        summary_message_id,
        workspace_key: session_log::path::normalize_workspace(workspace),
    })
}

fn pressure_sessions_visible(
    client: &SessionLogClient,
    summaries: &[RuntimeSummary],
    home: &Path,
) -> bool {
    if pending_queue_files(home).unwrap_or(usize::MAX) != 0 {
        return false;
    }

    for workspace_index in 0..WORKSPACE_COUNT {
        let workspace_summaries = summaries
            .iter()
            .filter(|summary| summary.workspace_index() == workspace_index)
            .collect::<Vec<_>>();
        let Some(first) = workspace_summaries.first() else {
            return false;
        };
        let Ok((page, sessions)) =
            client.list_sessions(first.workspace_key.clone(), 0, TASKS_PER_WORKSPACE as u64)
        else {
            return false;
        };
        if page.total != TASKS_PER_WORKSPACE as u64 || sessions.len() != TASKS_PER_WORKSPACE {
            return false;
        }
    }
    summaries.iter().all(|summary| {
        client
            .list_session_records(summary.session_id.clone(), 0, 200)
            .is_ok_and(|(page, records)| {
                page.total == EXPECTED_MESSAGES_PER_RUNTIME as u64
                    && records.len() == EXPECTED_MESSAGES_PER_RUNTIME
                    && records
                        .iter()
                        .any(|record| record.message_id == summary.summary_message_id)
            })
    })
}

fn session_payload(
    session_id: &str,
    workspace: &str,
    name: &str,
    state: &str,
    updated_at: i64,
) -> Value {
    json!({
        "id": session_id,
        "name": name,
        "directory": workspace,
        "created_at": 1,
        "updated_at": updated_at,
        "status": if state == "running" { "running" } else { "idle" },
        "management": {
            "session_id": session_id,
            "session_name": name,
            "state": state
        }
    })
}

fn message_payload(
    session_id: &str,
    message_id: &str,
    role: &str,
    text: &str,
    timestamp: i64,
) -> Value {
    json!({
        "id": message_id,
        "session_id": session_id,
        "role": role,
        "created_at": timestamp,
        "updated_at": timestamp,
        "parts": [{ "type": "text", "text": text }]
    })
}

fn rich_text_payload(
    workspace_index: usize,
    runtime_index: usize,
    record_index: usize,
    label: &str,
) -> String {
    format!(
        "### {label} workspace-{workspace_index} task-{runtime_index} record-{record_index}\n\n\
Runtime queue rich text record with markdown table, HTML, local link, and a code fence for end-to-end pressure.\n\n\
| component | workspace | task | record |\n\
| --- | ---: | ---: | ---: |\n\
| runtime | {workspace_index} | {runtime_index} | {record_index} |\n\
| session_db | {workspace_index} | {runtime_index} | {record_index} |\n\n\
```rs\n\
let workspace = {workspace_index};\n\
let task = {runtime_index};\n\
let record = {record_index};\n\
```\n\n\
<b>runtime rich marker</b> [workspace](file:///tmp/tura/runtime-{workspace_index}-{runtime_index})"
    )
}

fn pending_queue_files(home: &Path) -> Result<usize> {
    let pending = home
        .join("db")
        .join("session_log")
        .join("message_queue")
        .join("pending");
    if !pending.exists() {
        return Ok(0);
    }
    Ok(std::fs::read_dir(&pending)?
        .flatten()
        .filter(|entry| entry.path().extension().and_then(|ext| ext.to_str()) == Some("json"))
        .count())
}

#[derive(Clone, Debug)]
struct RuntimeSummary {
    session_id: String,
    summary_message_id: String,
    workspace_key: String,
}

impl RuntimeSummary {
    fn workspace_index(&self) -> usize {
        self.session_id
            .rsplit('-')
            .next()
            .and_then(|value| value.parse::<usize>().ok())
            .map(|runtime_index| runtime_index / TASKS_PER_WORKSPACE)
            .unwrap_or(usize::MAX)
    }
}

struct EnvGuard {
    previous: Vec<(&'static str, Option<std::ffi::OsString>)>,
}

impl EnvGuard {
    fn new(home: &Path) -> Self {
        let keys = [
            "TURA_HOME",
            "SESSION_LOG_DB_ROOT",
            "TURA_DB_ROOT",
            "TURA_SESSION_DB_PROBE_TIMEOUT_MS",
        ];
        let previous = keys
            .iter()
            .map(|key| (*key, std::env::var_os(key)))
            .collect::<Vec<_>>();
        std::env::set_var("TURA_HOME", home);
        std::env::remove_var("SESSION_LOG_DB_ROOT");
        std::env::remove_var("TURA_DB_ROOT");
        std::env::set_var("TURA_SESSION_DB_PROBE_TIMEOUT_MS", "20");
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

struct ServiceThread {
    handle: Option<std::thread::JoinHandle<Result<()>>>,
}

impl ServiceThread {
    fn start() -> Result<Self> {
        let handle = std::thread::spawn(session_log::service::run_socket_service);
        let started = Instant::now();
        while started.elapsed() < Duration::from_secs(10) {
            if session_log::ipc::service_is_running() {
                return Ok(Self {
                    handle: Some(handle),
                });
            }
            if handle.is_finished() {
                break;
            }
            std::thread::sleep(Duration::from_millis(25));
        }
        Err(anyhow!(
            "session_db service did not become reachable within 10s"
        ))
    }
}

impl Drop for ServiceThread {
    fn drop(&mut self) {
        let _ = session_log::ipc::call_service(&SessionLogCommand::Shutdown);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

async fn wait_until(timeout: Duration, mut condition: impl FnMut() -> bool) -> Result<()> {
    let started = Instant::now();
    while started.elapsed() < timeout {
        if condition() {
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    Err(anyhow!(
        "condition was not met within {}ms",
        timeout.as_millis()
    ))
}
