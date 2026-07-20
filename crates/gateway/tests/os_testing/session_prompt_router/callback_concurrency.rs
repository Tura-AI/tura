use super::helpers::*;

#[tokio::test]
async fn gateway_prompt_business_flow_uses_runtime_callback_message_instead_of_fallback(
) -> Result<()> {
    let _guard = ENV_LOCK.lock().await;
    let root = tempfile::tempdir().context("temp root")?;
    let home = root.path().join("home");
    let workspace = root.path().join("workspace");
    std::fs::create_dir_all(&home)?;
    std::fs::create_dir_all(&workspace)?;
    let _env = EnvGuard::new(&home, &workspace);
    let service = ServiceThread::start()?;

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
    let callback_session_id = session.id.clone();
    let router = FakeRouter::start(
        &home,
        vec![RouterReply::CallbackThenPayload(
            Arc::new(move |request| {
                let runtime_id = request["payload"]["turn_id"]
                    .as_str()
                    .unwrap_or("runtime-callback-turn")
                    .to_string();
                let async_runtime =
                    tokio::runtime::Runtime::new().expect("runtime callback test tokio runtime");
                async_runtime.block_on(async {
                    let Json(response) = send_agent_message(
                        Path(callback_session_id.clone()),
                        Json(SendAgentMessageRequest {
                            reply_message: "Runtime callback completed the real local task."
                                .to_string(),
                            new_learning: "callback learning".to_string(),
                            step_summary: Some("callback step summary".to_string()),
                            media: Vec::new(),
                            runtime_id: Some(runtime_id.clone()),
                            tool_call: Some(SendAgentToolCall {
                                tool_name: "command_run".to_string(),
                                call_id: "callback-command-run".to_string(),
                                state: json!({
                                    "status": "completed",
                                    "input": {
                                        "commands": [{
                                            "step": 1,
                                            "command_type": "shell_command",
                                            "command_line": "Write-Output callback"
                                        }]
                                    },
                                    "output": {
                                        "results": [{
                                            "step": 1,
                                            "success": true,
                                            "output": "Exit code: 0\ncallback\n"
                                        }]
                                    },
                                    "metadata": {
                                        "kind": "mano_tool_call",
                                        "runtime_id": runtime_id
                                    }
                                }),
                                metadata: Some(json!({
                                    "kind": "mano_tool_call",
                                    "runtime_id": runtime_id,
                                    "tool": "command_run"
                                })),
                            }),
                            runtime_status: None,
                            context_tokens: None,
                            usage: None,
                            command_updates: Vec::new(),
                            created_at: chrono::Utc::now().timestamp_millis(),
                            updated_at: chrono::Utc::now().timestamp_millis(),
                        }),
                    )
                    .await;
                    assert!(
                        response.ok,
                        "runtime callback should be accepted: {response:?}"
                    );
                });
            }),
            json!({
                "ok": true,
                "accepted": true,
                "worker_id": "callback-worker"
            }),
        )],
    )?;

    let response = prompt_async(
        Path(session.id.clone()),
        Json(json!({
            "messageID": "runtime-callback-user-message",
            "parts": [
                {
                    "id": "runtime-callback-turn",
                    "type": "text",
                    "text": "Let the runtime callback produce the assistant result"
                }
            ],
            "system": "callback business context"
        })),
    )
    .await
    .into_response();
    assert_eq!(response.status(), axum::http::StatusCode::NO_CONTENT);

    let request = router.next_request(Duration::from_secs(10))?;
    assert_eq!(request["kind"], "call");
    assert_eq!(request["method"], "execution.enqueue_turn");
    assert_eq!(request["payload"]["turn_id"], "runtime-callback-turn");
    assert_eq!(request["payload"]["session_id"], session.id);
    wait_until(Duration::from_secs(10), || {
        session_store()
            .get_session(&session.id)
            .is_some_and(|session| session.status == SessionStatus::Idle)
    })?;

    let messages = session_store().get_messages(&session.id);
    assert!(
        messages.iter().any(|message| {
            message.role == MessageRole::Assistant
                && message.parts.iter().any(|part| {
                    part.text.as_deref() == Some("Runtime callback completed the real local task.")
                        && part
                            .metadata
                            .as_ref()
                            .and_then(|metadata| metadata.get("step_summary"))
                            .and_then(Value::as_str)
                            == Some("callback step summary")
                })
        }),
        "runtime callback assistant message should be retained: {messages:#?}"
    );
    assert!(
        !messages.iter().any(|message| {
            message.role == MessageRole::Assistant
                && message.parts.iter().any(|part| {
                    part.part_type == "tool"
                        && part.tool.as_deref() == Some("command_run")
                        && part.call_id.as_deref() == Some("callback-command-run")
                })
        }),
        "gateway must not retain command_run callback tool messages: {messages:#?}"
    );
    assert!(
        !messages.iter().any(|message| {
            message.role == MessageRole::Assistant
                && message.parts.iter().any(|part| {
                    part.text
                        .as_deref()
                        .unwrap_or_default()
                        .starts_with("Done: ")
                })
        }),
        "gateway must not synthesize a success fallback when runtime callback produced a final message"
    );
    assert_gateway_kept_canonical_session(&session.id)?;

    drop(router);
    drop(service);
    Ok(())
}

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
            RouterReply::Payload(
                json!({"ok": true, "accepted": true, "worker_id": "concurrent-worker-1"}),
            ),
            RouterReply::Payload(
                json!({"ok": true, "accepted": true, "worker_id": "concurrent-worker-2"}),
            ),
            RouterReply::Payload(
                json!({"ok": true, "accepted": true, "worker_id": "concurrent-worker-3"}),
            ),
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
                        "parts": [
                            {
                                "id": format!("concurrent-turn-{index}"),
                                "type": "text",
                                "text": format!("Concurrent prompt {index}")
                            }
                        ],
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
            request["payload"]["turn_id"]
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
        assert!(
            prompt.starts_with("Concurrent prompt "),
            "prompt payload should belong to one concurrent request: {request}"
        );
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
        assert!(
            messages.iter().any(|message| {
                message.id == format!("concurrent-message-{index}")
                    && message.role == MessageRole::User
                    && message.parts.iter().any(|part| {
                        part.text.as_deref() == Some(&format!("Concurrent prompt {index}"))
                    })
            }),
            "concurrent user message should stay attached to its session"
        );
        assert!(
            !messages
                .iter()
                .any(|message| message.role == MessageRole::Assistant),
            "successful router handoff without runtime callback must not synthesize assistant fallback for session {}",
            session.id
        );
        assert_gateway_kept_canonical_session(&session.id)?;
    }

    drop(router);
    drop(service);
    Ok(())
}
