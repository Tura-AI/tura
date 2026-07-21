mod support;

use axum::body;
use axum::extract::{Json, Path};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use gateway::api::session::{delete_session, fork_session};
use gateway::contracts::{ForkSessionRequest, Session};
use gateway::session_store;
use lifecycle::SessionCommand;
use session_log_contract::{
    GetSessionRequest, ListSessionRecordsRequest, PersistSessionDeltaRequest, SessionContextRecord,
    SessionDeltaEntry, SessionLogCommand, SessionLogResponse, SessionRecord,
    SessionRecordProjection, SessionSnapshot,
};
use support::TestSessionDb;

#[tokio::test]
async fn fork_and_delete_are_applied_to_session_db() -> anyhow::Result<()> {
    let service = TestSessionDb::start()?;
    let workspace = service.workspace();

    let workspace_key = session_log::path::normalize_workspace(&workspace.to_string_lossy());
    let source_info = session_store().build_session_info(
        Some(workspace_key.clone()),
        Some("db-test-model".to_string()),
        Some("thoughtful".to_string()),
        Some("coding".to_string()),
        false,
        false,
        false,
        None,
        false,
        false,
    );
    let source_task_plan = source_info.management.task_plan.clone();
    let mut source_management = source_info.management.clone();
    let source = session_store()
        .create_canonical_session(
            source_info,
            SessionCommand::CreateSession {
                task_plan: source_task_plan,
            },
        )
        .map_err(anyhow::Error::msg)?;
    source_management.session_log.clear();
    source_management.session_log_retention.omitted_entries = 0;
    persist_source_message(&source.id, source_management)?;

    let response = fork_session(
        Path(source.id.clone()),
        Json(ForkSessionRequest {
            directory: Some(workspace_key),
            model: None,
            agent: None,
            copy_context: Some(true),
        }),
    )
    .await
    .into_response();
    assert_eq!(response.status(), StatusCode::OK);
    let body = body::to_bytes(response.into_body(), usize::MAX).await?;
    let forked: Session = serde_json::from_slice(&body)?;

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

fn persist_source_message(
    session_id: &str,
    management: lifecycle::SessionManagement,
) -> anyhow::Result<()> {
    let message_id = "source-user-message";
    let record = serde_json::json!({
        "id": message_id,
        "session_id": session_id,
        "role": "user",
        "parent_id": null,
        "parts": [{
            "id": "source-user-part",
            "type": "text",
            "content": "persist this context before fork",
            "text": "persist this context before fork",
            "metadata": null,
            "call_id": null,
            "tool": null,
            "state": null
        }],
        "created_at": 10,
        "updated_at": 11
    });
    let response = session_log_contract::client::call_service(
        &SessionLogCommand::PersistSessionDelta(Box::new(PersistSessionDeltaRequest {
            session_id: session_id.to_string(),
            management_sequence: 0,
            management_delta: lifecycle::SessionManagement::persistence_delta(None, &management),
            retained_from_sequence: 0,
            entries: vec![SessionDeltaEntry {
                context: SessionContextRecord {
                    sequence: 0,
                    raw_record: r#"{"role":"user","content":"persist this context before fork"}"#
                        .to_string(),
                },
                projection: Some(SessionRecordProjection {
                    session_id: session_id.to_string(),
                    message_id: message_id.to_string(),
                    role: "user".to_string(),
                    created_at: 10,
                    updated_at: 11,
                    record,
                }),
            }],
        })),
    )?;
    match response {
        SessionLogResponse::SessionDeltaPersisted {
            next_sequence: 1,
            next_management_sequence: 1,
        } => Ok(()),
        SessionLogResponse::Error { error } => anyhow::bail!("{error}"),
        other => anyhow::bail!("unexpected session delta response: {other:?}"),
    }
}

fn get_persisted_session(session_id: &str) -> anyhow::Result<Option<Box<SessionSnapshot>>> {
    let response = session_log_contract::client::call_service(&SessionLogCommand::GetSession(
        GetSessionRequest {
            session_id: session_id.to_string(),
        },
    ))?;
    match response {
        SessionLogResponse::Session { session } => Ok(session),
        SessionLogResponse::Error { error } => anyhow::bail!("{error}"),
        other => anyhow::bail!("unexpected session_log get session response: {other:?}"),
    }
}

fn list_persisted_records(session_id: &str) -> anyhow::Result<Vec<SessionRecord>> {
    let response = session_log_contract::client::call_service(
        &SessionLogCommand::ListSessionRecords(ListSessionRecordsRequest {
            session_id: session_id.to_string(),
            page: 0,
            page_size: 50,
        }),
    )?;
    match response {
        SessionLogResponse::Records { records, .. } => Ok(records),
        SessionLogResponse::Error { error } => anyhow::bail!("{error}"),
        other => anyhow::bail!("unexpected session_log records response: {other:?}"),
    }
}
