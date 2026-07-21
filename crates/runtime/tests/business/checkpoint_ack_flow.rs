use anyhow::{anyhow, Context, Result};
use chrono::{Duration as ChronoDuration, Utc};
use runtime::checkpoint::{
    checkpoint_command_ready, checkpoint_command_run_finished, checkpoint_command_run_started,
    checkpoint_command_started, checkpoint_streamed_command_finished, StreamedCommandCheckpoint,
};
use rusqlite::Connection;
use serde_json::json;
use session_log_contract::SessionLogCommand;
use std::path::Path;
use std::time::{Duration, Instant};

static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[test]
fn runtime_checkpoint_business_flow_writes_applied_rows_idempotently() -> Result<()> {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let temp = tempfile::tempdir().context("temp runtime checkpoint root")?;
    let home = temp.path().join("home");
    std::fs::create_dir_all(&home)?;
    let _env = EnvGuard::new(&home);
    let service = ServiceThread::start()?;

    let session_id = format!("runtime-checkpoint-online-{}", uuid::Uuid::new_v4());
    let runtime_id = "runtime-worker-online";
    let command_run_id = "command-run-online";
    let command_id = "command-online-1";
    let run_started_at = Utc::now();
    let ready_at = run_started_at + ChronoDuration::milliseconds(1);
    let command_started_at = run_started_at + ChronoDuration::milliseconds(2);
    let command_finished_at = run_started_at + ChronoDuration::milliseconds(3);
    let run_finished_at = run_started_at + ChronoDuration::milliseconds(4);
    let command = json!({
        "id": command_id,
        "step": 1,
        "command": "shell_command",
        "command_line": "echo checkpoint-online"
    });
    checkpoint_ok(checkpoint_command_run_started(
        &session_id,
        runtime_id,
        command_run_id,
        run_started_at,
    ))?;
    checkpoint_ok(checkpoint_command_ready(
        &session_id,
        runtime_id,
        command_run_id,
        command_id,
        0,
        &command,
        ready_at,
    ))?;
    checkpoint_ok(checkpoint_command_started(
        &session_id,
        runtime_id,
        command_run_id,
        command_id,
        0,
        &command,
        command_started_at,
    ))?;
    let result = json!({
        "id": command_id,
        "command_type": "shell_command",
        "command_line": "echo checkpoint-online",
        "success": true,
        "stdout": "checkpoint-online\n",
        "changes": [{ "path": "checkpoint.txt", "kind": "created" }]
    });
    let input = StreamedCommandCheckpoint {
        session_id: &session_id,
        runtime_id,
        runtime_worker_id: runtime_id,
        command_run_id,
        index: 0,
        result: &result,
        finished_at: command_finished_at,
    };
    checkpoint_ok(checkpoint_streamed_command_finished(input.clone()))?;
    checkpoint_ok(checkpoint_streamed_command_finished(input))?;
    checkpoint_ok(checkpoint_command_run_finished(
        &session_id,
        runtime_id,
        command_run_id,
        "success",
        1,
        run_started_at,
        run_finished_at,
    ))?;

    let rows = wait_for_checkpoint_rows(&home, &session_id, 5, Duration::from_secs(10))?;
    assert_eq!(
        rows.len(),
        5,
        "duplicate command_finished ACK should update the existing idempotency row"
    );
    let mut event_types = rows
        .iter()
        .map(|row| row.event_type.as_str())
        .collect::<Vec<_>>();
    event_types.sort_unstable();
    assert_eq!(
        event_types,
        vec![
            "command_finished",
            "command_ready",
            "command_run_finished",
            "command_run_started",
            "command_started",
        ]
    );
    let finished = rows
        .iter()
        .find(|row| row.event_type == "command_finished")
        .ok_or_else(|| anyhow!("missing command_finished row"))?;
    let run_started = rows
        .iter()
        .find(|row| row.event_type == "command_run_started")
        .ok_or_else(|| anyhow!("missing command_run_started row"))?;
    let ready = rows
        .iter()
        .find(|row| row.event_type == "command_ready")
        .ok_or_else(|| anyhow!("missing command_ready row"))?;
    let started = rows
        .iter()
        .find(|row| row.event_type == "command_started")
        .ok_or_else(|| anyhow!("missing command_started row"))?;
    let run_finished = rows
        .iter()
        .find(|row| row.event_type == "command_run_finished")
        .ok_or_else(|| anyhow!("missing command_run_finished row"))?;
    let run_started_at_text = run_started_at.to_rfc3339();
    let ready_at_text = ready_at.to_rfc3339();
    let command_started_at_text = command_started_at.to_rfc3339();
    let command_finished_at_text = command_finished_at.to_rfc3339();
    let run_finished_at_text = run_finished_at.to_rfc3339();
    assert_eq!(
        run_started.started_at.as_deref(),
        Some(run_started_at_text.as_str())
    );
    assert_eq!(ready.started_at.as_deref(), Some(ready_at_text.as_str()));
    assert_eq!(
        started.started_at.as_deref(),
        Some(command_started_at_text.as_str())
    );
    assert_eq!(
        finished.finished_at.as_deref(),
        Some(command_finished_at_text.as_str())
    );
    assert_eq!(
        run_finished.started_at.as_deref(),
        Some(run_started_at_text.as_str())
    );
    assert_eq!(
        run_finished.finished_at.as_deref(),
        Some(run_finished_at_text.as_str())
    );
    assert_eq!(run_finished.changes["status"], "success");
    assert_eq!(finished.command_type.as_deref(), Some("shell_command"));
    assert_eq!(
        finished.output_summary.as_deref(),
        Some("checkpoint-online\n")
    );
    assert_eq!(finished.changes[0]["path"], "checkpoint.txt");

    drop(service);
    wait_until(Duration::from_secs(5), || {
        !session_log_contract::client::service_is_running()
    })?;
    Ok(())
}

#[test]
fn runtime_checkpoint_business_flow_queues_offline_ack_and_drains_on_service_start() -> Result<()> {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let temp = tempfile::tempdir().context("temp runtime checkpoint offline root")?;
    let home = temp.path().join("home");
    std::fs::create_dir_all(&home)?;
    let _env = EnvGuard::new(&home);
    assert!(
        !session_log_contract::client::service_is_running(),
        "checkpoint offline test starts without a session_db service"
    );

    let session_id = format!("runtime-checkpoint-offline-{}", uuid::Uuid::new_v4());
    let run_started_at = Utc::now();
    checkpoint_ok(checkpoint_command_run_started(
        &session_id,
        "runtime-worker-offline",
        "command-run-offline",
        run_started_at,
    ))?;
    assert!(
        pending_queue_files(&home)? >= 1,
        "offline runtime checkpoint ACK should create a durable file queue item"
    );

    let service = ServiceThread::start()?;
    let rows = wait_for_checkpoint_rows(&home, &session_id, 1, Duration::from_secs(10))?;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].event_type, "command_run_started");

    drop(service);
    wait_until(Duration::from_secs(5), || {
        !session_log_contract::client::service_is_running()
    })?;
    Ok(())
}

#[test]
fn runtime_checkpoint_business_flow_drains_offline_command_batch_idempotently() -> Result<()> {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let temp = tempfile::tempdir().context("temp runtime checkpoint batch root")?;
    let home = temp.path().join("home");
    std::fs::create_dir_all(&home)?;
    let _env = EnvGuard::new(&home);
    assert!(
        !session_log_contract::client::service_is_running(),
        "checkpoint batch test starts without a session_db service"
    );

    let session_id = format!("runtime-checkpoint-batch-{}", uuid::Uuid::new_v4());
    let runtime_id = "runtime-worker-batch";
    let command_run_id = "command-run-batch";
    let command_id = "command-batch-1";
    let run_started_at = Utc::now();
    let ready_at = run_started_at + ChronoDuration::milliseconds(1);
    let command_started_at = run_started_at + ChronoDuration::milliseconds(2);
    let command_finished_at = run_started_at + ChronoDuration::milliseconds(3);
    let run_finished_at = run_started_at + ChronoDuration::milliseconds(4);
    let command = json!({
        "id": command_id,
        "step": 1,
        "command": "shell_command",
        "command_line": "printf checkpoint-batch"
    });
    let result = json!({
        "id": command_id,
        "command_type": "shell_command",
        "command_line": "printf checkpoint-batch",
        "success": true,
        "stdout": "checkpoint-batch",
        "changes": [
            { "path": "batch.txt", "kind": "created" },
            { "path": "trace.log", "kind": "updated" }
        ]
    });

    checkpoint_ok(checkpoint_command_run_started(
        &session_id,
        runtime_id,
        command_run_id,
        run_started_at,
    ))?;
    checkpoint_ok(checkpoint_command_ready(
        &session_id,
        runtime_id,
        command_run_id,
        command_id,
        0,
        &command,
        ready_at,
    ))?;
    checkpoint_ok(checkpoint_command_started(
        &session_id,
        runtime_id,
        command_run_id,
        command_id,
        0,
        &command,
        command_started_at,
    ))?;
    let finished = StreamedCommandCheckpoint {
        session_id: &session_id,
        runtime_id,
        runtime_worker_id: runtime_id,
        command_run_id,
        index: 0,
        result: &result,
        finished_at: command_finished_at,
    };
    checkpoint_ok(checkpoint_streamed_command_finished(finished.clone()))?;
    checkpoint_ok(checkpoint_streamed_command_finished(finished))?;
    checkpoint_ok(checkpoint_command_run_finished(
        &session_id,
        runtime_id,
        command_run_id,
        "success",
        1,
        run_started_at,
        run_finished_at,
    ))?;
    assert!(
        pending_queue_files(&home)? >= 6,
        "offline command batch should be durably queued before session_db starts"
    );

    let service = ServiceThread::start()?;
    let rows = wait_for_checkpoint_rows(&home, &session_id, 5, Duration::from_secs(10))?;
    let row_event_types = rows
        .iter()
        .map(|row| row.event_type.clone())
        .collect::<Vec<_>>();
    assert_eq!(
        rows.len(),
        5,
        "duplicate offline command_finished ACK should update one durable checkpoint row; rows={row_event_types:?}; failed_queue={:?}",
        failed_queue_errors(&home)?
    );
    let mut event_types = rows
        .iter()
        .map(|row| row.event_type.as_str())
        .collect::<Vec<_>>();
    event_types.sort_unstable();
    assert_eq!(
        event_types,
        vec![
            "command_finished",
            "command_ready",
            "command_run_finished",
            "command_run_started",
            "command_started",
        ]
    );
    assert_eq!(
        rows.iter()
            .find(|row| row.event_type == "command_run_started")
            .and_then(|row| row.event_seq),
        Some(10)
    );
    let finished_row = rows
        .iter()
        .find(|row| row.event_type == "command_finished")
        .ok_or_else(|| anyhow!("missing command_finished row after offline drain"))?;
    assert_eq!(finished_row.command_type.as_deref(), Some("shell_command"));
    assert_eq!(
        finished_row.command_line.as_deref(),
        Some("printf checkpoint-batch")
    );
    assert_eq!(
        finished_row.output_summary.as_deref(),
        Some("checkpoint-batch")
    );
    assert_eq!(finished_row.changes[0]["path"], "batch.txt");
    assert_eq!(finished_row.changes[1]["path"], "trace.log");
    let run_finished = rows
        .iter()
        .find(|row| row.event_type == "command_run_finished")
        .ok_or_else(|| anyhow!("missing command_run_finished row after offline drain"))?;
    let run_started_at_text = run_started_at.to_rfc3339();
    let run_finished_at_text = run_finished_at.to_rfc3339();
    assert_eq!(
        run_finished.started_at.as_deref(),
        Some(run_started_at_text.as_str())
    );
    assert_eq!(
        run_finished.finished_at.as_deref(),
        Some(run_finished_at_text.as_str())
    );
    assert_eq!(run_finished.changes["status"], "success");

    drop(service);
    wait_until(Duration::from_secs(5), || {
        !session_log_contract::client::service_is_running()
    })?;
    Ok(())
}

#[derive(Debug)]
struct CheckpointRow {
    event_type: String,
    event_seq: Option<i64>,
    command_type: Option<String>,
    command_line: Option<String>,
    output_summary: Option<String>,
    changes: serde_json::Value,
    started_at: Option<String>,
    finished_at: Option<String>,
}

fn checkpoint_rows(home: &Path, session_id: &str) -> Result<Vec<CheckpointRow>> {
    let db = home.join("db").join("session_log").join("index.sqlite3");
    let conn = Connection::open(&db).with_context(|| format!("open {}", db.display()))?;
    let mut stmt = conn.prepare(
        "SELECT checkpoint_type, event_seq, command_type, command_line,
                output_summary, changes_json, started_at, finished_at
         FROM command_checkpoints
         WHERE session_id = ?1
         ORDER BY COALESCE(event_seq, 0), idempotency_key",
    )?;
    let rows = stmt
        .query_map([session_id], |row| {
            let event_type: String = row.get(0)?;
            let event_seq: Option<i64> = row.get(1)?;
            let changes_json: String = row.get(5)?;
            let changes: serde_json::Value = serde_json::from_str(&changes_json)
                .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?;
            Ok(CheckpointRow {
                event_type,
                event_seq,
                command_type: row.get(2)?,
                command_line: row.get(3)?,
                output_summary: row.get(4)?,
                changes,
                started_at: row.get(6)?,
                finished_at: row.get(7)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(rows)
}

fn checkpoint_ok(result: std::result::Result<(), String>) -> Result<()> {
    result.map_err(|error| anyhow!(error))
}

fn queue_pending_dir(home: &Path) -> std::path::PathBuf {
    home.join("db")
        .join("session_log")
        .join("message_queue")
        .join("pending")
}

fn pending_queue_files(home: &Path) -> Result<usize> {
    queue_file_count(&queue_pending_dir(home))
}

fn processing_queue_files(home: &Path) -> Result<usize> {
    queue_file_count(
        &home
            .join("db")
            .join("session_log")
            .join("message_queue")
            .join("processing"),
    )
}

fn failed_queue_files(home: &Path) -> Result<usize> {
    queue_file_count(
        &home
            .join("db")
            .join("session_log")
            .join("message_queue")
            .join("failed"),
    )
}

fn queue_file_count(dir: &Path) -> Result<usize> {
    if !dir.exists() {
        return Ok(0);
    }
    Ok(std::fs::read_dir(dir)?
        .flatten()
        .filter(|entry| entry.path().extension().and_then(|ext| ext.to_str()) == Some("json"))
        .count())
}

fn wait_for_checkpoint_rows(
    home: &Path,
    session_id: &str,
    expected: usize,
    timeout: Duration,
) -> Result<Vec<CheckpointRow>> {
    let started = Instant::now();
    while started.elapsed() < timeout {
        let rows = checkpoint_rows(home, session_id)?;
        let pending = pending_queue_files(home)?;
        let processing = processing_queue_files(home)?;
        let failed = failed_queue_files(home)?;
        if rows.len() == expected && pending == 0 && processing == 0 && failed == 0 {
            return Ok(rows);
        }
        std::thread::sleep(Duration::from_millis(25));
    }

    let rows = checkpoint_rows(home, session_id).unwrap_or_default();
    let row_event_types = rows
        .iter()
        .map(|row| row.event_type.as_str())
        .collect::<Vec<_>>();
    Err(anyhow!(
        "expected {expected} applied checkpoint rows for {session_id}; rows={row_event_types:?}; pending={}; processing={}; failed={}; failed_queue={:?}",
        pending_queue_files(home).unwrap_or_default(),
        processing_queue_files(home).unwrap_or_default(),
        failed_queue_files(home).unwrap_or_default(),
        failed_queue_errors(home).unwrap_or_default()
    ))
}

fn failed_queue_errors(home: &Path) -> Result<Vec<String>> {
    let failed = home
        .join("db")
        .join("session_log")
        .join("message_queue")
        .join("failed");
    if !failed.exists() {
        return Ok(Vec::new());
    }
    let mut errors = Vec::new();
    for entry in std::fs::read_dir(&failed)?.flatten() {
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) == Some("txt") {
            errors.push(
                std::fs::read_to_string(&path)
                    .unwrap_or_else(|error| format!("failed to read {}: {error}", path.display())),
            );
        }
    }
    errors.sort();
    Ok(errors)
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
        wait_until(
            Duration::from_secs(10),
            session_log_contract::client::service_is_running,
        )?;
        Ok(Self {
            handle: Some(handle),
        })
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

fn wait_until(timeout: Duration, mut condition: impl FnMut() -> bool) -> Result<()> {
    let started = Instant::now();
    while started.elapsed() < timeout {
        if condition() {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    Err(anyhow!(
        "condition was not met within {}ms",
        timeout.as_millis()
    ))
}
