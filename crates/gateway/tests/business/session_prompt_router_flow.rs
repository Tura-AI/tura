use anyhow::{anyhow, Context, Result};
use axum::extract::{Json, Path};
use axum::response::IntoResponse;
use gateway::api::session::{
    prompt_async, send_agent_message, SendAgentMessageRequest, SendAgentToolCall,
};
use gateway::api::types::SessionStatus;
use gateway::session::config::{save_config, TuraSessionConfig};
use gateway::session::MessageRole;
use gateway::session_store;
use serde_json::{json, Value};
use session_log::{SessionLogCommand, SessionLogStore};
use std::collections::VecDeque;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path as FsPath, PathBuf};
use std::sync::{mpsc, Arc, Mutex as StdMutex};
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

static ENV_LOCK: Mutex<()> = Mutex::const_new(());

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
async fn gateway_prompt_business_flow_persists_session_and_enqueues_router_turn() -> Result<()> {
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
        "prompt should persist the frontend user message before enqueue"
    );
    assert!(
        !messages
            .iter()
            .any(|message| message.role == MessageRole::Assistant),
        "successful router enqueue without a runtime callback must not synthesize an assistant fallback from the user prompt"
    );
    let persisted = gateway::session_db_client::SessionDbClient::discover()?
        .get_session(session.id.clone())?
        .ok_or_else(|| anyhow!("session should have been ACKed to session_db before enqueue"))?;
    assert_eq!(persisted.session_id, session.id);
    assert!(
        persisted
            .session
            .get("messages")
            .is_none_or(|messages| messages.is_null()),
        "session_db snapshot should keep records separate from the session object"
    );
    assert!(persisted.message_count >= 1);

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
        Some("Inspect the local screenshot context. Then continue with the saved workspace state.")
    );
    let persisted = gateway::session_db_client::SessionDbClient::discover()?
        .get_session(session.id.clone())?
        .ok_or_else(|| anyhow!("multipart session should be ACKed to session_db"))?;
    assert_eq!(persisted.session_id, session.id);
    assert!(persisted.message_count >= 1);

    drop(router);
    drop(service);
    Ok(())
}

#[tokio::test]
async fn gateway_prompt_business_flow_repeated_turns_keep_session_stable_and_persisted(
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
        let persisted = gateway::session_db_client::SessionDbClient::discover()?
            .get_session(session.id.clone())?
            .ok_or_else(|| anyhow!("repeated prompt session should be persisted"))?;
        assert!(
            persisted.message_count >= (index + 1) as u64,
            "each completed repeated turn should persist the user prompt without a synthesized assistant fallback"
        );
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
    let first_router = FakeRouter::start(
        &home,
        vec![RouterReply::Payload(json!({
            "ok": true,
            "accepted": true,
            "worker_id": "first-router-worker"
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
    assert_eq!(first_request["payload"]["turn_id"], "stale-router-turn-1");
    wait_until(Duration::from_secs(10), || {
        session_store()
            .get_session(&session.id)
            .is_some_and(|session| session.status == SessionStatus::Idle)
    })?;

    drop(first_router);
    let second_router = FakeRouter::start(
        &home,
        vec![RouterReply::Payload(json!({
            "ok": true,
            "accepted": true,
            "worker_id": "second-router-worker"
        }))],
    )?;

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
    assert_eq!(second_request["payload"]["turn_id"], "stale-router-turn-2");
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
    let persisted = gateway::session_db_client::SessionDbClient::discover()?
        .get_session(session.id.clone())?
        .ok_or_else(|| anyhow!("stale endpoint recovery session should be persisted"))?;
    assert!(
        persisted.message_count >= 2,
        "session_db should retain both user prompts without synthesized fallback assistant messages"
    );

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
    let router = FakeRouter::start(
        &home,
        vec![RouterReply::DelayedPayload(
            json!({
                "ok": true,
                "accepted": true,
                "worker_id": "cancel-race-worker"
            }),
            Duration::from_millis(250),
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

    let request = router.next_request(Duration::from_secs(10))?;
    assert_eq!(request["payload"]["turn_id"], "cancel-race-turn");
    session_store().mark_cancelled(&session.id);

    wait_until(Duration::from_secs(10), || {
        session_store()
            .get_session(&session.id)
            .is_some_and(|session| session.status == SessionStatus::Idle)
    })?;
    assert!(
        session_store().is_cancelled(&session.id),
        "cancellation marker should survive until the next prompt clears it"
    );
    let messages = session_store().get_messages(&session.id);
    assert!(
        messages.iter().any(|message| {
            message.id == "cancel-race-message" && message.role == MessageRole::User
        }),
        "cancelled prompt should keep the user message that was ACKed before router handoff"
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
    let persisted = gateway::session_db_client::SessionDbClient::discover()?
        .get_session(session.id.clone())?
        .ok_or_else(|| anyhow!("cancelled prompt session should be ACKed to session_db"))?;
    assert_eq!(persisted.session_id, session.id);
    assert!(persisted.message_count >= 1);

    drop(router);
    drop(service);
    Ok(())
}

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
                            message_id: Some("runtime-callback-message".to_string()),
                            part_id: Some("runtime-callback-part".to_string()),
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
            message.id == "runtime-callback-message"
                && message.role == MessageRole::Assistant
                && message.parts.iter().any(|part| {
                    part.id == "runtime-callback-part"
                        && part.text.as_deref()
                            == Some("Runtime callback completed the real local task.")
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
        messages.iter().any(|message| {
            message.role == MessageRole::Assistant
                && message.parts.iter().any(|part| {
                    part.part_type == "tool"
                        && part.tool.as_deref() == Some("command_run")
                        && part.call_id.as_deref() == Some("callback-command-run")
                        && part
                            .state
                            .as_ref()
                            .and_then(|state| state.get("status"))
                            .and_then(Value::as_str)
                            == Some("completed")
                })
        }),
        "runtime callback tool message should be retained: {messages:#?}"
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
    let persisted = gateway::session_db_client::SessionDbClient::discover()?
        .get_session(session.id.clone())?
        .ok_or_else(|| anyhow!("callback session should be persisted"))?;
    assert!(
        persisted.message_count >= 3,
        "session_db should contain user, assistant, and tool callback messages"
    );
    let (_page, records) = gateway::session_db_client::SessionDbClient::discover()?
        .list_session_records(session.id.clone(), 0, 50)?;
    assert!(
        records
            .iter()
            .any(|record| record.message_id == "runtime-callback-message"),
        "session_db records should include callback assistant message: {records:#?}"
    );
    assert!(
        records.iter().any(|record| {
            record
                .record
                .get("parts")
                .and_then(Value::as_array)
                .is_some_and(|parts| {
                    parts.iter().any(|part| {
                        part.get("type").and_then(Value::as_str) == Some("tool")
                            && part.get("tool").and_then(Value::as_str) == Some("command_run")
                    })
                })
        }),
        "session_db records should include callback tool message: {records:#?}"
    );

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
            session_store().create_session(
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

    let client = gateway::session_db_client::SessionDbClient::discover()?;
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
        let persisted = client
            .get_session(session.id.clone())?
            .ok_or_else(|| anyhow!("concurrent session should be ACKed to session_db"))?;
        assert_eq!(persisted.session_id, session.id);
        assert!(
            persisted.message_count >= 1,
            "session_db should keep each concurrent session visible"
        );
    }

    drop(router);
    drop(service);
    Ok(())
}

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
    let persisted = gateway::session_db_client::SessionDbClient::discover()?
        .get_session(session.id.clone())?
        .ok_or_else(|| anyhow!("session should still be ACKed before router rejection"))?;
    assert_eq!(persisted.session_id, session.id);
    assert!(persisted.message_count >= 1);

    drop(router);
    drop(service);
    Ok(())
}

#[tokio::test]
async fn gateway_prompt_business_flow_records_router_transport_error_after_session_ack(
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
        "user message should stay persisted when router transport fails"
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
    let persisted = gateway::session_db_client::SessionDbClient::discover()?
        .get_session(session.id.clone())?
        .ok_or_else(|| anyhow!("session should have been ACKed before router transport failure"))?;
    assert_eq!(persisted.session_id, session.id);
    assert!(
        persisted.message_count >= 1,
        "session_db ACK should preserve the pre-enqueue user message"
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
    agent_config.provider["model_reasoning_effort"] = json!("high");
    agent_config.provider["model_acceleration_enabled"] = json!(true);
    tura_agents::store::save_dynamic_agent(
        &workspace,
        &agent_config,
        Some("Use the business runtime agent settings."),
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
        active_agent: Some("workspace-config-agent".to_string()),
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

struct EnvGuard {
    previous: Vec<(&'static str, Option<std::ffi::OsString>)>,
}

impl EnvGuard {
    fn new(home: &FsPath, workspace: &FsPath) -> Self {
        let keys = [
            "TURA_HOME",
            "SESSION_LOG_DB_ROOT",
            "TURA_DB_ROOT",
            "TURA_PROJECT_ROOT",
            "TURA_CWD",
            "TURA_SESSION_DB_PROBE_TIMEOUT_MS",
        ];
        let previous = keys
            .iter()
            .map(|key| (*key, std::env::var_os(key)))
            .collect::<Vec<_>>();
        std::env::set_var("TURA_HOME", home);
        std::env::remove_var("SESSION_LOG_DB_ROOT");
        std::env::remove_var("TURA_DB_ROOT");
        std::env::set_var("TURA_PROJECT_ROOT", workspace);
        std::env::set_var("TURA_CWD", workspace);
        std::env::set_var("TURA_SESSION_DB_PROBE_TIMEOUT_MS", "20");
        Self { previous }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        for (key, value) in self.previous.drain(..) {
            match value {
                Some(value) => std::env::set_var(key, value),
                None => std::env::remove_var(key),
            }
        }
    }
}

struct ServiceThread {
    handle: Option<std::thread::JoinHandle<Result<()>>>,
}

impl ServiceThread {
    fn start() -> Result<Self> {
        let store = SessionLogStore::open_default().context("open session log store")?;
        let handle = std::thread::spawn(move || session_log::ipc::serve_blocking(store));
        wait_until(
            Duration::from_secs(10),
            session_log::ipc::service_is_running,
        )?;
        Ok(Self {
            handle: Some(handle),
        })
    }
}

impl Drop for ServiceThread {
    fn drop(&mut self) {
        let _ = session_log::ipc::call_service(&SessionLogCommand::Shutdown);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

#[derive(Clone)]
enum RouterReply {
    Payload(Value),
    DelayedPayload(Value, Duration),
    CallbackThenPayload(Arc<dyn Fn(Value) + Send + Sync>, Value),
    RawLine(String),
}

struct FakeRouter {
    received: mpsc::Receiver<Value>,
    stop: Arc<std::sync::atomic::AtomicBool>,
    handle: Option<std::thread::JoinHandle<()>>,
    connection_handles: Arc<StdMutex<Vec<std::thread::JoinHandle<()>>>>,
    addr_path: PathBuf,
    addr: std::net::SocketAddr,
}

impl FakeRouter {
    fn start(home: &FsPath, replies: Vec<RouterReply>) -> Result<Self> {
        let listener = TcpListener::bind(("127.0.0.1", 0)).context("bind fake router")?;
        listener.set_nonblocking(true)?;
        let addr = listener.local_addr()?;
        let addr_path = home.join("db").join("session_log").join("router.addr");
        if let Some(parent) = addr_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(
            &addr_path,
            serde_json::to_string(&json!({
                "addr": addr.to_string(),
                "version": tura_path::instance_version(),
            }))?,
        )?;
        let (tx, rx) = mpsc::channel();
        let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let thread_stop = Arc::clone(&stop);
        let replies = Arc::new(StdMutex::new(VecDeque::from(replies)));
        let connection_handles = Arc::new(StdMutex::new(Vec::new()));
        let thread_replies = Arc::clone(&replies);
        let thread_connection_handles = Arc::clone(&connection_handles);
        let handle = std::thread::spawn(move || {
            while !thread_stop.load(std::sync::atomic::Ordering::SeqCst) {
                match listener.accept() {
                    Ok((stream, _)) => {
                        let tx = tx.clone();
                        let replies = Arc::clone(&thread_replies);
                        let handle = std::thread::spawn(move || {
                            let _ = handle_router_connection(stream, &tx, &replies);
                        });
                        thread_connection_handles
                            .lock()
                            .expect("fake router connection handles lock")
                            .push(handle);
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(10));
                    }
                    Err(_) => break,
                }
            }
        });
        Ok(Self {
            received: rx,
            stop,
            handle: Some(handle),
            connection_handles,
            addr_path,
            addr,
        })
    }

    fn next_request(&self, timeout: Duration) -> Result<Value> {
        self.received
            .recv_timeout(timeout)
            .context("fake router did not receive request")
    }
}

impl Drop for FakeRouter {
    fn drop(&mut self) {
        self.stop.store(true, std::sync::atomic::Ordering::SeqCst);
        let _ = TcpStream::connect(self.addr);
        let _ = std::fs::remove_file(&self.addr_path);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
        let mut handles = self
            .connection_handles
            .lock()
            .expect("fake router connection handles lock");
        while let Some(handle) = handles.pop() {
            let _ = handle.join();
        }
    }
}

fn handle_router_connection(
    stream: TcpStream,
    received: &mpsc::Sender<Value>,
    replies: &StdMutex<VecDeque<RouterReply>>,
) -> Result<()> {
    let mut writer = stream.try_clone()?;
    let mut line = String::new();
    BufReader::new(stream).read_line(&mut line)?;
    if line.trim().is_empty() {
        return Ok(());
    }
    let request: Value = serde_json::from_str(line.trim()).context("decode router request")?;
    if request["kind"] == "health_check" || request["method"] == "health_check" {
        let response = json!({
            "ok": true,
            "request_id": request.get("request_id").cloned().unwrap_or(Value::Null),
            "payload": {
                "status": "ok"
            }
        });
        writer.write_all(serde_json::to_string(&response)?.as_bytes())?;
        writer.write_all(b"\n")?;
        writer.flush()?;
        return Ok(());
    }

    let _ = received.send(request.clone());
    let reply = replies
        .lock()
        .expect("fake router replies lock")
        .pop_front()
        .ok_or_else(|| anyhow!("fake router has no reply for request: {request}"))?;
    let response = match reply {
        RouterReply::Payload(payload) => json!({
            "ok": true,
            "request_id": request.get("request_id").cloned().unwrap_or(Value::Null),
            "payload": payload
        }),
        RouterReply::DelayedPayload(payload, delay) => {
            std::thread::sleep(delay);
            json!({
                "ok": true,
                "request_id": request.get("request_id").cloned().unwrap_or(Value::Null),
                "payload": payload
            })
        }
        RouterReply::CallbackThenPayload(callback, payload) => {
            callback(request.clone());
            json!({
                "ok": true,
                "request_id": request.get("request_id").cloned().unwrap_or(Value::Null),
                "payload": payload
            })
        }
        RouterReply::RawLine(line) => {
            writer.write_all(line.as_bytes())?;
            writer.write_all(b"\n")?;
            writer.flush()?;
            return Ok(());
        }
    };
    writer.write_all(serde_json::to_string(&response)?.as_bytes())?;
    writer.write_all(b"\n")?;
    writer.flush()?;
    Ok(())
}

fn wait_until(timeout: Duration, mut condition: impl FnMut() -> bool) -> Result<()> {
    let started = Instant::now();
    while started.elapsed() < timeout {
        if condition() {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    Err(anyhow!(
        "condition was not met within {}ms",
        timeout.as_millis()
    ))
}
