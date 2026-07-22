use lifecycle::{
    PlanStatus, ProviderConfig, RuntimeAggregate, RuntimeEvent, RuntimeProviderConfig,
    SessionCommand, SessionEvent, SessionManagement, SessionState, SessionTaskPlanPatch,
    StartCondition, TaskPlan, TaskStep, ToolChoice,
};
use session_log::{file_queue, SessionLogStore};
use session_log_contract::client::enqueue_command;
use session_log_contract::{
    ActivateRuntimeLeaseRequest, AppendSessionFeedEventRequest, CheckpointType, CommandCheckpoint,
    CommitRuntimeEventRequest, CreateSessionRequest, DeleteSessionRequest, DeleteWorkspaceRequest,
    ExecuteSessionCommandRequest, GetSessionRequest, ListSessionRecordsRequest,
    ListSessionsRequest, MarkSessionInterruptedRequest, PersistSessionDeltaRequest,
    ReadContextSliceRequest, ReadSessionFeedRequest, RegisterRuntimeRequest, ReplayRuntimeRequest,
    RuntimeEventCommitOutcome, RuntimeLeaseOutcome, RuntimeRegistrationOutcome,
    SessionContextRecord, SessionDeltaEntry, SessionFeedAppendOutcome, SessionFeedEvent,
    SessionLogCommand, SessionMetadataPatch, SessionRecordProjection, UpdateSessionRequest,
    UpdateSessionTodosRequest,
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
        .create_session(typed_create_request(
            &workspace,
            &session_id,
            "Build",
            10,
            TaskPlan {
                plan_summary: "Plan".to_string(),
                ..TaskPlan::default()
            },
        ))
        .expect("create session");
    persist_typed_entries(
        &store,
        &session_id,
        vec![
            simple_delta_entry(&session_id, 0, "m1", "user", 1),
            simple_delta_entry(&session_id, 1, "m2", "assistant", 2),
            simple_delta_entry(&session_id, 2, "m3", "assistant", 3),
        ],
    );

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
    assert_eq!(
        sessions[0].lifecycle_projection.task_plan.plan_summary,
        "Plan"
    );
    assert!(sessions[0].todos.is_empty());

    let loaded = store
        .get_session(GetSessionRequest {
            session_id: session_id.clone(),
        })
        .expect("get session")
        .expect("session should exist");
    assert_eq!(loaded.session_id, session_id);
    assert_eq!(loaded.lifecycle_projection.task_plan.plan_summary, "Plan");
    assert!(loaded.todos.is_empty());

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

    for (session_id, last_user_message_at) in
        [("assistant-updated-later", 10), ("user-sent-later", 200)]
    {
        store
            .create_session(typed_create_request(
                &workspace,
                &format!("{session_id}-{nonce}"),
                session_id,
                last_user_message_at,
                TaskPlan::default(),
            ))
            .expect("create session");
    }
    execute_typed_command(
        &store,
        &format!("assistant-updated-later-{nonce}"),
        SessionCommand::ApplyTaskStatus {
            task_plan: TaskPlan::default(),
        },
    );

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
fn running_sessions_are_marked_interrupted_with_one_canonical_state_source() {
    let db = DirectDbGuard::new();
    let store = SessionLogStore::open_default().expect("store");
    let session_id = format!("interrupted-{}", uuid::Uuid::new_v4());
    let workspace = db.workspace("interrupted-workspace");
    create_typed_session(&store, &workspace, &session_id);
    execute_typed_command(&store, &session_id, SessionCommand::StartUserTurn);

    assert_eq!(
        store
            .get_session(GetSessionRequest {
                session_id: session_id.clone()
            })
            .expect("get before mark")
            .expect("session exists")
            .lifecycle_projection
            .state
            .ui_status(),
        "busy",
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

    assert_eq!(loaded.lifecycle_projection.state, SessionState::Interrupted);
    assert_eq!(loaded.lifecycle_projection.state.ui_status(), "error");
    assert_eq!(loaded.management.state, SessionState::Interrupted);
}

#[test]
fn reads_do_not_mutate_historically_timestamped_running_sessions() {
    let db = DirectDbGuard::new();
    let store = SessionLogStore::open_default().expect("store");
    let session_id = format!("historical-running-{}", uuid::Uuid::new_v4());
    let workspace = db.workspace("stale-running-workspace");
    create_typed_session(&store, &workspace, &session_id);
    execute_typed_command(&store, &session_id, SessionCommand::StartUserTurn);
    let conn = rusqlite::Connection::open(db.index_db()).expect("index db");
    conn.execute(
        "UPDATE sessions SET updated_at = ?2 WHERE session_id = ?1",
        rusqlite::params![session_id, 1_i64],
    )
    .expect("age running session index");

    let normalized_workspace = session_log::path::normalize_workspace(&workspace);
    let (_page, sessions) = store
        .list_sessions(ListSessionsRequest {
            workspace: normalized_workspace,
            page: 0,
            page_size: 10,
        })
        .expect("list sessions without state mutation");

    let loaded = sessions
        .iter()
        .find(|session| session.session_id == session_id)
        .expect("historically timestamped session listed");
    assert_eq!(loaded.lifecycle_projection.state, SessionState::Running);
    assert_eq!(loaded.lifecycle_projection.state.ui_status(), "busy");
}

#[test]
fn mark_session_interrupted_targets_only_one_session() {
    let db = DirectDbGuard::new();
    let store = SessionLogStore::open_default().expect("store");
    let target_id = format!("target-{}", uuid::Uuid::new_v4());
    let other_id = format!("other-{}", uuid::Uuid::new_v4());
    let workspace = db.workspace("single-interrupt-workspace");
    for session_id in [target_id.clone(), other_id.clone()] {
        create_typed_session(&store, &workspace, &session_id);
        execute_typed_command(&store, &session_id, SessionCommand::StartUserTurn);
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

    assert_eq!(target.lifecycle_projection.state, SessionState::Interrupted);
    assert_eq!(other.lifecycle_projection.state, SessionState::Running);
}

#[test]
fn reads_authoritative_workspace_snapshot_when_index_is_stale() {
    let db = DirectDbGuard::new();
    let store = SessionLogStore::open_default().expect("store");
    let session_id = format!("workspace-authority-{}", uuid::Uuid::new_v4());
    let workspace = db.workspace("workspace-authority");
    create_typed_session(&store, &workspace, &session_id);
    execute_typed_command(&store, &session_id, SessionCommand::StartUserTurn);
    assert!(store
        .mark_session_interrupted(MarkSessionInterruptedRequest {
            session_id: session_id.clone(),
        })
        .expect("interrupt authoritative workspace session"));
    let authoritative_updated_at = store
        .get_session(GetSessionRequest {
            session_id: session_id.clone(),
        })
        .expect("get authoritative snapshot")
        .expect("authoritative snapshot exists")
        .updated_at;

    let conn = rusqlite::Connection::open(db.index_db()).expect("index db");
    conn.execute(
        "UPDATE sessions SET state = 'running' WHERE session_id = ?1",
        rusqlite::params![session_id],
    )
    .expect("make index stale");

    let loaded = store
        .get_session(GetSessionRequest {
            session_id: session_id.clone(),
        })
        .expect("get session")
        .expect("session exists");
    assert_eq!(loaded.lifecycle_projection.state, SessionState::Interrupted);
    assert_eq!(loaded.lifecycle_projection.state.ui_status(), "error");
    assert_eq!(loaded.updated_at, authoritative_updated_at);
    assert_eq!(loaded.management.state, SessionState::Interrupted);

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
    assert_eq!(listed.lifecycle_projection.state, SessionState::Interrupted);
    assert_eq!(listed.management.state, SessionState::Interrupted);
}

#[test]
fn command_checkpoints_are_idempotent_and_reject_conflicting_content() {
    let db = DirectDbGuard::new();
    let store = SessionLogStore::open_default().expect("store");
    let session_id = format!("checkpoint-replay-{}", uuid::Uuid::new_v4());
    let checkpoint = CommandCheckpoint {
        session_id,
        runtime_id: "runtime-1".to_string(),
        runtime_worker_id: Some("worker-1".to_string()),
        provider_call_id: Some("provider-1".to_string()),
        command_run_id: Some("run-1".to_string()),
        command_id: Some("cmd-1".to_string()),
        event_seq: Some(1),
        command_type: Some("shell_command".to_string()),
        command_line: Some("echo ok".to_string()),
        checkpoint_type: CheckpointType::CommandFinished,
        output_summary: Some("ok".to_string()),
        changes: serde_json::json!({ "files": [] }),
        started_at: Some("2026-06-11T00:00:00Z".to_string()),
        finished_at: Some("2026-06-11T00:00:01Z".to_string()),
    };
    let key = checkpoint.idempotency_key();
    store
        .apply_command_checkpoint(checkpoint.clone())
        .expect("apply checkpoint");
    store
        .apply_command_checkpoint(checkpoint.clone())
        .expect("repeat identical checkpoint");
    let conn = rusqlite::Connection::open(db.index_db()).expect("index db");
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM command_checkpoints WHERE idempotency_key = ?1",
            [key],
            |row| row.get(0),
        )
        .expect("checkpoint count");
    assert_eq!(count, 1);

    let mut conflict = checkpoint;
    conflict.output_summary = Some("different".to_string());
    let error = store
        .apply_command_checkpoint(conflict)
        .expect_err("same key with different content must fail");
    assert!(error.to_string().contains("reused with different content"));
}

#[test]
fn delete_session_and_workspace_update_index_and_workspace_dbs() {
    let db = DirectDbGuard::new();
    let store = SessionLogStore::open_default().expect("store");
    let first_workspace = db.workspace("delete-session-workspace");
    let second_workspace = db.workspace("delete-workspace-workspace");
    let first_session = format!("delete-session-{}", uuid::Uuid::new_v4());
    let second_session = format!("delete-workspace-{}", uuid::Uuid::new_v4());

    create_typed_session(&store, &first_workspace, &first_session);
    create_typed_session(&store, &second_workspace, &second_session);

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
fn delete_workspace_keeps_index_until_workspace_files_are_removed() {
    let db = DirectDbGuard::new();
    let store = SessionLogStore::open_default().expect("store");
    let workspace = db.workspace("delete-workspace-recovery");
    let session_id = format!("delete-workspace-recovery-{}", uuid::Uuid::new_v4());
    create_typed_session(&store, &workspace, &session_id);
    let workspace_db = db.workspace_db(&workspace);
    std::fs::remove_file(&workspace_db).expect("remove workspace database for failure injection");
    std::fs::create_dir(&workspace_db).expect("replace database with directory");
    std::fs::write(
        workspace_db.join("block-delete"),
        b"keep directory non-empty",
    )
    .expect("create deletion blocker");

    let error = store
        .delete_workspace(DeleteWorkspaceRequest {
            workspace: workspace.clone(),
        })
        .expect_err("workspace file deletion failure must be visible");
    assert!(
        format!("{error:#}").contains("failed to remove"),
        "unexpected delete failure: {error:#}"
    );
    let index = rusqlite::Connection::open(db.index_db()).expect("index db");
    let indexed: i64 = index
        .query_row(
            "SELECT COUNT(*) FROM sessions WHERE session_id = ?1",
            [&session_id],
            |row| row.get(0),
        )
        .expect("query retained index row");
    assert_eq!(indexed, 1, "failed deletion must retain its recovery path");
    drop(index);

    std::fs::remove_dir_all(&workspace_db).expect("clear deletion blocker");
    store
        .delete_workspace(DeleteWorkspaceRequest { workspace })
        .expect("replayed workspace deletion");
    let index = rusqlite::Connection::open(db.index_db()).expect("index db after replay");
    let indexed: i64 = index
        .query_row(
            "SELECT COUNT(*) FROM sessions WHERE session_id = ?1",
            [&session_id],
            |row| row.get(0),
        )
        .expect("query removed index row");
    assert_eq!(indexed, 0);
}

#[test]
fn missing_workspace_db_removes_index_snapshot() {
    let db = DirectDbGuard::new();
    let store = SessionLogStore::open_default().expect("store");
    let workspace = db.workspace("missing-workspace-db");
    let session_id = format!("missing-workspace-{}", uuid::Uuid::new_v4());

    create_typed_session(&store, &workspace, &session_id);
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
fn corrupted_workspace_session_json_returns_contextual_error() {
    let db = DirectDbGuard::new();
    let store = SessionLogStore::open_default().expect("store");
    let workspace = db.workspace("corrupt-session-json");
    let session_id = format!("corrupt-session-json-{}", uuid::Uuid::new_v4());

    create_typed_session(&store, &workspace, &session_id);
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
fn corrupted_session_event_history_returns_contextual_error() {
    let db = DirectDbGuard::new();
    let store = SessionLogStore::open_default().expect("store");
    let workspace = db.workspace("corrupt-session-events");
    let session_id = format!("corrupt-session-events-{}", uuid::Uuid::new_v4());

    create_typed_session(&store, &workspace, &session_id);
    let conn = rusqlite::Connection::open(db.workspace_db(&workspace)).expect("workspace db");
    let invalid_first = serde_json::to_string(&SessionEvent::RuntimeStarted {
        runtime_id: "runtime-corrupt".to_string(),
        state: SessionState::Running,
    })
    .expect("serialize invalid first event");
    conn.execute(
        "UPDATE session_events SET event_json = ?2 WHERE session_id = ?1 AND event_seq = 0",
        rusqlite::params![session_id, invalid_first],
    )
    .expect("corrupt session event");
    drop(conn);

    let error = store
        .execute_session_command(ExecuteSessionCommandRequest {
            command_id: "corrupt-history-command".to_string(),
            session_id: session_id.clone(),
            session_command: SessionCommand::SubmitUserInput,
            message_projection: None,
        })
        .expect_err("noncanonical session history should fail");
    let text = format!("{error:#}");
    assert!(
        text.contains("invalid canonical lifecycle history"),
        "unexpected error: {text}"
    );
    assert!(text.contains(&session_id), "unexpected error: {text}");
    assert!(
        text.contains("first session event is not a creation event"),
        "unexpected error: {text}"
    );
}

#[test]
fn create_session_receipt_replays_result_and_repairs_missing_index() {
    let db = DirectDbGuard::new();
    let store = SessionLogStore::open_default().expect("store");
    let workspace = db.workspace("create-receipt-replay");
    let session_id = format!("create-receipt-replay-{}", uuid::Uuid::new_v4());
    let request = typed_create_request(
        &workspace,
        &session_id,
        "Receipt session",
        10,
        TaskPlan::default(),
    );

    let first = store
        .create_session(request.clone())
        .expect("initial create");
    let index = rusqlite::Connection::open(db.index_db()).expect("index db");
    index
        .execute("DELETE FROM sessions WHERE session_id = ?1", [&session_id])
        .expect("remove derived index row");
    drop(index);

    let replay = store.create_session(request).expect("replay create");
    assert_eq!(replay, first);

    let conflict = store
        .create_session(CreateSessionRequest {
            command_id: format!("create:{session_id}"),
            session_id: session_id.clone(),
            creation_command: SessionCommand::CreateSession {
                task_plan: TaskPlan::default(),
            },
            copy_context: false,
            workspace: workspace.clone(),
            session_directory: workspace.clone(),
            name: "Conflicting session".to_string(),
            created_at: 10,
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
            initial_task_plan_patch: None,
        })
        .expect_err("conflicting create receipt must fail");
    assert!(
        conflict
            .to_string()
            .contains("was reused with different content"),
        "unexpected conflict: {conflict:#}"
    );

    let workspace_db =
        rusqlite::Connection::open(db.workspace_db(&workspace)).expect("workspace db");
    let (events, receipts): (i64, i64) = workspace_db
        .query_row(
            "SELECT
                (SELECT COUNT(*) FROM session_events WHERE session_id = ?1),
                (SELECT COUNT(*) FROM session_command_receipts WHERE session_id = ?1)",
            [&session_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("count create facts");
    assert_eq!((events, receipts), (1, 1));

    let index = rusqlite::Connection::open(db.index_db()).expect("repaired index db");
    let repaired: i64 = index
        .query_row(
            "SELECT COUNT(*) FROM sessions WHERE session_id = ?1 AND state = 'created'",
            [&session_id],
            |row| row.get(0),
        )
        .expect("query repaired index");
    assert_eq!(repaired, 1);
}

#[test]
fn execute_session_command_receipt_is_idempotent_and_rejects_conflicts() {
    let db = DirectDbGuard::new();
    let store = SessionLogStore::open_default().expect("store");
    let workspace = db.workspace("execute-receipt-replay");
    let session_id = format!("execute-receipt-replay-{}", uuid::Uuid::new_v4());
    create_typed_session(&store, &workspace, &session_id);
    let request = ExecuteSessionCommandRequest {
        command_id: "execute-receipt".to_string(),
        session_id: session_id.clone(),
        session_command: SessionCommand::SubmitUserInput,
        message_projection: Some(user_message_projection(
            &session_id,
            "execute-receipt-message",
            "persist exactly once",
            10,
        )),
    };

    let first = store
        .execute_session_command(request.clone())
        .expect("initial command");
    let index = rusqlite::Connection::open(db.index_db()).expect("index db");
    index
        .execute(
            "UPDATE sessions SET state = 'failed' WHERE session_id = ?1",
            [&session_id],
        )
        .expect("make command index stale");
    drop(index);
    let replay = store
        .execute_session_command(request)
        .expect("replay command");
    assert_eq!(replay, first);
    let (feed, cursor) = store
        .read_session_feed(ReadSessionFeedRequest {
            session_id: session_id.clone(),
            after_cursor: 0,
            limit: 10,
        })
        .expect("read command message feed");
    assert_eq!(cursor, 3);
    assert_eq!(
        feed.len(),
        3,
        "receipt replay must not append duplicate feed events"
    );
    assert!(matches!(
        &feed[0].event,
        SessionFeedEvent::SessionSnapshotCreated { .. }
    ));
    assert!(matches!(
        &feed[1].event,
        SessionFeedEvent::MessageUpserted { message }
            if message.message_id == "execute-receipt-message"
    ));
    assert!(matches!(
        &feed[2].event,
        SessionFeedEvent::SessionProjectionUpdated { .. }
    ));
    let index = rusqlite::Connection::open(db.index_db()).expect("repaired index db");
    let repaired_state: String = index
        .query_row(
            "SELECT state FROM sessions WHERE session_id = ?1",
            [&session_id],
            |row| row.get(0),
        )
        .expect("query repaired command index");
    assert_eq!(repaired_state, "created");

    let conflict = store
        .execute_session_command(ExecuteSessionCommandRequest {
            command_id: "execute-receipt".to_string(),
            session_id: session_id.clone(),
            session_command: SessionCommand::InterruptSession,
            message_projection: None,
        })
        .expect_err("conflicting receipt must fail");
    assert!(
        conflict
            .to_string()
            .contains("was reused with different content"),
        "unexpected conflict: {conflict:#}"
    );

    let workspace_db =
        rusqlite::Connection::open(db.workspace_db(&workspace)).expect("workspace db");
    let (events, receipts, records, message_count): (i64, i64, i64, i64) = workspace_db
        .query_row(
            "SELECT
                (SELECT COUNT(*) FROM session_events WHERE session_id = ?1),
                 (SELECT COUNT(*) FROM session_command_receipts WHERE session_id = ?1),
                 (SELECT COUNT(*) FROM session_records WHERE session_id = ?1),
                 message_count
             FROM sessions WHERE session_id = ?1",
            [&session_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .expect("count execute facts");
    assert_eq!((events, receipts, records, message_count), (2, 2, 1, 1));
    let snapshot = store
        .get_session(GetSessionRequest { session_id })
        .expect("get session after atomic input")
        .expect("session exists after atomic input");
    assert_eq!(snapshot.last_user_message_at, Some(10));
    let management = snapshot.management;
    assert_eq!(management.input.user_input, "persist exactly once");
    assert_eq!(management.session_log.len(), 1);
}

#[test]
fn terminal_command_persists_assistant_message_without_user_metadata_side_effects() {
    let db = DirectDbGuard::new();
    let store = SessionLogStore::open_default().expect("store");
    let workspace = db.workspace("terminal-command-message");
    let session_id = format!("terminal-command-message-{}", uuid::Uuid::new_v4());
    let runtime_id = "runtime-terminal-message".to_string();
    create_typed_session(&store, &workspace, &session_id);

    let assistant_as_input = store
        .execute_session_command(ExecuteSessionCommandRequest {
            command_id: "reject-assistant-input".to_string(),
            session_id: session_id.clone(),
            session_command: SessionCommand::SubmitUserInput,
            message_projection: Some(assistant_message_projection(
                &session_id,
                "assistant-input-message",
                "not user input",
                10,
            )),
        })
        .expect_err("input command must reject an assistant message");
    assert!(
        assistant_as_input
            .to_string()
            .contains("requires message projection role user"),
        "unexpected error: {assistant_as_input:#}"
    );

    execute_typed_command(
        &store,
        &session_id,
        SessionCommand::RuntimeStarted {
            runtime_id: runtime_id.clone(),
        },
    );
    let user_as_terminal = store
        .execute_session_command(ExecuteSessionCommandRequest {
            command_id: "reject-user-terminal".to_string(),
            session_id: session_id.clone(),
            session_command: SessionCommand::RuntimeCompleted {
                runtime_id: runtime_id.clone(),
            },
            message_projection: Some(user_message_projection(
                &session_id,
                "user-terminal-message",
                "not a fallback",
                20,
            )),
        })
        .expect_err("terminal command must reject a user message");
    assert!(
        user_as_terminal
            .to_string()
            .contains("requires message projection role assistant"),
        "unexpected error: {user_as_terminal:#}"
    );

    let workspace_db =
        rusqlite::Connection::open(db.workspace_db(&workspace)).expect("workspace db");
    let (rejected_receipts, rejected_records): (i64, i64) = workspace_db
        .query_row(
            "SELECT
                (SELECT COUNT(*) FROM session_command_receipts
                 WHERE command_id IN ('reject-assistant-input', 'reject-user-terminal')),
                (SELECT COUNT(*) FROM session_records WHERE session_id = ?1)",
            [&session_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("count rejected command side effects");
    assert_eq!((rejected_receipts, rejected_records), (0, 0));
    let running = store
        .get_session(GetSessionRequest {
            session_id: session_id.clone(),
        })
        .expect("get session after rejected terminal command")
        .expect("session remains after rejected terminal command");
    assert_eq!(running.state.as_deref(), Some("running"));

    let result = store
        .execute_session_command(ExecuteSessionCommandRequest {
            command_id: "complete-with-assistant".to_string(),
            session_id: session_id.clone(),
            session_command: SessionCommand::RuntimeCompleted { runtime_id },
            message_projection: Some(assistant_message_projection(
                &session_id,
                "terminal-fallback-message",
                "Runtime stopped before producing a final response.",
                30,
            )),
        })
        .expect("complete runtime with assistant fallback");
    assert_eq!(result.projection.state, SessionState::Completed);
    assert_eq!(result.message_count, 1);
    assert_eq!(result.last_user_message_at, Some(1));

    let snapshot = store
        .get_session(GetSessionRequest {
            session_id: session_id.clone(),
        })
        .expect("get terminal session")
        .expect("terminal session exists");
    assert_eq!(snapshot.lifecycle_projection.state, SessionState::Completed);
    assert_eq!(snapshot.lifecycle_projection.state.ui_status(), "idle");
    assert_eq!(snapshot.message_count, 1);
    assert_eq!(snapshot.last_user_message_at, Some(1));
    let management = snapshot.management;
    assert!(management.input.user_input.is_empty());
    assert!(management.session_log.is_empty());

    let (_, records) = store
        .list_session_records(ListSessionRecordsRequest {
            session_id: session_id.clone(),
            page: 0,
            page_size: 10,
        })
        .expect("list terminal records");
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].message_id, "terminal-fallback-message");
    assert_eq!(records[0].role, "assistant");
    let (feed, cursor) = store
        .read_session_feed(ReadSessionFeedRequest {
            session_id,
            after_cursor: 0,
            limit: 10,
        })
        .expect("read terminal feed");
    assert_eq!(cursor, 4);
    assert_eq!(feed.len(), 4);
    assert!(matches!(
        &feed[2].event,
        SessionFeedEvent::MessageUpserted { message }
            if message.message_id == "terminal-fallback-message"
    ));
    assert!(matches!(
        &feed[3].event,
        SessionFeedEvent::SessionProjectionUpdated { projection, .. }
            if projection.state == SessionState::Completed
    ));
}

#[test]
fn update_session_is_atomic_idempotent_and_durable() {
    let db = DirectDbGuard::new();
    let store = SessionLogStore::open_default().expect("store");
    let workspace = db.workspace("update-session-durable");
    let session_id = format!("update-session-durable-{}", uuid::Uuid::new_v4());
    create_typed_session(&store, &workspace, &session_id);
    let request = UpdateSessionRequest {
        command_id: "update-session-durable".to_string(),
        session_id: session_id.clone(),
        metadata: SessionMetadataPatch {
            name: Some("Durable title".to_string()),
            model: Some("durable-model".to_string()),
            agent: Some("durable-agent".to_string()),
            session_type: Some("general".to_string()),
            validator_enabled: Some(true),
            force_planning: Some(true),
            disable_permission_restrictions: Some(true),
            use_last_tool_call_response: Some(false),
            auto_session_name: Some(true),
            ..SessionMetadataPatch::default()
        },
        task_plan_patch: Some(SessionTaskPlanPatch {
            plan_summary: Some("Durable plan".to_string()),
            tasks: None,
            task: None,
            generated_task_ids: Vec::new(),
            generated_task_id: "unused-task-id".to_string(),
            now: chrono::Utc::now(),
        }),
    };

    let updated = store
        .update_session(request.clone())
        .expect("update metadata and task plan atomically");
    assert_eq!(updated.name.as_deref(), Some("Durable title"));
    assert_eq!(updated.metadata.model.as_deref(), Some("durable-model"));
    assert_eq!(updated.metadata.agent.as_deref(), Some("durable-agent"));
    assert_eq!(updated.metadata.session_type, "general");
    assert!(updated.metadata.validator_enabled);
    assert!(updated.metadata.force_planning);
    assert_eq!(
        updated.lifecycle_projection.task_plan.plan_summary,
        "Durable plan"
    );
    assert!(updated.management.auto_session_name);
    assert!(updated.management.disable_permission_restrictions);
    assert!(!updated.management.use_last_tool_call_response);

    let replay = store
        .update_session(request.clone())
        .expect("replay update receipt");
    assert_eq!(replay, updated);
    let conflict = store
        .update_session(UpdateSessionRequest {
            metadata: SessionMetadataPatch {
                name: Some("Conflicting title".to_string()),
                ..request.metadata.clone()
            },
            ..request
        })
        .expect_err("conflicting update receipt must fail");
    assert!(
        conflict
            .to_string()
            .contains("was reused with different content"),
        "unexpected conflict: {conflict:#}"
    );

    let (feed, cursor) = store
        .read_session_feed(ReadSessionFeedRequest {
            session_id: session_id.clone(),
            after_cursor: 0,
            limit: 10,
        })
        .expect("read update snapshot feed");
    assert_eq!(cursor, 2);
    assert_eq!(feed.len(), 2);
    assert!(matches!(
        &feed[0].event,
        SessionFeedEvent::SessionSnapshotCreated { .. }
    ));
    assert!(matches!(
        &feed[1].event,
        SessionFeedEvent::SessionSnapshotUpdated { snapshot }
            if snapshot.as_ref() == &updated
    ));

    drop(store);
    let reopened = SessionLogStore::open_default().expect("reopen store");
    assert_eq!(
        reopened
            .get_session(GetSessionRequest { session_id })
            .expect("read reopened session")
            .expect("reopened session exists"),
        updated
    );
}

#[test]
fn update_session_todos_is_atomic_idempotent_and_durable() {
    let db = DirectDbGuard::new();
    let store = SessionLogStore::open_default().expect("store");
    let workspace = db.workspace("update-session-todos-durable");
    let session_id = format!("update-session-todos-{}", uuid::Uuid::new_v4());
    create_typed_session(&store, &workspace, &session_id);
    let todos = vec![serde_json::json!({
        "id": "todo-durable",
        "content": "Persist canonical todos",
        "status": "in_progress"
    })];
    let request = UpdateSessionTodosRequest {
        command_id: "update-session-todos-durable".to_string(),
        session_id: session_id.clone(),
        todos: todos.clone(),
        updated_at: 42,
    };

    assert_eq!(
        store
            .update_session_todos(request.clone())
            .expect("update canonical todos"),
        todos
    );
    assert_eq!(
        store
            .update_session_todos(request.clone())
            .expect("replay canonical todo update"),
        todos
    );
    let conflict = store
        .update_session_todos(UpdateSessionTodosRequest {
            todos: vec![serde_json::json!({"id": "different"})],
            ..request.clone()
        })
        .expect_err("reused todo command id must preserve content");
    assert!(
        conflict
            .to_string()
            .contains("was reused with different content"),
        "unexpected conflict: {conflict:#}"
    );
    let newer_todos = vec![serde_json::json!({"id": "newer-todo"})];
    store
        .update_session_todos(UpdateSessionTodosRequest {
            command_id: "update-session-todos-newer".to_string(),
            session_id: session_id.clone(),
            todos: newer_todos.clone(),
            updated_at: 43,
        })
        .expect("write newer canonical todos");
    assert_eq!(
        store
            .update_session_todos(request)
            .expect("late replay of older todo command"),
        newer_todos,
        "late replay must return current canonical todos"
    );
    let (feed, cursor) = store
        .read_session_feed(ReadSessionFeedRequest {
            session_id: session_id.clone(),
            after_cursor: 0,
            limit: 10,
        })
        .expect("read canonical todo feed");
    assert_eq!(cursor, 3);
    assert_eq!(feed.len(), 3);
    assert!(matches!(
        &feed[1].event,
        SessionFeedEvent::TodosUpdated { todos: actual, .. } if actual == &todos
    ));
    assert!(matches!(
        &feed[2].event,
        SessionFeedEvent::TodosUpdated { todos: actual, .. } if actual == &newer_todos
    ));

    drop(store);
    let reopened = SessionLogStore::open_default().expect("reopen store");
    assert_eq!(
        reopened
            .get_session(GetSessionRequest { session_id })
            .expect("read reopened todo session")
            .expect("reopened todo session exists")
            .todos,
        newer_todos
    );
}

#[test]
fn update_session_todos_feed_failure_rolls_back_canonical_projection() {
    let db = DirectDbGuard::new();
    let store = SessionLogStore::open_default().expect("store");
    let workspace = db.workspace("update-session-todos-rollback");
    let session_id = format!("update-session-todos-rollback-{}", uuid::Uuid::new_v4());
    create_typed_session(&store, &workspace, &session_id);
    let workspace_db_path = db.workspace_db(&workspace);
    let workspace_db = rusqlite::Connection::open(&workspace_db_path).expect("workspace db");
    workspace_db
        .execute_batch(
            "CREATE TRIGGER reject_todo_feed
             BEFORE INSERT ON session_feed_events
             BEGIN
                 SELECT RAISE(ABORT, 'todo feed rejected');
             END;",
        )
        .expect("install todo feed failure trigger");
    drop(workspace_db);

    let error = store
        .update_session_todos(UpdateSessionTodosRequest {
            command_id: "todo-update-must-rollback".to_string(),
            session_id: session_id.clone(),
            todos: vec![serde_json::json!({"id": "must-not-survive"})],
            updated_at: 2,
        })
        .expect_err("feed failure must roll back canonical todos");
    assert!(
        format!("{error:#}").contains("todo feed rejected"),
        "unexpected failure: {error:#}"
    );
    assert!(store
        .get_session(GetSessionRequest {
            session_id: session_id.clone(),
        })
        .expect("read rolled back todo session")
        .expect("rolled back todo session exists")
        .todos
        .is_empty());
    let (_, cursor) = store
        .read_session_feed(ReadSessionFeedRequest {
            session_id,
            after_cursor: 0,
            limit: 10,
        })
        .expect("read rolled back todo feed");
    assert_eq!(cursor, 1);
}

#[test]
fn update_session_feed_failure_rolls_back_metadata_task_plan_and_receipt() {
    let db = DirectDbGuard::new();
    let store = SessionLogStore::open_default().expect("store");
    let workspace = db.workspace("update-session-rollback");
    let session_id = format!("update-session-rollback-{}", uuid::Uuid::new_v4());
    create_typed_session(&store, &workspace, &session_id);
    let before = store
        .get_session(GetSessionRequest {
            session_id: session_id.clone(),
        })
        .expect("read baseline session")
        .expect("baseline session exists");
    let workspace_db_path = db.workspace_db(&workspace);
    let workspace_db = rusqlite::Connection::open(&workspace_db_path).expect("workspace db");
    workspace_db
        .execute_batch(
            "CREATE TRIGGER reject_update_snapshot_feed
             BEFORE INSERT ON session_feed_events
             BEGIN
                 SELECT RAISE(ABORT, 'update snapshot feed rejected');
             END;",
        )
        .expect("install update feed failure trigger");
    drop(workspace_db);

    let error = store
        .update_session(UpdateSessionRequest {
            command_id: "update-session-must-rollback".to_string(),
            session_id: session_id.clone(),
            metadata: SessionMetadataPatch {
                name: Some("Must not survive".to_string()),
                model: Some("rolled-back-model".to_string()),
                validator_enabled: Some(true),
                ..SessionMetadataPatch::default()
            },
            task_plan_patch: Some(SessionTaskPlanPatch {
                plan_summary: Some("Rolled back plan".to_string()),
                tasks: None,
                task: None,
                generated_task_ids: Vec::new(),
                generated_task_id: "unused-task-id".to_string(),
                now: chrono::Utc::now(),
            }),
        })
        .expect_err("feed failure must roll back the update transaction");
    assert!(
        format!("{error:#}").contains("update snapshot feed rejected"),
        "unexpected failure: {error:#}"
    );
    assert_eq!(
        store
            .get_session(GetSessionRequest {
                session_id: session_id.clone(),
            })
            .expect("read rolled back session")
            .expect("rolled back session exists"),
        before
    );
    let workspace_db = rusqlite::Connection::open(workspace_db_path).expect("workspace db");
    let facts: (i64, i64, i64) = workspace_db
        .query_row(
            "SELECT
                 (SELECT COUNT(*) FROM session_events WHERE session_id = ?1),
                 (SELECT COUNT(*) FROM session_command_receipts WHERE session_id = ?1),
                 (SELECT COUNT(*) FROM session_feed_events WHERE session_id = ?1)",
            [&session_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .expect("query rolled back update facts");
    assert_eq!(facts, (1, 1, 1));
}

#[test]
fn receipt_insert_failure_rolls_back_session_event_and_projection() {
    let db = DirectDbGuard::new();
    let store = SessionLogStore::open_default().expect("store");
    let workspace = db.workspace("receipt-atomicity");
    let session_id = format!("receipt-atomicity-{}", uuid::Uuid::new_v4());
    create_typed_session(&store, &workspace, &session_id);
    let workspace_db_path = db.workspace_db(&workspace);
    let workspace_db = rusqlite::Connection::open(&workspace_db_path).expect("workspace db");
    workspace_db
        .execute_batch(
            "CREATE TRIGGER reject_session_command_receipt
             BEFORE INSERT ON session_command_receipts
             BEGIN
                 SELECT RAISE(ABORT, 'receipt insert rejected');
             END;",
        )
        .expect("install receipt failure trigger");
    drop(workspace_db);

    let error = store
        .execute_session_command(ExecuteSessionCommandRequest {
            command_id: "receipt-must-rollback".to_string(),
            session_id: session_id.clone(),
            session_command: SessionCommand::SubmitUserInput,
            message_projection: None,
        })
        .expect_err("receipt failure must fail command");
    assert!(
        format!("{error:#}").contains("receipt insert rejected"),
        "unexpected failure: {error:#}"
    );

    let workspace_db = rusqlite::Connection::open(workspace_db_path).expect("workspace db");
    let (events, receipts, state): (i64, i64, String) = workspace_db
        .query_row(
            "SELECT
                (SELECT COUNT(*) FROM session_events WHERE session_id = ?1),
                (SELECT COUNT(*) FROM session_command_receipts WHERE session_id = ?1),
                state
             FROM sessions WHERE session_id = ?1",
            [&session_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .expect("query rolled back session");
    assert_eq!((events, receipts, state.as_str()), (1, 1, "created"));
}

#[test]
fn message_feed_failure_rolls_back_lifecycle_message_and_metadata() {
    let db = DirectDbGuard::new();
    let store = SessionLogStore::open_default().expect("store");
    let workspace = db.workspace("message-feed-atomicity");
    let session_id = format!("message-feed-atomicity-{}", uuid::Uuid::new_v4());
    create_typed_session(&store, &workspace, &session_id);
    let workspace_db_path = db.workspace_db(&workspace);
    let workspace_db = rusqlite::Connection::open(&workspace_db_path).expect("workspace db");
    workspace_db
        .execute_batch(
            "CREATE TRIGGER reject_message_feed
             BEFORE INSERT ON session_feed_events
             BEGIN
                 SELECT RAISE(ABORT, 'message feed insert rejected');
             END;",
        )
        .expect("install feed failure trigger");
    drop(workspace_db);

    let error = store
        .execute_session_command(ExecuteSessionCommandRequest {
            command_id: "message-feed-must-rollback".to_string(),
            session_id: session_id.clone(),
            session_command: SessionCommand::StartUserTurn,
            message_projection: Some(user_message_projection(
                &session_id,
                "rolled-back-message",
                "must not survive",
                20,
            )),
        })
        .expect_err("feed failure must fail the entire command transaction");
    assert!(
        format!("{error:#}").contains("message feed insert rejected"),
        "unexpected failure: {error:#}"
    );

    let workspace_db = rusqlite::Connection::open(workspace_db_path).expect("workspace db");
    let facts: (i64, i64, i64, i64, i64, String) = workspace_db
        .query_row(
            "SELECT
                 (SELECT COUNT(*) FROM session_events WHERE session_id = ?1),
                 (SELECT COUNT(*) FROM session_command_receipts WHERE session_id = ?1),
                 (SELECT COUNT(*) FROM session_records WHERE session_id = ?1),
                 (SELECT COUNT(*) FROM session_feed_events WHERE session_id = ?1),
                 message_count,
                 state
             FROM sessions WHERE session_id = ?1",
            [&session_id],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                ))
            },
        )
        .expect("query rolled back message transaction");
    assert_eq!(facts, (1, 1, 0, 1, 0, "created".to_string()));
    let management = typed_management(&store, &session_id);
    assert!(management.input.user_input.is_empty());
    assert!(management.session_log.is_empty());
}

#[test]
fn scheduler_claim_atomically_persists_task_state_and_user_message() {
    let db = DirectDbGuard::new();
    let store = SessionLogStore::open_default().expect("store");
    let workspace = db.workspace("scheduler-message-atomicity");
    let session_id = format!("scheduler-message-{}", uuid::Uuid::new_v4());
    let now = chrono::Utc::now();
    let task_plan = TaskPlan {
        plan_summary: "Scheduled plan".to_string(),
        detailed_tasks: vec![TaskStep {
            task_id: "scheduled-task".to_string(),
            start_at: now,
            start_condition: StartCondition::ScheduledTask,
            status: PlanStatus::Todo,
            task_summary: "Run atomically".to_string(),
            ..TaskStep::default()
        }],
    };
    store
        .create_session(typed_create_request(
            &workspace,
            &session_id,
            "Scheduler transaction",
            1,
            task_plan,
        ))
        .expect("create scheduled session");

    let result = store
        .execute_session_command(ExecuteSessionCommandRequest {
            command_id: "scheduler-message-commit".to_string(),
            session_id: session_id.clone(),
            session_command: SessionCommand::StartScheduledTask {
                task_id: "scheduled-task".to_string(),
                task_summary: "Run atomically".to_string(),
                start_condition: StartCondition::ScheduledTask,
                now,
            },
            message_projection: Some(user_message_projection(
                &session_id,
                "scheduler-user-message",
                "Run atomically",
                20,
            )),
        })
        .expect("claim task and persist scheduler message");

    assert!(matches!(
        result.event,
        SessionEvent::ScheduledTaskClaimed { ref task_id, .. } if task_id == "scheduled-task"
    ));
    assert_eq!(result.projection.state, SessionState::Running);
    assert_eq!(
        result.projection.task_plan.detailed_tasks[0].status,
        PlanStatus::Doing
    );
    assert_eq!(result.message_count, 1);
    assert_eq!(result.last_user_message_at, Some(20));
    let (_, records) = store
        .list_session_records(ListSessionRecordsRequest {
            session_id: session_id.clone(),
            page: 0,
            page_size: 10,
        })
        .expect("list scheduler records");
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].message_id, "scheduler-user-message");
    let (feed, cursor) = store
        .read_session_feed(ReadSessionFeedRequest {
            session_id,
            after_cursor: 0,
            limit: 10,
        })
        .expect("read scheduler feed");
    assert_eq!(cursor, 3);
    assert_eq!(feed.len(), 3);
    assert!(matches!(
        feed[0].event,
        SessionFeedEvent::SessionSnapshotCreated { .. }
    ));
    assert!(matches!(
        feed[1].event,
        SessionFeedEvent::MessageUpserted { .. }
    ));
    assert!(matches!(
        feed[2].event,
        SessionFeedEvent::SessionProjectionUpdated { .. }
    ));
}

#[test]
fn scheduler_message_feed_failure_rolls_back_task_state_and_message() {
    let db = DirectDbGuard::new();
    let store = SessionLogStore::open_default().expect("store");
    let workspace = db.workspace("scheduler-message-rollback");
    let session_id = format!("scheduler-rollback-{}", uuid::Uuid::new_v4());
    let now = chrono::Utc::now();
    let task_plan = TaskPlan {
        plan_summary: "Scheduled plan".to_string(),
        detailed_tasks: vec![TaskStep {
            task_id: "scheduled-task".to_string(),
            start_at: now,
            start_condition: StartCondition::ScheduledTask,
            status: PlanStatus::Todo,
            task_summary: "Must roll back".to_string(),
            ..TaskStep::default()
        }],
    };
    store
        .create_session(typed_create_request(
            &workspace,
            &session_id,
            "Scheduler rollback",
            1,
            task_plan,
        ))
        .expect("create scheduled session");
    let workspace_db_path = db.workspace_db(&workspace);
    let workspace_db = rusqlite::Connection::open(&workspace_db_path).expect("workspace db");
    workspace_db
        .execute_batch(
            "CREATE TRIGGER reject_scheduler_message_feed
             BEFORE INSERT ON session_feed_events
             BEGIN
                 SELECT RAISE(ABORT, 'scheduler feed insert rejected');
             END;",
        )
        .expect("install scheduler feed failure trigger");
    drop(workspace_db);

    let error = store
        .execute_session_command(ExecuteSessionCommandRequest {
            command_id: "scheduler-message-rollback".to_string(),
            session_id: session_id.clone(),
            session_command: SessionCommand::StartScheduledTask {
                task_id: "scheduled-task".to_string(),
                task_summary: "Must roll back".to_string(),
                start_condition: StartCondition::ScheduledTask,
                now,
            },
            message_projection: Some(user_message_projection(
                &session_id,
                "rolled-back-scheduler-message",
                "Must roll back",
                30,
            )),
        })
        .expect_err("scheduler feed failure must roll back the entire transaction");
    assert!(
        format!("{error:#}").contains("scheduler feed insert rejected"),
        "unexpected failure: {error:#}"
    );

    let snapshot = store
        .get_session(GetSessionRequest {
            session_id: session_id.clone(),
        })
        .expect("get rolled back scheduler session")
        .expect("scheduler session exists");
    assert_eq!(snapshot.lifecycle_projection.state, SessionState::Created);
    assert_eq!(
        snapshot.lifecycle_projection.task_plan.detailed_tasks[0].status,
        PlanStatus::Todo
    );
    assert_eq!(snapshot.message_count, 0);
    assert_eq!(snapshot.last_user_message_at, Some(1));
    let workspace_db = rusqlite::Connection::open(workspace_db_path).expect("workspace db");
    let facts: (i64, i64, i64, i64) = workspace_db
        .query_row(
            "SELECT
                 (SELECT COUNT(*) FROM session_events WHERE session_id = ?1),
                 (SELECT COUNT(*) FROM session_command_receipts WHERE session_id = ?1),
                 (SELECT COUNT(*) FROM session_records WHERE session_id = ?1),
                 (SELECT COUNT(*) FROM session_feed_events WHERE session_id = ?1)
             FROM sessions WHERE session_id = ?1",
            [&session_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .expect("query rolled back scheduler transaction");
    assert_eq!(facts, (1, 1, 0, 1));
}

#[test]
fn busy_root_command_atomically_persists_the_child_message() {
    let db = DirectDbGuard::new();
    let store = SessionLogStore::open_default().expect("store");
    let workspace = db.workspace("busy-child-input");
    let root_id = format!("busy-root-{}", uuid::Uuid::new_v4());
    let child_id = format!("busy-child-{}", uuid::Uuid::new_v4());
    create_typed_session(&store, &workspace, &root_id);
    let mut child_request =
        typed_create_request(&workspace, &child_id, "Busy child", 2, TaskPlan::default());
    child_request.creation_command = SessionCommand::RegisterChildSession {
        parent_id: root_id.clone(),
    };
    store
        .create_session(child_request)
        .expect("create child session");
    execute_typed_command(&store, &root_id, SessionCommand::StartUserTurn);

    let result = store
        .execute_session_command(ExecuteSessionCommandRequest {
            command_id: "queue-child-message".to_string(),
            session_id: root_id,
            session_command: SessionCommand::QueueUserInputWhileBusy {
                input: "child guidance".to_string(),
            },
            message_projection: Some(user_message_projection(
                &child_id,
                "child-guidance-message",
                "child guidance",
                30,
            )),
        })
        .expect("queue root command with child message");
    assert_eq!(
        result.projection.pending_user_inputs,
        vec!["child guidance"]
    );
    assert_eq!(
        result.message_count, 0,
        "command result belongs to root session"
    );

    let child = store
        .get_session(GetSessionRequest {
            session_id: child_id.clone(),
        })
        .expect("get child")
        .expect("child exists");
    assert_eq!(child.message_count, 1);
    assert_eq!(child.last_user_message_at, Some(30));
    let (_, records) = store
        .list_session_records(ListSessionRecordsRequest {
            session_id: child_id.clone(),
            page: 0,
            page_size: 10,
        })
        .expect("list child records");
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].message_id, "child-guidance-message");
    let (feed, cursor) = store
        .read_session_feed(ReadSessionFeedRequest {
            session_id: child_id,
            after_cursor: 0,
            limit: 10,
        })
        .expect("read child message feed");
    assert_eq!(cursor, 2);
    assert_eq!(feed.len(), 2);
    assert!(matches!(
        feed[0].event,
        SessionFeedEvent::SessionSnapshotCreated { .. }
    ));
    assert!(matches!(
        feed[1].event,
        SessionFeedEvent::MessageUpserted { .. }
    ));
}

#[test]
fn command_message_rejects_a_different_workspace_without_side_effects() {
    let db = DirectDbGuard::new();
    let store = SessionLogStore::open_default().expect("store");
    let command_workspace = db.workspace("command-workspace");
    let message_workspace = db.workspace("message-workspace");
    let command_session_id = format!("command-session-{}", uuid::Uuid::new_v4());
    let message_session_id = format!("message-session-{}", uuid::Uuid::new_v4());
    create_typed_session(&store, &command_workspace, &command_session_id);
    create_typed_session(&store, &message_workspace, &message_session_id);

    let error = store
        .execute_session_command(ExecuteSessionCommandRequest {
            command_id: "cross-workspace-message".to_string(),
            session_id: command_session_id.clone(),
            session_command: SessionCommand::SubmitUserInput,
            message_projection: Some(user_message_projection(
                &message_session_id,
                "cross-workspace-message",
                "reject me",
                40,
            )),
        })
        .expect_err("cross-workspace command message must fail");
    assert!(
        error.to_string().contains("belong to different workspaces"),
        "unexpected error: {error:#}"
    );
    for (workspace, session_id) in [
        (&command_workspace, &command_session_id),
        (&message_workspace, &message_session_id),
    ] {
        let conn = rusqlite::Connection::open(db.workspace_db(workspace)).expect("workspace db");
        let facts: (i64, i64, i64, i64) = conn
            .query_row(
                "SELECT
                     (SELECT COUNT(*) FROM session_events WHERE session_id = ?1),
                     (SELECT COUNT(*) FROM session_command_receipts WHERE session_id = ?1),
                     (SELECT COUNT(*) FROM session_records WHERE session_id = ?1),
                     message_count
                 FROM sessions WHERE session_id = ?1",
                [session_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .expect("query unchanged workspace session");
        assert_eq!(facts, (1, 1, 0, 0));
    }
}

#[test]
fn execute_session_command_rejects_a_second_creation_event() {
    let db = DirectDbGuard::new();
    let store = SessionLogStore::open_default().expect("store");
    let workspace = db.workspace("duplicate-session-creation");
    let session_id = format!("duplicate-session-creation-{}", uuid::Uuid::new_v4());

    create_typed_session(&store, &workspace, &session_id);
    let error = store
        .execute_session_command(ExecuteSessionCommandRequest {
            command_id: "rejected-second-create".to_string(),
            session_id: session_id.clone(),
            session_command: SessionCommand::CreateSession {
                task_plan: TaskPlan::default(),
            },
            message_projection: None,
        })
        .expect_err("creation command must not append to an existing stream");
    assert!(
        error
            .to_string()
            .contains("creation and deletion commands must use their dedicated store methods"),
        "unexpected error: {error:#}"
    );

    let conn = rusqlite::Connection::open(db.workspace_db(&workspace)).expect("workspace db");
    let event_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM session_events WHERE session_id = ?1",
            [&session_id],
            |row| row.get(0),
        )
        .expect("count canonical events");
    assert_eq!(event_count, 1, "rejected creation must not alter history");

    let error = store
        .execute_session_command(ExecuteSessionCommandRequest {
            command_id: "rejected-delete".to_string(),
            session_id: session_id.clone(),
            session_command: SessionCommand::DeleteSession,
            message_projection: None,
        })
        .expect_err("delete command must use the dedicated store method");
    assert!(
        error
            .to_string()
            .contains("creation and deletion commands must use their dedicated store methods"),
        "unexpected error: {error:#}"
    );
    let event_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM session_events WHERE session_id = ?1",
            [&session_id],
            |row| row.get(0),
        )
        .expect("recount canonical events");
    assert_eq!(event_count, 1, "rejected delete must not alter history");
}

#[test]
fn corrupted_record_json_returns_contextual_error() {
    let db = DirectDbGuard::new();
    let store = SessionLogStore::open_default().expect("store");
    let workspace = db.workspace("corrupt-record-json");
    let session_id = format!("corrupt-record-json-{}", uuid::Uuid::new_v4());

    create_typed_session(&store, &workspace, &session_id);
    persist_typed_entries(
        &store,
        &session_id,
        vec![simple_delta_entry(
            &session_id,
            0,
            "corrupt-record",
            "assistant",
            1,
        )],
    );
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

#[test]
fn runtime_event_store_rejects_duplicate_order_revision_and_stale_lease() {
    let db = DirectDbGuard::new();
    let store = SessionLogStore::open_default().expect("store");
    let workspace = db.workspace("runtime-event-conflicts");
    let session_id = format!("runtime-events-{}", uuid::Uuid::new_v4());
    let runtime_id = format!("runtime-{}", uuid::Uuid::new_v4());
    create_typed_session(&store, &workspace, &session_id);
    let created = store
        .get_session(GetSessionRequest {
            session_id: session_id.clone(),
        })
        .expect("read created runtime session")
        .expect("created runtime session exists");

    assert!(matches!(
        store
            .register_runtime(RegisterRuntimeRequest {
                runtime_id: runtime_id.clone(),
                session_id: session_id.clone(),
                fallback_from_id: None,
            })
            .expect("register runtime"),
        RuntimeRegistrationOutcome::Registered {
            revision: 0,
            next_event_seq: 1,
            ..
        }
    ));
    let registered = store
        .get_session(GetSessionRequest {
            session_id: session_id.clone(),
        })
        .expect("read session after runtime registration")
        .expect("registered runtime session exists");
    assert_eq!(registered.lifecycle_projection.state, SessionState::Running);
    assert!(
        registered.updated_at > created.updated_at,
        "runtime registration must refresh the canonical activity timestamp before stale reads"
    );
    let management_updated_at = registered
        .management
        .session_last_update_at
        .timestamp_millis();
    assert_eq!(management_updated_at, registered.updated_at);
    let conn = rusqlite::Connection::open(db.index_db()).expect("index db");
    let index_updated_at: i64 = conn
        .query_row(
            "SELECT updated_at FROM sessions WHERE session_id = ?1",
            rusqlite::params![session_id],
            |row| row.get(0),
        )
        .expect("read registered index activity timestamp");
    assert_eq!(index_updated_at, registered.updated_at);
    conn.execute(
        "UPDATE sessions SET state = 'created' WHERE session_id = ?1",
        rusqlite::params![session_id],
    )
    .expect("make runtime registration index stale");
    assert!(matches!(
        store
            .register_runtime(RegisterRuntimeRequest {
                runtime_id: runtime_id.clone(),
                session_id: session_id.clone(),
                fallback_from_id: None,
            })
            .expect("repeat runtime registration"),
        RuntimeRegistrationOutcome::AlreadyRegistered { .. }
    ));
    let index_state: String = conn
        .query_row(
            "SELECT state FROM sessions WHERE session_id = ?1",
            rusqlite::params![session_id],
            |row| row.get(0),
        )
        .expect("read repaired registration index state");
    assert_eq!(index_state, "running");
    assert_eq!(
        store
            .activate_runtime_lease(ActivateRuntimeLeaseRequest {
                runtime_id: runtime_id.clone(),
                lease_id: "lease-current".to_string(),
            })
            .expect("activate lease"),
        RuntimeLeaseOutcome::Activated
    );

    let mut runtime = runtime_aggregate(&runtime_id, &session_id);
    let created = runtime.take_uncommitted_events().remove(0);
    let created_request = CommitRuntimeEventRequest {
        runtime_id: runtime_id.clone(),
        event_seq: 1,
        expected_revision: 0,
        lease_id: "lease-current".to_string(),
        idempotency_key: format!("{runtime_id}:1"),
        event: created,
    };
    assert!(matches!(
        store
            .commit_runtime_event(created_request.clone())
            .expect("commit created"),
        RuntimeEventCommitOutcome::Applied {
            revision: 1,
            next_event_seq: 2,
            ..
        }
    ));
    assert_eq!(
        store
            .commit_runtime_event(created_request)
            .expect("duplicate commit"),
        RuntimeEventCommitOutcome::Duplicate {
            revision: 1,
            next_event_seq: 2,
        }
    );

    let called_at = runtime.created_at + chrono::Duration::milliseconds(1);
    runtime.mark_called(called_at).expect("mark called");
    let call_started = runtime.take_uncommitted_events().remove(0);
    let request = |event_seq, expected_revision, lease_id: &str| CommitRuntimeEventRequest {
        runtime_id: runtime_id.clone(),
        event_seq,
        expected_revision,
        lease_id: lease_id.to_string(),
        idempotency_key: format!("{runtime_id}:{event_seq}:{expected_revision}:{lease_id}"),
        event: call_started.clone(),
    };
    assert_eq!(
        store
            .commit_runtime_event(request(3, 1, "lease-current"))
            .expect("out of order"),
        RuntimeEventCommitOutcome::OutOfOrder {
            expected_event_seq: 2,
            received_event_seq: 3,
        }
    );
    assert_eq!(
        store
            .commit_runtime_event(request(2, 0, "lease-current"))
            .expect("revision conflict"),
        RuntimeEventCommitOutcome::RevisionConflict {
            current_revision: 1,
            expected_revision: 0,
        }
    );
    assert_eq!(
        store
            .commit_runtime_event(request(2, 1, "lease-stale"))
            .expect("stale lease"),
        RuntimeEventCommitOutcome::StaleLease
    );
}

#[test]
fn runtime_event_store_replays_and_reduces_terminal_session_state() {
    let db = DirectDbGuard::new();
    let store = SessionLogStore::open_default().expect("store");
    let workspace = db.workspace("runtime-event-replay");
    let session_id = format!("runtime-replay-{}", uuid::Uuid::new_v4());
    let runtime_id = format!("runtime-{}", uuid::Uuid::new_v4());
    create_typed_session(&store, &workspace, &session_id);
    let registration = store
        .register_runtime(RegisterRuntimeRequest {
            runtime_id: runtime_id.clone(),
            session_id: session_id.clone(),
            fallback_from_id: None,
        })
        .expect("register runtime");
    assert!(matches!(
        registration,
        RuntimeRegistrationOutcome::Registered { .. }
    ));
    assert!(matches!(
        store
            .register_runtime(RegisterRuntimeRequest {
                runtime_id: runtime_id.clone(),
                session_id: session_id.clone(),
                fallback_from_id: None,
            })
            .expect("repeat runtime registration"),
        RuntimeRegistrationOutcome::AlreadyRegistered { .. }
    ));
    let (registration_entries, registration_cursor) = store
        .read_session_feed(ReadSessionFeedRequest {
            session_id: session_id.clone(),
            after_cursor: 0,
            limit: 10,
        })
        .expect("read registration projection feed");
    assert_eq!(registration_cursor, 2);
    assert_eq!(registration_entries.len(), 2);
    assert!(matches!(
        &registration_entries[0],
        session_log_contract::SessionFeedEntry {
            cursor: 1,
            runtime_id: None,
            event: SessionFeedEvent::SessionSnapshotCreated { .. },
            ..
        }
    ));
    assert!(matches!(
        &registration_entries[1],
        session_log_contract::SessionFeedEntry {
            cursor: 2,
            runtime_id: Some(actual_runtime_id),
            event: SessionFeedEvent::SessionProjectionUpdated { projection, .. },
            ..
        } if actual_runtime_id == &runtime_id
            && projection.state == SessionState::Running
            && projection.active_runtime_id.as_deref() == Some(runtime_id.as_str())
    ));
    store
        .activate_runtime_lease(ActivateRuntimeLeaseRequest {
            runtime_id: runtime_id.clone(),
            lease_id: "lease-replay".to_string(),
        })
        .expect("activate lease");

    let mut runtime = runtime_aggregate(&runtime_id, &session_id);
    runtime
        .mark_called(runtime.created_at + chrono::Duration::milliseconds(1))
        .expect("mark called");
    runtime
        .mark_waiting_first_token()
        .expect("mark waiting first token");
    runtime
        .mark_first_token(runtime.created_at + chrono::Duration::milliseconds(2))
        .expect("mark first token");
    runtime
        .finish_success(runtime.created_at + chrono::Duration::milliseconds(3), None)
        .expect("finish runtime");
    let events = runtime.take_uncommitted_events();
    let terminal_event = events.last().cloned().expect("terminal runtime event");
    for (index, event) in events.into_iter().enumerate() {
        let event_seq = index as u64 + 1;
        let outcome = store
            .commit_runtime_event(CommitRuntimeEventRequest {
                runtime_id: runtime_id.clone(),
                event_seq,
                expected_revision: event_seq - 1,
                lease_id: "lease-replay".to_string(),
                idempotency_key: format!("{runtime_id}:{event_seq}"),
                event,
            })
            .expect("commit ordered event");
        assert!(matches!(outcome, RuntimeEventCommitOutcome::Applied { .. }));
    }

    let conn = rusqlite::Connection::open(db.index_db()).expect("index db");
    conn.execute(
        "UPDATE sessions SET state = 'running' WHERE session_id = ?1",
        rusqlite::params![session_id],
    )
    .expect("make terminal runtime index stale");
    assert_eq!(
        store
            .commit_runtime_event(CommitRuntimeEventRequest {
                runtime_id: runtime_id.clone(),
                event_seq: 5,
                expected_revision: 4,
                lease_id: "lease-replay".to_string(),
                idempotency_key: format!("{runtime_id}:5"),
                event: terminal_event,
            })
            .expect("replay terminal runtime event"),
        RuntimeEventCommitOutcome::Duplicate {
            revision: 5,
            next_event_seq: 6,
        }
    );
    let index_state: String = conn
        .query_row(
            "SELECT state FROM sessions WHERE session_id = ?1",
            rusqlite::params![session_id],
            |row| row.get(0),
        )
        .expect("read repaired terminal index state");
    assert_eq!(index_state, "completed");

    let replay = store
        .replay_runtime(ReplayRuntimeRequest {
            runtime_id: runtime_id.clone(),
        })
        .expect("replay runtime")
        .expect("runtime exists");
    assert_eq!(replay.aggregate, runtime);
    assert_eq!(replay.aggregate.fallback_from_id, None);
    assert_eq!(replay.revision, 5);
    assert_eq!(replay.next_event_seq, 6);

    let session = store
        .get_session(GetSessionRequest { session_id })
        .expect("get terminal session")
        .expect("session exists");
    let projection = session.lifecycle_projection;
    assert_eq!(projection.state, SessionState::Completed);
    assert_eq!(projection.active_runtime_id, None);
    assert_eq!(projection.runtime_ids, vec![runtime_id.clone()]);
    assert_eq!(
        store
            .activate_runtime_lease(ActivateRuntimeLeaseRequest {
                runtime_id: runtime_id.clone(),
                lease_id: "lease-replacement".to_string(),
            })
            .expect("terminal lease attempt"),
        RuntimeLeaseOutcome::RuntimeTerminal
    );
    assert_eq!(
        store
            .commit_runtime_event(CommitRuntimeEventRequest {
                runtime_id: runtime_id.clone(),
                event_seq: 6,
                expected_revision: 5,
                lease_id: "lease-replay".to_string(),
                idempotency_key: format!("{runtime_id}:6"),
                event: RuntimeEvent::TextAppended {
                    chunk: "late".to_string(),
                },
            })
            .expect("terminal event attempt"),
        RuntimeEventCommitOutcome::RuntimeTerminal
    );
}

#[test]
fn session_feed_is_lease_guarded_idempotent_and_cursor_replayable() {
    let db = DirectDbGuard::new();
    let store = SessionLogStore::open_default().expect("store");
    let workspace = db.workspace("session-feed");
    let session_id = format!("feed-session-{}", uuid::Uuid::new_v4());
    let runtime_id = format!("feed-runtime-{}", uuid::Uuid::new_v4());
    create_typed_session(&store, &workspace, &session_id);
    store
        .register_runtime(RegisterRuntimeRequest {
            runtime_id: runtime_id.clone(),
            session_id: session_id.clone(),
            fallback_from_id: None,
        })
        .expect("register runtime");
    store
        .activate_runtime_lease(ActivateRuntimeLeaseRequest {
            runtime_id: runtime_id.clone(),
            lease_id: "feed-lease".to_string(),
        })
        .expect("activate lease");

    let request = AppendSessionFeedEventRequest {
        runtime_id: runtime_id.clone(),
        target_session_id: session_id.clone(),
        lease_id: "feed-lease".to_string(),
        event_id: format!("{runtime_id}:feed:1"),
        event: SessionFeedEvent::AssistantTextDelta {
            message_id: format!("{runtime_id}.message"),
            part_id: format!("{runtime_id}.message"),
            delta: "first".to_string(),
            created_at: 1,
            updated_at: 2,
        },
    };
    assert_eq!(
        store
            .append_session_feed_event(request.clone())
            .expect("append first feed event"),
        SessionFeedAppendOutcome::Applied { cursor: 3 }
    );
    assert_eq!(
        store
            .append_session_feed_event(request.clone())
            .expect("repeat feed event"),
        SessionFeedAppendOutcome::Duplicate { cursor: 3 }
    );
    let mut conflict = request.clone();
    conflict.event = SessionFeedEvent::AssistantTextDelta {
        message_id: format!("{runtime_id}.message"),
        part_id: format!("{runtime_id}.message"),
        delta: "different".to_string(),
        created_at: 1,
        updated_at: 2,
    };
    assert_eq!(
        store
            .append_session_feed_event(conflict)
            .expect("conflicting feed event"),
        SessionFeedAppendOutcome::EventIdConflict
    );

    let second = AppendSessionFeedEventRequest {
        event_id: format!("{runtime_id}:feed:2"),
        event: SessionFeedEvent::TodosUpdated {
            todos: vec![serde_json::json!({
                "id": "task-plan-1",
                "content": "Verify feed",
                "status": "doing",
                "priority": "medium"
            })],
            updated_at: 3,
        },
        ..request.clone()
    };
    assert_eq!(
        store
            .append_session_feed_event(second)
            .expect("append second feed event"),
        SessionFeedAppendOutcome::Applied { cursor: 4 }
    );
    let snapshot = store
        .get_session(GetSessionRequest {
            session_id: session_id.clone(),
        })
        .expect("read session-owned feed fixture")
        .expect("session-owned feed fixture exists");
    let session_owned_events = [
        SessionFeedEvent::MessageUpserted {
            message: SessionRecordProjection {
                session_id: session_id.clone(),
                message_id: "forged-user-message".to_string(),
                role: "user".to_string(),
                created_at: 4,
                updated_at: 4,
                record: serde_json::json!({}),
            },
        },
        SessionFeedEvent::SessionProjectionUpdated {
            projection: snapshot.lifecycle_projection.clone(),
            session_name: snapshot.name.clone(),
            updated_at: snapshot.updated_at,
        },
        SessionFeedEvent::SessionSnapshotCreated {
            snapshot: Box::new(snapshot.clone()),
        },
        SessionFeedEvent::SessionSnapshotUpdated {
            snapshot: Box::new(snapshot),
        },
        SessionFeedEvent::SessionDeleted {},
    ];
    for (index, event) in session_owned_events.into_iter().enumerate() {
        assert_eq!(
            store
                .append_session_feed_event(AppendSessionFeedEventRequest {
                    event_id: format!("{runtime_id}:forged-session-event:{index}"),
                    event,
                    ..request.clone()
                })
                .expect("reject Session-owned runtime feed event"),
            SessionFeedAppendOutcome::SessionOwnedEvent
        );
    }
    let (entries, next_cursor) = store
        .read_session_feed(ReadSessionFeedRequest {
            session_id: session_id.clone(),
            after_cursor: 0,
            limit: 1,
        })
        .expect("read first feed page");
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].cursor, 1);
    assert!(matches!(
        entries[0].event,
        SessionFeedEvent::SessionSnapshotCreated { .. }
    ));
    assert_eq!(next_cursor, 1);
    let (entries, next_cursor) = store
        .read_session_feed(ReadSessionFeedRequest {
            session_id: session_id.clone(),
            after_cursor: next_cursor,
            limit: 1,
        })
        .expect("read second feed page");
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].cursor, 2);
    assert!(matches!(
        entries[0].event,
        SessionFeedEvent::SessionProjectionUpdated { .. }
    ));
    assert_eq!(next_cursor, 2);
    let (entries, next_cursor) = store
        .read_session_feed(ReadSessionFeedRequest {
            session_id: session_id.clone(),
            after_cursor: next_cursor,
            limit: 10,
        })
        .expect("read third feed page");
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].cursor, 3);
    assert!(matches!(
        entries[0].event,
        SessionFeedEvent::AssistantTextDelta { .. }
    ));
    assert_eq!(entries[1].cursor, 4);
    assert!(matches!(
        entries[1].event,
        SessionFeedEvent::TodosUpdated { .. }
    ));
    assert_eq!(next_cursor, 4);

    let stale = AppendSessionFeedEventRequest {
        lease_id: "stale-lease".to_string(),
        event_id: format!("{runtime_id}:feed:3"),
        ..request.clone()
    };
    assert_eq!(
        store
            .append_session_feed_event(stale)
            .expect("reject stale lease"),
        SessionFeedAppendOutcome::StaleLease
    );

    let mut runtime = runtime_aggregate(&runtime_id, &session_id);
    runtime
        .mark_called(runtime.created_at + chrono::Duration::milliseconds(1))
        .expect("mark called");
    runtime.mark_waiting_first_token().expect("mark waiting");
    runtime
        .mark_first_token(runtime.created_at + chrono::Duration::milliseconds(2))
        .expect("mark first token");
    runtime
        .finish_success(runtime.created_at + chrono::Duration::milliseconds(3), None)
        .expect("finish runtime");
    for (index, event) in runtime.take_uncommitted_events().into_iter().enumerate() {
        let event_seq = index as u64 + 1;
        assert!(matches!(
            store
                .commit_runtime_event(CommitRuntimeEventRequest {
                    runtime_id: runtime_id.clone(),
                    event_seq,
                    expected_revision: event_seq - 1,
                    lease_id: "feed-lease".to_string(),
                    idempotency_key: format!("{runtime_id}:{event_seq}"),
                    event,
                })
                .expect("commit runtime event"),
            RuntimeEventCommitOutcome::Applied { .. }
        ));
    }
    let (terminal_entries, terminal_cursor) = store
        .read_session_feed(ReadSessionFeedRequest {
            session_id,
            after_cursor: 4,
            limit: 10,
        })
        .expect("read terminal projection feed");
    assert_eq!(terminal_cursor, 5);
    assert_eq!(terminal_entries.len(), 1);
    assert!(matches!(
        &terminal_entries[0],
        session_log_contract::SessionFeedEntry {
            cursor: 5,
            runtime_id: Some(actual_runtime_id),
            event: SessionFeedEvent::SessionProjectionUpdated { projection, .. },
            ..
        } if actual_runtime_id == &runtime_id
            && projection.state == SessionState::Completed
            && projection.active_runtime_id.is_none()
    ));
    let after_terminal = AppendSessionFeedEventRequest {
        event_id: format!("{runtime_id}:feed:4"),
        ..request
    };
    assert_eq!(
        store
            .append_session_feed_event(after_terminal)
            .expect("reject terminal runtime"),
        SessionFeedAppendOutcome::RuntimeTerminal
    );
}

#[test]
fn runtime_retry_registration_requires_latest_failed_predecessor_and_matching_creation() {
    let db = DirectDbGuard::new();
    let store = SessionLogStore::open_default().expect("store");
    let workspace = db.workspace("runtime-retry-registration");
    let session_id = format!("runtime-retry-{}", uuid::Uuid::new_v4());
    let failed_runtime_id = format!("runtime-failed-{}", uuid::Uuid::new_v4());
    let retry_runtime_id = format!("runtime-retry-{}", uuid::Uuid::new_v4());
    create_typed_session(&store, &workspace, &session_id);

    store
        .register_runtime(RegisterRuntimeRequest {
            runtime_id: failed_runtime_id.clone(),
            session_id: session_id.clone(),
            fallback_from_id: None,
        })
        .expect("register failed runtime");
    store
        .activate_runtime_lease(ActivateRuntimeLeaseRequest {
            runtime_id: failed_runtime_id.clone(),
            lease_id: "lease-failed".to_string(),
        })
        .expect("activate failed runtime lease");
    let mut failed_runtime = runtime_aggregate(&failed_runtime_id, &session_id);
    failed_runtime
        .finish_failure(
            failed_runtime.created_at,
            lifecycle::RuntimeError {
                error_code: Some("RATE_LIMITED".to_string()),
                error_text: Some("retry me".to_string()),
                retry_allowed: true,
                fallback_allowed: true,
                fallback_to_id: None,
            },
            lifecycle::RuntimeState::Failed,
            None,
        )
        .expect("runtime may fail before producing output");
    for (index, event) in failed_runtime
        .take_uncommitted_events()
        .into_iter()
        .enumerate()
    {
        let event_seq = index as u64 + 1;
        assert!(matches!(
            store
                .commit_runtime_event(CommitRuntimeEventRequest {
                    runtime_id: failed_runtime_id.clone(),
                    event_seq,
                    expected_revision: event_seq - 1,
                    lease_id: "lease-failed".to_string(),
                    idempotency_key: format!("{failed_runtime_id}:{event_seq}"),
                    event,
                })
                .expect("commit failed runtime event"),
            RuntimeEventCommitOutcome::Applied { .. }
        ));
    }

    assert!(store
        .register_runtime(RegisterRuntimeRequest {
            runtime_id: retry_runtime_id.clone(),
            session_id: session_id.clone(),
            fallback_from_id: Some("runtime-stale".to_string()),
        })
        .is_err());
    assert!(matches!(
        store
            .register_runtime(RegisterRuntimeRequest {
                runtime_id: retry_runtime_id.clone(),
                session_id: session_id.clone(),
                fallback_from_id: Some(failed_runtime_id.clone()),
            })
            .expect("register valid retry"),
        RuntimeRegistrationOutcome::Registered { .. }
    ));
    assert!(matches!(
        store
            .register_runtime(RegisterRuntimeRequest {
                runtime_id: retry_runtime_id.clone(),
                session_id: session_id.clone(),
                fallback_from_id: Some("runtime-conflict".to_string()),
            })
            .expect("repeat registration should return an identity conflict"),
        RuntimeRegistrationOutcome::RuntimeIdConflict
    ));
    store
        .activate_runtime_lease(ActivateRuntimeLeaseRequest {
            runtime_id: retry_runtime_id.clone(),
            lease_id: "lease-retry".to_string(),
        })
        .expect("activate retry lease");

    let mismatched_created = runtime_aggregate(&retry_runtime_id, &session_id)
        .take_uncommitted_events()
        .remove(0);
    let request = |event, suffix: &str| CommitRuntimeEventRequest {
        runtime_id: retry_runtime_id.clone(),
        event_seq: 1,
        expected_revision: 0,
        lease_id: "lease-retry".to_string(),
        idempotency_key: format!("{retry_runtime_id}:1:{suffix}"),
        event,
    };
    assert!(matches!(
        store
            .commit_runtime_event(request(mismatched_created, "mismatch"))
            .expect("mismatched creation should be a typed rejection"),
        RuntimeEventCommitOutcome::InvalidEvent { error }
            if error.contains("fallback_from_id")
    ));

    let retry_created = runtime_aggregate_with_fallback(
        &retry_runtime_id,
        &session_id,
        Some(failed_runtime_id.clone()),
    )
    .take_uncommitted_events()
    .remove(0);
    assert!(matches!(
        store
            .commit_runtime_event(request(retry_created, "valid"))
            .expect("matching creation should commit"),
        RuntimeEventCommitOutcome::Applied { .. }
    ));
    let replay = store
        .replay_runtime(ReplayRuntimeRequest {
            runtime_id: retry_runtime_id,
        })
        .expect("replay retry runtime")
        .expect("retry runtime exists");
    assert_eq!(
        replay.aggregate.fallback_from_id.as_deref(),
        Some(failed_runtime_id.as_str())
    );
}

#[test]
fn session_delta_is_idempotent_rejects_conflicts_and_merges_projection_parts() {
    let db = DirectDbGuard::new();
    let store = SessionLogStore::open_default().expect("store");
    let workspace = db.workspace("session-delta-idempotency");
    let session_id = format!("session-delta-{}", uuid::Uuid::new_v4());
    create_typed_session(&store, &workspace, &session_id);
    let management = typed_management(&store, &session_id);

    let assistant = delta_entry(
        &session_id,
        0,
        r#"{"role":"assistant","content":"answer"}"#,
        "runtime-delta.message",
        serde_json::json!([{
            "id": "runtime-delta.message",
            "type": "text",
            "text": "answer",
            "content": "answer"
        }]),
        10,
        11,
    );
    assert_eq!(
        store
            .persist_session_delta(delta_request(
                &session_id,
                0,
                None,
                &management,
                0,
                vec![assistant],
            ))
            .expect("persist assistant delta"),
        (1, 1)
    );

    let tool = delta_entry(
        &session_id,
        1,
        r#"{"type":"tool_result","runtime_id":"runtime-delta"}"#,
        "runtime-delta.message",
        serde_json::json!([{
            "id": "runtime-delta.tool.command_run",
            "type": "tool",
            "tool": "command_run",
            "state": {"status": "completed"}
        }]),
        12,
        20,
    );
    let tool_request = delta_request(
        &session_id,
        1,
        Some(&management),
        &management,
        0,
        vec![tool],
    );
    assert_eq!(
        store
            .persist_session_delta(tool_request.clone())
            .expect("persist tool delta"),
        (2, 2)
    );
    assert_eq!(
        store
            .persist_session_delta(tool_request)
            .expect("repeat identical tool delta"),
        (2, 2)
    );
    let mixed_replay = delta_request(
        &session_id,
        1,
        Some(&management),
        &management,
        0,
        vec![delta_entry(
            &session_id,
            2,
            r#"{"role":"assistant","content":"new"}"#,
            "runtime-mixed-replay.message",
            serde_json::json!([]),
            13,
            13,
        )],
    );
    let error = store
        .persist_session_delta(mixed_replay)
        .expect_err("historical management sequence must not append new context");
    assert!(error
        .to_string()
        .contains("historical management delta cannot append new context"));
    let cursors = store
        .read_context_slice(ReadContextSliceRequest {
            session_id: session_id.clone(),
            max_estimated_tokens: 1_000,
        })
        .expect("read cursors after rejected mixed replay");
    assert_eq!(cursors.next_sequence, 2);
    assert_eq!(cursors.next_management_sequence, 2);

    let (_, records) = store
        .list_session_records(ListSessionRecordsRequest {
            session_id: session_id.clone(),
            page: 0,
            page_size: 10,
        })
        .expect("list merged projection");
    assert_eq!(records.len(), 1);
    assert_eq!(
        records[0].record["parts"]
            .as_array()
            .expect("merged projection parts should be an array")
            .len(),
        2
    );
    assert_eq!(records[0].record["parts"][0]["type"], "text");
    assert_eq!(records[0].record["parts"][1]["type"], "tool");
    assert_eq!(records[0].record["created_at"], 10);
    assert_eq!(records[0].record["updated_at"], 20);

    let legacy_update = delta_entry(
        &session_id,
        2,
        r#"{"role":"assistant","content":"updated"}"#,
        "runtime-delta.message",
        serde_json::json!([{
            "type": "text",
            "text": "updated",
            "content": "updated"
        }]),
        10,
        21,
    );
    assert_eq!(
        store
            .persist_session_delta(delta_request(
                &session_id,
                2,
                Some(&management),
                &management,
                0,
                vec![legacy_update],
            ))
            .expect("replace legacy projection without part ids"),
        (3, 3)
    );
    let (_, records) = store
        .list_session_records(ListSessionRecordsRequest {
            session_id: session_id.clone(),
            page: 0,
            page_size: 10,
        })
        .expect("list replaced legacy projection");
    assert_eq!(records.len(), 1);
    assert_eq!(
        records[0].record["parts"]
            .as_array()
            .expect("replaced projection parts should be an array")
            .len(),
        1
    );
    assert_eq!(records[0].record["parts"][0]["text"], "updated");

    let conflict = delta_entry(
        &session_id,
        1,
        r#"{"type":"tool_result","runtime_id":"different"}"#,
        "runtime-delta.message",
        serde_json::json!([]),
        12,
        21,
    );
    let error = store
        .persist_session_delta(delta_request(
            &session_id,
            3,
            Some(&management),
            &management,
            0,
            vec![conflict],
        ))
        .expect_err("conflicting duplicate must fail");
    assert!(error.to_string().contains("different entry"));

    let gap = delta_entry(
        &session_id,
        4,
        r#"{"role":"assistant","content":"gap"}"#,
        "runtime-gap.message",
        serde_json::json!([]),
        30,
        30,
    );
    let error = store
        .persist_session_delta(delta_request(
            &session_id,
            3,
            Some(&management),
            &management,
            0,
            vec![gap],
        ))
        .expect_err("sequence gap must fail");
    assert!(error.to_string().contains("out-of-order context record"));

    let context = store
        .read_context_slice(ReadContextSliceRequest {
            session_id,
            max_estimated_tokens: 1_000,
        })
        .expect("read unchanged context");
    assert_eq!(context.next_sequence, 3);
    assert_eq!(context.records.len(), 3);
}

#[test]
fn context_slice_is_bounded_and_compaction_cutoff_is_canonical() {
    let db = DirectDbGuard::new();
    let store = SessionLogStore::open_default().expect("store");
    let workspace = db.workspace("session-delta-window");
    let session_id = format!("session-window-{}", uuid::Uuid::new_v4());
    create_typed_session(&store, &workspace, &session_id);
    let management = typed_management(&store, &session_id);
    let entries = (0..4)
        .map(|sequence| {
            delta_entry(
                &session_id,
                sequence,
                &format!("record-{sequence}-xxxxxxxx"),
                &format!("message-{sequence}"),
                serde_json::json!([]),
                sequence as i64 + 1,
                sequence as i64 + 1,
            )
        })
        .collect();
    assert_eq!(
        store
            .persist_session_delta(delta_request(&session_id, 0, None, &management, 0, entries,))
            .expect("persist context window"),
        (4, 1)
    );

    let bounded = store
        .read_context_slice(ReadContextSliceRequest {
            session_id: session_id.clone(),
            max_estimated_tokens: 5,
        })
        .expect("read bounded context");
    assert_eq!(bounded.next_sequence, 4);
    assert_eq!(bounded.retained_from_sequence, 0);
    assert_eq!(bounded.records.len(), 1);
    assert_eq!(bounded.records[0].sequence, 3);

    let mut compacted_management = management.clone();
    compacted_management.session_log_retention.omitted_entries = 2;
    assert_eq!(
        store
            .persist_session_delta(delta_request(
                &session_id,
                1,
                Some(&management),
                &compacted_management,
                2,
                Vec::new(),
            ))
            .expect("advance compaction cutoff"),
        (4, 2)
    );
    let retained = store
        .read_context_slice(ReadContextSliceRequest {
            session_id,
            max_estimated_tokens: 1_000,
        })
        .expect("read retained context");
    assert_eq!(retained.retained_from_sequence, 2);
    assert_eq!(
        retained
            .records
            .iter()
            .map(|record| record.sequence)
            .collect::<Vec<_>>(),
        vec![2, 3]
    );
}

#[test]
fn appending_session_delta_never_updates_existing_context_rows() {
    let db = DirectDbGuard::new();
    let store = SessionLogStore::open_default().expect("store");
    let workspace = db.workspace("session-delta-append-only");
    let session_id = format!("session-append-only-{}", uuid::Uuid::new_v4());
    create_typed_session(&store, &workspace, &session_id);
    let management = typed_management(&store, &session_id);
    let first = delta_entry(
        &session_id,
        0,
        "first-context-record",
        "message-first",
        serde_json::json!([]),
        1,
        1,
    );
    store
        .persist_session_delta(delta_request(
            &session_id,
            0,
            None,
            &management,
            0,
            vec![first],
        ))
        .expect("persist first context row");

    let conn = rusqlite::Connection::open(db.workspace_db(&workspace)).expect("workspace db");
    conn.execute_batch(
        "CREATE TRIGGER reject_context_updates
         BEFORE UPDATE ON session_context_records
         BEGIN SELECT RAISE(ABORT, 'context rows are append-only'); END;
         CREATE TRIGGER reject_context_deletes
         BEFORE DELETE ON session_context_records
         BEGIN SELECT RAISE(ABORT, 'context rows are append-only'); END;",
    )
    .expect("install append-only test triggers");
    drop(conn);

    let second = delta_entry(
        &session_id,
        1,
        "second-context-record",
        "message-second",
        serde_json::json!([]),
        2,
        2,
    );
    assert_eq!(
        store
            .persist_session_delta(delta_request(
                &session_id,
                1,
                Some(&management),
                &management,
                0,
                vec![second],
            ))
            .expect("append without rewriting context history"),
        (2, 2)
    );
}

#[test]
fn fork_copies_frontend_projection_across_workspaces_without_copying_raw_context() {
    let db = DirectDbGuard::new();
    let store = SessionLogStore::open_default().expect("store");
    let source_workspace = db.workspace("fork-source");
    let target_workspace = db.workspace("fork-target");
    let source_id = format!("fork-source-{}", uuid::Uuid::new_v4());
    let target_id = format!("fork-target-{}", uuid::Uuid::new_v4());
    create_typed_session(&store, &source_workspace, &source_id);
    let management = typed_management(&store, &source_id);
    store
        .persist_session_delta(delta_request(
            &source_id,
            0,
            None,
            &management,
            0,
            vec![
                message_delta_entry(
                    0,
                    "source-user",
                    &source_id,
                    "user",
                    None,
                    "source-user-part",
                    "fork question",
                    10,
                    11,
                ),
                message_delta_entry(
                    1,
                    "source-assistant",
                    &source_id,
                    "assistant",
                    Some("source-user"),
                    "source-assistant-part",
                    "fork answer",
                    12,
                    13,
                ),
                message_delta_entry(
                    2,
                    "source-system",
                    &source_id,
                    "system",
                    None,
                    "source-system-part",
                    "do not copy",
                    14,
                    15,
                ),
            ],
        ))
        .expect("persist fork source projection");
    let source_db = rusqlite::Connection::open(db.workspace_db(&source_workspace))
        .expect("source workspace DB");
    source_db
        .execute(
            "UPDATE sessions SET todos_json = ?2 WHERE session_id = ?1",
            rusqlite::params![source_id, r#"[{"id":"todo-source"}]"#],
        )
        .expect("seed source todos");
    drop(source_db);

    let result = store
        .create_session(CreateSessionRequest {
            command_id: format!("create:{target_id}"),
            session_id: target_id.clone(),
            creation_command: SessionCommand::ForkSession {
                parent_id: source_id.clone(),
            },
            copy_context: true,
            workspace: target_workspace.clone(),
            session_directory: target_workspace,
            name: "Fork".to_string(),
            created_at: 20,
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
            initial_task_plan_patch: None,
        })
        .expect("create atomic fork");
    assert_eq!(result.message_count, 2);
    assert_eq!(result.last_user_message_at, Some(11));
    assert_eq!(
        result.projection.parent_id.as_deref(),
        Some(source_id.as_str())
    );

    let snapshot = store
        .get_session(GetSessionRequest {
            session_id: target_id.clone(),
        })
        .expect("read fork")
        .expect("fork exists");
    assert_eq!(snapshot.message_count, 2);
    assert_eq!(
        snapshot.todos,
        vec![serde_json::json!({"id": "todo-source"})]
    );
    let (_, records) = store
        .list_session_records(ListSessionRecordsRequest {
            session_id: target_id.clone(),
            page: 0,
            page_size: 10,
        })
        .expect("read fork projection");
    assert_eq!(records.len(), 2);
    assert_eq!(records[0].role, "user");
    assert_eq!(records[1].role, "assistant");
    assert_ne!(records[0].message_id, "source-user");
    assert_ne!(records[1].message_id, "source-assistant");
    assert_eq!(records[0].record["session_id"], target_id);
    assert_eq!(records[1].record["session_id"], target_id);
    assert_ne!(records[0].record["parts"][0]["id"], "source-user-part");
    assert_ne!(records[1].record["parts"][0]["id"], "source-assistant-part");
    assert_eq!(records[1].record["parent_id"], records[0].record["id"]);
    assert_eq!(records[0].record["parts"][0]["text"], "fork question");
    assert_eq!(records[1].record["parts"][0]["text"], "fork answer");
    let context = store
        .read_context_slice(ReadContextSliceRequest {
            session_id: target_id,
            max_estimated_tokens: 1_000,
        })
        .expect("read fork context");
    assert!(context.records.is_empty());
    assert_eq!(context.next_sequence, 0);
}

#[test]
fn invalid_fork_projection_does_not_create_target_session() {
    let db = DirectDbGuard::new();
    let store = SessionLogStore::open_default().expect("store");
    let source_workspace = db.workspace("invalid-fork-source");
    let target_workspace = db.workspace("invalid-fork-target");
    let source_id = format!("invalid-fork-source-{}", uuid::Uuid::new_v4());
    let target_id = format!("invalid-fork-target-{}", uuid::Uuid::new_v4());
    create_typed_session(&store, &source_workspace, &source_id);
    let management = typed_management(&store, &source_id);
    let invalid = SessionDeltaEntry {
        context: SessionContextRecord {
            sequence: 0,
            raw_record: "invalid projection fixture".to_string(),
        },
        projection: Some(SessionRecordProjection {
            session_id: source_id.clone(),
            message_id: "invalid-source-message".to_string(),
            role: "user".to_string(),
            created_at: 1,
            updated_at: 1,
            record: serde_json::json!({
                "id": "invalid-source-message",
                "session_id": source_id,
                "role": "user"
            }),
        }),
    };
    store
        .persist_session_delta(delta_request(
            &source_id,
            0,
            None,
            &management,
            0,
            vec![invalid],
        ))
        .expect("persist invalid fork source fixture");

    let error = store
        .create_session(CreateSessionRequest {
            command_id: format!("create:{target_id}"),
            session_id: target_id.clone(),
            creation_command: SessionCommand::ForkSession {
                parent_id: source_id,
            },
            copy_context: true,
            workspace: target_workspace.clone(),
            session_directory: target_workspace,
            name: "Invalid fork".to_string(),
            created_at: 2,
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
            initial_task_plan_patch: None,
        })
        .expect_err("invalid source projection must fail the fork");
    assert!(error.to_string().contains("has no parts"));
    assert!(
        store
            .get_session(GetSessionRequest {
                session_id: target_id,
            })
            .expect("query failed fork")
            .is_none(),
        "failed fork must not leave a target session"
    );
}

fn typed_create_request(
    workspace: &str,
    session_id: &str,
    name: &str,
    created_at: i64,
    task_plan: TaskPlan,
) -> CreateSessionRequest {
    CreateSessionRequest {
        command_id: format!("create:{session_id}"),
        session_id: session_id.to_string(),
        creation_command: SessionCommand::CreateSession { task_plan },
        copy_context: false,
        workspace: workspace.to_string(),
        session_directory: workspace.to_string(),
        name: name.to_string(),
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
        initial_task_plan_patch: None,
    }
}

fn create_typed_session(store: &SessionLogStore, workspace: &str, session_id: &str) {
    store
        .create_session(typed_create_request(
            workspace,
            session_id,
            "Runtime event session",
            1,
            TaskPlan::default(),
        ))
        .expect("create typed session");
}

fn execute_typed_command(
    store: &SessionLogStore,
    session_id: &str,
    session_command: SessionCommand,
) {
    store
        .execute_session_command(ExecuteSessionCommandRequest {
            command_id: uuid::Uuid::new_v4().to_string(),
            session_id: session_id.to_string(),
            session_command,
            message_projection: None,
        })
        .expect("execute typed session command");
}

fn typed_management(store: &SessionLogStore, session_id: &str) -> SessionManagement {
    let snapshot = store
        .get_session(GetSessionRequest {
            session_id: session_id.to_string(),
        })
        .expect("get typed session")
        .expect("typed session exists");
    snapshot
        .into_management()
        .expect("typed snapshot must contain one canonical lifecycle projection")
}

fn delta_request(
    session_id: &str,
    management_sequence: u64,
    previous_management: Option<&SessionManagement>,
    management: &SessionManagement,
    retained_from_sequence: u64,
    entries: Vec<SessionDeltaEntry>,
) -> PersistSessionDeltaRequest {
    let mut management = management.clone();
    management.session_log.clear();
    management.session_log_retention.omitted_entries = retained_from_sequence;
    PersistSessionDeltaRequest {
        session_id: session_id.to_string(),
        management_sequence,
        management_delta: SessionManagement::persistence_delta(previous_management, &management),
        retained_from_sequence,
        entries,
    }
}

fn persist_typed_entries(
    store: &SessionLogStore,
    session_id: &str,
    entries: Vec<SessionDeltaEntry>,
) {
    let expected_next_sequence = entries.len() as u64;
    store
        .persist_session_delta(delta_request(
            session_id,
            0,
            None,
            &typed_management(store, session_id),
            0,
            entries,
        ))
        .map(|(next_sequence, next_management_sequence)| {
            assert_eq!(next_sequence, expected_next_sequence);
            assert_eq!(next_management_sequence, 1);
        })
        .expect("persist typed session entries");
}

fn simple_delta_entry(
    session_id: &str,
    sequence: u64,
    message_id: &str,
    role: &str,
    timestamp: i64,
) -> SessionDeltaEntry {
    SessionDeltaEntry {
        context: SessionContextRecord {
            sequence,
            raw_record: serde_json::json!({ "role": role, "content": message_id }).to_string(),
        },
        projection: Some(SessionRecordProjection {
            session_id: session_id.to_string(),
            message_id: message_id.to_string(),
            role: role.to_string(),
            created_at: timestamp,
            updated_at: timestamp,
            record: serde_json::json!({
                "id": message_id,
                "session_id": session_id,
                "role": role,
                "parts": [],
                "created_at": timestamp,
                "updated_at": timestamp,
            }),
        }),
    }
}

fn user_message_projection(
    session_id: &str,
    message_id: &str,
    text: &str,
    timestamp: i64,
) -> SessionRecordProjection {
    SessionRecordProjection {
        session_id: session_id.to_string(),
        message_id: message_id.to_string(),
        role: "user".to_string(),
        created_at: timestamp,
        updated_at: timestamp,
        record: serde_json::json!({
            "id": message_id,
            "session_id": session_id,
            "role": "user",
            "parent_id": null,
            "parts": [{
                "id": format!("{message_id}-part"),
                "type": "text",
                "content": text,
                "text": text,
                "metadata": null,
                "call_id": null,
                "tool": null,
                "state": null
            }],
            "created_at": timestamp,
            "updated_at": timestamp
        }),
    }
}

fn assistant_message_projection(
    session_id: &str,
    message_id: &str,
    text: &str,
    timestamp: i64,
) -> SessionRecordProjection {
    let mut projection = user_message_projection(session_id, message_id, text, timestamp);
    projection.role = "assistant".to_string();
    projection.record["role"] = serde_json::Value::String("assistant".to_string());
    projection
}

fn delta_entry(
    session_id: &str,
    sequence: u64,
    raw_record: &str,
    message_id: &str,
    parts: serde_json::Value,
    created_at: i64,
    updated_at: i64,
) -> SessionDeltaEntry {
    SessionDeltaEntry {
        context: SessionContextRecord {
            sequence,
            raw_record: raw_record.to_string(),
        },
        projection: Some(SessionRecordProjection {
            session_id: session_id.to_string(),
            message_id: message_id.to_string(),
            role: "assistant".to_string(),
            created_at,
            updated_at,
            record: serde_json::json!({
                "id": message_id,
                "session_id": session_id,
                "role": "assistant",
                "parts": parts,
                "created_at": created_at,
                "updated_at": updated_at,
            }),
        }),
    }
}

#[expect(
    clippy::too_many_arguments,
    reason = "projection fixture keeps wire fields explicit"
)]
fn message_delta_entry(
    sequence: u64,
    message_id: &str,
    session_id: &str,
    role: &str,
    parent_id: Option<&str>,
    part_id: &str,
    text: &str,
    created_at: i64,
    updated_at: i64,
) -> SessionDeltaEntry {
    SessionDeltaEntry {
        context: SessionContextRecord {
            sequence,
            raw_record: format!(r#"{{"role":"{role}","content":"{text}"}}"#),
        },
        projection: Some(SessionRecordProjection {
            session_id: session_id.to_string(),
            message_id: message_id.to_string(),
            role: role.to_string(),
            created_at,
            updated_at,
            record: serde_json::json!({
                "id": message_id,
                "session_id": session_id,
                "role": role,
                "parent_id": parent_id,
                "parts": [{
                    "id": part_id,
                    "type": "text",
                    "content": text,
                    "text": text,
                    "metadata": null,
                    "call_id": null,
                    "tool": null,
                    "state": null
                }],
                "created_at": created_at,
                "updated_at": updated_at
            }),
        }),
    }
}

fn runtime_aggregate(runtime_id: &str, session_id: &str) -> RuntimeAggregate {
    runtime_aggregate_with_fallback(runtime_id, session_id, None)
}

fn runtime_aggregate_with_fallback(
    runtime_id: &str,
    session_id: &str,
    fallback_from_id: Option<String>,
) -> RuntimeAggregate {
    RuntimeAggregate::new_with_fallback(
        runtime_id.to_string(),
        session_id.to_string(),
        "agent-test".to_string(),
        RuntimeProviderConfig {
            base: ProviderConfig {
                tura_llm_name: "provider-test".to_string(),
                default_model_tier: None,
                current_model: Some("model-test".to_string()),
                stream: true,
                temperature: 0.0,
                max_tokens: 1024,
                tool_choice: ToolChoice::Auto,
                time_out_ms: 1_000,
            },
            thinking: false,
            provider_name: "provider-test".to_string(),
            model_name: "model-test".to_string(),
            provider_url_name: "provider-test".to_string(),
            llm_provider_name: "provider-test".to_string(),
        },
        chrono::Utc::now(),
        fallback_from_id,
    )
    .expect("runtime fixture should be valid")
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

    enqueue_command(&SessionLogCommand::CreateSession(typed_create_request(
        &workspace,
        &session_id,
        "File Queue",
        1,
        TaskPlan::default(),
    )))
    .expect("enqueue create");
    assert_eq!(file_queue::drain_queue(&store, 10).expect("drain"), 1);
    enqueue_command(&SessionLogCommand::PersistSessionDelta(Box::new(
        delta_request(
            &session_id,
            0,
            None,
            &typed_management(&store, &session_id),
            0,
            vec![simple_delta_entry(
                &session_id,
                0,
                &format!("m-{session_id}"),
                "assistant",
                1,
            )],
        ),
    )))
    .expect("enqueue delta");
    assert_eq!(file_queue::drain_queue(&store, 10).expect("drain"), 1);
    let queued_todos = vec![serde_json::json!({"id": "queued-todo"})];
    enqueue_command(&SessionLogCommand::UpdateSessionTodos(
        UpdateSessionTodosRequest {
            command_id: format!("{session_id}:queued-todos"),
            session_id: session_id.clone(),
            todos: queued_todos.clone(),
            updated_at: 2,
        },
    ))
    .expect("enqueue todos");
    assert_eq!(file_queue::drain_queue(&store, 10).expect("drain"), 1);
    let session = store
        .get_session(GetSessionRequest { session_id })
        .expect("get session")
        .expect("session should be written");
    assert_eq!(session.name.as_deref(), Some("File Queue"));
    assert_eq!(session.message_count, 1);
    assert_eq!(session.todos, queued_todos);
}

#[test]
fn file_queue_recovers_orphaned_processing_items() {
    let db = DirectDbGuard::new();
    let store = SessionLogStore::open_default().expect("store");
    let workspace = db.workspace("file-queue-recover-workspace");
    let session_id = format!("file-queue-recover-{}", uuid::Uuid::new_v4());
    let command = SessionLogCommand::CreateSession(typed_create_request(
        &workspace,
        &session_id,
        "Recovered queue session",
        1,
        TaskPlan::default(),
    ));

    let pending = enqueue_command(&command).expect("enqueue");
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
