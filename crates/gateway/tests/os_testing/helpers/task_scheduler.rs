pub(crate) use axum::extract::{Json, Path};
pub(crate) use chrono::{DateTime, Utc};
pub(crate) use gateway::api::session::{
    run_due_task_scheduler_tick_for_business_test,
    run_due_task_scheduler_tick_for_store_business_test, update_session_task_management,
    update_session_task_management_value,
};
pub(crate) use gateway::contracts::{
    SessionStatus as ApiSessionStatus, UpdateSessionTaskManagementRequest,
};
pub(crate) use gateway::session::MessageRole;
pub(crate) use gateway::{session_store, SessionStore};
pub(crate) use lifecycle::SessionCommand;
pub(crate) use serde_json::json;
pub(crate) use session_log::SessionLogStore;
pub(crate) use session_log_contract::{
    SessionLogCommand, SessionLogResponse, UpsertSessionRequest,
};
pub(crate) use std::path::Path as StdPath;
pub(crate) use std::sync::Arc;
pub(crate) use std::time::{Duration, Instant};
pub(crate) use tokio::sync::Barrier;

pub(crate) static SCHEDULER_ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

pub(crate) fn create_scheduler_session(
    store: &SessionStore,
    directory: Option<String>,
    model: Option<String>,
    agent: Option<String>,
    session_type: Option<String>,
    kill_processes_on_start: bool,
    validator_enabled: bool,
    force_planning: bool,
    model_variant: Option<String>,
    model_acceleration_enabled: bool,
    disable_permission_restrictions: bool,
) -> gateway::contracts::Session {
    let info = store.build_session_info(
        directory,
        model,
        agent,
        session_type,
        kill_processes_on_start,
        validator_enabled,
        force_planning,
        model_variant,
        model_acceleration_enabled,
        disable_permission_restrictions,
    );
    let task_plan = info.management.task_plan.clone();
    store
        .create_canonical_session(info, SessionCommand::CreateSession { task_plan })
        .expect("canonical scheduler session should be created")
}

pub(crate) fn execute_scheduler_command(session_id: &str, command: SessionCommand) {
    session_store()
        .execute_canonical_session_command(session_id, command)
        .expect("canonical scheduler command should succeed");
}

pub(crate) struct SchedulerTestDb {
    _guard: std::sync::MutexGuard<'static, ()>,
    _root: tempfile::TempDir,
    _env: SchedulerEnvGuard,
    _service: SchedulerServiceThread,
}

impl SchedulerTestDb {
    pub(crate) fn start() -> Self {
        let guard = SCHEDULER_ENV_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let root = tempfile::tempdir().expect("scheduler test DB root");
        let home = root.path().join("home");
        std::fs::create_dir_all(&home).expect("scheduler test DB home");
        let env = SchedulerEnvGuard::new(&home);
        let service = SchedulerServiceThread::start().expect("start scheduler test DB");
        Self {
            _guard: guard,
            _root: root,
            _env: env,
            _service: service,
        }
    }
}

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
    let response =
        session_log::ipc::call_service(&SessionLogCommand::UpsertSession(UpsertSessionRequest {
            session: serde_json::to_value(info)?,
            parent_id: None,
            messages: store
                .get_messages(session_id)
                .into_iter()
                .map(serde_json::to_value)
                .collect::<Result<Vec<_>, _>>()?,
            todos: store.get_todos(session_id),
        }))?;
    match response {
        SessionLogResponse::Ok => Ok(()),
        SessionLogResponse::Error { error } => anyhow::bail!("{error}"),
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
}

impl SchedulerServiceThread {
    pub(crate) fn start() -> anyhow::Result<Self> {
        let store = SessionLogStore::open_default()?;
        let handle = std::thread::spawn(move || session_log::ipc::serve_blocking(store));
        wait_for_scheduler_condition(
            Duration::from_secs(10),
            session_log::ipc::service_is_running,
        )?;
        Ok(Self {
            handle: Some(handle),
        })
    }

    pub(crate) fn shutdown(mut self, timeout: Duration) -> anyhow::Result<()> {
        self.shutdown_with_timeout(timeout)
    }

    fn shutdown_with_timeout(&mut self, timeout: Duration) -> anyhow::Result<()> {
        if self.handle.is_none() {
            return Ok(());
        }
        let shutdown_error = request_session_db_shutdown_with_timeout(timeout).err();
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

fn request_session_db_shutdown_with_timeout(timeout: Duration) -> anyhow::Result<()> {
    let (sender, receiver) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let result = session_log::ipc::call_service(&SessionLogCommand::Shutdown);
        let _ = sender.send(result);
    });
    match receiver.recv_timeout(timeout) {
        Ok(Ok(SessionLogResponse::Ok)) => Ok(()),
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
