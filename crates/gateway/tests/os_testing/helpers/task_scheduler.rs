pub(crate) use axum::extract::{Json, Path};
pub(crate) use chrono::{DateTime, Utc};
pub(crate) use gateway::api::session::{
    run_due_task_scheduler_tick_for_business_test,
    run_due_task_scheduler_tick_for_store_business_test, update_session_task_management,
    UpdateSessionTaskManagementRequest,
};
pub(crate) use gateway::contracts::SessionStatus as ApiSessionStatus;
pub(crate) use gateway::session::MessageRole;
pub(crate) use gateway::{session_store, SessionStatus as StoreSessionStatus, SessionStore};
pub(crate) use serde_json::json;
pub(crate) use session_log::{SessionLogCommand, SessionLogStore};
use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
pub(crate) use std::path::Path as StdPath;
pub(crate) use std::sync::Arc;
pub(crate) use std::time::{Duration, Instant};
pub(crate) use tokio::sync::Barrier;

pub(crate) static SCHEDULER_ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

pub(crate) async fn set_polling_task_due(session_id: &str, summary: &str, due: DateTime<Utc>) {
    let _ = update_session_task_management(
        Path(session_id.to_string()),
        Json(UpdateSessionTaskManagementRequest {
            task_management: json!({
                "task_id": "poll-loop",
                "task_summary": summary,
                "status": "todo",
                "start_at": due.to_rfc3339(),
                "poll_interval": { "d": 0, "h": 0, "m": 0, "s": 30 }
            }),
        }),
    )
    .await;
}

pub(crate) fn assert_scheduler_triggered_in_store(
    store: &SessionStore,
    session_id: &str,
    expected_condition: &str,
    expected_reason: &str,
    expected_summary: &str,
) {
    let session = store
        .get_session(session_id)
        .expect("scheduled session should exist");
    assert_eq!(session.status, ApiSessionStatus::Busy);
    assert_task_management_marks_expected_task_doing(&session.task_management, expected_summary);

    let messages = store.get_messages(session_id);
    assert_eq!(messages.len(), 1);
    let message = &messages[0];
    assert_eq!(message.role, MessageRole::User);
    let message_text = message
        .parts
        .iter()
        .find_map(|part| part.text.as_deref().or(part.content.as_deref()))
        .expect("scheduler message should have text content");
    assert!(
        message_text.contains(expected_reason),
        "scheduler prompt should explain trigger reason: {message_text}"
    );
    assert!(
        message_text.contains(expected_summary),
        "scheduler prompt should include task summary: {message_text}"
    );
    assert_eq!(
        message
            .parts
            .first()
            .and_then(|part| part.metadata.as_ref())
            .and_then(|metadata| metadata.get("kind")),
        Some(&json!("task_scheduler"))
    );
    assert_eq!(
        message
            .parts
            .first()
            .and_then(|part| part.metadata.as_ref())
            .and_then(|metadata| metadata.get("start_condition")),
        Some(&json!(expected_condition))
    );

    let todos = store.get_todos(session_id);
    assert_eq!(todos.len(), 1);
    assert_eq!(todos[0]["status"], "in_progress");
    assert_eq!(todos[0]["content"], expected_summary);
}

pub(crate) fn assert_scheduler_triggered(
    session_id: &str,
    expected_condition: &str,
    expected_reason: &str,
    expected_summary: &str,
) {
    let session = session_store()
        .get_session(session_id)
        .expect("scheduled session should exist");
    assert_eq!(session.status, ApiSessionStatus::Busy);
    assert_task_management_marks_expected_task_doing(&session.task_management, expected_summary);

    let messages = session_store().get_messages(session_id);
    assert_eq!(messages.len(), 1);
    let message = &messages[0];
    assert_eq!(message.role, MessageRole::User);
    let message_text = message
        .parts
        .iter()
        .find_map(|part| part.text.as_deref().or(part.content.as_deref()))
        .expect("scheduler message should have text content");
    assert!(
        message_text.contains(expected_reason),
        "scheduler prompt should explain trigger reason: {message_text}"
    );
    assert!(
        message_text.contains(expected_summary),
        "scheduler prompt should include task summary: {message_text}"
    );
    assert_eq!(
        message
            .parts
            .first()
            .and_then(|part| part.metadata.as_ref())
            .and_then(|metadata| metadata.get("kind")),
        Some(&json!("task_scheduler"))
    );
    assert_eq!(
        message
            .parts
            .first()
            .and_then(|part| part.metadata.as_ref())
            .and_then(|metadata| metadata.get("start_condition")),
        Some(&json!(expected_condition))
    );

    let todos = session_store().get_todos(session_id);
    assert_eq!(todos.len(), 1);
    assert_eq!(todos[0]["status"], "in_progress");
    assert_eq!(todos[0]["content"], expected_summary);
}

pub(crate) fn assert_task_management_marks_expected_task_doing(
    task_management: &serde_json::Value,
    expected_summary: &str,
) {
    if let Some(tasks) = task_management
        .get("tasks")
        .and_then(serde_json::Value::as_array)
    {
        let task = tasks
            .iter()
            .find(|task| task.get("task_summary") == Some(&json!(expected_summary)))
            .unwrap_or_else(|| panic!("expected triggered task summary {expected_summary}"));
        assert_eq!(task["status"], "doing");
    } else {
        assert_eq!(task_management["status"], "doing");
        assert_eq!(task_management["task_summary"], expected_summary);
    }
}

pub(crate) fn task_by_id<'a>(
    tasks: &'a [serde_json::Value],
    task_id: &str,
) -> &'a serde_json::Value {
    tasks
        .iter()
        .find(|task| task.get("task_id") == Some(&json!(task_id)))
        .unwrap_or_else(|| panic!("task {task_id} should exist"))
}

pub(crate) fn task_array(task_management: &serde_json::Value) -> &[serde_json::Value] {
    task_management
        .get("tasks")
        .and_then(serde_json::Value::as_array)
        .map(Vec::as_slice)
        .expect("task_management.tasks should be an array")
}

pub(crate) fn assert_no_scheduler_side_effects(
    session_id: &str,
    expected_status: ApiSessionStatus,
    label: &str,
) {
    assert_eq!(
        session_store()
            .get_session(session_id)
            .unwrap_or_else(|| panic!("{label} session should exist"))
            .status,
        expected_status
    );
    assert_eq!(
        session_store().get_messages(session_id).len(),
        0,
        "{label} session should not receive scheduler messages"
    );
    assert!(
        session_store().get_todos(session_id).is_empty(),
        "{label} session should not receive scheduler todos"
    );
}

pub(crate) fn upsert_runtime_owned_scheduler_snapshot(
    store: &SessionStore,
    session_id: &str,
    workspace: &StdPath,
) -> anyhow::Result<()> {
    let mut info = store
        .get_session_info(session_id)
        .unwrap_or_else(|| panic!("session {session_id} should exist before runtime DB upsert"));
    info.directory = Some(workspace.to_string_lossy().to_string());
    info.message_count = store.get_messages(session_id).len();
    let response = session_log::ipc::call_service(&SessionLogCommand::UpsertSession(
        session_log::UpsertSessionRequest {
            session: serde_json::to_value(info)?,
            parent_id: None,
            messages: store
                .get_messages(session_id)
                .into_iter()
                .map(serde_json::to_value)
                .collect::<Result<Vec<_>, _>>()?,
            todos: store.get_todos(session_id),
        },
    ))?;
    match response {
        session_log::SessionLogResponse::Ok => Ok(()),
        session_log::SessionLogResponse::Error { error } => anyhow::bail!("{error}"),
        other => anyhow::bail!("unexpected session_log upsert response: {other:?}"),
    }
}

pub(crate) struct SchedulerEnvGuard {
    previous: Vec<(&'static str, Option<std::ffi::OsString>)>,
}

impl SchedulerEnvGuard {
    pub(crate) fn new(home: &StdPath) -> Self {
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

impl Drop for SchedulerEnvGuard {
    fn drop(&mut self) {
        for (key, value) in self.previous.drain(..) {
            match value {
                Some(value) => std::env::set_var(key, value),
                None => std::env::remove_var(key),
            }
        }
    }
}

pub(crate) struct SchedulerServiceThread {
    handle: Option<std::thread::JoinHandle<anyhow::Result<()>>>,
    endpoint: session_log::ipc::ServiceEndpoint,
}

impl SchedulerServiceThread {
    pub(crate) fn start() -> anyhow::Result<Self> {
        let store = SessionLogStore::open_default()?;
        let handle = std::thread::spawn(move || session_log::ipc::serve_blocking(store));
        wait_for_scheduler_condition(
            Duration::from_secs(10),
            session_log::ipc::service_is_running,
        )?;
        let endpoint = read_scheduler_service_endpoint()?;
        Ok(Self {
            handle: Some(handle),
            endpoint,
        })
    }

    pub(crate) fn shutdown(mut self, timeout: Duration) -> anyhow::Result<()> {
        self.shutdown_with_timeout(timeout)
    }

    fn shutdown_with_timeout(&mut self, timeout: Duration) -> anyhow::Result<()> {
        if self.handle.is_none() {
            return Ok(());
        }
        let shutdown_error =
            request_session_db_shutdown_with_timeout(timeout, Some(&self.endpoint.addr)).err();
        let handle = self
            .handle
            .take()
            .expect("checked scheduler service handle");
        let (sender, receiver) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _ = sender.send(handle.join());
        });
        match receiver.recv_timeout(timeout) {
            Ok(Ok(Ok(()))) => Ok(()),
            Ok(Ok(Err(error))) => Err(error),
            Ok(Err(_)) => anyhow::bail!("session_db service thread panicked during shutdown"),
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                if let Some(error) = shutdown_error {
                    anyhow::bail!(
                        "session_db shutdown request failed and service thread did not stop within {}ms: {error:#}",
                        timeout.as_millis()
                    );
                }
                anyhow::bail!(
                    "session_db service thread did not stop within {}ms",
                    timeout.as_millis()
                );
            }
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                anyhow::bail!("session_db service shutdown waiter disconnected")
            }
        }
    }
}

impl Drop for SchedulerServiceThread {
    fn drop(&mut self) {
        if let Err(error) = self.shutdown_with_timeout(Duration::from_secs(5)) {
            eprintln!("failed to stop scheduler session_db service cleanly: {error:#}");
        }
    }
}

fn request_session_db_shutdown_with_timeout(
    timeout: Duration,
    fallback_addr: Option<&str>,
) -> anyhow::Result<()> {
    let (sender, receiver) = std::sync::mpsc::channel();
    let fallback_addr = fallback_addr.map(ToString::to_string);
    std::thread::spawn(move || {
        let result = session_log::ipc::call_service(&SessionLogCommand::Shutdown).or_else(|error| {
            let Some(addr) = fallback_addr.as_deref() else {
                return Err(error);
            };
            send_shutdown_to_addr(addr, timeout).map_err(|fallback_error| {
                anyhow::anyhow!(
                    "session_db shutdown via addr file failed: {error:#}; fallback addr {addr} failed: {fallback_error:#}"
                )
            })
        });
        let _ = sender.send(result);
    });
    match receiver.recv_timeout(timeout) {
        Ok(Ok(session_log::SessionLogResponse::Ok)) => Ok(()),
        Ok(Ok(response)) => anyhow::bail!("unexpected session_db shutdown response: {response:?}"),
        Ok(Err(error)) => Err(error),
        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => anyhow::bail!(
            "session_db shutdown request did not complete within {}ms",
            timeout.as_millis()
        ),
        Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
            anyhow::bail!("session_db shutdown request waiter disconnected")
        }
    }
}

fn read_scheduler_service_endpoint() -> anyhow::Result<session_log::ipc::ServiceEndpoint> {
    let path = session_log::ipc::service_addr_path();
    let raw = std::fs::read_to_string(&path)
        .map_err(|error| anyhow::anyhow!("failed to read {}: {error}", path.display()))?;
    serde_json::from_str(raw.trim()).map_err(|error| {
        anyhow::anyhow!(
            "failed to parse scheduler session_db endpoint {}: {error}",
            path.display()
        )
    })
}

fn send_shutdown_to_addr(
    addr: &str,
    timeout: Duration,
) -> anyhow::Result<session_log::SessionLogResponse> {
    let mut stream = TcpStream::connect(addr)
        .map_err(|error| anyhow::anyhow!("failed to connect to {addr}: {error}"))?;
    stream.set_read_timeout(Some(timeout))?;
    stream.set_write_timeout(Some(timeout))?;
    let line = serde_json::to_string(&SessionLogCommand::Shutdown)?;
    stream.write_all(line.as_bytes())?;
    stream.write_all(b"\n")?;
    stream.flush()?;

    let mut response = String::new();
    BufReader::new(stream).read_line(&mut response)?;
    serde_json::from_str(response.trim()).map_err(|error| {
        anyhow::anyhow!("invalid session_db shutdown response from {addr}: {error}")
    })
}

pub(crate) fn wait_for_scheduler_condition(
    timeout: Duration,
    mut condition: impl FnMut() -> bool,
) -> anyhow::Result<()> {
    let started = Instant::now();
    while started.elapsed() < timeout {
        if condition() {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    anyhow::bail!("condition was not met within {}ms", timeout.as_millis())
}
