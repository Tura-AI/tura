use super::helpers::*;

#[tokio::test]
async fn gateway_prompt_business_flow_recovers_cached_stale_router_endpoint_between_turns(
) -> Result<()> {
    let _guard = ENV_LOCK.lock().await;
    let root = tempfile::tempdir().context("temp root")?;
    let home = root.path().join("home");
    let workspace = root.path().join("workspace");
    std::fs::create_dir_all(&home)?;
    std::fs::create_dir_all(&workspace)?;
    let _env = EnvGuard::new(&home, &workspace);
    let service = ServiceThread::start()?;
    let first_router = FakeRouter::start(&home, vec![RouterReply::Completed])?;

    let session = create_canonical_test_session(
        Some(workspace.to_string_lossy().to_string()),
        Some("openai/gpt-5.5".to_string()),
        None,
        Some("coding".to_string()),
        false,
        false,
        false,
        None,
        false,
        false,
    );

    let response = prompt_async(
        Path(session.id.clone()),
        Json(json!({
            "messageID": "stale-router-message-1",
            "parts": [
                {
                    "id": "stale-router-turn-1",
                    "type": "text",
                    "text": "First prompt reaches the original router endpoint"
                }
            ]
        })),
    )
    .await
    .into_response();
    assert_eq!(response.status(), axum::http::StatusCode::NO_CONTENT);
    let first_request = first_router.next_request(Duration::from_secs(10))?;
    assert_eq!(
        first_request["payload"]["runtime_id"],
        "stale-router-turn-1"
    );
    wait_until(Duration::from_secs(10), || {
        session_store()
            .get_session(&session.id)
            .is_some_and(|session| session.status == SessionStatus::Idle)
    })?;

    drop(first_router);
    let second_router = FakeRouter::start(&home, vec![RouterReply::Completed])?;

    let response = prompt_async(
        Path(session.id.clone()),
        Json(json!({
            "messageID": "stale-router-message-2",
            "parts": [
                {
                    "id": "stale-router-turn-2",
                    "type": "text",
                    "text": "Second prompt must recover the new router endpoint"
                }
            ]
        })),
    )
    .await
    .into_response();
    assert_eq!(response.status(), axum::http::StatusCode::NO_CONTENT);
    let second_request = second_router.next_request(Duration::from_secs(10))?;
    assert_eq!(
        second_request["payload"]["runtime_id"],
        "stale-router-turn-2"
    );
    assert_eq!(second_request["payload"]["session_id"], session.id);
    assert_eq!(
        second_request["payload"]["payload"]["prompt"],
        "Second prompt must recover the new router endpoint"
    );
    wait_until(Duration::from_secs(10), || {
        session_store()
            .get_session(&session.id)
            .is_some_and(|session| session.status == SessionStatus::Idle)
    })?;

    let messages = session_store().get_messages(&session.id);
    assert_eq!(
        messages
            .iter()
            .filter(|message| message.role == MessageRole::User)
            .count(),
        2
    );
    assert_eq!(
        messages
            .iter()
            .filter(|message| message.role == MessageRole::Assistant)
            .count(),
        0
    );
    assert_gateway_kept_canonical_session(&session.id)?;

    drop(second_router);
    drop(service);
    Ok(())
}

#[tokio::test]
async fn gateway_prompt_business_flow_cancel_after_router_enqueue_preserves_user_message_without_success_fallback(
) -> Result<()> {
    let _guard = ENV_LOCK.lock().await;
    let root = tempfile::tempdir().context("temp root")?;
    let home = root.path().join("home");
    let workspace = root.path().join("workspace");
    std::fs::create_dir_all(&home)?;
    std::fs::create_dir_all(&workspace)?;
    let _env = EnvGuard::new(&home, &workspace);
    let service = ServiceThread::start()?;
    let (release_reply, wait_for_release) = mpsc::channel();
    let router = FakeRouter::start(
        &home,
        vec![RouterReply::GatedPayload(
            json!({
                "ok": true,
                "accepted": true,
                "worker_id": "cancel-race-worker"
            }),
            Arc::new(StdMutex::new(wait_for_release)),
        )],
    )?;

    let session = create_canonical_test_session(
        Some(workspace.to_string_lossy().to_string()),
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
    let response = prompt_async(
        Path(session.id.clone()),
        Json(json!({
            "messageID": "cancel-race-message",
            "parts": [
                {
                    "id": "cancel-race-turn",
                    "type": "text",
                    "text": "Start work that will be cancelled after enqueue"
                }
            ]
        })),
    )
    .await
    .into_response();
    assert_eq!(response.status(), axum::http::StatusCode::NO_CONTENT);
    assert!(
        session_store()
            .get_todos(&session.id)
            .iter()
            .any(|todo| { todo.get("status").and_then(Value::as_str) == Some("in_progress") }),
        "prompt handoff should create an in-progress todo before router dispatch"
    );

    let request = router.next_request(Duration::from_secs(10))?;
    assert_eq!(request["payload"]["runtime_id"], "cancel-race-turn");
    execute_canonical_test_command(&session.id, SessionCommand::CancelSession);

    wait_until(Duration::from_secs(10), || {
        let todos = session_store().get_todos(&session.id);
        !todos.is_empty()
            && todos
                .iter()
                .all(|todo| todo.get("status").and_then(Value::as_str) == Some("cancelled"))
    })?;
    let cancelled_todo_cursor = session_store()
        .todo_cursor_for_business_test(&session.id)
        .context("cancelled todo cursor should exist")?;
    release_reply
        .send(())
        .context("release delayed fake router reply")?;
    wait_until(Duration::from_secs(10), || {
        session_store()
            .todo_cursor_for_business_test(&session.id)
            .is_some_and(|cursor| cursor > cancelled_todo_cursor)
    })?;
    assert!(
        session_store()
            .session_lifecycle_projection(&session.id)
            .is_some_and(|projection| projection.cancelled),
        "cancellation marker should survive until the next prompt clears it"
    );
    let messages = session_store().get_messages(&session.id);
    assert!(
        messages.iter().any(|message| {
            message.id == "cancel-race-message" && message.role == MessageRole::User
        }),
        "cancelled prompt should keep the user message accepted before router handoff"
    );
    assert!(
        !messages
            .iter()
            .any(|message| message.role == MessageRole::Assistant),
        "cancelled prompt must not add a success fallback after the delayed router ACK"
    );
    let todos = session_store().get_todos(&session.id);
    assert!(
        todos
            .iter()
            .all(|todo| todo.get("status").and_then(Value::as_str) == Some("cancelled")),
        "cancelled prompt should mark in-progress todos cancelled: {todos:?}"
    );
    assert_gateway_kept_canonical_session(&session.id)?;

    drop(router);
    drop(service);
    Ok(())
}
