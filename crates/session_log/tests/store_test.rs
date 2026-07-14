use session_log::{
    file_queue, CommandCheckpoint, DeleteSessionRequest, DeleteWorkspaceRequest, GetSessionRequest,
    ListSessionRecordsRequest, ListSessionsRequest, MarkSessionInterruptedRequest,
    SessionLogCommand, SessionLogStore, UpsertSessionRequest,
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
                "management": { "session_id": session_id, "session_name": "Build", "state": "running" }
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
fn list_sessions_orders_by_last_user_message_at_only() {
    let db = DirectDbGuard::new();
    let store = SessionLogStore::open_default().expect("store");
    let nonce = uuid::Uuid::new_v4().to_string();
    let workspace = db.workspace(&format!("repo-last-user-{nonce}"));
    let normalized_workspace = session_log::path::normalize_workspace(&workspace);

    for (session_id, updated_at, last_user_message_at) in [
        ("assistant-updated-later", 1_000, 10),
        ("user-sent-later", 20, 200),
    ] {
        store
            .upsert_session(UpsertSessionRequest {
                session: serde_json::json!({
                    "id": format!("{session_id}-{nonce}"),
                    "name": session_id,
                    "directory": workspace,
                    "created_at": 1,
                    "updated_at": updated_at,
                    "last_user_message_at": last_user_message_at,
                    "management": {
                        "session_id": format!("{session_id}-{nonce}"),
                        "session_name": session_id,
                        "state": "running"
                    }
                }),
                parent_id: None,
                messages: vec![],
                todos: vec![],
            })
            .expect("upsert session");
    }

    let (_page, sessions) = store
        .list_sessions(ListSessionsRequest {
            workspace: normalized_workspace,
            page: 0,
            page_size: 10,
        })
        .expect("sessions");

    assert_eq!(sessions.len(), 2);
    assert!(sessions[0].session_id.starts_with("user-sent-later-"));
    assert_eq!(sessions[0].last_user_message_at, Some(200));
    assert!(sessions[1]
        .session_id
        .starts_with("assistant-updated-later-"));
    assert_eq!(sessions[1].last_user_message_at, Some(10));
}

#[test]
fn rejects_non_canonical_internal_session_state() {
    let db = DirectDbGuard::new();
    let store = SessionLogStore::open_default().expect("store");
    let session_id = format!("bad-state-{}", uuid::Uuid::new_v4());
    let workspace = db.workspace("bad-state-workspace");

    let error = store
        .upsert_session(UpsertSessionRequest {
            session: serde_json::json!({
                "id": session_id,
                "name": "Bad State",
                "directory": workspace,
                "created_at": 1,
                "updated_at": 1,
                "management": {
                    "session_id": session_id,
                    "session_name": "Bad State",
                    "state": "Running"
                }
            }),
            parent_id: None,
            messages: vec![],
            todos: vec![],
        })
        .expect_err("internal PascalCase SessionState must be rejected");

    assert!(
        error
            .to_string()
            .contains("invalid canonical session state"),
        "unexpected error: {error:#}"
    );
}

#[test]
fn running_sessions_are_marked_interrupted_with_one_canonical_state_source() {
    let db = DirectDbGuard::new();
    let store = SessionLogStore::open_default().expect("store");
    let session_id = format!("interrupted-{}", uuid::Uuid::new_v4());
    let workspace = db.workspace("interrupted-workspace");
    let now_ms = chrono::Utc::now().timestamp_millis();

    store
        .upsert_session(UpsertSessionRequest {
            session: serde_json::json!({
                "id": session_id,
                "name": "Interrupted",
                "directory": workspace,
                "created_at": now_ms,
                "updated_at": now_ms,
                "status": "idle",
                "management": {
                    "session_id": session_id,
                    "session_name": "Interrupted",
                    "session_created_at": "2026-06-11T00:00:00.000Z",
                    "session_last_update_at": "2026-06-11T00:00:01.000Z",
                    "state": "running"
                }
            }),
            parent_id: None,
            messages: vec![serde_json::json!({
                "id": "m-running",
                "role": "assistant",
                "created_at": 1,
                "updated_at": 1
            })],
            todos: vec![],
        })
        .expect("running upsert");

    assert_eq!(
        store
            .get_session(GetSessionRequest {
                session_id: session_id.clone()
            })
            .expect("get before mark")
            .expect("session exists")
            .status
            .as_deref(),
        Some("busy"),
        "status must be derived from running state, not copied from session.status"
    );

    assert_eq!(
        store
            .mark_running_sessions_interrupted()
            .expect("mark interrupted"),
        1
    );
    let loaded = store
        .get_session(GetSessionRequest { session_id })
        .expect("get after mark")
        .expect("session exists");

    assert_eq!(loaded.state.as_deref(), Some("interrupted"));
    assert_eq!(loaded.status.as_deref(), Some("error"));
    assert_eq!(loaded.management["state"], "interrupted");
    assert_eq!(loaded.session["status"], "error");
    assert_eq!(
        serde_json::from_value::<session_log::SessionState>(loaded.management["state"].clone())
            .expect("persisted interrupted state should deserialize"),
        session_log::SessionState::Interrupted
    );
}

#[test]
fn stale_running_sessions_are_interrupted_during_reads_after_two_minutes() {
    let db = DirectDbGuard::new();
    let store = SessionLogStore::open_default().expect("store");
    let fresh_id = format!("fresh-{}", uuid::Uuid::new_v4());
    let stale_id = format!("stale-{}", uuid::Uuid::new_v4());
    let workspace = db.workspace("stale-running-workspace");
    let now_ms = chrono::Utc::now().timestamp_millis();

    for (session_id, updated_at) in [
        (fresh_id.clone(), now_ms),
        (stale_id.clone(), now_ms - 121_000),
    ] {
        store
            .upsert_session(UpsertSessionRequest {
                session: serde_json::json!({
                    "id": session_id,
                    "name": session_id,
                    "directory": workspace,
                    "created_at": updated_at,
                    "updated_at": updated_at,
                    "management": {
                        "session_id": session_id,
                        "session_name": session_id,
                        "state": "running"
                    }
                }),
                parent_id: None,
                messages: vec![],
                todos: vec![],
            })
            .expect("upsert running session");
    }

    let normalized_workspace = session_log::path::normalize_workspace(&workspace);
    let (_page, sessions) = store
        .list_sessions(ListSessionsRequest {
            workspace: normalized_workspace,
            page: 0,
            page_size: 10,
        })
        .expect("list sessions triggers stale cleanup");

    let fresh = sessions
        .iter()
        .find(|session| session.session_id == fresh_id)
        .expect("fresh session listed");
    let stale = sessions
        .iter()
        .find(|session| session.session_id == stale_id)
        .expect("stale session listed");

    assert_eq!(fresh.state.as_deref(), Some("running"));
    assert_eq!(fresh.status.as_deref(), Some("busy"));
    assert_eq!(stale.state.as_deref(), Some("interrupted"));
    assert_eq!(stale.status.as_deref(), Some("error"));
}

#[test]
fn mark_session_interrupted_targets_only_one_session() {
    let db = DirectDbGuard::new();
    let store = SessionLogStore::open_default().expect("store");
    let target_id = format!("target-{}", uuid::Uuid::new_v4());
    let other_id = format!("other-{}", uuid::Uuid::new_v4());
    let workspace = db.workspace("single-interrupt-workspace");
    let now_ms = chrono::Utc::now().timestamp_millis();

    for session_id in [target_id.clone(), other_id.clone()] {
        store
            .upsert_session(UpsertSessionRequest {
                session: serde_json::json!({
                    "id": session_id,
                    "name": session_id,
                    "directory": workspace,
                    "created_at": now_ms,
                    "updated_at": now_ms,
                    "management": {
                        "session_id": session_id,
                        "session_name": session_id,
                        "state": "running"
                    }
                }),
                parent_id: None,
                messages: vec![],
                todos: vec![],
            })
            .expect("upsert running session");
    }

    assert!(store
        .mark_session_interrupted(MarkSessionInterruptedRequest {
            session_id: target_id.clone()
        })
        .expect("mark target interrupted"));

    let target = store
        .get_session(GetSessionRequest {
            session_id: target_id,
        })
        .expect("get target")
        .expect("target exists");
    let other = store
        .get_session(GetSessionRequest {
            session_id: other_id,
        })
        .expect("get other")
        .expect("other exists");

    assert_eq!(target.state.as_deref(), Some("interrupted"));
    assert_eq!(other.state.as_deref(), Some("running"));
}

#[test]
fn reads_authoritative_workspace_snapshot_when_index_is_stale() {
    let db = DirectDbGuard::new();
    let store = SessionLogStore::open_default().expect("store");
    let session_id = format!("workspace-authority-{}", uuid::Uuid::new_v4());
    let workspace = db.workspace("workspace-authority");

    store
        .upsert_session(UpsertSessionRequest {
            session: serde_json::json!({
                "id": session_id,
                "name": "Workspace Authority",
                "directory": workspace,
                "created_at": 1,
                "updated_at": 2,
                "management": {
                    "session_id": session_id,
                    "session_name": "Workspace Authority",
                    "state": "running"
                }
            }),
            parent_id: None,
            messages: vec![],
            todos: vec![],
        })
        .expect("upsert");

    let conn = rusqlite::Connection::open(db.workspace_db(&workspace)).expect("workspace db");
    let (session_json, management_json): (String, String) = conn
        .query_row(
            "SELECT session_json, management_json FROM sessions WHERE session_id = ?1",
            rusqlite::params![session_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("workspace snapshot");
    let mut session: serde_json::Value = serde_json::from_str(&session_json).expect("session json");
    let mut management: serde_json::Value =
        serde_json::from_str(&management_json).expect("management json");
    session["status"] = serde_json::json!("error");
    session["updated_at"] = serde_json::json!(99);
    management["state"] = serde_json::json!("interrupted");
    conn.execute(
        "UPDATE sessions
         SET state = ?2, status = ?3, updated_at = ?4, session_json = ?5, management_json = ?6
         WHERE session_id = ?1",
        rusqlite::params![
            session_id,
            "interrupted",
            "error",
            99_i64,
            serde_json::to_string(&session).expect("session to json"),
            serde_json::to_string(&management).expect("management to json"),
        ],
    )
    .expect("make workspace authoritative");

    let loaded = store
        .get_session(GetSessionRequest {
            session_id: session_id.clone(),
        })
        .expect("get session")
        .expect("session exists");
    assert_eq!(loaded.state.as_deref(), Some("interrupted"));
    assert_eq!(loaded.status.as_deref(), Some("error"));
    assert_eq!(loaded.updated_at, 99);
    assert_eq!(loaded.management["state"], "interrupted");

    let (_page, sessions) = store
        .list_sessions(ListSessionsRequest {
            workspace: session_log::path::normalize_workspace(&workspace),
            page: 0,
            page_size: 10,
        })
        .expect("list sessions");
    let listed = sessions
        .iter()
        .find(|snapshot| snapshot.session_id == session_id)
        .expect("listed session");
    assert_eq!(listed.state.as_deref(), Some("interrupted"));
    assert_eq!(listed.management["state"], "interrupted");
}

#[test]
fn upsert_session_records_replace_absent_records_from_full_snapshot() {
    let db = DirectDbGuard::new();
    let store = SessionLogStore::open_default().expect("store");
    let session_id = format!("record-upsert-{}", uuid::Uuid::new_v4());
    let workspace = db.workspace("record-upsert-workspace");

    let session = |updated_at| {
        serde_json::json!({
            "id": session_id,
            "name": "Record Upsert",
            "directory": workspace,
            "created_at": 1,
            "updated_at": updated_at,
            "management": {
                "session_id": session_id,
                "session_name": "Record Upsert",
                "state": "running"
            }
        })
    };

    store
        .upsert_session(UpsertSessionRequest {
            session: session(10),
            parent_id: None,
            messages: vec![
                serde_json::json!({
                    "id": "m1",
                    "role": "user",
                    "created_at": 1,
                    "updated_at": 1,
                    "content": "first"
                }),
                serde_json::json!({
                    "id": "m2",
                    "role": "assistant",
                    "created_at": 2,
                    "updated_at": 2,
                    "content": "second"
                }),
            ],
            todos: vec![],
        })
        .expect("initial upsert");

    store
        .upsert_session(UpsertSessionRequest {
            session: session(20),
            parent_id: None,
            messages: vec![
                serde_json::json!({
                    "id": "m2",
                    "role": "assistant",
                    "created_at": 2,
                    "updated_at": 22,
                    "content": "second revision"
                }),
                serde_json::json!({
                    "id": "m3",
                    "role": "tool",
                    "created_at": 3,
                    "updated_at": 3,
                    "content": "third"
                }),
            ],
            todos: vec![],
        })
        .expect("partial upsert");

    let (page, records) = store
        .list_session_records(ListSessionRecordsRequest {
            session_id,
            page: 0,
            page_size: 10,
        })
        .expect("records");

    assert_eq!(page.total, 2);
    assert_eq!(
        records
            .iter()
            .map(|record| record.message_id.as_str())
            .collect::<Vec<_>>(),
        vec!["m2", "m3"]
    );
    assert_eq!(
        records
            .iter()
            .filter(|record| record.message_id == "m2")
            .count(),
        1,
        "same message_id must update in place instead of duplicating"
    );
    let updated_m2 = records
        .iter()
        .find(|record| record.message_id == "m2")
        .expect("m2 should exist");
    assert_eq!(updated_m2.updated_at, 22);
    assert_eq!(updated_m2.record["content"], "second revision");
}

#[test]
fn compacted_management_upsert_preserves_unlisted_session_records() {
    let db = DirectDbGuard::new();
    let store = SessionLogStore::open_default().expect("store");
    let nonce = uuid::Uuid::new_v4().to_string();
    let session_id = format!("compact-tail-{nonce}");
    let workspace = db.workspace(&format!("repo-compact-tail-{nonce}"));
    let session = |updated_at, management: serde_json::Value| {
        serde_json::json!({
            "id": session_id,
            "name": "Compact Tail",
            "directory": workspace,
            "created_at": 1,
            "updated_at": updated_at,
            "management": management
        })
    };

    store
        .upsert_session(UpsertSessionRequest {
            session: session(
                10,
                serde_json::json!({
                    "session_id": session_id,
                    "session_name": "Compact Tail",
                    "state": "running",
                    "session_log": ["old", "tail"]
                }),
            ),
            parent_id: None,
            messages: vec![
                serde_json::json!({"id": "m1", "role": "user", "created_at": 1, "updated_at": 1}),
                serde_json::json!({"id": "m2", "role": "assistant", "created_at": 2, "updated_at": 2}),
            ],
            todos: vec![],
        })
        .expect("initial upsert");

    store
        .upsert_session(UpsertSessionRequest {
            session: session(
                20,
                serde_json::json!({
                    "session_id": session_id,
                    "session_name": "Compact Tail",
                    "state": "running",
                    "session_log": ["tail"],
                    "session_log_retention": {
                        "omitted_entries": 1,
                        "last_compaction": {
                            "compact_entry_index": 1,
                            "retained_before": 1,
                            "retained_from_index": 1,
                            "compacted_at": "2026-06-11T00:00:01Z"
                        }
                    }
                }),
            ),
            parent_id: None,
            messages: vec![serde_json::json!({
                "id": "m2",
                "role": "assistant",
                "created_at": 2,
                "updated_at": 22
            })],
            todos: vec![],
        })
        .expect("compacted upsert");

    let loaded = store
        .get_session(GetSessionRequest {
            session_id: session_id.clone(),
        })
        .expect("get session")
        .expect("session should exist");
    assert_eq!(
        loaded.management["session_log"]
            .as_array()
            .expect("log")
            .len(),
        1
    );
    assert_eq!(
        loaded.management["session_log_retention"]["omitted_entries"],
        1
    );

    let (page, records) = store
        .list_session_records(ListSessionRecordsRequest {
            session_id,
            page: 0,
            page_size: 10,
        })
        .expect("records");

    assert_eq!(page.total, 2);
    assert_eq!(
        records
            .iter()
            .map(|record| record.message_id.as_str())
            .collect::<Vec<_>>(),
        vec!["m1", "m2"]
    );
    let updated_m2 = records
        .iter()
        .find(|record| record.message_id == "m2")
        .expect("m2 should exist");
    assert_eq!(updated_m2.updated_at, 22);
}

#[test]
fn pending_checkpoint_queue_items_replay_and_ack_idempotently() {
    let db = DirectDbGuard::new();
    let store = SessionLogStore::open_default().expect("store");
    let session_id = format!("checkpoint-replay-{}", uuid::Uuid::new_v4());
    let checkpoint = CommandCheckpoint {
        session_id: session_id.clone(),
        turn_id: "turn-1".to_string(),
        runtime_worker_id: Some("worker-1".to_string()),
        provider_call_id: Some("provider-1".to_string()),
        command_run_id: Some("run-1".to_string()),
        command_id: Some("cmd-1".to_string()),
        event_seq: Some(1),
        command_type: Some("shell_command".to_string()),
        command_line: Some("echo ok".to_string()),
        status: "command_finished".to_string(),
        output_summary: Some("ok".to_string()),
        changes: serde_json::json!({ "files": [] }),
        started_at: Some("2026-06-11T00:00:00Z".to_string()),
        finished_at: Some("2026-06-11T00:00:01Z".to_string()),
    };
    let key = checkpoint.idempotency_key();
    let payload = serde_json::to_string(&checkpoint).expect("checkpoint json");
    let conn = rusqlite::Connection::open(db.index_db()).expect("index db");
    conn.execute(
        "INSERT INTO session_write_queue(
            idempotency_key, session_id, turn_id, runtime_worker_id, command_run_id,
            command_id, event_seq, event_type, payload_json, status, retry_count, created_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'apply_command_checkpoint', ?8, 'pending', 0, 1)",
        rusqlite::params![
            key,
            &session_id,
            "turn-1",
            "worker-1",
            "run-1",
            "cmd-1",
            1_i64,
            payload,
        ],
    )
    .expect("insert pending checkpoint");

    assert_eq!(store.replay_pending_write_queue().expect("replay"), 1);
    assert_eq!(
        store.replay_pending_write_queue().expect("second replay"),
        0
    );

    let (status, retry_count, last_error): (String, i64, Option<String>) = conn
        .query_row(
            "SELECT status, retry_count, last_error
             FROM session_write_queue
             WHERE idempotency_key = ?1",
            rusqlite::params![checkpoint.idempotency_key()],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .expect("checkpoint queue row");
    assert_eq!(status, "applied");
    assert_eq!(retry_count, 0);
    assert_eq!(last_error, None);
}

#[test]
fn pending_delete_session_queue_item_replays_without_unsupported_event_error() {
    let db = DirectDbGuard::new();
    let store = SessionLogStore::open_default().expect("store");
    let workspace = db.workspace("delete-replay-workspace");
    let session_id = format!("delete-replay-{}", uuid::Uuid::new_v4());

    store
        .upsert_session(upsert(&session_id, &workspace, 1))
        .expect("upsert");
    assert!(store
        .get_session(GetSessionRequest {
            session_id: session_id.clone(),
        })
        .expect("get before delete")
        .is_some());

    let request = DeleteSessionRequest {
        session_id: session_id.clone(),
    };
    let conn = rusqlite::Connection::open(db.index_db()).expect("index db");
    conn.execute(
        "INSERT INTO session_write_queue(
            idempotency_key, session_id, event_type, payload_json, status, retry_count, created_at
        ) VALUES (?1, ?2, 'delete_session', ?3, 'pending', 0, 1)",
        rusqlite::params![
            format!("delete:{session_id}"),
            &session_id,
            serde_json::to_string(&request).expect("delete json"),
        ],
    )
    .expect("insert pending delete");

    assert_eq!(store.replay_pending_write_queue().expect("replay"), 1);
    assert!(store
        .get_session(GetSessionRequest { session_id })
        .expect("get after delete")
        .is_none());
}

#[test]
fn dirty_session_write_queue_items_are_deleted_instead_of_blocking_service_start() {
    let db = DirectDbGuard::new();
    let store = SessionLogStore::open_default().expect("store");
    let conn = rusqlite::Connection::open(db.index_db()).expect("index db");
    let bad_state_session_id = format!("dirty-state-{}", uuid::Uuid::new_v4());
    let workspace = db.workspace("dirty-state-workspace");
    let bad_state_payload = serde_json::to_string(&upsert(&bad_state_session_id, &workspace, 1))
        .expect("upsert json")
        .replace("\"state\":\"created\"", "\"state\":\"Created\"");

    conn.execute(
        "INSERT INTO session_write_queue(
            idempotency_key, session_id, event_type, payload_json, status, retry_count, created_at
        ) VALUES
            ('dirty-json', 'dirty-json-session', 'upsert_session', '{not-json', 'pending', 0, 1),
            ('dirty-state', ?1, 'upsert_session', ?2, 'pending', 0, 2)",
        rusqlite::params![bad_state_session_id, bad_state_payload],
    )
    .expect("insert dirty queue rows");

    assert_eq!(store.replay_pending_write_queue().expect("replay"), 0);
    let remaining: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM session_write_queue WHERE status = 'pending'",
            [],
            |row| row.get(0),
        )
        .expect("pending count");
    assert_eq!(remaining, 0);
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
                "state": "created"
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
fn dirty_file_queue_item_is_quarantined_in_failed_without_retries() {
    let db = DirectDbGuard::new();
    let store = SessionLogStore::open_default().expect("store");
    let root = db.root().join("session_log").join("message_queue");
    let pending = root.join("pending");
    let failed = root.join("failed");
    std::fs::create_dir_all(&pending).expect("pending dir");
    let file_name = "00000000000000000001-1-00000000000000000001.json";
    let dirty = pending.join(file_name);
    std::fs::write(&dirty, "{not-json").expect("dirty queue item");

    assert_eq!(file_queue::drain_queue(&store, 10).expect("drain"), 0);
    assert!(!dirty.exists(), "dirty pending file should leave pending");
    let failed_json = failed.join(file_name);
    let failed_error = failed_json.with_extension("error.txt");
    assert!(
        failed_json.exists(),
        "dirty queue item should be retained in failed"
    );
    assert!(
        failed_error.exists(),
        "dirty queue item should have an error sidecar"
    );
    let error = std::fs::read_to_string(&failed_error).expect("failed sidecar");
    assert!(
        error.contains("failed to parse session queue item"),
        "failed sidecar should explain the parse error: {error}"
    );
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
