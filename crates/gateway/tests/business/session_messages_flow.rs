use axum::extract::{Json, Path, Query};
use gateway::api::session::{
    get_message, get_message_part, get_todos, list_messages, send_agent_message,
    stream_agent_message, update_todos, MessageListParams, SendAgentMedia, SendAgentMessageRequest,
    SendAgentToolCall, StreamAgentTextRequest,
};
use gateway::api::types::GlobalEvent;
use gateway::session_store;
use serde_json::{json, Value};
use std::collections::BTreeSet;

#[tokio::test]
async fn gateway_session_messages_business_flow_streams_then_persists_agent_reply(
) -> anyhow::Result<()> {
    let workspace = tempfile::tempdir()?;
    let session_id = create_business_session(workspace.path().to_string_lossy().to_string());
    drain_events();

    let Json(streamed) = stream_agent_message(
        Path(session_id.clone()),
        Json(StreamAgentTextRequest {
            message_id: "agent-message-business-1".to_string(),
            part_id: "agent-part-business-1".to_string(),
            delta: "partial token".to_string(),
            runtime_id: Some("runtime-business-1".to_string()),
        }),
    )
    .await;
    assert_eq!(streamed["ok"], true);
    assert_eq!(streamed["session_id"], session_id);

    assert_stream_delta_event(
        &session_id,
        "agent-message-business-1",
        "agent-part-business-1",
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
            message_id: "agent-message-business-1".to_string(),
            part_id: "agent-part-business-1".to_string(),
            delta: String::new(),
            runtime_id: Some("runtime-business-1".to_string()),
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
            runtime_id: Some("runtime-business-1".to_string()),
            tool_call: None,
            message_id: Some("agent-message-business-1".to_string()),
            part_id: Some("agent-part-business-1".to_string()),
        }),
    )
    .await;
    assert!(response.ok);
    assert_eq!(response.session_id, session_id);
    assert_eq!(
        response.message_id.as_deref(),
        Some("agent-message-business-1")
    );
    assert!(matches!(
        response.event,
        Some(GlobalEvent::MessageUpdated { .. })
    ));

    let Json(messages) = list_messages(Path(session_id.clone()), message_list_query()).await;
    assert_eq!(messages.len(), 1);
    let message = &messages[0];
    assert_eq!(message["info"]["id"], "agent-message-business-1");
    assert_eq!(message["info"]["sessionID"], session_id);
    assert_eq!(message["info"]["role"], "assistant");
    assert_eq!(message["parts"][0]["id"], "agent-part-business-1");
    assert_eq!(message["parts"][0]["type"], "text");
    assert_text_contains(&message["parts"][0], "Final agent reply");
    assert_text_contains(&message["parts"][0], "[MEDIA:artifacts/result.png:MEDIA]");
    assert_eq!(
        message["parts"][0]["metadata"]["step_summary"],
        "Stored reply summary"
    );

    let Json(fetched_message) = get_message(Path((
        session_id.clone(),
        "agent-message-business-1".to_string(),
    )))
    .await;
    assert_eq!(
        fetched_message["info"]["id"],
        json!("agent-message-business-1")
    );
    assert_eq!(
        fetched_message["parts"][0]["id"],
        json!("agent-part-business-1")
    );

    let Json(fetched_part) = get_message_part(Path((
        session_id.clone(),
        "agent-message-business-1".to_string(),
        "agent-part-business-1".to_string(),
    )))
    .await;
    assert_eq!(fetched_part["id"], "agent-part-business-1");
    assert_text_contains(&fetched_part, "Final agent reply");

    let Json(missing_part) = get_message_part(Path((
        session_id,
        "missing-message".to_string(),
        "missing-part".to_string(),
    )))
    .await;
    assert_eq!(missing_part["id"], "missing-part");
    assert_eq!(missing_part["text"], "");

    Ok(())
}

#[tokio::test]
async fn gateway_session_messages_business_flow_updates_planning_tool_message_and_todos_idempotently(
) -> anyhow::Result<()> {
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
            message_id: None,
            part_id: None,
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
            message_id: None,
            part_id: None,
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
    assert_eq!(messages[0]["parts"][0]["type"], "tool");
    assert_eq!(messages[0]["parts"][0]["tool"], "planning");
    assert_eq!(messages[0]["parts"][0]["callID"], "planning-call-business");
    assert_eq!(messages[0]["parts"][0]["state"]["status"], "completed");
    assert_eq!(
        messages[0]["parts"][0]["metadata"]["output"]["steps"][2]["task_summary"],
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
async fn gateway_session_messages_business_flow_runtime_usage_recovers_final_stream_text(
) -> anyhow::Result<()> {
    let workspace = tempfile::tempdir()?;
    let session_id = create_business_session(workspace.path().to_string_lossy().to_string());
    drain_events();

    let message_id = "msg-stream-runtime-business-recovery".to_string();
    let part_id = "part-stream-runtime-business-recovery".to_string();
    let runtime_id = "runtime-business-recovery".to_string();
    let Json(streamed) = stream_agent_message(
        Path(session_id.clone()),
        Json(StreamAgentTextRequest {
            message_id: message_id.clone(),
            part_id: part_id.clone(),
            delta: "partial token".to_string(),
            runtime_id: Some(runtime_id.clone()),
        }),
    )
    .await;
    assert_eq!(streamed["ok"], true);
    assert_stream_delta_event(&session_id, &message_id, &part_id);

    let Json(messages_before_usage) =
        list_messages(Path(session_id.clone()), message_list_query()).await;
    assert!(
        messages_before_usage.is_empty(),
        "streaming overlay should still wait for a durable final record"
    );

    let runtime_usage_call = SendAgentToolCall {
        tool_name: "runtime".to_string(),
        call_id: runtime_id.clone(),
        state: json!({
            "status": "completed",
            "output": {
                "output_text": "Recovered final reply from runtime usage"
            },
            "metadata": {
                "kind": "mano_runtime_usage"
            }
        }),
        metadata: Some(json!({
            "kind": "mano_runtime_usage",
            "runtime_id": runtime_id,
            "output": {
                "output_text": "Recovered final reply from runtime usage"
            },
            "usage": {
                "input_tokens": 10,
                "output_tokens": 5,
                "total_tokens": 15
            }
        })),
    };

    let Json(response) = send_agent_message(
        Path(session_id.clone()),
        Json(SendAgentMessageRequest {
            reply_message: String::new(),
            new_learning: String::new(),
            step_summary: None,
            media: Vec::new(),
            runtime_id: Some("runtime-business-recovery".to_string()),
            tool_call: Some(runtime_usage_call.clone()),
            message_id: Some(message_id.clone()),
            part_id: Some(part_id.clone()),
        }),
    )
    .await;
    assert!(response.ok);
    assert_eq!(response.message_id.as_deref(), Some(message_id.as_str()));

    let Json(messages) = list_messages(Path(session_id.clone()), message_list_query()).await;
    assert_eq!(
        messages.len(),
        1,
        "runtime usage recovery should produce one ordinary assistant message"
    );
    let message = &messages[0];
    assert_eq!(message["info"]["id"], message_id);
    assert_eq!(message["info"]["role"], "assistant");
    assert_eq!(message["parts"][0]["id"], part_id);
    assert_eq!(message["parts"][0]["type"], "text");
    assert_text_contains(
        &message["parts"][0],
        "Recovered final reply from runtime usage",
    );
    assert_eq!(message["parts"][1]["type"], "tool");
    assert_eq!(message["parts"][1]["tool"], "runtime");
    assert_eq!(
        message["parts"][1]["metadata"]["kind"],
        "mano_runtime_usage"
    );

    let Json(second_response) = send_agent_message(
        Path(session_id.clone()),
        Json(SendAgentMessageRequest {
            reply_message: String::new(),
            new_learning: String::new(),
            step_summary: None,
            media: Vec::new(),
            runtime_id: Some("runtime-business-recovery".to_string()),
            tool_call: Some(runtime_usage_call),
            message_id: Some(message_id.clone()),
            part_id: Some(part_id),
        }),
    )
    .await;
    assert!(second_response.ok);

    let Json(messages_after_update) =
        list_messages(Path(session_id.clone()), message_list_query()).await;
    assert_eq!(messages_after_update.len(), 1);
    assert_eq!(
        messages_after_update[0]["parts"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|part| part["type"] == "text")
            .count(),
        1,
        "replayed runtime usage must not duplicate the recovered final text"
    );

    Ok(())
}

#[tokio::test]
async fn gateway_session_messages_business_flow_publishes_session_name_updates(
) -> anyhow::Result<()> {
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
            message_id: None,
            part_id: None,
        }),
    )
    .await;

    assert!(response.ok);
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
        "auto session name changes must publish session.updated so GUI/TUI do not stare at stale names like decorative furniture"
    );

    Ok(())
}

#[tokio::test]
async fn gateway_session_messages_business_flow_concurrent_agent_writes_stay_session_scoped(
) -> anyhow::Result<()> {
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
                    runtime_id: Some(format!("runtime-concurrent-{index}")),
                    tool_call: None,
                    message_id: Some(format!("message-concurrent-{index}")),
                    part_id: Some(format!("part-concurrent-{index}")),
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
        assert_eq!(
            response.message_id.as_deref(),
            Some(format!("message-concurrent-{index}").as_str())
        );

        let Json(messages) = list_messages(Path(session_id.clone()), message_list_query()).await;
        assert_eq!(messages.len(), 1);
        let message = &messages[0];
        assert_eq!(message["info"]["id"], format!("message-concurrent-{index}"));
        assert_eq!(message["info"]["sessionID"], session_id);
        assert_eq!(
            message["parts"][0]["id"],
            format!("part-concurrent-{index}")
        );
        assert_text_contains(
            &message["parts"][0],
            &format!("Concurrent reply for session {index}"),
        );
        assert_text_contains(
            &message["parts"][0],
            &format!("[MEDIA:artifacts/session-{index}.txt:MEDIA]"),
        );
        assert_eq!(
            message["parts"][0]["metadata"]["step_summary"],
            format!("summary-{index}")
        );

        for other in 0..session_ids.len() {
            if other != index {
                let serialized = serde_json::to_string(message)?;
                assert!(
                    !serialized.contains(&format!("session-{other}.txt")),
                    "session {index} message must not include another session artifact: {serialized}"
                );
                assert!(
                    !serialized.contains(&format!("message-concurrent-{other}")),
                    "session {index} message must not include another message id: {serialized}"
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

fn assert_text_contains(value: &Value, expected: &str) {
    let text = value
        .get("text")
        .and_then(Value::as_str)
        .or_else(|| value.get("content").and_then(Value::as_str))
        .unwrap_or_default();
    assert!(
        text.contains(expected),
        "expected text to contain {expected:?}, got {text:?}"
    );
}

fn drain_events() {
    while session_store().pop_event().is_some() {}
}
