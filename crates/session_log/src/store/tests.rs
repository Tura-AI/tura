use super::connection::{init_workspace_db, with_connection};
use super::helpers::{
    bounded_page, millis_to_rfc3339, parse_json_field, remove_sqlite_files, session_state_text,
    string_at, task_management_value,
};
use super::payload::load_workspace_session_payload;
use lifecycle::{
    SessionAggregate, SessionEvent, SessionInput, SessionManagement, SessionQuery, SessionState,
    TaskPlan,
};
use rusqlite::params;
use serde_json::{json, Value};
use session_log_contract::{
    CreateSessionRequest, GetSessionRequest, ListSessionRecordsRequest, PersistSessionDeltaRequest,
    ReadContextSliceRequest, ReadSessionFeedRequest, SessionContextRecord, SessionDeltaEntry,
    SessionFeedEvent, SessionMetadata, SessionRecordProjection,
};
use std::path::Path;

fn insert_workspace_session(
    db_path: &Path,
    session_id: &str,
    state: SessionState,
    updated_at: i64,
) {
    let state_text = session_state_text(state).expect("state text");
    let task_plan = serde_json::from_value::<TaskPlan>(json!({
        "plan_summary": "Plan",
        "detailed_tasks": [{"task_id": "task-1"}]
    }))
    .expect("task plan");
    let mut events = vec![SessionEvent::SessionCreated {
        task_plan: task_plan.clone(),
    }];
    if state != SessionState::Created {
        events.push(SessionEvent::RuntimeStarted {
            runtime_id: "runtime-fixture".to_string(),
            state: SessionState::Running,
        });
    }
    match state {
        SessionState::Created | SessionState::Running => {}
        SessionState::Paused => events.push(SessionEvent::RuntimeStateApplied { state }),
        SessionState::Completed => events.push(SessionEvent::RuntimeCompleted {
            runtime_id: "runtime-fixture".to_string(),
            state,
        }),
        SessionState::Failed => events.push(SessionEvent::RuntimeFailed {
            runtime_id: "runtime-fixture".to_string(),
            state,
        }),
        SessionState::Cancelled => events.push(SessionEvent::RuntimeCancelled {
            runtime_id: "runtime-fixture".to_string(),
            state,
        }),
        SessionState::Interrupted => {
            events.push(SessionEvent::SessionInterrupted { state, task_plan })
        }
    }
    let aggregate = SessionAggregate::replay(session_id.to_string(), events.clone())
        .expect("fixture events should replay");
    let timestamp = chrono::DateTime::<chrono::Utc>::from_timestamp_millis(updated_at)
        .expect("fixture timestamp");
    let projection = aggregate.query(SessionQuery::Lifecycle);
    let mut management = SessionManagement::new(
        session_id.to_string(),
        "Test session".to_string(),
        "C:/workspace".into(),
        false,
        Vec::<String>::new(),
        SessionInput {
            user_input: String::new(),
            file_input: Vec::new(),
            agent: None,
            runtime_context: None,
            planning_mode_override: None,
        },
        String::new(),
        timestamp,
    );
    management.replace_lifecycle_projection(projection);
    let metadata = SessionMetadata {
        session_directory: "C:/workspace".to_string(),
        model: None,
        agent: None,
        session_type: "coding".to_string(),
        kill_processes_on_start: false,
        validator_enabled: false,
        force_planning: false,
        model_variant: None,
        model_acceleration_enabled: false,
        disable_permission_restrictions: management.disable_permission_restrictions,
        use_last_tool_call_response: management.use_last_tool_call_response,
        auto_session_name: management.auto_session_name,
        context_tokens: management.context_tokens,
        runtime_usage: management.runtime_usage.clone(),
    };
    with_connection(db_path, init_workspace_db, |conn| {
        conn.execute(
            "INSERT INTO sessions(
                    session_id, workspace, name, parent_id, created_at, updated_at,
                    state, status, message_count, task_management_json, management_json,
                    session_json, todos_json
                ) VALUES (?1, ?2, ?3, NULL, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                session_id,
                "C:/workspace",
                "Test session",
                10_i64,
                updated_at,
                state_text,
                state.ui_status(),
                0_i64,
                serde_json::to_string(&task_management_value(&aggregate.task_plan))
                    .expect("task json"),
                serde_json::to_string(&management).expect("management json"),
                serde_json::to_string(&metadata).expect("session json"),
                "[]",
            ],
        )?;
        for (event_seq, event) in events.iter().enumerate() {
            conn.execute(
                "INSERT INTO session_events(session_id, event_seq, event_json)
                 VALUES (?1, ?2, ?3)",
                params![
                    session_id,
                    event_seq as u64,
                    serde_json::to_string(event).expect("event json")
                ],
            )?;
        }
        Ok(())
    })
    .expect("insert workspace session");
}

#[test]
fn state_text_uses_canonical_snake_case() {
    assert_eq!(
        session_state_text(SessionState::Interrupted).expect("state text"),
        "interrupted"
    );
}

#[test]
fn workspace_schema_rejects_pre_canonical_database() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db_path = dir.path().join("legacy-workspace.sqlite3");
    let conn = rusqlite::Connection::open(&db_path).expect("legacy db");
    conn.execute_batch(
        "CREATE TABLE sessions (
            session_id TEXT PRIMARY KEY,
            workspace TEXT NOT NULL,
            name TEXT,
            parent_id TEXT,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL,
            last_user_message_at INTEGER,
            state TEXT,
            status TEXT,
            message_count INTEGER NOT NULL DEFAULT 0,
            task_management_json TEXT NOT NULL,
            management_json TEXT NOT NULL,
            session_json TEXT NOT NULL,
            todos_json TEXT NOT NULL DEFAULT '[]'
        );",
    )
    .expect("legacy schema");
    let error = init_workspace_db(&conn)
        .expect_err("pre-canonical schema must be rejected")
        .to_string();
    assert!(error.contains("incompatible workspace session database schema"));
    assert!(error.contains("start with a clean canonical database"));
}

#[test]
fn workspace_schema_requires_runtime_fallback_source_column() {
    let conn = rusqlite::Connection::open_in_memory().expect("workspace db");
    init_workspace_db(&conn).expect("initialize canonical workspace schema");
    conn.execute_batch("ALTER TABLE runtimes DROP COLUMN fallback_from_id;")
        .expect("remove fallback column to model the previous schema");

    let error = init_workspace_db(&conn)
        .expect_err("workspace schema without runtime fallback source must be rejected")
        .to_string();
    assert!(error.contains("incompatible workspace session database schema"));
    assert!(error.contains("table runtimes has columns"));
    assert!(error.contains("fallback_from_id"));
}

#[test]
fn index_schema_does_not_store_canonical_lifecycle_aggregate() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db_path = dir.path().join("index.sqlite3");
    let conn = rusqlite::Connection::open(&db_path).expect("index db");
    super::connection::init_index_db(&conn).expect("initialize index");
    let columns = conn
        .prepare("PRAGMA table_info(sessions)")
        .expect("prepare columns")
        .query_map([], |row| row.get::<_, String>(1))
        .expect("query columns")
        .collect::<std::result::Result<Vec<_>, _>>()
        .expect("collect columns");
    assert!(!columns.iter().any(|column| column == "lifecycle_json"));
}

#[test]
fn workspace_payload_exposes_canonical_lifecycle_projection() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db_path = dir.path().join("workspace.sqlite3");
    insert_workspace_session(&db_path, "s-running", SessionState::Running, 100);

    let payload = load_workspace_session_payload(&db_path, "s-running")
        .expect("load payload")
        .expect("payload exists");
    assert_eq!(payload.lifecycle_projection.session_id, "s-running");
    assert_eq!(payload.lifecycle_projection.state, SessionState::Running);
    assert_eq!(
        payload.lifecycle_projection.task_plan.detailed_tasks[0].task_id,
        "task-1"
    );
    assert!(!payload.lifecycle_projection.cancelled);
    assert_eq!(payload.updated_at, 100);
}

#[test]
fn load_workspace_payload_reports_json_corruption_with_session_context() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db_path = dir.path().join("workspace.sqlite3");
    insert_workspace_session(&db_path, "s-bad-json", SessionState::Running, 100);
    with_connection(&db_path, init_workspace_db, |conn| {
        conn.execute(
            "UPDATE sessions SET management_json = '{bad json' WHERE session_id = ?1",
            params!["s-bad-json"],
        )?;
        Ok(())
    })
    .expect("corrupt json");

    let error = match load_workspace_session_payload(&db_path, "s-bad-json") {
        Ok(_) => panic!("corrupt JSON should fail"),
        Err(error) => error.to_string(),
    };

    assert!(error.contains("failed to parse management_json for session s-bad-json"));
}

#[test]
fn helper_extractors_cover_nested_strings() {
    let value = json!({ "a": { "b": "text" } });

    assert_eq!(string_at(&value, &["a", "b"]).as_deref(), Some("text"));
    assert_eq!(string_at(&value, &["a", "missing"]), None);
}

#[test]
fn pagination_bounds_match_session_and_record_listing_contracts() {
    assert_eq!(bounded_page(0, 25, 0, false), 0);
    assert_eq!(bounded_page(99, 10, 95, false), 9);
    assert_eq!(bounded_page(0, 10, 95, false), 0);
    assert_eq!(bounded_page(0, 10, 95, true), 9);
    assert_eq!(bounded_page(2, 10, 95, true), 2);
}

#[test]
fn internal_prompt_context_is_stored_fully_but_never_projected_to_the_frontend_feed() {
    let root = tempfile::tempdir().expect("session store root");
    let workspace = root.path().join("workspace");
    std::fs::create_dir_all(&workspace).expect("workspace");
    let store = super::SessionLogStore::open(root.path().join("db")).expect("session store");
    let session_id = "context-visibility-contract";
    let workspace_text = workspace.to_string_lossy().to_string();
    store
        .create_session(CreateSessionRequest {
            command_id: format!("create:{session_id}"),
            session_id: session_id.to_string(),
            creation_command: lifecycle::SessionCommand::CreateSession {
                task_plan: TaskPlan::default(),
            },
            copy_context: false,
            workspace: workspace_text.clone(),
            session_directory: workspace_text,
            name: "context visibility".to_string(),
            created_at: 1_000,
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
        .expect("create session");
    let snapshot = store
        .get_session(GetSessionRequest {
            session_id: session_id.to_string(),
        })
        .expect("read session")
        .expect("created session");
    let management = snapshot
        .into_management()
        .expect("canonical management snapshot");

    let prompt_style = json!({
        "type": "prompt_style",
        "role": "system",
        "content": "FULL_PROMPT_STYLE_SENTINEL",
        "created_at": 1_001,
        "updated_at": 1_001
    });
    let manual = json!({
        "type": "runtime_prompt_manual",
        "task_type": "debug",
        "manual_name": "Debug",
        "role": "system",
        "content": "FULL_OPERATION_MANUAL_SENTINEL",
        "created_at": 1_002,
        "updated_at": 1_002
    });
    let user_record =
        frontend_message_record(session_id, "context-user", "user", "visible user", 1_003);
    let assistant_record = frontend_message_record(
        session_id,
        "context-assistant",
        "assistant",
        "visible assistant",
        1_004,
    );
    let raw_records = [
        prompt_style.to_string(),
        manual.to_string(),
        user_record.to_string(),
        assistant_record.to_string(),
    ];
    let entries = vec![
        context_only_entry(0, &raw_records[0]),
        context_only_entry(1, &raw_records[1]),
        projected_context_entry(2, &raw_records[2], user_record),
        projected_context_entry(3, &raw_records[3], assistant_record),
    ];
    let management_delta = SessionManagement::persistence_delta(Some(&management), &management);

    assert_eq!(
        store
            .persist_session_delta(PersistSessionDeltaRequest {
                session_id: session_id.to_string(),
                management_sequence: 0,
                management_delta,
                retained_from_sequence: 0,
                entries,
            })
            .expect("persist context delta"),
        (4, 1)
    );

    let context = store
        .read_context_slice(ReadContextSliceRequest {
            session_id: session_id.to_string(),
            max_estimated_tokens: u64::MAX,
        })
        .expect("read full context");
    assert_eq!(
        context
            .records
            .iter()
            .map(|record| record.raw_record.as_str())
            .collect::<Vec<_>>(),
        raw_records.iter().map(String::as_str).collect::<Vec<_>>()
    );

    let (_, records) = store
        .list_session_records(ListSessionRecordsRequest {
            session_id: session_id.to_string(),
            page: 0,
            page_size: 100,
        })
        .expect("list frontend records");
    assert_eq!(
        records
            .iter()
            .map(|record| record.role.as_str())
            .collect::<Vec<_>>(),
        vec!["user", "assistant"]
    );
    let serialized_records = serde_json::to_string(&records).expect("serialize frontend records");
    assert!(!serialized_records.contains("FULL_PROMPT_STYLE_SENTINEL"));
    assert!(!serialized_records.contains("FULL_OPERATION_MANUAL_SENTINEL"));

    let (feed, _) = store
        .read_session_feed(ReadSessionFeedRequest {
            session_id: session_id.to_string(),
            after_cursor: 0,
            limit: 100,
        })
        .expect("read frontend feed");
    let projected_roles = feed
        .iter()
        .filter_map(|entry| match &entry.event {
            SessionFeedEvent::MessageUpserted { message } => Some(message.role.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(projected_roles, vec!["user", "assistant"]);
    let serialized_feed = serde_json::to_string(&feed).expect("serialize feed");
    assert!(!serialized_feed.contains("FULL_PROMPT_STYLE_SENTINEL"));
    assert!(!serialized_feed.contains("FULL_OPERATION_MANUAL_SENTINEL"));
}

fn frontend_message_record(
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
        "parent_id": null,
        "parts": [{
            "id": format!("{message_id}:part"),
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
    })
}

fn context_only_entry(sequence: u64, raw_record: &str) -> SessionDeltaEntry {
    SessionDeltaEntry {
        context: SessionContextRecord {
            sequence,
            raw_record: raw_record.to_string(),
        },
        projection: None,
    }
}

fn projected_context_entry(sequence: u64, raw_record: &str, record: Value) -> SessionDeltaEntry {
    SessionDeltaEntry {
        context: SessionContextRecord {
            sequence,
            raw_record: raw_record.to_string(),
        },
        projection: Some(SessionRecordProjection {
            session_id: record["session_id"]
                .as_str()
                .expect("projection session id")
                .to_string(),
            message_id: record["id"]
                .as_str()
                .expect("projection message id")
                .to_string(),
            role: record["role"]
                .as_str()
                .expect("projection role")
                .to_string(),
            created_at: record["created_at"]
                .as_i64()
                .expect("projection created_at"),
            updated_at: record["updated_at"]
                .as_i64()
                .expect("projection updated_at"),
            record,
        }),
    }
}

#[test]
fn parse_json_field_and_timestamp_errors_are_contextual() {
    let parsed: Value =
        parse_json_field(r#"{"ok":true}"#, "payload", Some("s1")).expect("valid json");
    assert_eq!(parsed["ok"], true);

    let error = parse_json_field::<Value>("{bad", "payload", Some("s1"))
        .expect_err("bad json")
        .to_string();
    assert!(error.contains("failed to parse payload for session s1"));

    let error = millis_to_rfc3339(i64::MAX)
        .expect_err("timestamp overflow should fail")
        .to_string();
    assert!(error.contains("invalid session timestamp millis"));
}

#[test]
fn remove_sqlite_files_removes_db_wal_and_shm_idempotently() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db_path = dir.path().join("store.sqlite3");
    std::fs::write(&db_path, b"db").expect("db file");
    std::fs::write(format!("{}-wal", db_path.display()), b"wal").expect("wal file");
    std::fs::write(format!("{}-shm", db_path.display()), b"shm").expect("shm file");

    remove_sqlite_files(&db_path).expect("remove sqlite files");
    assert!(!db_path.exists());
    assert!(!Path::new(&format!("{}-wal", db_path.display())).exists());
    assert!(!Path::new(&format!("{}-shm", db_path.display())).exists());

    remove_sqlite_files(&db_path).expect("second remove is idempotent");
}
