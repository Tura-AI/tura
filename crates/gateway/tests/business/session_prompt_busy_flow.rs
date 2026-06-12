use axum::extract::{Json, Path};
use axum::response::IntoResponse;
use gateway::api::session::{
    append_session_user_command, prompt_async, session_user_commands,
    update_session_status_for_runtime, AppendUserCommandRequest, RuntimeSessionStatusRequest,
};
use gateway::api::types::SessionStatus as ApiSessionStatus;
use gateway::{session_store, SessionStatus};
use serde_json::json;

#[tokio::test]
async fn busy_session_prompt_business_flow_queues_user_command_without_router_dispatch() {
    let directory = std::env::temp_dir()
        .join(format!("tura-busy-prompt-{}", uuid::Uuid::new_v4()))
        .to_string_lossy()
        .to_string();
    let session = session_store().create_session(
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
    session_store().update_session_status(&session.id, SessionStatus::Busy);
    session_store().mark_cancelled(&session.id);

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
    assert!(
        !session_store().is_cancelled(&session.id),
        "new prompt should clear stale cancellation state"
    );

    let Json(commands) = session_user_commands(Path(session.id.clone())).await;
    assert_eq!(commands["session_id"], session.id);
    assert_eq!(
        commands["commands"],
        json!(["continue after current tool finishes"])
    );
    let Json(empty) = session_user_commands(Path(session.id.clone())).await;
    assert_eq!(empty["commands"], json!([]));
}

#[tokio::test]
async fn busy_session_prompt_business_flow_queues_multiple_commands_fifo_and_uses_text_only_parts()
{
    let directory = std::env::temp_dir()
        .join(format!("tura-busy-prompt-fifo-{}", uuid::Uuid::new_v4()))
        .to_string_lossy()
        .to_string();
    let session = session_store().create_session(
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
    session_store().update_session_status(&session.id, SessionStatus::Busy);
    session_store().mark_cancelled(&session.id);

    let first = prompt_async(
        Path(session.id.clone()),
        Json(json!({
            "message_id": "busy-message-fifo-1",
            "parts": [
                { "id": "ignored-image", "type": "image", "text": "image text must not queue" },
                { "id": "busy-part-fifo-1", "type": "text", "text": "first queued " },
                { "id": "busy-part-fifo-2", "type": "text", "text": "command" }
            ]
        })),
    )
    .await
    .into_response();
    assert_eq!(first.status(), axum::http::StatusCode::NO_CONTENT);
    assert!(
        !session_store().is_cancelled(&session.id),
        "the first queued command should clear stale cancellation state"
    );

    session_store().mark_cancelled(&session.id);
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
    assert!(
        !session_store().is_cancelled(&session.id),
        "each new queued command should clear stale cancellation state"
    );

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
    assert_eq!(
        queued[0].parts[0].text.as_deref(),
        Some("first queued command")
    );
    assert_eq!(queued[1].id, "busy-message-fifo-2");
    assert_eq!(queued[1].parts[0].id, "busy-part-fifo-3");
    assert_eq!(
        queued[1].parts[0].text.as_deref(),
        Some("second queued command")
    );
    assert_eq!(queued[2].id, "busy-message-fifo-3");
    assert_eq!(queued[2].parts[0].text.as_deref(), Some("Prompt submitted"));

    let Json(commands) = session_user_commands(Path(session.id.clone())).await;
    assert_eq!(
        commands["commands"],
        json!([
            "first queued command",
            "second queued command",
            "Prompt submitted"
        ])
    );
    let Json(empty) = session_user_commands(Path(session.id.clone())).await;
    assert_eq!(empty["commands"], json!([]));
}

#[tokio::test]
async fn busy_session_prompt_business_flow_runtime_status_requires_canonical_values_and_preserves_queue(
) {
    let directory = std::env::temp_dir()
        .join(format!(
            "tura-runtime-status-command-{}",
            uuid::Uuid::new_v4()
        ))
        .to_string_lossy()
        .to_string();
    let session = session_store().create_session(
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

    let first = append_session_user_command(
        Path(session.id.clone()),
        Json(AppendUserCommandRequest {
            command: "inspect current files".to_string(),
        }),
    )
    .await;
    assert_eq!(first["commands"], json!(["inspect current files"]));
    let second = append_session_user_command(
        Path(session.id.clone()),
        Json(AppendUserCommandRequest {
            command: "continue after inspection".to_string(),
        }),
    )
    .await;
    assert_eq!(
        second["commands"],
        json!(["inspect current files", "continue after inspection"])
    );

    let Json(commands) = session_user_commands(Path(session.id.clone())).await;
    assert_eq!(
        commands["commands"],
        json!(["inspect current files", "continue after inspection"])
    );
    let Json(empty) = session_user_commands(Path(session.id.clone())).await;
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
