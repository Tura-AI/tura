use super::helpers::*;

#[tokio::test]
async fn gateway_prompt_business_flow_concurrent_sessions_enqueue_independent_router_turns(
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
        vec![
            RouterReply::Completed,
            RouterReply::Completed,
            RouterReply::Completed,
        ],
    )?;

    let sessions = (0..3)
        .map(|index| {
            create_canonical_test_session(
                Some(workspace.to_string_lossy().to_string()),
                Some(format!("codex/gpt-5.5-{index}")),
                None,
                Some("coding".to_string()),
                false,
                index == 1,
                false,
                None,
                false,
                false,
            )
        })
        .collect::<Vec<_>>();

    let responses =
        futures::future::join_all(sessions.iter().enumerate().map(|(index, session)| {
            let session_id = session.id.clone();
            async move {
                prompt_async(
                    Path(session_id),
                    Json(json!({
                        "messageID": format!("concurrent-message-{index}"),
                        "parts": [{
                            "id": format!("concurrent-turn-{index}"),
                            "type": "text",
                            "text": format!("Concurrent prompt {index}")
                        }],
                        "variant": if index == 2 { "high" } else { "default" },
                        "model_acceleration_enabled": index == 0,
                    })),
                )
                .await
                .into_response()
            }
        }))
        .await;

    for response in responses {
        assert_eq!(response.status(), axum::http::StatusCode::NO_CONTENT);
    }

    let mut seen_turns = Vec::new();
    let mut seen_sessions = Vec::new();
    for _ in 0..sessions.len() {
        let request = router.next_request(Duration::from_secs(10))?;
        assert_eq!(request["kind"], "call");
        assert_eq!(request["method"], "execution.enqueue_turn");
        seen_turns.push(
            request["payload"]["runtime_id"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
        );
        seen_sessions.push(
            request["payload"]["session_id"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
        );
        let prompt = request["payload"]["payload"]["prompt"]
            .as_str()
            .unwrap_or_default();
        assert!(prompt.starts_with("Concurrent prompt "));
    }
    seen_turns.sort();
    seen_sessions.sort();
    let mut expected_turns = vec![
        "concurrent-turn-0".to_string(),
        "concurrent-turn-1".to_string(),
        "concurrent-turn-2".to_string(),
    ];
    expected_turns.sort();
    let mut expected_sessions = sessions
        .iter()
        .map(|session| session.id.clone())
        .collect::<Vec<_>>();
    expected_sessions.sort();
    assert_eq!(seen_turns, expected_turns);
    assert_eq!(seen_sessions, expected_sessions);

    for (index, session) in sessions.iter().enumerate() {
        wait_until(Duration::from_secs(10), || {
            session_store()
                .get_session(&session.id)
                .is_some_and(|session| session.status == SessionStatus::Idle)
        })?;
        let messages = session_store().get_messages(&session.id);
        assert!(messages.iter().any(|message| {
            message.id == format!("concurrent-message-{index}")
                && message.role == MessageRole::User
                && message
                    .parts
                    .iter()
                    .any(|part| part.text.as_deref() == Some(&format!("Concurrent prompt {index}")))
        }));
        assert!(!messages
            .iter()
            .any(|message| message.role == MessageRole::Assistant));
        assert_gateway_kept_canonical_session(&session.id)?;
    }

    drop(router);
    drop(service);
    Ok(())
}
