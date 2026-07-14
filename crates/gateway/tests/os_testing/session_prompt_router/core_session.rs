use super::helpers::*;

#[tokio::test]
async fn gateway_prompt_business_flow_rejects_missing_session_without_side_effects() -> Result<()> {
    let _guard = ENV_LOCK.lock().await;
    let root = tempfile::tempdir().context("temp root")?;
    let home = root.path().join("home");
    let workspace = root.path().join("workspace");
    std::fs::create_dir_all(&home)?;
    std::fs::create_dir_all(&workspace)?;
    let _env = EnvGuard::new(&home, &workspace);
    let missing_session_id = format!("missing-session-{}", uuid::Uuid::new_v4());

    let response = prompt_async(
        Path(missing_session_id.clone()),
        Json(json!({
            "messageID": "missing-session-message",
            "parts": [
                {
                    "id": "missing-session-turn",
                    "type": "text",
                    "text": "This prompt must not create orphan session state"
                }
            ]
        })),
    )
    .await
    .into_response();

    assert_eq!(response.status(), axum::http::StatusCode::NOT_FOUND);
    assert!(
        session_store().get_session(&missing_session_id).is_none(),
        "missing prompt should not create a session"
    );
    assert!(
        session_store().get_messages(&missing_session_id).is_empty(),
        "missing prompt should not leave orphan messages"
    );
    Ok(())
}

#[tokio::test]
async fn gateway_prompt_business_flow_enqueues_router_turn_without_session_db_prewrite(
) -> Result<()> {
    let _guard = ENV_LOCK.lock().await;
    let root = tempfile::tempdir().context("temp root")?;
    let home = root.path().join("home");
    let workspace = root.path().join("workspace");
    std::fs::create_dir_all(&home)?;
    std::fs::create_dir_all(&workspace)?;
    let _env = EnvGuard::new(&home, &workspace);
    let service = ServiceThread::start()?;
    let router = FakeRouter::start(
        &home,
        vec![RouterReply::Payload(json!({
            "ok": true,
            "accepted": true,
            "worker_id": "mock-runtime-worker"
        }))],
    )?;

    let session = session_store().create_session(
        Some(workspace.to_string_lossy().to_string()),
        None,
        Some("codex/gpt-5.5".to_string()),
        Some("coding".to_string()),
        false,
        false,
        false,
        None,
        false,
        true,
    );
    let response = prompt_async(
        Path(session.id.clone()),
        Json(json!({
            "messageID": "router-user-message",
            "parts": [
                {
                    "id": "router-turn-1",
                    "type": "text",
                    "text": "Use the local fixture to prove router enqueue"
                }
            ],
            "model": {
                "providerID": "openai-api",
                "modelID": "gpt-5.5"
            },
            "variant": "high",
            "model_acceleration_enabled": true,
            "command_run_shell": "shell_command",
            "system": "business runtime context"
        })),
    )
    .await
    .into_response();
    assert_eq!(response.status(), axum::http::StatusCode::NO_CONTENT);

    let request = router.next_request(Duration::from_secs(10))?;
    assert_eq!(request["kind"], "call");
    assert_eq!(request["method"], "execution.enqueue_turn");
    assert_eq!(request["payload"]["turn_id"], "router-turn-1");
    assert_eq!(request["payload"]["session_id"], session.id);
    assert_eq!(
        request["payload"]["payload"]["prompt"],
        "Use the local fixture to prove router enqueue"
    );
    assert_eq!(request["payload"]["payload"]["model"], "openai/gpt-5.5");
    assert_eq!(
        request["payload"]["payload"]["runtime_context"],
        "business runtime context"
    );
    assert_eq!(
        request["payload"]["payload"]["worker_env"]["TURA_SESSION_REASONING_EFFORT"],
        "high"
    );
    assert_eq!(
        request["payload"]["payload"]["worker_env"]["TURA_SESSION_ACCELERATION_ENABLED"],
        "1"
    );
    assert_eq!(
        request["payload"]["payload"]["worker_env"]["TURA_COMMAND_RUN_SHELL"],
        "shell_command"
    );
    assert_eq!(
        request["payload"]["payload"]["worker_env"]["TURA_FRONTEND_MESSAGE_ID"],
        "router-user-message"
    );
    assert_eq!(
        request["payload"]["payload"]["worker_env"]["TURA_FRONTEND_PART_ID"],
        "router-turn-1"
    );

    wait_until(Duration::from_secs(10), || {
        session_store()
            .get_session(&session.id)
            .is_some_and(|session| session.status == SessionStatus::Idle)
    })?;
    let messages = session_store().get_messages(&session.id);
    assert!(
        messages
            .iter()
            .any(|message| message.id == "router-user-message"),
        "prompt should keep the frontend user message in gateway live state before enqueue"
    );
    assert!(
        !messages
            .iter()
            .any(|message| message.role == MessageRole::Assistant),
        "successful router enqueue without a runtime callback must not synthesize an assistant fallback from the user prompt"
    );
    assert_gateway_did_not_prewrite_session_db(&session.id)?;

    drop(router);
    drop(service);
    Ok(())
}

#[tokio::test]
async fn gateway_prompt_business_flow_injects_generated_frontend_ids_when_client_omits_them(
) -> Result<()> {
    let _guard = ENV_LOCK.lock().await;
    let root = tempfile::tempdir().context("temp root")?;
    let home = root.path().join("home");
    let workspace = root.path().join("workspace");
    std::fs::create_dir_all(&home)?;
    std::fs::create_dir_all(&workspace)?;
    let _env = EnvGuard::new(&home, &workspace);
    let service = ServiceThread::start()?;
    let router = FakeRouter::start(
        &home,
        vec![RouterReply::Payload(json!({
            "ok": true,
            "accepted": true,
            "worker_id": "generated-id-worker"
        }))],
    )?;

    let session = session_store().create_session(
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
            "parts": [
                {
                    "type": "text",
                    "text": "Client omitted frontend ids but reopened history must keep this user text"
                }
            ]
        })),
    )
    .await
    .into_response();
    assert_eq!(response.status(), axum::http::StatusCode::NO_CONTENT);

    let request = router.next_request(Duration::from_secs(10))?;
    let messages = session_store().get_messages(&session.id);
    let user_message = messages
        .iter()
        .find(|message| message.role == MessageRole::User)
        .ok_or_else(|| anyhow!("prompt should persist the user message before router enqueue"))?;
    let user_part = user_message
        .parts
        .first()
        .ok_or_else(|| anyhow!("prompt should persist a user text part"))?;
    assert!(!user_message.id.trim().is_empty());
    assert!(!user_part.id.trim().is_empty());
    assert_eq!(request["kind"], "call");
    assert_eq!(request["method"], "execution.enqueue_turn");
    assert_eq!(request["payload"]["turn_id"], user_part.id);
    assert_eq!(
        request["payload"]["payload"]["worker_env"]["TURA_FRONTEND_MESSAGE_ID"],
        user_message.id
    );
    assert_eq!(
        request["payload"]["payload"]["worker_env"]["TURA_FRONTEND_PART_ID"],
        user_part.id
    );

    wait_until(Duration::from_secs(10), || {
        session_store()
            .get_session(&session.id)
            .is_some_and(|session| session.status == SessionStatus::Idle)
    })?;
    assert_gateway_did_not_prewrite_session_db(&session.id)?;

    drop(router);
    drop(service);
    Ok(())
}

#[tokio::test]
async fn gateway_prompt_business_flow_routes_only_text_parts_and_keeps_first_text_part_id(
) -> Result<()> {
    let _guard = ENV_LOCK.lock().await;
    let root = tempfile::tempdir().context("temp root")?;
    let home = root.path().join("home");
    let workspace = root.path().join("workspace");
    std::fs::create_dir_all(&home)?;
    std::fs::create_dir_all(&workspace)?;
    let _env = EnvGuard::new(&home, &workspace);
    let service = ServiceThread::start()?;
    let router = FakeRouter::start(
        &home,
        vec![RouterReply::Payload(json!({
            "ok": true,
            "accepted": true,
            "worker_id": "multipart-worker"
        }))],
    )?;

    let session = session_store().create_session(
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
            "message_id": "multipart-message",
            "parts": [
                {
                    "id": "ignored-image-part",
                    "type": "image",
                    "url": "file:///local/screenshot.png",
                    "text": "image text must not enter prompt"
                },
                {
                    "id": "first-text-turn",
                    "type": "text",
                    "text": "Inspect the local screenshot context. "
                },
                {
                    "id": "ignored-file-part",
                    "type": "file",
                    "path": "notes.md",
                    "text": "file text must not enter prompt"
                },
                {
                    "id": "second-text-part",
                    "type": "text",
                    "text": "Then continue with the saved workspace state."
                }
            ],
            "system": "multipart business context"
        })),
    )
    .await
    .into_response();
    assert_eq!(response.status(), axum::http::StatusCode::NO_CONTENT);

    let request = router.next_request(Duration::from_secs(10))?;
    assert_eq!(request["kind"], "call");
    assert_eq!(request["method"], "execution.enqueue_turn");
    assert_eq!(request["payload"]["turn_id"], "first-text-turn");
    assert_eq!(
        request["payload"]["payload"]["prompt"],
        "Inspect the local screenshot context. Then continue with the saved workspace state."
    );
    assert_eq!(
        request["payload"]["payload"]["runtime_context"],
        "multipart business context"
    );
    assert!(
        !request["payload"]["payload"]["prompt"]
            .as_str()
            .unwrap_or_default()
            .contains("must not enter prompt"),
        "non-text parts must not leak into the router prompt"
    );

    wait_until(Duration::from_secs(10), || {
        session_store()
            .get_session(&session.id)
            .is_some_and(|session| session.status == SessionStatus::Idle)
    })?;
    let messages = session_store().get_messages(&session.id);
    let user_message = messages
        .iter()
        .find(|message| message.id == "multipart-message")
        .ok_or_else(|| anyhow!("multipart prompt should persist the user message"))?;
    assert_eq!(user_message.parts[0].id, "first-text-turn");
    assert_eq!(
        user_message.parts[0].text.as_deref(),
        Some("Inspect the local screenshot context. ")
    );
    assert_eq!(user_message.parts[1].id, "second-text-part");
    assert_eq!(
        user_message.parts[1].text.as_deref(),
        Some("Then continue with the saved workspace state.")
    );
    assert_gateway_did_not_prewrite_session_db(&session.id)?;

    drop(router);
    drop(service);
    Ok(())
}

#[tokio::test]
async fn gateway_prompt_business_flow_repeated_turns_keep_session_stable_without_db_prewrite(
) -> Result<()> {
    let _guard = ENV_LOCK.lock().await;
    let root = tempfile::tempdir().context("temp root")?;
    let home = root.path().join("home");
    let workspace = root.path().join("workspace");
    std::fs::create_dir_all(&home)?;
    std::fs::create_dir_all(&workspace)?;
    let _env = EnvGuard::new(&home, &workspace);
    let service = ServiceThread::start()?;
    let turns = 6;
    let router = FakeRouter::start(
        &home,
        (0..turns)
            .map(|index| {
                RouterReply::Payload(json!({
                    "ok": true,
                    "accepted": true,
                    "worker_id": format!("repeated-worker-{index}")
                }))
            })
            .collect(),
    )?;

    let session = session_store().create_session(
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

    for index in 0..turns {
        let response = prompt_async(
            Path(session.id.clone()),
            Json(json!({
                "messageID": format!("repeated-message-{index}"),
                "parts": [
                    {
                        "id": format!("repeated-turn-{index}"),
                        "type": "text",
                        "text": format!("Repeated prompt turn {index}")
                    }
                ],
                "system": format!("repeated runtime context {index}")
            })),
        )
        .await
        .into_response();
        assert_eq!(response.status(), axum::http::StatusCode::NO_CONTENT);

        let request = router.next_request(Duration::from_secs(10))?;
        assert_eq!(request["kind"], "call");
        assert_eq!(request["method"], "execution.enqueue_turn");
        assert_eq!(
            request["payload"]["turn_id"],
            format!("repeated-turn-{index}")
        );
        assert_eq!(request["payload"]["session_id"], session.id);
        assert_eq!(
            request["payload"]["payload"]["prompt"],
            format!("Repeated prompt turn {index}")
        );
        assert_eq!(
            request["payload"]["payload"]["runtime_context"],
            format!("repeated runtime context {index}")
        );

        wait_until(Duration::from_secs(10), || {
            session_store()
                .get_session(&session.id)
                .is_some_and(|session| session.status == SessionStatus::Idle)
        })?;
        assert_gateway_did_not_prewrite_session_db(&session.id)?;
    }

    let messages = session_store().get_messages(&session.id);
    let user_messages = messages
        .iter()
        .filter(|message| message.role == MessageRole::User)
        .count();
    let assistant_messages = messages
        .iter()
        .filter(|message| message.role == MessageRole::Assistant)
        .count();
    assert_eq!(user_messages, turns as usize);
    assert_eq!(assistant_messages, 0);
    for index in 0..turns {
        assert!(
            messages.iter().any(|message| {
                message.id == format!("repeated-message-{index}")
                    && message.role == MessageRole::User
                    && message.parts.iter().any(|part| {
                        part.id == format!("repeated-turn-{index}")
                            && part.text.as_deref()
                                == Some(format!("Repeated prompt turn {index}").as_str())
                            && part
                                .metadata
                                .as_ref()
                                .is_none_or(|metadata| metadata.get("kind").is_none())
                    })
            }),
            "repeated user turn {index} should stay as a normal prompt message"
        );
    }
    let final_session = session_store()
        .get_session(&session.id)
        .expect("repeated session should remain listed");
    assert_eq!(final_session.status, SessionStatus::Idle);
    assert_eq!(final_session.message_count, turns as usize);

    drop(router);
    drop(service);
    Ok(())
}
