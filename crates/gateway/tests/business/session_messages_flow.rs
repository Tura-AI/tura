use axum::extract::{Json, Path, Query};
use gateway::api::session::{
    get_message, get_message_part, get_todos, list_messages, send_agent_message,
    stream_agent_message, update_todos, MessageListParams, SendAgentMedia, SendAgentMessageRequest,
    SendAgentToolCall, StreamAgentTextRequest,
};
use gateway::contracts::{GlobalEvent, Message, MessagePart};
use gateway::session_store;
use runtime::state_machine::runtime_management::{
    RuntimeCallResultStatus, RuntimeSessionSyncStatus, RuntimeState,
};
use serde_json::{json, Value};
use session_log::{SessionLogCommand, SessionLogStore};
use std::collections::BTreeSet;
use std::path::Path as FsPath;
use std::time::{Duration, Instant};

static SESSION_DB_ENV_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
static SESSION_MESSAGES_FLOW_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

#[tokio::test]
async fn gateway_session_messages_business_flow_streams_then_persists_agent_reply(
) -> anyhow::Result<()> {
    let _flow_guard = SESSION_MESSAGES_FLOW_LOCK.lock().await;
    let workspace = tempfile::tempdir()?;
    let session_id = create_business_session(workspace.path().to_string_lossy().to_string());
    drain_events();

    let Json(streamed) = stream_agent_message(
        Path(session_id.clone()),
        Json(StreamAgentTextRequest {
            delta: "partial token".to_string(),
            runtime_id: "runtime-business-1".to_string(),
            created_at: 0,
            updated_at: 0,
            context_tokens: None,
            usage: None,
        }),
    )
    .await;
    assert_eq!(streamed["ok"], true);
    assert_eq!(streamed["session_id"], session_id);

    assert_stream_delta_event(
        &session_id,
        "runtime-business-1.message",
        "runtime-business-1.message",
    );

    let Json(messages_before_final) =
        list_messages(Path(session_id.clone()), message_list_query()).await;
    assert!(
        messages_before_final.is_empty(),
        "streaming deltas are frontend overlays and must not persist as stored messages"
    );

    let Json(empty_delta) = stream_agent_message(
        Path(session_id.clone()),
        Json(StreamAgentTextRequest {
            delta: String::new(),
            runtime_id: "runtime-business-1".to_string(),
            created_at: 0,
            updated_at: 0,
            context_tokens: None,
            usage: None,
        }),
    )
    .await;
    assert_eq!(empty_delta["ok"], true);
    assert!(session_store().pop_event().is_none());

    let Json(response) = send_agent_message(
        Path(session_id.clone()),
        Json(SendAgentMessageRequest {
            reply_message: "Final agent reply".to_string(),
            new_learning: String::new(),
            step_summary: Some("Stored reply summary".to_string()),
            media: vec![SendAgentMedia {
                path: "artifacts/result.png".to_string(),
                media_type: Some("image/png".to_string()),
            }],
            runtime_id: None,
            tool_call: None,
            runtime_status: None,
            context_tokens: None,
            usage: None,
            command_updates: Vec::new(),
            created_at: 0,
            updated_at: 0,
        }),
    )
    .await;
    assert!(response.ok);
    assert_eq!(response.session_id, session_id);
    let response_message_id = response
        .message_id
        .clone()
        .expect("stored assistant message should have an id");
    assert!(matches!(
        response.event,
        Some(GlobalEvent::MessageUpdated { .. })
    ));

    let Json(messages) = list_messages(Path(session_id.clone()), message_list_query()).await;
    assert_eq!(messages.len(), 1);
    let message = &messages[0];
    assert_eq!(message.id, response_message_id);
    assert_eq!(message.session_id, session_id);
    assert_eq!(message_value(message)["role"], "assistant");
    let response_part = message
        .parts
        .first()
        .expect("stored assistant text part should be present");
    let response_part_id = response_part.id.clone();
    assert_eq!(response_part.part_type, "text");
    assert_text_contains(response_part, "Final agent reply");
    assert_text_contains(response_part, "[MEDIA:artifacts/result.png:MEDIA]");
    assert_eq!(
        response_part.metadata.as_ref().expect("metadata")["step_summary"],
        "Stored reply summary"
    );

    let Json(fetched_message) =
        get_message(Path((session_id.clone(), response_message_id.clone()))).await;
    assert_eq!(fetched_message.id, response_message_id);
    assert_eq!(fetched_message.parts[0].id, response_part_id);

    let Json(fetched_part) = get_message_part(Path((
        session_id.clone(),
        response_message_id,
        response_part_id.clone(),
    )))
    .await;
    assert_eq!(fetched_part.id, response_part_id);
    assert_text_contains(&fetched_part, "Final agent reply");

    let Json(missing_part) = get_message_part(Path((
        session_id,
        "missing-message".to_string(),
        "missing-part".to_string(),
    )))
    .await;
    assert_eq!(missing_part.id, "missing-part");
    assert_eq!(missing_part.text.as_deref(), Some(""));

    Ok(())
}

#[tokio::test]
async fn gateway_session_messages_business_flow_updates_planning_tool_message_and_todos_idempotently(
) -> anyhow::Result<()> {
    let _flow_guard = SESSION_MESSAGES_FLOW_LOCK.lock().await;
    let workspace = tempfile::tempdir()?;
    let session_id = create_business_session(workspace.path().to_string_lossy().to_string());
    drain_events();

    let Json(replaced_todos) = update_todos(
        Path(session_id.clone()),
        Json(vec![json!({
            "id": "manual-1",
            "content": "manual todo",
            "status": "pending",
            "priority": "low"
        })]),
    )
    .await;
    assert_eq!(replaced_todos.len(), 1);
    let Json(read_manual_todos) = get_todos(Path(session_id.clone())).await;
    assert_eq!(read_manual_todos, replaced_todos);

    let running_tool = planning_tool_call("running", None);
    let Json(first_response) = send_agent_message(
        Path(session_id.clone()),
        Json(SendAgentMessageRequest {
            reply_message: String::new(),
            new_learning: String::new(),
            step_summary: None,
            media: Vec::new(),
            runtime_id: Some("runtime-business-2".to_string()),
            tool_call: Some(running_tool),
            runtime_status: None,
            context_tokens: None,
            usage: None,
            command_updates: Vec::new(),
            created_at: 0,
            updated_at: 0,
        }),
    )
    .await;
    assert!(first_response.ok);
    let first_message_id = first_response.message_id.clone();

    let Json(running_todos) = get_todos(Path(session_id.clone())).await;
    assert_eq!(running_todos.len(), 3);
    assert_eq!(running_todos[0]["id"], "planning-call-business:1");
    assert_eq!(running_todos[0]["content"], "Inspect gateway state");
    assert_eq!(running_todos[0]["status"], "in_progress");
    assert_eq!(running_todos[1]["status"], "pending");
    assert_eq!(running_todos[2]["status"], "pending");

    let completed_tool = planning_tool_call(
        "completed",
        Some(json!({
            "steps": [
                { "index": 1, "ok": true, "task_summary": "Gateway message flow covered" },
                { "index": 2, "ok": false },
                { "index": 3, "ok": true, "task_summary": "Final gateway session message summary" }
            ]
        })),
    );
    let Json(second_response) = send_agent_message(
        Path(session_id.clone()),
        Json(SendAgentMessageRequest {
            reply_message: String::new(),
            new_learning: String::new(),
            step_summary: None,
            media: Vec::new(),
            runtime_id: Some("runtime-business-2".to_string()),
            tool_call: Some(completed_tool),
            runtime_status: None,
            context_tokens: None,
            usage: None,
            command_updates: Vec::new(),
            created_at: 0,
            updated_at: 0,
        }),
    )
    .await;
    assert!(second_response.ok);
    assert_eq!(second_response.message_id, first_message_id);

    let Json(messages) = list_messages(Path(session_id.clone()), message_list_query()).await;
    assert_eq!(
        messages.len(),
        1,
        "same planning call_id/tool_name should update the existing tool message"
    );
    let part = messages[0].parts.first().expect("planning part");
    assert_eq!(part.part_type, "tool");
    assert_eq!(part.tool.as_deref(), Some("planning"));
    assert_eq!(part.call_id.as_deref(), Some("planning-call-business"));
    assert_eq!(part.state.as_ref().expect("state")["status"], "completed");
    assert_eq!(
        part.metadata.as_ref().expect("metadata")["output"]["steps"][2]["task_summary"],
        "Final gateway session message summary"
    );

    let Json(completed_todos) = get_todos(Path(session_id.clone())).await;
    assert_eq!(completed_todos.len(), 3);
    assert_eq!(completed_todos[0]["status"], "completed");
    assert_eq!(completed_todos[1]["status"], "cancelled");
    assert_eq!(completed_todos[2]["status"], "completed");

    let updated_session = session_store()
        .get_session(&session_id)
        .ok_or_else(|| anyhow::anyhow!("created session should still exist"))?;
    assert_eq!(
        updated_session.name.as_deref(),
        Some("Final gateway session message summary")
    );

    Ok(())
}

#[tokio::test]
async fn gateway_session_messages_business_flow_publishes_session_name_updates(
) -> anyhow::Result<()> {
    let _flow_guard = SESSION_MESSAGES_FLOW_LOCK.lock().await;
    let workspace = tempfile::tempdir()?;
    let session_id = create_business_session(workspace.path().to_string_lossy().to_string());
    drain_events();

    let Json(response) = send_agent_message(
        Path(session_id.clone()),
        Json(SendAgentMessageRequest {
            reply_message: String::new(),
            new_learning: String::new(),
            step_summary: None,
            media: Vec::new(),
            runtime_id: Some("runtime-session-name-business".to_string()),
            tool_call: Some(SendAgentToolCall {
                tool_name: "command_run".to_string(),
                call_id: "session-name-call-business".to_string(),
                state: json!({
                    "status": "completed",
                    "metadata": {
                        "output": {
                            "results": [{
                                "output": {
                                    "status": {
                                        "task_detail": "Gateway Session Name Updated"
                                    }
                                }
                            }]
                        }
                    }
                }),
                metadata: None,
            }),
            runtime_status: None,
            context_tokens: None,
            usage: None,
            command_updates: Vec::new(),
            created_at: 0,
            updated_at: 0,
        }),
    )
    .await;

    assert!(response.ok);
    assert!(
        session_store().get_messages(&session_id).is_empty(),
        "gateway must not persist command_run callback tool records as transcript messages"
    );
    let updated_session = session_store()
        .get_session(&session_id)
        .ok_or_else(|| anyhow::anyhow!("created session should still exist"))?;
    assert_eq!(
        updated_session.name.as_deref(),
        Some("Gateway Session Name Updated")
    );
    assert_eq!(
        updated_session.session_display_name.as_deref(),
        Some("Gateway Session Name Updated")
    );

    let mut saw_session_updated = false;
    while let Some(event) = session_store().pop_event() {
        if let GlobalEvent::SessionUpdated { properties } = event {
            if properties.session_id == session_id {
                assert_eq!(
                    properties.info.name.as_deref(),
                    Some("Gateway Session Name Updated")
                );
                assert_eq!(
                    properties.info.session_display_name.as_deref(),
                    Some("Gateway Session Name Updated")
                );
                saw_session_updated = true;
            }
        }
    }
    assert!(
        saw_session_updated,
        "auto session name changes must publish session.updated for every client projection"
    );

    Ok(())
}

#[tokio::test]
async fn gateway_session_messages_business_flow_reads_projection_history_without_active_runtime_overlay_then_refreshes_db(
) -> anyhow::Result<()> {
    let _flow_guard = SESSION_MESSAGES_FLOW_LOCK.lock().await;
    let _guard = SESSION_DB_ENV_LOCK.lock().await;
    let root = tempfile::tempdir()?;
    let home = root.path().join("home");
    let workspace = root.path().join("workspace");
    std::fs::create_dir_all(&home)?;
    std::fs::create_dir_all(&workspace)?;
    let _env = EnvGuard::new(&home);
    let _service = ServiceThread::start()?;

    let workspace_key = session_log::path::normalize_workspace(&workspace.to_string_lossy());
    let session_id = create_business_session(workspace_key.clone());
    let runtime_id = "runtime-overlay-business".to_string();
    let runtime_message_id = format!("{runtime_id}.message");
    let text_part_id = format!("{runtime_id}.message");
    let command_part_id = format!("{runtime_id}.tool.command_run");
    let command_call_id = command_part_id.clone();
    let assistant_created_at = 1_781_514_293_000_i64;
    let assistant_final_updated_at = assistant_created_at + 7_000;
    let command_started_at = assistant_created_at + 200;
    let command_finished_at = assistant_created_at + 4_000;

    upsert_canonical_session(
        &session_id,
        &workspace_key,
        vec![db_text_message(
            &session_id,
            "db-user-message",
            "db-user-part",
            "user",
            "Persisted user request",
            assistant_created_at - 100,
            assistant_created_at - 100,
        )],
    )?;

    let Json(db_only) = list_messages(Path(session_id.clone()), message_list_query()).await;
    assert_eq!(db_only.len(), 1);
    assert_eq!(db_only[0].id, "db-user-message");

    let Json(live_text_response) = send_agent_message(
        Path(session_id.clone()),
        Json(SendAgentMessageRequest {
            reply_message: "Live assistant text before DB final".to_string(),
            new_learning: String::new(),
            step_summary: None,
            media: Vec::new(),
            runtime_id: Some(runtime_id.clone()),
            tool_call: None,
            runtime_status: Some(runtime_sync_status(&runtime_id, true)),
            context_tokens: None,
            usage: None,
            command_updates: Vec::new(),
            created_at: assistant_created_at,
            updated_at: assistant_created_at,
        }),
    )
    .await;
    assert!(live_text_response.ok);

    let Json(live_command_response) = send_agent_message(
        Path(session_id.clone()),
        Json(SendAgentMessageRequest {
            reply_message: String::new(),
            new_learning: String::new(),
            step_summary: None,
            media: Vec::new(),
            runtime_id: Some(runtime_id.clone()),
            tool_call: Some(command_run_tool_call(
                &command_call_id,
                "completed",
                "live-overlay",
                command_started_at,
                command_finished_at,
            )),
            runtime_status: Some(runtime_sync_status(&runtime_id, true)),
            context_tokens: None,
            usage: None,
            command_updates: Vec::new(),
            created_at: command_started_at,
            updated_at: command_finished_at,
        }),
    )
    .await;
    assert!(live_command_response.ok);

    let Json(duplicate_live_text_response) = send_agent_message(
        Path(session_id.clone()),
        Json(SendAgentMessageRequest {
            reply_message: "Live assistant text before DB final".to_string(),
            new_learning: String::new(),
            step_summary: None,
            media: Vec::new(),
            runtime_id: Some(runtime_id.clone()),
            tool_call: None,
            runtime_status: Some(runtime_sync_status(&runtime_id, true)),
            context_tokens: None,
            usage: None,
            command_updates: Vec::new(),
            created_at: assistant_created_at + 1,
            updated_at: assistant_created_at + 1,
        }),
    )
    .await;
    assert!(duplicate_live_text_response.ok);

    let Json(duplicate_live_command_response) = send_agent_message(
        Path(session_id.clone()),
        Json(SendAgentMessageRequest {
            reply_message: String::new(),
            new_learning: String::new(),
            step_summary: None,
            media: Vec::new(),
            runtime_id: Some(runtime_id.clone()),
            tool_call: Some(command_run_tool_call(
                &command_call_id,
                "completed",
                "live-overlay-duplicate",
                command_started_at,
                command_finished_at,
            )),
            runtime_status: Some(runtime_sync_status(&runtime_id, true)),
            context_tokens: None,
            usage: None,
            command_updates: Vec::new(),
            created_at: command_started_at + 1,
            updated_at: command_finished_at + 1,
        }),
    )
    .await;
    assert!(duplicate_live_command_response.ok);

    let Json(with_active_live) =
        list_messages(Path(session_id.clone()), message_list_query()).await;
    assert_eq!(
        with_active_live.len(),
        1,
        "session messages must stay a static projection and never include active runtime live overlays"
    );
    assert_eq!(with_active_live[0].id, "db-user-message");

    upsert_canonical_session(
        &session_id,
        &workspace_key,
        vec![
            db_text_message(
                &session_id,
                "db-user-message",
                "db-user-part",
                "user",
                "Persisted user request",
                assistant_created_at - 100,
                assistant_created_at - 100,
            ),
            db_runtime_message(
                &session_id,
                &runtime_message_id,
                &text_part_id,
                &command_part_id,
                &command_call_id,
                "DB canonical final assistant",
                assistant_created_at,
                assistant_final_updated_at,
                "completed",
                "db-canonical",
                command_started_at,
                command_finished_at,
            ),
        ],
    )?;

    let Json(final_response) = send_agent_message(
        Path(session_id.clone()),
        Json(SendAgentMessageRequest {
            reply_message: String::new(),
            new_learning: String::new(),
            step_summary: None,
            media: Vec::new(),
            runtime_id: Some(runtime_id.clone()),
            tool_call: Some(SendAgentToolCall {
                tool_name: "runtime".to_string(),
                call_id: runtime_id.clone(),
                state: json!({
                    "status": "completed"
                }),
                metadata: None,
            }),
            runtime_status: Some(runtime_sync_status(&runtime_id, false)),
            context_tokens: None,
            usage: None,
            command_updates: Vec::new(),
            created_at: assistant_created_at,
            updated_at: assistant_final_updated_at,
        }),
    )
    .await;
    assert!(final_response.ok);
    assert!(matches!(
        final_response.event,
        Some(GlobalEvent::MessageUpdated { .. })
    ));

    let Json(after_refresh) = list_messages(Path(session_id), message_list_query()).await;
    assert_eq!(after_refresh.len(), 2);
    assert_eq!(
        after_refresh
            .iter()
            .filter(|message| message.id == runtime_message_id)
            .count(),
        1,
        "finished runtime projection must replace the live copy instead of duplicating it"
    );
    let final_assistant = message_by_id(&after_refresh, &runtime_message_id);
    assert_eq!(
        final_assistant.parts[0].text.as_deref(),
        Some("DB canonical final assistant")
    );
    assert_eq!(final_assistant.created_at, assistant_created_at);
    assert_eq!(final_assistant.updated_at, assistant_final_updated_at);
    let final_command = part_by_id(final_assistant, &command_part_id);
    let final_command_metadata = final_command.metadata.as_ref().expect("metadata");
    let final_command_state = final_command.state.as_ref().expect("state");
    assert_eq!(final_command_metadata["source"], "db-canonical");
    assert_eq!(final_command_state["status"], "completed");
    assert_eq!(final_command_state["time"]["start"], command_started_at);
    assert_eq!(final_command_state["time"]["end"], command_finished_at);

    Ok(())
}

#[tokio::test]
async fn gateway_session_messages_business_flow_concurrent_agent_writes_stay_session_scoped(
) -> anyhow::Result<()> {
    let _flow_guard = SESSION_MESSAGES_FLOW_LOCK.lock().await;
    let workspace = tempfile::tempdir()?;
    let session_ids = (0..4)
        .map(|index| {
            create_business_session(
                workspace
                    .path()
                    .join(format!("concurrent-session-{index}"))
                    .to_string_lossy()
                    .to_string(),
            )
        })
        .collect::<Vec<_>>();
    drain_events();

    let mut writes = Vec::new();
    for (index, session_id) in session_ids.iter().enumerate() {
        let session_id = session_id.clone();
        writes.push(tokio::spawn(async move {
            let Json(response) = send_agent_message(
                Path(session_id.clone()),
                Json(SendAgentMessageRequest {
                    reply_message: format!("Concurrent reply for session {index}"),
                    new_learning: format!("learning-{index}"),
                    step_summary: Some(format!("summary-{index}")),
                    media: vec![SendAgentMedia {
                        path: format!("artifacts/session-{index}.txt"),
                        media_type: Some("text/plain".to_string()),
                    }],
                    runtime_id: None,
                    tool_call: None,
                    runtime_status: None,
                    context_tokens: None,
                    usage: None,
                    command_updates: Vec::new(),
                    created_at: 0,
                    updated_at: 0,
                }),
            )
            .await;
            (index, session_id, response)
        }));
    }

    let mut completed = Vec::new();
    for write in writes {
        completed.push(write.await.expect("concurrent agent write should join"));
    }
    completed.sort_by_key(|(index, _, _)| *index);

    for (index, session_id, response) in completed {
        assert!(response.ok);
        assert_eq!(response.session_id, session_id);
        let response_message_id = response
            .message_id
            .expect("concurrent assistant write should return a message id");

        let Json(messages) = list_messages(Path(session_id.clone()), message_list_query()).await;
        assert_eq!(messages.len(), 1);
        let message = &messages[0];
        assert_eq!(message.id, response_message_id);
        assert_eq!(message.session_id, session_id);
        let part = message.parts.first().expect("assistant text part");
        assert_text_contains(part, &format!("Concurrent reply for session {index}"));
        assert_text_contains(
            part,
            &format!("[MEDIA:artifacts/session-{index}.txt:MEDIA]"),
        );
        assert_eq!(
            part.metadata.as_ref().expect("metadata")["step_summary"],
            format!("summary-{index}")
        );

        for other in 0..session_ids.len() {
            if other != index {
                let serialized = serde_json::to_string(message)?;
                assert!(
                    !serialized.contains(&format!("session-{other}.txt")),
                    "session {index} message must not include another session artifact: {serialized}"
                );
            }
        }
    }

    let mut updated_sessions = BTreeSet::new();
    while let Some(event) = session_store().pop_event() {
        if let GlobalEvent::MessageUpdated { properties } = event {
            updated_sessions.insert(properties.session_id);
        }
    }
    assert_eq!(
        updated_sessions,
        session_ids.into_iter().collect::<BTreeSet<_>>(),
        "each concurrent agent write should publish one session-scoped update"
    );

    Ok(())
}

fn create_business_session(directory: String) -> String {
    session_store()
        .create_session(
            Some(directory),
            Some("business-model".to_string()),
            None,
            Some("coding".to_string()),
            false,
            false,
            false,
            None,
            false,
            false,
        )
        .id
}

fn runtime_sync_status(runtime_id: &str, live: bool) -> RuntimeSessionSyncStatus {
    RuntimeSessionSyncStatus {
        runtime_id: runtime_id.to_string(),
        state: if live {
            RuntimeState::Streaming
        } else {
            RuntimeState::Finished
        },
        call_result_status: if live {
            RuntimeCallResultStatus::Streaming
        } else {
            RuntimeCallResultStatus::Succeeded
        },
        live,
        session_db_refresh_required: !live,
    }
}

fn upsert_canonical_session(
    session_id: &str,
    workspace: &str,
    messages: Vec<Value>,
) -> anyhow::Result<()> {
    let mut info = session_store()
        .get_session_info(session_id)
        .ok_or_else(|| anyhow::anyhow!("session should exist before DB upsert"))?;
    info.directory = Some(workspace.to_string());
    info.message_count = messages.len();
    info.updated_at = messages
        .iter()
        .filter_map(|message| message.get("updated_at").and_then(Value::as_i64))
        .max()
        .unwrap_or(info.updated_at);
    let response = session_log::ipc::call_service(&SessionLogCommand::UpsertSession(
        session_log::UpsertSessionRequest {
            session: serde_json::to_value(info)?,
            parent_id: None,
            messages,
            todos: Vec::new(),
        },
    ))?;
    match response {
        session_log::SessionLogResponse::Ok => Ok(()),
        session_log::SessionLogResponse::Error { error } => anyhow::bail!("{error}"),
        other => anyhow::bail!("unexpected session_log response: {other:?}"),
    }
}

fn db_text_message(
    session_id: &str,
    message_id: &str,
    part_id: &str,
    role: &str,
    text: &str,
    created_at: i64,
    updated_at: i64,
) -> Value {
    json!({
        "id": message_id,
        "session_id": session_id,
        "role": role,
        "parent_id": null,
        "parts": [{
            "id": part_id,
            "type": "text",
            "content": text,
            "text": text,
            "metadata": null,
            "call_id": null,
            "tool": null,
            "state": null
        }],
        "created_at": created_at,
        "updated_at": updated_at
    })
}

fn db_command_message(
    session_id: &str,
    message_id: &str,
    part_id: &str,
    call_id: &str,
    status: &str,
    source: &str,
    started_at: i64,
    finished_at: i64,
) -> Value {
    json!({
        "id": message_id,
        "session_id": session_id,
        "role": "assistant",
        "parent_id": "db-user-message",
        "parts": [{
            "id": part_id,
            "type": "tool",
            "content": null,
            "text": null,
            "metadata": {
                "kind": "mano_tool_call",
                "tool": "command_run",
                "source": source
            },
            "call_id": call_id,
            "tool": "command_run",
            "state": {
                "status": status,
                "input": {
                    "commands": [{
                        "step": 1,
                        "command_type": "shell_command",
                        "command_line": "npm test"
                    }]
                },
                "output": {
                    "streamed_command_run_result": {
                        "results": [{
                            "step": 1,
                            "command_type": "shell_command",
                            "command_line": "npm test",
                            "success": true
                        }]
                    }
                },
                "time": {
                    "start": started_at,
                    "end": finished_at
                }
            }
        }],
        "created_at": started_at,
        "updated_at": finished_at
    })
}

fn db_runtime_message(
    session_id: &str,
    message_id: &str,
    text_part_id: &str,
    command_part_id: &str,
    call_id: &str,
    text: &str,
    created_at: i64,
    updated_at: i64,
    status: &str,
    source: &str,
    command_started_at: i64,
    command_finished_at: i64,
) -> Value {
    let command = db_command_message(
        session_id,
        message_id,
        command_part_id,
        call_id,
        status,
        source,
        command_started_at,
        command_finished_at,
    );
    let command_part = command["parts"][0].clone();
    json!({
        "id": message_id,
        "session_id": session_id,
        "role": "assistant",
        "parent_id": "db-user-message",
        "parts": [
            {
                "id": text_part_id,
                "type": "text",
                "content": text,
                "text": text,
                "metadata": null,
                "call_id": null,
                "tool": null,
                "state": null
            },
            command_part
        ],
        "created_at": created_at,
        "updated_at": updated_at
    })
}

fn command_run_tool_call(
    call_id: &str,
    status: &str,
    source: &str,
    started_at: i64,
    finished_at: i64,
) -> SendAgentToolCall {
    SendAgentToolCall {
        tool_name: "command_run".to_string(),
        call_id: call_id.to_string(),
        state: json!({
            "status": status,
            "input": {
                "commands": [{
                    "step": 1,
                    "command_type": "shell_command",
                    "command_line": "npm test"
                }]
            },
            "output": {
                "streamed_command_run_result": {
                    "results": [{
                        "step": 1,
                        "command_type": "shell_command",
                        "command_line": "npm test",
                        "success": true
                    }]
                }
            },
            "metadata": {
                "kind": "mano_tool_call",
                "tool": "command_run",
                "source": source,
                "transient": true
            },
            "time": {
                "start": started_at,
                "end": finished_at
            }
        }),
        metadata: Some(json!({
            "kind": "mano_tool_call",
            "tool": "command_run",
            "source": source,
            "transient": true
        })),
    }
}

fn message_value(message: &Message) -> Value {
    serde_json::to_value(message).expect("message should serialize")
}

fn message_by_id<'a>(messages: &'a [Message], message_id: &str) -> &'a Message {
    messages
        .iter()
        .find(|message| message.id == message_id)
        .unwrap_or_else(|| panic!("message {message_id} should be present: {messages:#?}"))
}

fn part_by_id<'a>(message: &'a Message, part_id: &str) -> &'a MessagePart {
    message
        .parts
        .iter()
        .find(|part| part.id == part_id)
        .unwrap_or_else(|| panic!("part {part_id} should be present: {message:#?}"))
}

struct ServiceThread {
    handle: Option<std::thread::JoinHandle<anyhow::Result<()>>>,
}

impl ServiceThread {
    fn start() -> anyhow::Result<Self> {
        let store = SessionLogStore::open_default()?;
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

struct EnvGuard {
    previous: Vec<(&'static str, Option<std::ffi::OsString>)>,
}

impl EnvGuard {
    fn new(home: &FsPath) -> Self {
        let keys = ["TURA_HOME", "TURA_DB_ROOT", "SESSION_LOG_DB_ROOT"];
        let previous = keys
            .iter()
            .map(|key| (*key, std::env::var_os(key)))
            .collect::<Vec<_>>();
        std::env::set_var("TURA_HOME", home);
        std::env::remove_var("TURA_DB_ROOT");
        std::env::remove_var("SESSION_LOG_DB_ROOT");
        Self { previous }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        for (key, value) in self.previous.drain(..).rev() {
            match value {
                Some(value) => std::env::set_var(key, value),
                None => std::env::remove_var(key),
            }
        }
    }
}

fn wait_until(timeout: Duration, mut condition: impl FnMut() -> bool) -> anyhow::Result<()> {
    let started = Instant::now();
    while started.elapsed() < timeout {
        if condition() {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    anyhow::bail!("condition was not met within {}ms", timeout.as_millis())
}

fn planning_tool_call(status: &str, output: Option<Value>) -> SendAgentToolCall {
    SendAgentToolCall {
        tool_name: "planning".to_string(),
        call_id: "planning-call-business".to_string(),
        state: json!({
            "status": status,
            "input": {
                "steps": [
                    { "step_goal": "Inspect gateway state" },
                    { "step_goal": "Persist agent message" },
                    { "task_instruction": "Verify final todo state" }
                ]
            }
        }),
        metadata: output.map(|output| {
            json!({
                "output": output
            })
        }),
    }
}

fn assert_stream_delta_event(session_id: &str, message_id: &str, part_id: &str) {
    match session_store().pop_event() {
        Some(GlobalEvent::MessagePartDelta { properties }) => {
            assert_eq!(properties.session_id, session_id);
            assert_eq!(properties.message_id, message_id);
            assert_eq!(properties.part_id, part_id);
            assert_eq!(properties.field, "text");
            assert_eq!(properties.delta, "partial token");
        }
        event => panic!("expected message.part.delta event, got {event:?}"),
    }
}

fn message_list_query() -> Query<MessageListParams> {
    Query(MessageListParams::default())
}

fn assert_text_contains(value: &MessagePart, expected: &str) {
    let text = value
        .text
        .as_deref()
        .or(value.content.as_deref())
        .unwrap_or_default();
    assert!(
        text.contains(expected),
        "expected text to contain {expected:?}, got {text:?}"
    );
}

fn drain_events() {
    while session_store().pop_event().is_some() {}
}
