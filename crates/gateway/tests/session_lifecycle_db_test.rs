use axum::extract::{Json, Path};
use gateway::api::session::{delete_session, fork_session};
use gateway::contracts::ForkSessionRequest;
use gateway::session::MessageRole;
use gateway::session_store;
use session_log::{SessionLogCommand, SessionLogStore};
use std::path::Path as FsPath;
use std::time::{Duration, Instant};

#[tokio::test]
async fn fork_and_delete_are_applied_to_session_db() -> anyhow::Result<()> {
    let root = tempfile::tempdir()?;
    let home = root.path().join("home");
    let workspace = root.path().join("workspace");
    std::fs::create_dir_all(&home)?;
    std::fs::create_dir_all(&workspace)?;
    let _env = EnvGuard::new(&home);
    let _service = ServiceThread::start()?;

    let workspace_key = session_log::path::normalize_workspace(&workspace.to_string_lossy());
    let source = session_store().create_session(
        Some(workspace_key.clone()),
        Some("db-test-model".to_string()),
        Some("thinking-planning".to_string()),
        Some("coding".to_string()),
        false,
        false,
        false,
        None,
        false,
        false,
    );
    session_store().add_message(
        &source.id,
        MessageRole::User,
        "persist this context before fork".to_string(),
    );

    let Json(forked) = fork_session(
        Path(source.id.clone()),
        Json(ForkSessionRequest {
            directory: Some(workspace_key),
            model: None,
            agent: None,
            copy_context: Some(true),
        }),
    )
    .await;

    assert_ne!(
        forked.id, source.id,
        "fork must create a real new session id for router/runtime turns"
    );
    assert_eq!(forked.parent_id.as_deref(), Some(source.id.as_str()));

    let persisted = get_persisted_session(&forked.id)?.expect("forked session should be in DB");
    assert_eq!(persisted.parent_id.as_deref(), Some(source.id.as_str()));
    assert_eq!(persisted.message_count, 1);
    let records = list_persisted_records(&forked.id)?;
    assert_eq!(records.len(), 1);
    assert!(
        serde_json::to_string(&records[0].record)?.contains("persist this context before fork"),
        "forked session DB record should contain copied context: {records:#?}"
    );

    let Json(deleted) = delete_session(Path(forked.id.clone())).await;
    assert!(deleted, "delete endpoint should report successful deletion");
    assert!(
        get_persisted_session(&forked.id)?.is_none(),
        "deleted session must not reappear from session DB after refresh"
    );
    assert!(
        session_store().get_session(&forked.id).is_none(),
        "deleted session must also be removed from gateway memory"
    );

    Ok(())
}

fn get_persisted_session(
    session_id: &str,
) -> anyhow::Result<Option<Box<session_log::SessionSnapshot>>> {
    let response = session_log::ipc::call_service(&SessionLogCommand::GetSession(
        session_log::GetSessionRequest {
            session_id: session_id.to_string(),
        },
    ))?;
    match response {
        session_log::SessionLogResponse::Session { session } => Ok(session),
        session_log::SessionLogResponse::Error { error } => anyhow::bail!("{error}"),
        other => anyhow::bail!("unexpected session_log get session response: {other:?}"),
    }
}

fn list_persisted_records(session_id: &str) -> anyhow::Result<Vec<session_log::SessionRecord>> {
    let response = session_log::ipc::call_service(&SessionLogCommand::ListSessionRecords(
        session_log::ListSessionRecordsRequest {
            session_id: session_id.to_string(),
            page: 0,
            page_size: 50,
        },
    ))?;
    match response {
        session_log::SessionLogResponse::Records { records, .. } => Ok(records),
        session_log::SessionLogResponse::Error { error } => anyhow::bail!("{error}"),
        other => anyhow::bail!("unexpected session_log records response: {other:?}"),
    }
}

struct ServiceThread {
    handle: Option<std::thread::JoinHandle<anyhow::Result<()>>>,
}

impl ServiceThread {
    fn start() -> anyhow::Result<Self> {
        let store = SessionLogStore::open_default()?;
        let handle = std::thread::spawn(move || session_log::ipc::serve_blocking(store));
        wait_until(
            Duration::from_secs(10),
            session_log::ipc::service_is_running,
        )?;
        Ok(Self {
            handle: Some(handle),
        })
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

struct EnvGuard {
    previous: Vec<(&'static str, Option<std::ffi::OsString>)>,
}

impl EnvGuard {
    fn new(home: &FsPath) -> Self {
        let keys = ["TURA_HOME", "TURA_DB_ROOT", "SESSION_LOG_DB_ROOT"];
        let previous = keys
            .iter()
            .map(|key| (*key, std::env::var_os(key)))
            .collect::<Vec<_>>();
        std::env::set_var("TURA_HOME", home);
        std::env::remove_var("TURA_DB_ROOT");
        std::env::remove_var("SESSION_LOG_DB_ROOT");
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

fn wait_until(timeout: Duration, mut condition: impl FnMut() -> bool) -> anyhow::Result<()> {
    let started = Instant::now();
    while started.elapsed() < timeout {
        if condition() {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    anyhow::bail!("condition was not met within {}ms", timeout.as_millis())
}
