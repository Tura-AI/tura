use session_log::{
    file_queue, DeleteSessionRequest, DeleteWorkspaceRequest, GetSessionRequest,
    ListSessionRecordsRequest, ListSessionsRequest, SessionLogCommand, SessionLogStore,
    UpsertSessionRequest,
};
use std::path::Path;
use std::process::Command;

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

    fn workspace_db(&self, workspace: &str) -> std::path::PathBuf {
        std::path::PathBuf::from(workspace)
            .join(".tura")
            .join("session_log.sqlite3")
    }

    fn index_db(&self) -> std::path::PathBuf {
        self.root.path().join("session_log").join("index.sqlite3")
    }
}

#[test]
fn stores_workspaces_sessions_and_last_record_page() {
    let db = DirectDbGuard::new();
    let store = SessionLogStore::open_default().expect("store");
    let nonce = uuid::Uuid::new_v4().to_string();
    let session_id = format!("s-{nonce}");
    let workspace = db.workspace(&format!("repo-{nonce}"));

    store
        .upsert_session(UpsertSessionRequest {
            session: serde_json::json!({
                "id": session_id,
                "name": "Build",
                "directory": workspace,
                "created_at": 10,
                "updated_at": 20,
                "status": "idle",
                "task_management": { "plan_summary": "Plan" },
                "management": { "session_id": session_id, "session_name": "Build", "state": "Running" }
            }),
            parent_id: None,
            messages: vec![
                serde_json::json!({"id": "m1", "role": "user", "created_at": 1, "updated_at": 1}),
                serde_json::json!({"id": "m2", "role": "assistant", "created_at": 2, "updated_at": 2}),
                serde_json::json!({"id": "m3", "role": "assistant", "created_at": 3, "updated_at": 3}),
            ],
            todos: vec![serde_json::json!({"id": "todo-1", "content": "Check DB"})],
        })
        .expect("upsert");

    let workspaces = store.list_workspaces().expect("workspaces");
    let normalized_workspace = session_log::path::normalize_workspace(&workspace);
    let workspace_summary = workspaces
        .iter()
        .find(|item| item.directory == normalized_workspace)
        .expect("unique workspace should be listed");
    assert_eq!(workspace_summary.session_count, 1);

    let (page, sessions) = store
        .list_sessions(ListSessionsRequest {
            workspace: normalized_workspace.clone(),
            page: 0,
            page_size: 10,
        })
        .expect("sessions");
    assert_eq!(page.total, 1);
    assert_eq!(sessions[0].session_id, session_id);
    assert_eq!(sessions[0].task_management["plan_summary"], "Plan");
    assert_eq!(sessions[0].todos[0]["id"], "todo-1");

    let loaded = store
        .get_session(GetSessionRequest {
            session_id: session_id.clone(),
        })
        .expect("get session")
        .expect("session should exist");
    assert_eq!(loaded.session["id"], session_id);
    assert_eq!(loaded.todos[0]["content"], "Check DB");

    let (page, records) = store
        .list_session_records(ListSessionRecordsRequest {
            session_id,
            page: 0,
            page_size: 2,
        })
        .expect("records");
    assert_eq!(page.page, 1);
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].message_id, "m3");
    assert!(db.index_db().exists(), "index db should stay under tura/db");
    assert!(
        db.workspace_db(&normalized_workspace).exists(),
        "workspace session log should live under <workspace>/.tura"
    );
}

#[test]
fn delete_session_and_workspace_update_index_and_workspace_dbs() {
    let db = DirectDbGuard::new();
    let store = SessionLogStore::open_default().expect("store");
    let first_workspace = db.workspace("delete-session-workspace");
    let second_workspace = db.workspace("delete-workspace-workspace");
    let first_session = format!("delete-session-{}", uuid::Uuid::new_v4());
    let second_session = format!("delete-workspace-{}", uuid::Uuid::new_v4());

    store
        .upsert_session(upsert(&first_session, &first_workspace, 1))
        .expect("first upsert");
    store
        .upsert_session(upsert(&second_session, &second_workspace, 2))
        .expect("second upsert");

    store
        .delete_session(DeleteSessionRequest {
            session_id: first_session.clone(),
        })
        .expect("delete session");
    assert!(store
        .get_session(GetSessionRequest {
            session_id: first_session.clone(),
        })
        .expect("get deleted session")
        .is_none());
    let (_page, records) = store
        .list_session_records(ListSessionRecordsRequest {
            session_id: first_session,
            page: 0,
            page_size: 50,
        })
        .expect("deleted session records");
    assert!(records.is_empty());

    store
        .delete_workspace(DeleteWorkspaceRequest {
            workspace: second_workspace.clone(),
        })
        .expect("delete workspace");
    assert!(store
        .get_session(GetSessionRequest {
            session_id: second_session,
        })
        .expect("get workspace-deleted session")
        .is_none());
    assert!(
        !db.workspace_db(&second_workspace).exists(),
        "delete_workspace should remove the workspace session log DB"
    );
}

#[test]
fn missing_workspace_db_removes_index_snapshot() {
    let db = DirectDbGuard::new();
    let store = SessionLogStore::open_default().expect("store");
    let workspace = db.workspace("missing-workspace-db");
    let session_id = format!("missing-workspace-{}", uuid::Uuid::new_v4());

    store
        .upsert_session(upsert(&session_id, &workspace, 1))
        .expect("upsert");
    assert!(db.workspace_db(&workspace).exists());
    std::fs::remove_dir_all(std::path::PathBuf::from(&workspace).join(".tura"))
        .expect("remove workspace db directory");

    assert!(store
        .get_session(GetSessionRequest { session_id })
        .expect("get after missing workspace db")
        .is_none());
    let workspaces = store.list_workspaces().expect("workspaces after sweep");
    assert!(!workspaces
        .iter()
        .any(|item| item.directory == session_log::path::normalize_workspace(&workspace)));
}

#[test]
fn rejects_upsert_without_session_id_with_context() {
    let _db = DirectDbGuard::new();
    let store = SessionLogStore::open_default().expect("store");

    let error = store
        .upsert_session(UpsertSessionRequest {
            session: serde_json::json!({
                "name": "Missing id",
                "directory": "C:/missing-id"
            }),
            parent_id: None,
            messages: vec![],
            todos: vec![],
        })
        .expect_err("missing session id should fail");

    assert!(
        error.to_string().contains("session id missing"),
        "unexpected error: {error:#}"
    );
}

#[test]
fn corrupted_workspace_session_json_returns_contextual_error() {
    let db = DirectDbGuard::new();
    let store = SessionLogStore::open_default().expect("store");
    let workspace = db.workspace("corrupt-session-json");
    let session_id = format!("corrupt-session-json-{}", uuid::Uuid::new_v4());

    store
        .upsert_session(upsert(&session_id, &workspace, 1))
        .expect("upsert");
    let conn = rusqlite::Connection::open(db.workspace_db(&workspace)).expect("workspace db");
    conn.execute(
        "UPDATE sessions SET session_json = ?2 WHERE session_id = ?1",
        rusqlite::params![session_id, "{not-json"],
    )
    .expect("corrupt session json");

    let error = store
        .get_session(GetSessionRequest {
            session_id: session_id.clone(),
        })
        .expect_err("corrupt session_json should fail");
    let text = error.to_string();
    assert!(text.contains("session_json"), "unexpected error: {error:#}");
    assert!(text.contains(&session_id), "unexpected error: {error:#}");
}

#[test]
fn corrupted_record_json_returns_contextual_error() {
    let db = DirectDbGuard::new();
    let store = SessionLogStore::open_default().expect("store");
    let workspace = db.workspace("corrupt-record-json");
    let session_id = format!("corrupt-record-json-{}", uuid::Uuid::new_v4());

    store
        .upsert_session(upsert(&session_id, &workspace, 1))
        .expect("upsert");
    let conn = rusqlite::Connection::open(db.workspace_db(&workspace)).expect("workspace db");
    conn.execute(
        "UPDATE session_records SET record_json = ?2 WHERE session_id = ?1",
        rusqlite::params![session_id, "{not-json"],
    )
    .expect("corrupt record json");

    let error = store
        .list_session_records(ListSessionRecordsRequest {
            session_id: session_id.clone(),
            page: 0,
            page_size: 100,
        })
        .expect_err("corrupt record_json should fail");
    let text = error.to_string();
    assert!(text.contains("record_json"), "unexpected error: {error:#}");
    assert!(text.contains(&session_id), "unexpected error: {error:#}");
}

fn upsert(session_id: &str, workspace: &str, sequence: i64) -> UpsertSessionRequest {
    UpsertSessionRequest {
        session: serde_json::json!({
            "id": session_id,
            "name": format!("Session {sequence}"),
            "directory": workspace,
            "created_at": sequence,
            "updated_at": sequence,
            "status": "idle",
            "management": {
                "session_id": session_id,
                "session_name": format!("Session {sequence}"),
                "state": "Idle"
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

#[test]
fn file_queue_write_drains_into_session_store() {
    let db = DirectDbGuard::new();
    let nonce = uuid::Uuid::new_v4().to_string();
    let session_id = format!("file-queue-{nonce}");
    let workspace = db.workspace(&format!("file-queue-{nonce}"));
    let current_exe = std::env::current_exe().expect("current test exe");

    let status = Command::new(&current_exe)
        .args(["--exact", "file_queue_session_log_helper", "--nocapture"])
        .env("SESSION_LOG_FILE_QUEUE_SESSION_ID", &session_id)
        .env("SESSION_LOG_FILE_QUEUE_WORKSPACE", &workspace)
        .env("SESSION_LOG_DB_ROOT", db.root())
        .status()
        .expect("file queue helper");
    assert!(status.success(), "file queue helper exited with {status}");
}

#[test]
fn file_queue_session_log_helper() {
    let Ok(session_id) = std::env::var("SESSION_LOG_FILE_QUEUE_SESSION_ID") else {
        return;
    };
    let workspace = std::env::var("SESSION_LOG_FILE_QUEUE_WORKSPACE").expect("workspace");
    let store = SessionLogStore::open_default().expect("store");

    file_queue::enqueue_command(&SessionLogCommand::UpsertSession(UpsertSessionRequest {
        session: serde_json::json!({
            "id": session_id,
            "name": "File Queue",
            "directory": workspace,
            "created_at": 1,
            "updated_at": 2,
            "status": "idle",
            "management": {
                "session_id": session_id,
                "session_name": "File Queue",
                "state": "Idle"
            }
        }),
        parent_id: None,
        messages: vec![serde_json::json!({
            "id": format!("m-{session_id}"),
            "role": "assistant",
            "created_at": 1,
            "updated_at": 1
        })],
        todos: vec![],
    }))
    .expect("enqueue");

    assert_eq!(file_queue::drain_queue(&store, 10).expect("drain"), 1);
    let session = store
        .get_session(GetSessionRequest { session_id })
        .expect("get session")
        .expect("session should be written");
    assert_eq!(session.name.as_deref(), Some("File Queue"));
    assert_eq!(session.message_count, 1);
}

#[test]
fn file_queue_recovers_orphaned_processing_items() {
    let db = DirectDbGuard::new();
    let store = SessionLogStore::open_default().expect("store");
    let workspace = db.workspace("file-queue-recover-workspace");
    let session_id = format!("file-queue-recover-{}", uuid::Uuid::new_v4());
    let command = SessionLogCommand::UpsertSession(upsert(&session_id, &workspace, 1));

    let pending = file_queue::enqueue_command(&command).expect("enqueue");
    let root = db.root().join("session_log").join("message_queue");
    let processing_dir = root.join("processing");
    std::fs::create_dir_all(&processing_dir).expect("processing dir");
    let processing = processing_dir.join(pending.file_name().expect("queue item name"));
    std::fs::rename(&pending, &processing).expect("simulate crash while processing");

    assert_eq!(file_queue::drain_queue(&store, 10).expect("drain"), 1);
    assert!(
        !processing.exists(),
        "orphaned processing item should be consumed"
    );
    assert!(store
        .get_session(GetSessionRequest { session_id })
        .expect("get recovered session")
        .is_some());
}

#[test]
fn open_default_without_service_creates_sqlite_index() {
    let _serial = SERIAL.lock().unwrap_or_else(|error| error.into_inner());
    let temp = tempfile::tempdir().expect("tempdir");
    let current_exe = std::env::current_exe().expect("current test exe");

    let status = Command::new(&current_exe)
        .args(["--exact", "open_default_helper", "--nocapture"])
        .env("SESSION_LOG_OPEN_DEFAULT_MODE", "1")
        .env("SESSION_LOG_DB_ROOT", temp.path())
        .status()
        .expect("open default helper");
    assert!(status.success(), "open default helper exited with {status}");
    assert!(temp
        .path()
        .join("session_log")
        .join("index.sqlite3")
        .exists());
}

#[test]
fn open_default_helper() {
    if std::env::var("SESSION_LOG_OPEN_DEFAULT_MODE").is_err() {
        return;
    }
    SessionLogStore::open_default().expect("sqlite store");
}
