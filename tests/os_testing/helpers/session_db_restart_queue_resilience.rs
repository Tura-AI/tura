//! Required root-package business E2E for session_db restart and file-queue
//! resilience.
//!
//! The flow is intentionally local-only: it uses the real session_db socket
//! service, the durable file queue, the embedded SQLite index DB, and workspace
//! `.tura/session_log.sqlite3` stores without public network access, API keys,
//! or third-party services.

pub(crate) use anyhow::{anyhow, bail, Context, Result};
pub(crate) use lifecycle::{PlanStatus, SessionCommand, SessionState, TaskPlan, TaskStep};
pub(crate) use rusqlite::Connection;
pub(crate) use serde_json::json;
pub(crate) use session_log::file_queue;
pub(crate) use session_log_contract::{
    CommandCheckpoint, CreateSessionRequest, ExecuteSessionCommandRequest, GetSessionRequest,
    ListSessionRecordsRequest, ListSessionsRequest, PersistSessionPayloadRequest,
    SessionLogCommand, SessionLogResponse,
};
pub(crate) use std::{
    path::{Path, PathBuf},
    sync::Mutex,
    thread,
    time::{Duration, Instant},
};

pub(crate) static SERIAL: Mutex<()> = Mutex::new(());

pub(crate) fn enqueue(command: SessionLogCommand) -> Result<()> {
    file_queue::enqueue_command(&command)?;
    Ok(())
}

pub(crate) fn task_plan(tasks: &[(&str, PlanStatus)]) -> TaskPlan {
    TaskPlan {
        plan_summary: "restart recovery plan".to_string(),
        detailed_tasks: tasks
            .iter()
            .enumerate()
            .map(|(index, (task_id, status))| TaskStep {
                task_id: (*task_id).to_string(),
                step: index as u64 + 1,
                task_summary: format!("task {task_id}"),
                step_task: format!("task {task_id}"),
                status: *status,
                ..TaskStep::default()
            })
            .collect(),
    }
}

pub(crate) fn create_session_command(
    session_id: &str,
    workspace: &str,
    created_at: i64,
    tasks: &[(&str, PlanStatus)],
) -> SessionLogCommand {
    SessionLogCommand::CreateSession(CreateSessionRequest {
        session_id: session_id.to_string(),
        creation_command: SessionCommand::CreateSession {
            task_plan: task_plan(tasks),
        },
        workspace: workspace.to_string(),
        session_directory: workspace.to_string(),
        name: format!("Session {session_id}"),
        created_at,
        model: None,
        agent: None,
        session_type: "coding".to_string(),
        kill_processes_on_start: false,
        validator_enabled: false,
        force_planning: false,
        model_variant: None,
        model_acceleration_enabled: false,
        disable_permission_restrictions: false,
        use_last_tool_call_response: false,
        auto_session_name: false,
    })
}

pub(crate) fn execute_session_command(
    session_id: &str,
    session_command: SessionCommand,
) -> SessionLogCommand {
    SessionLogCommand::ExecuteSessionCommand(ExecuteSessionCommandRequest {
        session_id: session_id.to_string(),
        session_command,
    })
}

pub(crate) fn persist_session_payload_command(
    session_id: &str,
    updated_at: i64,
    messages: &[&str],
    todos: &[(&str, PlanStatus)],
) -> SessionLogCommand {
    SessionLogCommand::PersistSessionPayload(PersistSessionPayloadRequest {
        session_id: session_id.to_string(),
        records: messages
            .iter()
            .enumerate()
            .map(|(index, message_id)| {
                json!({
                    "id": message_id,
                    "role": if index == 0 { "user" } else { "assistant" },
                    "created_at": updated_at + index as i64,
                    "updated_at": updated_at + index as i64,
                    "content": format!("content for {message_id}")
                })
            })
            .collect(),
        todos: todos
            .iter()
            .map(|(id, status)| {
                json!({
                    "id": id,
                    "content": format!("todo {id}"),
                    "status": status
                })
            })
            .collect(),
    })
}

pub(crate) fn create_session_commands(
    session_id: &str,
    workspace: &str,
    created_at: i64,
    state: SessionState,
    messages: &[&str],
    tasks: &[(&str, PlanStatus)],
) -> Vec<SessionLogCommand> {
    let mut commands = vec![create_session_command(
        session_id, workspace, created_at, tasks,
    )];
    match state {
        SessionState::Created => {}
        SessionState::Running => commands.push(execute_session_command(
            session_id,
            SessionCommand::RuntimeStarted,
        )),
        SessionState::Paused => {
            commands.push(execute_session_command(
                session_id,
                SessionCommand::RuntimeStarted,
            ));
            commands.push(execute_session_command(
                session_id,
                SessionCommand::ApplyRuntimeState {
                    state: SessionState::Paused,
                },
            ));
        }
        SessionState::Completed => {
            commands.push(execute_session_command(
                session_id,
                SessionCommand::RuntimeStarted,
            ));
            commands.push(execute_session_command(
                session_id,
                SessionCommand::RuntimeCompleted,
            ));
        }
        SessionState::Failed => {
            commands.push(execute_session_command(
                session_id,
                SessionCommand::RuntimeStarted,
            ));
            commands.push(execute_session_command(
                session_id,
                SessionCommand::RuntimeFailed,
            ));
        }
        SessionState::Cancelled => commands.push(execute_session_command(
            session_id,
            SessionCommand::CancelSession,
        )),
        SessionState::Interrupted => {
            commands.push(execute_session_command(
                session_id,
                SessionCommand::RuntimeStarted,
            ));
            commands.push(execute_session_command(
                session_id,
                SessionCommand::InterruptSession,
            ));
        }
    }
    commands.push(persist_session_payload_command(
        session_id, created_at, messages, tasks,
    ));
    commands
}

pub(crate) fn checkpoint(session_id: &str, seq: i64, status: &str) -> CommandCheckpoint {
    CommandCheckpoint {
        session_id: session_id.to_string(),
        turn_id: "turn-restart-queue".to_string(),
        runtime_worker_id: Some("runtime-worker-restart-queue".to_string()),
        provider_call_id: Some("provider-restart-queue".to_string()),
        command_run_id: Some("command-run-restart-queue".to_string()),
        command_id: Some(format!("command-{seq}")),
        event_seq: Some(seq),
        command_type: Some("shell_command".to_string()),
        command_line: Some(format!("Write-Output restart-queue-{seq}")),
        status: status.to_string(),
        output_summary: Some(format!("restart queue checkpoint {seq}")),
        changes: json!({ "seq": seq, "files": [format!("file-{seq}.txt")] }),
        started_at: Some("2026-06-12T00:00:00Z".to_string()),
        finished_at: Some("2026-06-12T00:00:01Z".to_string()),
    }
}

pub(crate) fn assert_ok(response: SessionLogResponse) -> Result<()> {
    match response {
        SessionLogResponse::Ok | SessionLogResponse::SessionCommandApplied { .. } => Ok(()),
        SessionLogResponse::Error { error } => bail!("session_db returned error: {error}"),
        other => bail!("session_db returned unexpected response: {other:?}"),
    }
}

pub(crate) fn assert_session_snapshot(
    session_id: &str,
    workspace: &str,
    state: &str,
    status: &str,
    message_count: u64,
    expected_task_status: Option<&str>,
) -> Result<()> {
    match session_log::ipc::call_service(&SessionLogCommand::GetSession(GetSessionRequest {
        session_id: session_id.to_string(),
    }))? {
        SessionLogResponse::Session { session } => {
            let snapshot = session.ok_or_else(|| anyhow!("session {session_id} missing"))?;
            assert_eq!(snapshot.session_id, session_id);
            assert_eq!(snapshot.workspace, workspace);
            assert_eq!(snapshot.state.as_deref(), Some(state));
            assert_eq!(snapshot.status.as_deref(), Some(status));
            assert_eq!(snapshot.message_count, message_count);
            assert_eq!(snapshot.session["status"].as_str(), Some(status));
            assert_eq!(snapshot.management["state"].as_str(), Some(state));
            if let Some(expected_task_status) = expected_task_status {
                let tasks = snapshot
                    .task_management
                    .get("tasks")
                    .and_then(serde_json::Value::as_array)
                    .ok_or_else(|| anyhow!("task management tasks missing"))?;
                assert!(
                    tasks
                        .iter()
                        .any(|task| task["status"].as_str() == Some(expected_task_status)),
                    "session {session_id} should retain task status {expected_task_status}: {tasks:?}"
                );
            }
            Ok(())
        }
        other => bail!("unexpected get session response: {other:?}"),
    }
}

pub(crate) fn assert_session_missing(session_id: &str) -> Result<()> {
    match session_log::ipc::call_service(&SessionLogCommand::GetSession(GetSessionRequest {
        session_id: session_id.to_string(),
    }))? {
        SessionLogResponse::Session { session } => {
            assert!(session.is_none(), "session {session_id} should be missing");
            Ok(())
        }
        other => bail!("unexpected get missing session response: {other:?}"),
    }
}

pub(crate) fn assert_records(session_id: &str, expected: &[&str]) -> Result<()> {
    assert_eq!(record_ids(session_id)?, expected);
    Ok(())
}

pub(crate) fn record_ids(session_id: &str) -> Result<Vec<String>> {
    match session_log::ipc::call_service(&SessionLogCommand::ListSessionRecords(
        ListSessionRecordsRequest {
            session_id: session_id.to_string(),
            page: 0,
            page_size: 50,
        },
    ))? {
        SessionLogResponse::Records { page, records } => {
            assert_eq!(page.total as usize, records.len());
            Ok(records
                .into_iter()
                .map(|record| record.message_id)
                .collect())
        }
        other => bail!("unexpected records response: {other:?}"),
    }
}

pub(crate) fn assert_workspace_page(
    workspace: &str,
    page: u64,
    page_size: u64,
    total: u64,
    expected_len: usize,
) -> Result<()> {
    match session_log::ipc::call_service(&SessionLogCommand::ListSessions(ListSessionsRequest {
        workspace: workspace.to_string(),
        page,
        page_size,
    }))? {
        SessionLogResponse::Sessions {
            page: actual_page,
            sessions,
        } => {
            assert_eq!(actual_page.total, total);
            assert_eq!(actual_page.page_size, page_size.clamp(1, 500));
            assert_eq!(sessions.len(), expected_len);
            assert!(sessions
                .iter()
                .all(|session| session.workspace == workspace));
            Ok(())
        }
        other => bail!("unexpected list sessions response: {other:?}"),
    }
}

pub(crate) fn workspace_session_total(workspace: &str) -> Option<u64> {
    match session_log::ipc::call_service(&SessionLogCommand::ListSessions(ListSessionsRequest {
        workspace: workspace.to_string(),
        page: 0,
        page_size: 1,
    })) {
        Ok(SessionLogResponse::Sessions { page, .. }) => Some(page.total),
        _ => None,
    }
}

pub(crate) fn assert_workspace_summaries(expected: &[(String, u64)]) -> Result<()> {
    match session_log::ipc::call_service(&SessionLogCommand::ListWorkspaces)? {
        SessionLogResponse::Workspaces { workspaces } => {
            for (workspace, count) in expected {
                let summary = workspaces
                    .iter()
                    .find(|summary| &summary.directory == workspace)
                    .ok_or_else(|| anyhow!("workspace summary missing for {workspace}"))?;
                assert_eq!(summary.session_count, *count);
                assert!(summary.last_updated_at > 0);
            }
            Ok(())
        }
        other => bail!("unexpected workspace summary response: {other:?}"),
    }
}

pub(crate) fn assert_checkpoint_rows(home: &Path, session_id: &str, expected: i64) -> Result<()> {
    let count = checkpoint_row_count(home, session_id);
    assert_eq!(
        count, expected,
        "session {session_id} should have {expected} durable checkpoint rows"
    );
    Ok(())
}

pub(crate) fn checkpoint_row_count(home: &Path, session_id: &str) -> i64 {
    let Ok(conn) = Connection::open(index_db_path(home)) else {
        return 0;
    };
    conn.query_row(
        "SELECT COUNT(*) FROM session_write_queue
         WHERE session_id = ?1
           AND turn_id = 'turn-restart-queue'
           AND runtime_worker_id = 'runtime-worker-restart-queue'
           AND command_run_id = 'command-run-restart-queue'
           AND status = 'applied'",
        [session_id],
        |row| row.get(0),
    )
    .unwrap_or(0)
}

pub(crate) fn assert_index_state_matches_workspace_state(
    home: &Path,
    session_ids: &[&str],
) -> Result<()> {
    let conn = Connection::open(index_db_path(home)).context("open index db")?;
    for session_id in session_ids {
        let (state, status, management): (String, String, String) = conn.query_row(
            "SELECT state, status, management_json FROM sessions WHERE session_id = ?1",
            [session_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )?;
        let snapshot = get_session_snapshot(session_id)?
            .ok_or_else(|| anyhow!("snapshot missing for {session_id}"))?;
        assert_eq!(Some(state.as_str()), snapshot.state.as_deref());
        assert_eq!(Some(status.as_str()), snapshot.status.as_deref());
        let management: serde_json::Value = serde_json::from_str(&management)?;
        assert_eq!(management["state"].as_str(), snapshot.state.as_deref());
    }
    Ok(())
}

pub(crate) fn get_session_snapshot(
    session_id: &str,
) -> Result<Option<Box<session_log_contract::SessionSnapshot>>> {
    match session_log::ipc::call_service(&SessionLogCommand::GetSession(GetSessionRequest {
        session_id: session_id.to_string(),
    }))? {
        SessionLogResponse::Session { session } => Ok(session),
        other => bail!("unexpected get session response: {other:?}"),
    }
}

pub(crate) fn session_visible(session_id: &str) -> bool {
    get_session_snapshot(session_id).ok().flatten().is_some()
}

pub(crate) fn session_missing(session_id: &str) -> bool {
    get_session_snapshot(session_id)
        .ok()
        .is_some_and(|session| session.is_none())
}

pub(crate) fn session_message_count(session_id: &str) -> Option<u64> {
    get_session_snapshot(session_id)
        .ok()
        .flatten()
        .map(|snapshot| snapshot.message_count)
}

pub(crate) fn session_state_status(session_id: &str) -> Option<(String, String)> {
    let snapshot = get_session_snapshot(session_id).ok().flatten()?;
    Some((snapshot.state?, snapshot.status?))
}

pub(crate) fn write_corrupt_pending_queue_item(home: &Path, label: &str) -> Result<PathBuf> {
    let pending = queue_dir(home, "pending");
    std::fs::create_dir_all(&pending)?;
    let path = pending.join(format!(
        "00000000000000000000-0-00000000000000000000-{label}.json"
    ));
    std::fs::write(&path, b"{ this is not valid json")?;
    Ok(path)
}

pub(crate) fn assert_failed_queue_contains_error(home: &Path, label: &str) -> Result<()> {
    let failed = queue_dir(home, "failed");
    let entries = std::fs::read_dir(&failed)
        .with_context(|| format!("read failed queue {}", failed.display()))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    let failed_json = entries.iter().any(|entry| {
        entry.file_name().to_string_lossy().contains(label)
            && entry.path().extension().and_then(|value| value.to_str()) == Some("json")
    });
    let failed_error = entries.iter().any(|entry| {
        entry.file_name().to_string_lossy().contains(label)
            && entry.path().extension().and_then(|value| value.to_str()) == Some("txt")
    });
    assert!(
        failed_json,
        "failed queue should retain the corrupt json file"
    );
    assert!(failed_error, "failed queue should include an error sidecar");
    Ok(())
}

pub(crate) fn assert_pending_queue_empty(home: &Path) -> Result<()> {
    let pending_json = pending_queue_items(home);
    assert_eq!(pending_json, 0, "pending queue should be empty");
    Ok(())
}

pub(crate) fn pending_queue_items(home: &Path) -> usize {
    std::fs::read_dir(queue_dir(home, "pending"))
        .ok()
        .into_iter()
        .flat_map(|entries| entries.flatten())
        .filter(|entry| entry.path().extension().and_then(|value| value.to_str()) == Some("json"))
        .count()
}

pub(crate) fn failed_queue_items(home: &Path) -> usize {
    std::fs::read_dir(queue_dir(home, "failed"))
        .ok()
        .into_iter()
        .flat_map(|entries| entries.flatten())
        .filter(|entry| entry.path().extension().and_then(|value| value.to_str()) == Some("json"))
        .count()
}

pub(crate) fn assert_workspace_db_exists(workspace: &Path) -> Result<()> {
    let db = workspace.join(".tura").join("session_log.sqlite3");
    assert!(db.exists(), "workspace DB should exist at {}", db.display());
    Ok(())
}

pub(crate) fn queue_dir(home: &Path, segment: &str) -> PathBuf {
    home.join("db")
        .join("session_log")
        .join("message_queue")
        .join(segment)
}

pub(crate) fn index_db_path(home: &Path) -> PathBuf {
    home.join("db").join("session_log").join("index.sqlite3")
}

pub(crate) fn wait_until(timeout: Duration, mut condition: impl FnMut() -> bool) -> Result<()> {
    let started = Instant::now();
    while started.elapsed() < timeout {
        if condition() {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(25));
    }
    bail!("timed out after {}ms", timeout.as_millis())
}

pub(crate) struct SessionDbService {
    handle: Option<thread::JoinHandle<Result<()>>>,
}

impl SessionDbService {
    pub(crate) fn start() -> Result<Self> {
        let handle = thread::spawn(session_log::service::run_socket_service);
        wait_until(
            Duration::from_secs(10),
            session_log::ipc::service_is_running,
        )
        .context("session_db service did not become reachable")?;
        Ok(Self {
            handle: Some(handle),
        })
    }

    pub(crate) fn shutdown(&mut self) -> Result<()> {
        if self.handle.is_none() {
            return Ok(());
        }
        assert_ok(session_log::ipc::call_service(
            &SessionLogCommand::Shutdown,
        )?)?;
        self.join(Duration::from_secs(10))
    }

    fn join(&mut self, timeout: Duration) -> Result<()> {
        let started = Instant::now();
        while started.elapsed() < timeout {
            if self
                .handle
                .as_ref()
                .is_some_and(thread::JoinHandle::is_finished)
            {
                let handle = self
                    .handle
                    .take()
                    .ok_or_else(|| anyhow!("service handle missing"))?;
                return handle
                    .join()
                    .map_err(|_| anyhow!("session_db service thread panicked"))?;
            }
            thread::sleep(Duration::from_millis(25));
        }
        bail!(
            "session_db service did not stop within {}ms",
            timeout.as_millis()
        )
    }
}

impl Drop for SessionDbService {
    fn drop(&mut self) {
        if self.handle.is_some() {
            let _ = session_log::ipc::call_service(&SessionLogCommand::Shutdown);
            let _ = self.join(Duration::from_secs(5));
        }
    }
}

pub(crate) struct TestEnv {
    _temp: tempfile::TempDir,
    pub(crate) home: PathBuf,
    root: PathBuf,
    _env: EnvGuard,
}

impl TestEnv {
    pub(crate) fn new(name: &str) -> Result<Self> {
        let temp = tempfile::tempdir().context("create temp test root")?;
        let root = temp.path().join(name);
        let home = root.join("home");
        std::fs::create_dir_all(&home)?;
        let env = EnvGuard::set(&[
            ("TURA_HOME", Some(home.as_path())),
            ("TURA_DB_ROOT", None),
            ("SESSION_LOG_DB_ROOT", None),
            ("TURA_SESSION_DB_PROBE_TIMEOUT_MS", Some(Path::new("25"))),
        ]);
        Ok(Self {
            _temp: temp,
            home,
            root,
            _env: env,
        })
    }

    pub(crate) fn workspace(&self, name: &str) -> Result<PathBuf> {
        let workspace = self.root.join(name);
        std::fs::create_dir_all(&workspace)?;
        Ok(workspace)
    }

    pub(crate) fn session_id(&self, name: &str) -> String {
        format!(
            "{}-{}-{}",
            name,
            std::process::id(),
            self.root
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or("session")
        )
    }
}

pub(crate) struct EnvGuard {
    previous: Vec<(&'static str, Option<std::ffi::OsString>)>,
}

impl EnvGuard {
    pub(crate) fn set(values: &[(&'static str, Option<&Path>)]) -> Self {
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
