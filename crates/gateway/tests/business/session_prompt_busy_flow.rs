use axum::body;
use axum::extract::{Json, Path};
use axum::response::IntoResponse;
use gateway::api::session::{
    append_session_user_command, prompt_async, session_user_commands,
    update_session_status_for_runtime,
};
use gateway::contracts::{
    AppendUserCommandRequest, RuntimeSessionStatusRequest, SessionStatus as ApiSessionStatus,
};
use gateway::session_store;
use lifecycle::{SessionCommand, SessionState};
use serde_json::{json, Value};
use session_log::SessionLogStore;
use session_log_contract::{SessionLogCommand, SessionLogResponse};
use std::time::{Duration, Instant};

static SESSION_DB_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[tokio::test]
async fn busy_session_prompt_business_flow_queues_user_command_without_router_dispatch() {
    let _service = TestSessionDb::start();
    let directory = std::env::temp_dir()
        .join(format!("tura-busy-prompt-{}", uuid::Uuid::new_v4()))
        .to_string_lossy()
        .to_string();
    let session = create_test_session(directory);
    execute_test_command(&session.id, SessionCommand::RuntimeStarted);

    let response = prompt_async(
        Path(session.id.clone()),
        Json(json!({
            "messageID": "busy-message-1",
            "parts": [
                { "id": "busy-part-1", "type": "text", "text": "continue after current tool finishes" }
            ]
        })),
    )
    .await
    .into_response();
    assert_eq!(response.status(), axum::http::StatusCode::NO_CONTENT);

    let messages = session_store().get_messages(&session.id);
    let user_message = messages
        .iter()
        .find(|message| message.id == "busy-message-1")
        .expect("busy prompt should still persist the user message");
    assert_eq!(user_message.parts[0].id, "busy-part-1");
    assert_eq!(
        user_message.parts[0]
            .metadata
            .as_ref()
            .and_then(|value| value.get("kind")),
        Some(&json!("user_new_command"))
    );
    assert_running_not_cancelled(&session.id);

    let commands = response_json(session_user_commands(Path(session.id.clone())).await).await;
    assert_eq!(commands["session_id"], session.id);
    assert_eq!(
        commands["commands"],
        json!(["continue after current tool finishes"])
    );
    let empty = response_json(session_user_commands(Path(session.id.clone())).await).await;
    assert_eq!(empty["commands"], json!([]));
}

#[tokio::test]
async fn busy_session_prompt_business_flow_queues_multiple_commands_fifo_and_preserves_voice_parts()
{
    let _service = TestSessionDb::start();
    let directory = std::env::temp_dir()
        .join(format!("tura-busy-prompt-fifo-{}", uuid::Uuid::new_v4()))
        .to_string_lossy()
        .to_string();
    let session = create_test_session(directory);
    execute_test_command(&session.id, SessionCommand::RuntimeStarted);

    let first = prompt_async(
        Path(session.id.clone()),
        Json(json!({
            "message_id": "busy-message-fifo-1",
            "parts": [
                { "id": "ignored-image", "type": "image", "text": "image text must not queue" },
                { "id": "busy-part-fifo-1", "type": "text", "text": "first queued " },
                { "id": "busy-part-fifo-2", "type": "text", "text": "command" },
                { "id": "busy-part-voice-1", "type": "voice", "metadata": { "voice_status": "pending" } }
            ]
        })),
    )
    .await
    .into_response();
    assert_eq!(first.status(), axum::http::StatusCode::NO_CONTENT);
    assert_running_not_cancelled(&session.id);

    let second = prompt_async(
        Path(session.id.clone()),
        Json(json!({
            "messageID": "busy-message-fifo-2",
            "parts": [
                { "id": "ignored-file", "type": "file", "text": "file text must not queue" },
                { "id": "busy-part-fifo-3", "type": "text", "text": "second queued command" }
            ]
        })),
    )
    .await
    .into_response();
    assert_eq!(second.status(), axum::http::StatusCode::NO_CONTENT);
    assert_running_not_cancelled(&session.id);

    let fallback = prompt_async(
        Path(session.id.clone()),
        Json(json!({
            "messageID": "busy-message-fifo-3",
            "parts": [
                { "id": "ignored-file-only", "type": "file", "text": "file-only payload" }
            ]
        })),
    )
    .await
    .into_response();
    assert_eq!(fallback.status(), axum::http::StatusCode::NO_CONTENT);

    let messages = session_store().get_messages(&session.id);
    let queued = messages
        .iter()
        .filter(|message| {
            message
                .parts
                .first()
                .and_then(|part| part.metadata.as_ref())
                .and_then(|metadata| metadata.get("kind"))
                == Some(&json!("user_new_command"))
        })
        .collect::<Vec<_>>();
    assert_eq!(queued.len(), 3);
    assert_eq!(queued[0].id, "busy-message-fifo-1");
    assert_eq!(queued[0].parts[0].id, "busy-part-fifo-1");
    assert_eq!(queued[0].parts[1].id, "busy-part-fifo-2");
    assert_eq!(queued[0].parts[2].id, "busy-part-voice-1");
    assert_eq!(queued[0].parts[2].part_type, "voice");
    assert_eq!(queued[0].parts[0].text.as_deref(), Some("first queued "));
    assert_eq!(queued[0].parts[1].text.as_deref(), Some("command"));
    assert_eq!(queued[1].id, "busy-message-fifo-2");
    assert_eq!(queued[1].parts[0].id, "busy-part-fifo-3");
    assert_eq!(
        queued[1].parts[0].text.as_deref(),
        Some("second queued command")
    );
    assert_eq!(queued[2].id, "busy-message-fifo-3");
    assert_eq!(queued[2].parts[0].text.as_deref(), Some("Prompt submitted"));

    let commands = response_json(session_user_commands(Path(session.id.clone())).await).await;
    assert_eq!(
        commands["commands"],
        json!([
            "first queued command",
            "second queued command",
            "Prompt submitted"
        ])
    );
    let empty = response_json(session_user_commands(Path(session.id.clone())).await).await;
    assert_eq!(empty["commands"], json!([]));
}

#[tokio::test]
async fn busy_session_prompt_business_flow_runtime_status_requires_canonical_values_and_preserves_queue(
) {
    let _service = TestSessionDb::start();
    let directory = std::env::temp_dir()
        .join(format!(
            "tura-runtime-status-command-{}",
            uuid::Uuid::new_v4()
        ))
        .to_string_lossy()
        .to_string();
    let session = create_test_session(directory);

    let busy = update_session_status_for_runtime(
        Path(session.id.clone()),
        Json(RuntimeSessionStatusRequest {
            status: "busy".to_string(),
        }),
    )
    .await
    .into_response();
    assert_eq!(busy.status(), axum::http::StatusCode::OK);
    assert_eq!(
        session_store()
            .get_session(&session.id)
            .map(|session| session.status),
        Some(ApiSessionStatus::Busy)
    );

    let running_alias = update_session_status_for_runtime(
        Path(session.id.clone()),
        Json(RuntimeSessionStatusRequest {
            status: "running".to_string(),
        }),
    )
    .await
    .into_response();
    assert_eq!(
        running_alias.status(),
        axum::http::StatusCode::BAD_REQUEST,
        "runtime status is an internal contract and must not accept alias spelling"
    );
    assert_eq!(
        session_store()
            .get_session(&session.id)
            .map(|session| session.status),
        Some(ApiSessionStatus::Busy),
        "invalid status must not silently rewrite the session to idle"
    );

    let first = response_json(append_session_user_command(
        Path(session.id.clone()),
        Json(AppendUserCommandRequest {
            command: "inspect current files".to_string(),
        }),
    )
    .await)
    .await;
    assert_eq!(first["commands"], json!(["inspect current files"]));
    let second = response_json(append_session_user_command(
        Path(session.id.clone()),
        Json(AppendUserCommandRequest {
            command: "continue after inspection".to_string(),
        }),
    )
    .await)
    .await;
    assert_eq!(
        second["commands"],
        json!(["inspect current files", "continue after inspection"])
    );

    let commands = response_json(session_user_commands(Path(session.id.clone())).await).await;
    assert_eq!(
        commands["commands"],
        json!(["inspect current files", "continue after inspection"])
    );
    let empty = response_json(session_user_commands(Path(session.id.clone())).await).await;
    assert_eq!(empty["commands"], json!([]));

    let idle_from_busy = update_session_status_for_runtime(
        Path(session.id.clone()),
        Json(RuntimeSessionStatusRequest {
            status: "idle".to_string(),
        }),
    )
    .await
    .into_response();
    assert_eq!(idle_from_busy.status(), axum::http::StatusCode::OK);
    assert_eq!(
        session_store()
            .get_session(&session.id)
            .map(|session| session.status),
        Some(ApiSessionStatus::Idle)
    );

    let busy_again = update_session_status_for_runtime(
        Path(session.id.clone()),
        Json(RuntimeSessionStatusRequest {
            status: "busy".to_string(),
        }),
    )
    .await
    .into_response();
    assert_eq!(busy_again.status(), axum::http::StatusCode::OK);
    assert_eq!(
        session_store()
            .get_session(&session.id)
            .map(|session| session.status),
        Some(ApiSessionStatus::Busy)
    );

    let failed_alias = update_session_status_for_runtime(
        Path(session.id.clone()),
        Json(RuntimeSessionStatusRequest {
            status: "failed".to_string(),
        }),
    )
    .await
    .into_response();
    assert_eq!(failed_alias.status(), axum::http::StatusCode::BAD_REQUEST);
    assert_eq!(
        session_store()
            .get_session(&session.id)
            .map(|session| session.status),
        Some(ApiSessionStatus::Busy)
    );

    let error = update_session_status_for_runtime(
        Path(session.id.clone()),
        Json(RuntimeSessionStatusRequest {
            status: "error".to_string(),
        }),
    )
    .await
    .into_response();
    assert_eq!(error.status(), axum::http::StatusCode::OK);
    assert_eq!(
        session_store()
            .get_session(&session.id)
            .map(|session| session.status),
        Some(ApiSessionStatus::Error)
    );

    let rejected_idle_after_error = update_session_status_for_runtime(
        Path(session.id.clone()),
        Json(RuntimeSessionStatusRequest {
            status: "idle".to_string(),
        }),
    )
    .await
    .into_response();
    assert_eq!(
        rejected_idle_after_error.status(),
        axum::http::StatusCode::CONFLICT,
        "terminal error state cannot be reported as successfully reset to idle"
    );
    assert_eq!(
        session_store()
            .get_session(&session.id)
            .map(|session| session.status),
        Some(ApiSessionStatus::Error)
    );
}

fn create_test_session(directory: String) -> gateway::contracts::Session {
    let store = session_store();
    let info = store.build_session_info(
        Some(directory),
        None,
        None,
        Some("coding".to_string()),
        false,
        false,
        false,
        None,
        false,
        false,
    );
    let task_plan = info.management.task_plan.clone();
    store
        .create_canonical_session(info, SessionCommand::CreateSession { task_plan })
        .expect("canonical busy-flow session should be created")
}

fn execute_test_command(session_id: &str, command: SessionCommand) {
    session_store()
        .execute_canonical_session_command(session_id, command)
        .expect("canonical busy-flow command should succeed");
}

fn assert_running_not_cancelled(session_id: &str) {
    let projection = session_store()
        .session_lifecycle_projection(session_id)
        .expect("busy-flow lifecycle projection");
    assert_eq!(projection.state, SessionState::Running);
    assert!(!projection.cancelled);
}

async fn response_json(response: impl IntoResponse) -> Value {
    let response = response.into_response();
    assert!(response.status().is_success(), "response was {response:?}");
    let bytes = body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("response body");
    serde_json::from_slice(&bytes).expect("JSON response")
}

struct TestSessionDb {
    _guard: std::sync::MutexGuard<'static, ()>,
    previous_home: Option<std::ffi::OsString>,
    root: tempfile::TempDir,
    handle: Option<std::thread::JoinHandle<anyhow::Result<()>>>,
}

impl TestSessionDb {
    fn start() -> Self {
        let guard = SESSION_DB_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let previous_home = std::env::var_os("TURA_HOME");
        let root = tempfile::tempdir().expect("busy-flow session DB root");
        std::env::set_var("TURA_HOME", root.path());
        let store = SessionLogStore::open_default().expect("open busy-flow session DB");
        let handle = std::thread::spawn(move || session_log::ipc::serve_blocking(store));
        let started = Instant::now();
        while started.elapsed() < Duration::from_secs(10) {
            if session_log::ipc::service_is_running() {
                return Self {
                    _guard: guard,
                    previous_home,
                    root,
                    handle: Some(handle),
                };
            }
            assert!(!handle.is_finished(), "busy-flow session DB exited early");
            std::thread::sleep(Duration::from_millis(25));
        }
        panic!("busy-flow session DB did not start within 10 seconds");
    }
}

impl Drop for TestSessionDb {
    fn drop(&mut self) {
        let response = session_log::ipc::call_service(&SessionLogCommand::Shutdown)
            .expect("stop busy-flow session DB");
        assert!(matches!(response, SessionLogResponse::Ok));
        if let Some(handle) = self.handle.take() {
            handle
                .join()
                .expect("join busy-flow session DB")
                .expect("busy-flow session DB result");
        }
        match self.previous_home.take() {
            Some(value) => std::env::set_var("TURA_HOME", value),
            None => std::env::remove_var("TURA_HOME"),
        }
        let _ = self.root.path();
    }
}
