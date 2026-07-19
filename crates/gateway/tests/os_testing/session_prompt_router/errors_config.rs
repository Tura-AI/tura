use super::helpers::*;

#[tokio::test]
async fn gateway_prompt_business_flow_records_router_rejection_as_session_error() -> Result<()> {
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
            "ok": false,
            "error": "mock router rejected the turn"
        }))],
    )?;

    let session = session_store().create_session(
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
            "message_id": "router-reject-message",
            "parts": [
                {
                    "id": "router-reject-turn",
                    "type": "text",
                    "text": "Trigger the local router rejection path"
                }
            ]
        })),
    )
    .await
    .into_response();
    assert_eq!(response.status(), axum::http::StatusCode::NO_CONTENT);
    let request = router.next_request(Duration::from_secs(10))?;
    assert_eq!(request["payload"]["turn_id"], "router-reject-turn");

    wait_until(Duration::from_secs(10), || {
        session_store()
            .get_session(&session.id)
            .is_some_and(|session| session.status == SessionStatus::Error)
    })?;
    let messages = session_store().get_messages(&session.id);
    assert!(
        messages.iter().any(|message| {
            message.role == MessageRole::Assistant
                && message.parts.iter().any(|part| {
                    part.text
                        .as_deref()
                        .unwrap_or_default()
                        .contains("mock router rejected the turn")
                })
        }),
        "router rejection should be visible as a session error fallback"
    );
    assert_gateway_did_not_prewrite_session_db(&session.id)?;

    drop(router);
    drop(service);
    Ok(())
}

#[tokio::test]
async fn gateway_prompt_business_flow_records_router_transport_error_without_db_prewrite(
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
        vec![RouterReply::RawLine(
            "this is not a router json response".to_string(),
        )],
    )?;

    let session = session_store().create_session(
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
            "message_id": "router-transport-error-message",
            "parts": [
                {
                    "id": "router-transport-error-turn",
                    "type": "text",
                    "text": "Trigger malformed router response handling"
                }
            ]
        })),
    )
    .await
    .into_response();
    assert_eq!(response.status(), axum::http::StatusCode::NO_CONTENT);

    let request = router.next_request(Duration::from_secs(10))?;
    assert_eq!(request["kind"], "call");
    assert_eq!(request["method"], "execution.enqueue_turn");
    assert_eq!(request["payload"]["turn_id"], "router-transport-error-turn");

    wait_until(Duration::from_secs(10), || {
        session_store()
            .get_session(&session.id)
            .is_some_and(|session| session.status == SessionStatus::Error)
    })?;
    let messages = session_store().get_messages(&session.id);
    assert!(
        messages.iter().any(|message| {
            message.id == "router-transport-error-message" && message.role == MessageRole::User
        }),
        "user message should stay in gateway live state when router transport fails"
    );
    assert!(
        messages.iter().any(|message| {
            message.role == MessageRole::Assistant
                && message.parts.iter().any(|part| {
                    let text = part.text.as_deref().unwrap_or_default();
                    text.contains("MANO failed while processing this prompt")
                        && text.contains("failed to enqueue turn router-transport-error-turn")
                })
        }),
        "malformed router response should produce a visible session error"
    );
    assert_gateway_did_not_prewrite_session_db(&session.id)?;

    drop(router);
    drop(service);
    Ok(())
}

#[tokio::test]
async fn gateway_prompt_business_flow_reports_runtime_stop_without_mano_failure() -> Result<()> {
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
        vec![RouterReply::RawLine(
            json!({
                "request_id": "runtime-stop-error",
                "ok": false,
                "payload": null,
                "error": "router execution enqueue failed: runtime worker invocation failed: one-shot worker cancelled"
            })
            .to_string(),
        )],
    )?;

    let session = session_store().create_session(
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
            "message_id": "runtime-stop-message",
            "parts": [
                {
                    "id": "runtime-stop-turn",
                    "type": "text",
                    "text": "Stop while this prompt is running"
                }
            ]
        })),
    )
    .await
    .into_response();
    assert_eq!(response.status(), axum::http::StatusCode::NO_CONTENT);

    let request = router.next_request(Duration::from_secs(10))?;
    assert_eq!(request["method"], "execution.enqueue_turn");
    assert_eq!(request["payload"]["turn_id"], "runtime-stop-turn");

    wait_until(Duration::from_secs(10), || {
        session_store()
            .get_session(&session.id)
            .is_some_and(|session| session.status == SessionStatus::Idle)
    })?;
    let messages = session_store().get_messages(&session.id);
    assert!(messages.iter().any(|message| {
        message.role == MessageRole::Assistant
            && message.parts.iter().any(|part| {
                part.text.as_deref() == Some("Runtime stopped.")
                    && part
                        .metadata
                        .as_ref()
                        .and_then(|metadata| metadata.get("code"))
                        .and_then(serde_json::Value::as_str)
                        == Some("runtime_stopped")
            })
    }));
    assert!(
        !messages.iter().any(|message| {
            message.role == MessageRole::Assistant
                && message.parts.iter().any(|part| {
                    let text = part.text.as_deref().unwrap_or_default();
                    text.contains("MANO failed") || text.contains("one-shot worker cancelled")
                })
        }),
        "runtime stop must not expose internal worker cancellation text"
    );

    drop(router);
    drop(service);
    Ok(())
}

#[tokio::test]
async fn gateway_prompt_business_flow_appends_prompt_when_router_reports_active_turn() -> Result<()>
{
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
            RouterReply::Payload(json!({
                "ok": false,
                "code": "session_active_turn",
                "session_id": "filled below by assertion only",
                "turn_id": "router-active-turn",
                "error": "session already has an active turn"
            })),
            RouterReply::Payload(json!({
                "ok": true,
                "commands": ["append this to the running runtime"]
            })),
        ],
    )?;

    let session = session_store().create_session(
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
            "message_id": "router-active-message",
            "parts": [
                {
                    "id": "router-active-turn",
                    "type": "text",
                    "text": "append this to the running runtime"
                }
            ]
        })),
    )
    .await
    .into_response();
    assert_eq!(response.status(), axum::http::StatusCode::NO_CONTENT);

    let enqueue = router.next_request(Duration::from_secs(10))?;
    assert_eq!(enqueue["kind"], "call");
    assert_eq!(enqueue["method"], "execution.enqueue_turn");
    assert_eq!(enqueue["payload"]["turn_id"], "router-active-turn");
    assert_eq!(enqueue["payload"]["session_id"], session.id);

    let append = router.next_request(Duration::from_secs(10))?;
    assert_eq!(append["kind"], "call");
    assert_eq!(append["method"], "session.append_user_command");
    assert_eq!(append["payload"]["session_id"], session.id);
    assert_eq!(append["payload"]["root_session_id"], session.id);
    assert_eq!(
        append["payload"]["command"],
        "append this to the running runtime"
    );

    wait_until(Duration::from_secs(10), || {
        session_store()
            .get_session(&session.id)
            .is_some_and(|session| session.status == SessionStatus::Busy)
    })?;
    let messages = session_store().get_messages(&session.id);
    assert!(
        messages.iter().any(|message| {
            message.id == "router-active-message" && message.role == MessageRole::User
        }),
        "active-turn fallback should preserve the submitted user prompt"
    );
    assert!(
        !messages.iter().any(|message| {
            message.role == MessageRole::Assistant
                && message.parts.iter().any(|part| {
                    part.text
                        .as_deref()
                        .unwrap_or_default()
                        .contains("MANO failed while processing this prompt")
                })
        }),
        "active-turn fallback must append to runtime instead of surfacing a MANO failure"
    );
    assert_eq!(
        session_store().take_user_commands_for_session(&session.id),
        vec!["append this to the running runtime".to_string()]
    );

    drop(router);
    drop(service);
    Ok(())
}

#[tokio::test]
async fn gateway_prompt_business_flow_inherits_agent_runtime_settings_for_router_payload(
) -> Result<()> {
    let _guard = ENV_LOCK.lock().await;
    let root = tempfile::tempdir().context("temp root")?;
    let home = root.path().join("home");
    let workspace = root.path().join("workspace");
    std::fs::create_dir_all(&home)?;
    std::fs::create_dir_all(&workspace)?;
    let _env = EnvGuard::new(&home, &workspace);
    let mut agent_config =
        tura_agents::store::default_agent_config(&workspace, "business-runtime-agent")
            .map_err(anyhow::Error::msg)?;
    agent_config.provider["current_model"] = json!("openai/gpt-5.5-pro");
    agent_config.provider["model_reasoning_effort"] = json!("high");
    agent_config.provider["model_acceleration_enabled"] = json!(true);
    tura_agents::store::save_dynamic_agent(
        &workspace,
        &agent_config,
        Some("Use the business runtime agent settings."),
    )
    .map_err(anyhow::Error::msg)?;
    save_config(
        &workspace,
        &TuraSessionConfig {
            active_agent: Some("workspace-config-agent".to_string()),
            ..TuraSessionConfig::default()
        },
    )
    .map_err(anyhow::Error::msg)?;

    let service = ServiceThread::start()?;
    let router = FakeRouter::start(
        &home,
        vec![RouterReply::Payload(json!({
            "ok": true,
            "accepted": true,
            "worker_id": "agent-runtime-worker"
        }))],
    )?;

    let session = session_store().create_session(
        Some(workspace.to_string_lossy().to_string()),
        Some("codex/gpt-5.5".to_string()),
        Some("business-runtime-agent".to_string()),
        Some("coding".to_string()),
        false,
        false,
        false,
        None,
        false,
        false,
    );
    let _ = std::fs::remove_file(gateway::session::config::config_path(&workspace));
    let response = prompt_async(
        Path(session.id.clone()),
        Json(json!({
            "messageID": "agent-runtime-message",
            "parts": [
                {
                    "id": "agent-runtime-turn",
                    "type": "text",
                    "text": "Inherit agent runtime settings"
                }
            ]
        })),
    )
    .await
    .into_response();
    assert_eq!(response.status(), axum::http::StatusCode::NO_CONTENT);

    let request = router.next_request(Duration::from_secs(10))?;
    assert_eq!(request["method"], "execution.enqueue_turn");
    assert_eq!(request["payload"]["turn_id"], "agent-runtime-turn");
    assert_eq!(
        request["payload"]["payload"]["agent"],
        "business-runtime-agent"
    );
    assert_eq!(request["payload"]["payload"]["model"], "codex/gpt-5.5");
    assert_eq!(
        request["payload"]["payload"]["worker_env"]["TURA_SESSION_REASONING_EFFORT"],
        "high"
    );
    assert_eq!(
        request["payload"]["payload"]["worker_env"]["TURA_SESSION_ACCELERATION_ENABLED"],
        "1"
    );
    assert_eq!(
        request["payload"]["payload"]["worker_env"]["TURA_COMMAND_RUN_STALL_CHECK_SECS"],
        "5"
    );
    let worker_env = request["payload"]["payload"]["worker_env"]
        .as_object()
        .expect("worker_env should be an object");
    assert!(
        !worker_env.keys().any(|key| key.contains("INVOKE_TIMEOUT")),
        "runtime worker payload must not include a session-wide invoke deadline"
    );
    wait_until(Duration::from_secs(10), || {
        session_store()
            .get_session(&session.id)
            .is_some_and(|session| session.status == SessionStatus::Idle)
    })?;

    drop(router);
    drop(service);
    Ok(())
}

#[tokio::test]
async fn gateway_prompt_business_flow_applies_workspace_runtime_config_to_router_payload(
) -> Result<()> {
    let _guard = ENV_LOCK.lock().await;
    let root = tempfile::tempdir().context("temp root")?;
    let home = root.path().join("home");
    let workspace = root.path().join("workspace");
    std::fs::create_dir_all(&home)?;
    std::fs::create_dir_all(&workspace)?;
    let _env = EnvGuard::new(&home, &workspace);
    let config = TuraSessionConfig {
        language: Some("zh-CN".to_string()),
        model: Some("openai/gpt-5.5".to_string()),
        active_provider: Some("openai".to_string()),
        active_model: Some("gpt-5.5".to_string()),
        active_agent: Some("workspace-config-agent".to_string()),
        active_persona: Some("workspace-persona".to_string()),
        model_variant: Some("high".to_string()),
        model_acceleration_enabled: Some(false),
        command_run_stall_guard_check_secs: Some(17),
        command_run_stall_guard_identical_checks: Some(3),
        ..TuraSessionConfig::default()
    };
    save_config(&workspace, &config).map_err(anyhow::Error::msg)?;

    let service = ServiceThread::start()?;
    let router = FakeRouter::start(
        &home,
        vec![RouterReply::Payload(json!({
            "ok": true,
            "accepted": true,
            "worker_id": "workspace-config-worker"
        }))],
    )?;

    let session = session_store().create_session(
        Some(workspace.to_string_lossy().to_string()),
        None,
        None,
        None,
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
            "messageID": "workspace-config-message",
            "parts": [
                {
                    "id": "workspace-config-turn",
                    "type": "text",
                    "text": "Use workspace runtime config"
                }
            ]
        })),
    )
    .await
    .into_response();
    assert_eq!(response.status(), axum::http::StatusCode::NO_CONTENT);

    let request = router.next_request(Duration::from_secs(10))?;
    assert_eq!(request["payload"]["turn_id"], "workspace-config-turn");
    assert_eq!(request["payload"]["payload"]["model"], "openai/gpt-5.5");
    assert_eq!(
        request["payload"]["payload"]["agent"],
        "workspace-config-agent"
    );
    assert_eq!(
        request["payload"]["payload"]["worker_env"]["TURA_SESSION_LANGUAGE"],
        "zh-CN"
    );
    assert_eq!(
        request["payload"]["payload"]["worker_env"]["TURA_SESSION_PERSONA"],
        "workspace-persona"
    );
    assert_eq!(
        request["payload"]["payload"]["worker_env"]["TURA_SESSION_REASONING_EFFORT"],
        "high"
    );
    assert!(
        request["payload"]["payload"]["worker_env"]
            .get("TURA_SESSION_ACCELERATION_ENABLED")
            .is_none(),
        "disabled acceleration should not be exported to the worker"
    );
    assert_eq!(
        request["payload"]["payload"]["worker_env"]["TURA_COMMAND_RUN_STALL_CHECK_SECS"],
        "17"
    );
    assert_eq!(
        request["payload"]["payload"]["worker_env"]["TURA_COMMAND_RUN_STALL_IDENTICAL_CHECKS"],
        "3"
    );

    wait_until(Duration::from_secs(10), || {
        session_store()
            .get_session(&session.id)
            .is_some_and(|session| session.status == SessionStatus::Idle)
    })?;

    drop(router);
    drop(service);
    Ok(())
}
