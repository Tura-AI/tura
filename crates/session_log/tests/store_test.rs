use session_log::{
    file_queue, GetSessionRequest, ListSessionRecordsRequest, ListSessionsRequest,
    SessionLogCommand, SessionLogStore, UpsertSessionRequest,
};
use std::process::Command;

#[test]
fn stores_workspaces_sessions_and_last_record_page() {
    let store = SessionLogStore::open_default().expect("store");
    let nonce = uuid::Uuid::new_v4().to_string();
    let session_id = format!("s-{nonce}");
    let workspace = format!(r"C:\repo-{nonce}\");

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
    let normalized_workspace = format!("C:/repo-{nonce}");
    let workspace_summary = workspaces
        .iter()
        .find(|item| item.directory == normalized_workspace)
        .expect("unique workspace should be listed");
    assert_eq!(workspace_summary.session_count, 1);

    let (page, sessions) = store
        .list_sessions(ListSessionsRequest {
            workspace: normalized_workspace,
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
}

#[test]
fn handles_concurrent_upserts_and_workspace_pagination() {
    let store = SessionLogStore::open_default().expect("store");
    let nonce = uuid::Uuid::new_v4().to_string();
    let workspace = format!("C:/stress-{nonce}");
    let started = std::time::Instant::now();
    let worker_count = 8;
    let per_worker = 25;

    let mut workers = Vec::new();
    for worker in 0..worker_count {
        let store = store.clone();
        let workspace = workspace.clone();
        let nonce = nonce.clone();
        workers.push(std::thread::spawn(move || {
            for index in 0..per_worker {
                let sequence = worker * per_worker + index;
                let session_id = format!("stress-{nonce}-{sequence}");
                store
                    .upsert_session(UpsertSessionRequest {
                        session: serde_json::json!({
                            "id": session_id,
                            "name": format!("Stress {sequence}"),
                            "directory": workspace,
                            "created_at": sequence,
                            "updated_at": sequence,
                            "status": "idle",
                            "management": {
                                "session_id": session_id,
                                "session_name": format!("Stress {sequence}"),
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
                    })
                    .expect("upsert");
            }
        }));
    }

    for worker in workers {
        worker.join().expect("worker");
    }

    let (page, sessions) = store
        .list_sessions(ListSessionsRequest {
            workspace,
            page: 0,
            page_size: 50,
        })
        .expect("sessions");

    assert_eq!(page.total, worker_count * per_worker);
    assert_eq!(page.page_size, 50);
    assert_eq!(sessions.len(), 50);
    assert!(
        started.elapsed() < std::time::Duration::from_secs(30),
        "concurrent upsert smoke test took {:?}",
        started.elapsed()
    );
}

#[test]
fn cross_process_open_default_uses_one_queued_local_database() {
    let temp = tempfile::tempdir().expect("tempdir");
    let port = reserve_local_port();
    let nonce = uuid::Uuid::new_v4().to_string();
    let workspace = format!("C:/cross-process-{nonce}");
    let worker_count = 12;
    let current_exe = std::env::current_exe().expect("current test exe");

    let mut children = Vec::new();
    for worker in 0..worker_count {
        let session_id = format!("cross-process-{nonce}-{worker}");
        children.push(
            Command::new(&current_exe)
                .args([
                    "--ignored",
                    "--exact",
                    "cross_process_session_log_helper",
                    "--nocapture",
                ])
                .env("SESSION_LOG_CROSS_PROCESS_MODE", "upsert")
                .env("SESSION_LOG_CROSS_PROCESS_SESSION_ID", session_id)
                .env("SESSION_LOG_CROSS_PROCESS_WORKSPACE", &workspace)
                .env("SESSION_LOG_DB_ROOT", temp.path())
                .env("session_log_POSTGRES_PORT", port.to_string())
                .env_remove("session_log_DATABASE_URL")
                .env_remove("DATABASE_URL")
                .spawn()
                .expect("spawn helper"),
        );
    }

    for mut child in children {
        let status = child.wait().expect("wait helper");
        assert!(status.success(), "helper exited with {status}");
    }

    let status = Command::new(&current_exe)
        .args([
            "--ignored",
            "--exact",
            "cross_process_session_log_helper",
            "--nocapture",
        ])
        .env("SESSION_LOG_CROSS_PROCESS_MODE", "verify")
        .env("SESSION_LOG_CROSS_PROCESS_WORKSPACE", &workspace)
        .env(
            "SESSION_LOG_CROSS_PROCESS_EXPECTED",
            worker_count.to_string(),
        )
        .env("SESSION_LOG_DB_ROOT", temp.path())
        .env("session_log_POSTGRES_PORT", port.to_string())
        .env_remove("session_log_DATABASE_URL")
        .env_remove("DATABASE_URL")
        .status()
        .expect("verify helper");
    assert!(status.success(), "verify helper exited with {status}");
}

#[test]
fn file_queue_write_drains_into_session_store() {
    let temp = tempfile::tempdir().expect("tempdir");
    let port = reserve_local_port();
    let nonce = uuid::Uuid::new_v4().to_string();
    let session_id = format!("file-queue-{nonce}");
    let workspace = format!("C:/file-queue-{nonce}");
    let current_exe = std::env::current_exe().expect("current test exe");

    let status = Command::new(&current_exe)
        .args([
            "--ignored",
            "--exact",
            "file_queue_session_log_helper",
            "--nocapture",
        ])
        .env("SESSION_LOG_FILE_QUEUE_SESSION_ID", &session_id)
        .env("SESSION_LOG_FILE_QUEUE_WORKSPACE", &workspace)
        .env("SESSION_LOG_DB_ROOT", temp.path())
        .env("session_log_POSTGRES_PORT", port.to_string())
        .env_remove("session_log_DATABASE_URL")
        .env_remove("DATABASE_URL")
        .status()
        .expect("file queue helper");
    assert!(status.success(), "file queue helper exited with {status}");
}

#[test]
#[ignore]
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
#[ignore]
fn cross_process_session_log_helper() {
    let Ok(mode) = std::env::var("SESSION_LOG_CROSS_PROCESS_MODE") else {
        return;
    };
    let store = SessionLogStore::open_default().expect("store");
    let workspace = std::env::var("SESSION_LOG_CROSS_PROCESS_WORKSPACE").expect("workspace");

    if mode == "upsert" {
        let session_id = std::env::var("SESSION_LOG_CROSS_PROCESS_SESSION_ID").expect("session id");
        store
            .upsert_session(UpsertSessionRequest {
                session: serde_json::json!({
                    "id": session_id,
                    "name": "Cross Process",
                    "directory": workspace,
                    "created_at": 1,
                    "updated_at": 1,
                    "status": "idle",
                    "management": {
                        "session_id": session_id,
                        "session_name": "Cross Process",
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
            })
            .expect("upsert");
        return;
    }

    assert_eq!(mode, "verify");
    let expected = std::env::var("SESSION_LOG_CROSS_PROCESS_EXPECTED")
        .expect("expected")
        .parse::<u64>()
        .expect("expected number");
    let (page, sessions) = store
        .list_sessions(ListSessionsRequest {
            workspace,
            page: 0,
            page_size: 100,
        })
        .expect("sessions");
    assert_eq!(page.total, expected);
    assert_eq!(sessions.len() as u64, expected);
}

fn reserve_local_port() -> u16 {
    let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).expect("reserve port");
    listener.local_addr().expect("local addr").port()
}
