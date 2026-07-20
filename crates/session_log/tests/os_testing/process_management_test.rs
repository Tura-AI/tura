//! Process-management stability tests (plan §A, T-PM).
//!
//! These exercise the single-DB-owner + concurrent socket IPC invariants
//! end-to-end against a real `tura_session_db` service process, with no LLM
//! involved. They are the long-term regression for the refactor's core risk:
//! one session_db owner, concurrent multiplexed clients, no head-of-line
//! blocking, SQLite workspace-log placement, and version handshake.

use std::path::Path;
use std::process::{Child, Command};
use std::time::{Duration, Instant};

use session_log_contract::{
    DeleteSessionRequest, DeleteWorkspaceRequest, GetSessionRequest, ListSessionsRequest,
    SessionLogCommand, SessionLogResponse, UpsertSessionRequest,
};

/// Path to the `tura_session_db` binary built by cargo for this crate.
const SESSION_DB_BIN: &str = env!("CARGO_BIN_EXE_tura_session_db");

/// These tests drive process-global env (`SESSION_LOG_DB_ROOT`) and a real
/// service, so they must run one at a time even under the parallel runner.
static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());

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

struct ServiceGuard {
    child: Child,
    _env: EnvRestore,
}

impl Drop for ServiceGuard {
    fn drop(&mut self) {
        let _ = session_log::ipc::call_service(&SessionLogCommand::Shutdown);
        if !wait_for_child_exit(&mut self.child, Duration::from_secs(10)) {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

fn wait_for_child_exit(child: &mut Child, timeout: Duration) -> bool {
    let started = Instant::now();
    while started.elapsed() < timeout {
        if matches!(child.try_wait(), Ok(Some(_))) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    false
}

/// Start a `tura_session_db` service against an isolated db root and wait
/// until its socket is reachable. The current test process is pointed at the
/// same db root so `session_log::ipc` resolves the same endpoint file.
fn start_service(db_root: &Path) -> ServiceGuard {
    let env = EnvRestore::capture(&["SESSION_LOG_DB_ROOT", "TURA_DB_ROOT"]);
    std::env::set_var("SESSION_LOG_DB_ROOT", db_root);
    std::env::remove_var("TURA_DB_ROOT");

    let child = Command::new(SESSION_DB_BIN)
        .env("SESSION_LOG_DB_ROOT", db_root)
        .env("TURA_ROLE", "session_db")
        .spawn()
        .expect("spawn tura_session_db");

    let guard = ServiceGuard { child, _env: env };
    let started = Instant::now();
    while started.elapsed() < Duration::from_secs(90) {
        if session_log::ipc::service_is_running() {
            return guard;
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    panic!("session_db service did not become reachable within 90s");
}

fn upsert(session_id: &str, workspace: &str) -> SessionLogCommand {
    SessionLogCommand::UpsertSession(UpsertSessionRequest {
        session: serde_json::json!({
            "id": session_id,
            "name": "PM",
            "directory": workspace,
            "created_at": 1,
            "updated_at": 1,
            "status": "idle",
            "management": { "session_id": session_id, "session_name": "PM", "state": "created" }
        }),
        parent_id: None,
        messages: vec![serde_json::json!({
            "id": format!("m-{session_id}"), "role": "assistant", "created_at": 1, "updated_at": 1
        })],
        todos: vec![],
    })
}

/// T-PM1 / T-PM3: 16 concurrent clients write+read through the single service
/// over the socket; every request is answered correctly (no head-of-line
/// blocking, no cross-talk) and logs are placed in workspace-local SQLite DBs.
#[test]
fn concurrent_clients_share_single_owner_over_socket() {
    let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let temp = tempfile::tempdir().expect("tempdir");
    let workspace_root = temp.path().join("workspaces");
    std::fs::create_dir_all(&workspace_root).expect("workspace root");
    let _service = start_service(temp.path());

    let nonce = uuid::Uuid::new_v4().to_string();
    let worker_count = 16;
    let mut handles = Vec::new();
    for worker in 0..worker_count {
        let nonce = nonce.clone();
        let workspace_root = workspace_root.clone();
        handles.push(std::thread::spawn(move || {
            let session_id = format!("pm-{nonce}-{worker}");
            let workspace = workspace_root.join(format!("pm-{nonce}-{worker}"));
            std::fs::create_dir_all(&workspace).expect("workspace");
            let workspace = workspace.to_string_lossy().replace('\\', "/");
            // Write over the socket.
            match session_log::ipc::call_service(&upsert(&session_id, &workspace))
                .expect("upsert call")
            {
                SessionLogResponse::Ok => {}
                other => panic!("unexpected upsert response: {other:?}"),
            }
            // Read it back over the socket; the response must correspond to THIS
            // request (no multiplexing cross-talk).
            match session_log::ipc::call_service(&SessionLogCommand::GetSession(
                GetSessionRequest {
                    session_id: session_id.clone(),
                },
            ))
            .expect("get call")
            {
                SessionLogResponse::Session { session } => {
                    let session = session.expect("session should exist");
                    assert_eq!(session.session["id"], session_id);
                }
                other => panic!("unexpected get response: {other:?}"),
            }
        }));
    }
    for handle in handles {
        handle.join().expect("client thread");
    }

    let index_db = temp.path().join("session_log").join("index.sqlite3");
    assert!(
        index_db.exists(),
        "session index must stay under tura/db ({})",
        index_db.display()
    );
    let first_workspace_db = workspace_root
        .join(format!("pm-{nonce}-0"))
        .join(".tura")
        .join("session_log.sqlite3");
    assert!(
        first_workspace_db.exists(),
        "workspace log DB must be under <workspace>/.tura ({})",
        first_workspace_db.display()
    );
}

/// T-PM5 (version handshake half): a client built as a different version must
/// refuse to use the running service rather than silently driving it.
#[test]
fn version_mismatch_is_refused() {
    let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let temp = tempfile::tempdir().expect("tempdir");
    let _service = start_service(temp.path());

    // Sanity: a matching client succeeds.
    assert!(matches!(
        session_log::ipc::call_service(&SessionLogCommand::ListWorkspaces).expect("list"),
        SessionLogResponse::Workspaces { .. }
    ));

    // Now rewrite the published endpoint with a foreign version and confirm the
    // client refuses (the codex-style handshake). We restore it afterwards.
    let addr_path = session_log::ipc::service_addr_path();
    let original = std::fs::read_to_string(&addr_path).expect("read endpoint");
    let endpoint: serde_json::Value = serde_json::from_str(&original).expect("endpoint json");
    let addr = endpoint["addr"].as_str().expect("addr").to_string();
    std::fs::write(
        &addr_path,
        serde_json::to_string(&serde_json::json!({
            "addr": addr,
            "version": "0.0.0-foreign+release"
        }))
        .expect("foreign endpoint json"),
    )
    .expect("rewrite endpoint");

    let error = session_log::ipc::call_service(&SessionLogCommand::ListWorkspaces)
        .expect_err("a foreign-version service must be refused");
    assert!(
        error.to_string().contains("different build"),
        "unexpected error: {error}"
    );

    std::fs::write(&addr_path, original).expect("restore endpoint");
}

#[test]
fn delete_commands_are_served_over_socket() {
    let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let temp = tempfile::tempdir().expect("tempdir");
    let workspace = temp.path().join("delete-workspace");
    std::fs::create_dir_all(&workspace).expect("workspace");
    let workspace = workspace.to_string_lossy().replace('\\', "/");
    let _service = start_service(temp.path());

    let session_id = format!("delete-socket-{}", uuid::Uuid::new_v4());
    assert!(matches!(
        session_log::ipc::call_service(&upsert(&session_id, &workspace)).expect("upsert"),
        SessionLogResponse::Ok
    ));
    assert!(matches!(
        session_log::ipc::call_service(&SessionLogCommand::DeleteSession(DeleteSessionRequest {
            session_id: session_id.clone()
        }))
        .expect("delete session"),
        SessionLogResponse::Ok
    ));
    match session_log::ipc::call_service(&SessionLogCommand::GetSession(GetSessionRequest {
        session_id,
    }))
    .expect("get deleted")
    {
        SessionLogResponse::Session { session } => assert!(session.is_none()),
        other => panic!("unexpected get response: {other:?}"),
    }

    let session_id = format!("delete-workspace-{}", uuid::Uuid::new_v4());
    assert!(matches!(
        session_log::ipc::call_service(&upsert(&session_id, &workspace)).expect("upsert 2"),
        SessionLogResponse::Ok
    ));
    assert!(matches!(
        session_log::ipc::call_service(&SessionLogCommand::DeleteWorkspace(
            DeleteWorkspaceRequest {
                workspace: workspace.clone()
            }
        ))
        .expect("delete workspace"),
        SessionLogResponse::Ok
    ));
    match session_log::ipc::call_service(&SessionLogCommand::ListSessions(ListSessionsRequest {
        workspace,
        page: 0,
        page_size: 50,
    }))
    .expect("list deleted workspace")
    {
        SessionLogResponse::Sessions { page, sessions } => {
            assert_eq!(page.total, 0);
            assert!(sessions.is_empty());
        }
        other => panic!("unexpected list response: {other:?}"),
    }
}
