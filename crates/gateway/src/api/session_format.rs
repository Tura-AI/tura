use super::*;
use crate::session::store::{frontend_safe_part_state, frontend_safe_part_value};

pub(crate) fn api_message_from_store(message: crate::session::store::Message) -> Message {
    let session_id = message.session_id.clone();
    let message_id = message.id.clone();
    Message {
        id: message.id,
        session_id: message.session_id,
        role: match message.role {
            SessionMessageRole::User => MessageRole::User,
            SessionMessageRole::Assistant => MessageRole::Assistant,
            SessionMessageRole::System => MessageRole::System,
        },
        parts: message
            .parts
            .into_iter()
            .map(|part| MessagePart {
                id: part.id.clone(),
                session_id: session_id.clone(),
                message_id: message_id.clone(),
                part_type: part.part_type.clone(),
                content: part.content.clone(),
                text: part.text.clone().or(part.content.clone()),
                metadata: frontend_safe_part_value(&part, part.metadata.clone()),
                call_id: part.call_id.clone(),
                tool: part.tool.clone(),
                state: frontend_safe_part_state(&part, part.state.clone()),
            })
            .collect(),
        created_at: message.created_at,
        updated_at: message.updated_at,
        parent_id: message.parent_id,
    }
}

pub(super) fn message_with_parts_from_store(message: crate::session::store::Message) -> Message {
    api_message_from_store(message)
}

pub(super) fn part_json(
    session_id: &str,
    message_id: &str,
    part: crate::session::store::MessagePart,
) -> MessagePart {
    MessagePart {
        id: part.id.clone(),
        session_id: session_id.to_string(),
        message_id: message_id.to_string(),
        part_type: part.part_type.clone(),
        content: part.content.clone(),
        text: part.text.clone().or(part.content.clone()),
        metadata: frontend_safe_part_value(&part, part.metadata.clone()),
        call_id: part.call_id.clone(),
        tool: part.tool.clone(),
        state: frontend_safe_part_state(&part, part.state.clone()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frontend_safe_part_value_removes_internal_keys_recursively() {
        let part = store_part("tool", Some("shell"), None, None, None, None);
        let value = serde_json::json!({
            "visible": true,
            "runtime_id": "runtime-secret",
            "nested": {
                "new_learning": "private",
                "keep": "public"
            },
            "items": [
                { "runtime_id": "nested-secret", "text": "shown" },
                "literal"
            ]
        });

        let sanitized = frontend_safe_part_value(&part, Some(value)).expect("sanitized value");

        assert_eq!(
            sanitized,
            serde_json::json!({
                "visible": true,
                "nested": {
                    "keep": "public"
                },
                "items": [
                    { "text": "shown" },
                    "literal"
                ]
            })
        );
    }

    #[test]
    fn runtime_tool_parts_keep_runtime_metadata_and_state() {
        let part = store_part(
            "tool",
            Some("runtime"),
            Some("content fallback"),
            None,
            Some(serde_json::json!({
                "runtime_id": "runtime-1",
                "new_learning": "allowed for runtime event",
                "visible": "metadata"
            })),
            Some(serde_json::json!({
                "runtime_id": "runtime-1",
                "state": "Running"
            })),
        );

        let value =
            serde_json::to_value(part_json("session-1", "message-1", part)).expect("part json");

        assert_eq!(value["text"], "content fallback");
        assert_eq!(value["metadata"]["runtime_id"], "runtime-1");
        assert_eq!(
            value["metadata"]["new_learning"],
            "allowed for runtime event"
        );
        assert_eq!(value["state"]["runtime_id"], "runtime-1");
    }

    #[test]
    fn non_runtime_tool_parts_are_sanitized_and_prefer_text_over_content() {
        let part = store_part(
            "tool",
            Some("shell"),
            Some("content fallback"),
            Some("visible text"),
            Some(serde_json::json!({
                "runtime_id": "hidden",
                "new_learning": "hidden",
                "ok": true
            })),
            Some(serde_json::json!({
                "runtime_id": "hidden",
                "exit_code": 0
            })),
        );

        let value =
            serde_json::to_value(part_json("session-1", "message-1", part)).expect("part json");

        assert_eq!(value["sessionID"], "session-1");
        assert_eq!(value["messageID"], "message-1");
        assert_eq!(value["text"], "visible text");
        assert_eq!(value["metadata"], serde_json::json!({ "ok": true }));
        assert_eq!(value["state"], serde_json::json!({ "exit_code": 0 }));
    }

    #[test]
    fn command_run_parts_expose_canonical_commands_from_legacy_state() {
        let part = store_part(
            "tool",
            Some("command_run"),
            None,
            None,
            None,
            Some(serde_json::json!({
                "status": "running",
                "input": {
                    "commands": [
                        {
                            "step": 3,
                            "command_type": "shell_command",
                            "command_line": "npm run build"
                        }
                    ]
                },
                "output": {
                    "streamed_command_run_result": {
                        "results": [
                            {
                                "step": 3,
                                "status": "completed",
                                "command_type": "shell_command",
                                "command_line": "npm run build"
                            }
                        ]
                    }
                }
            })),
        );

        let value =
            serde_json::to_value(part_json("session-1", "message-1", part)).expect("part json");

        assert_eq!(
            value["state"]["commands"],
            serde_json::json!([
                {
                    "command": "npm run build",
                    "name": "shell_command",
                    "step": 3,
                    "status": "completed"
                }
            ])
        );
    }

    #[test]
    fn api_message_from_store_maps_roles_parts_and_parent_id() {
        let message = crate::session::store::Message {
            id: "message-1".to_string(),
            session_id: "session-1".to_string(),
            role: crate::session::store::MessageRole::Assistant,
            parent_id: Some("parent-1".to_string()),
            parts: vec![store_part(
                "text",
                None,
                None,
                Some("hello"),
                Some(serde_json::json!({ "runtime_id": "hidden", "keep": "yes" })),
                None,
            )],
            created_at: 10,
            updated_at: 20,
        };

        let api = api_message_from_store(message);

        assert_eq!(api.id, "message-1");
        assert_eq!(api.session_id, "session-1");
        assert!(matches!(api.role, MessageRole::Assistant));
        assert_eq!(api.parent_id.as_deref(), Some("parent-1"));
        assert_eq!(api.created_at, 10);
        assert_eq!(api.updated_at, 20);
        assert_eq!(api.parts.len(), 1);
        assert_eq!(api.parts[0].text.as_deref(), Some("hello"));
        assert_eq!(
            api.parts[0].metadata,
            Some(serde_json::json!({ "keep": "yes" }))
        );
    }

    #[test]
    fn message_with_parts_keeps_info_and_flat_parts_in_sync() {
        let message = crate::session::store::Message {
            id: "message-2".to_string(),
            session_id: "session-2".to_string(),
            role: crate::session::store::MessageRole::User,
            parent_id: None,
            parts: vec![
                store_part("text", None, None, Some("first"), None, None),
                store_part("text", None, Some("second content"), None, None, None),
            ],
            created_at: 11,
            updated_at: 12,
        };

        let value =
            serde_json::to_value(message_with_parts_from_store(message)).expect("message json");

        assert_eq!(value["id"], "message-2");
        assert_eq!(value["sessionID"], "session-2");
        assert_eq!(value["parts"].as_array().expect("parts").len(), 2);
        assert_eq!(value["parts"][0]["text"], "first");
        assert_eq!(value["parts"][1]["text"], "second content");
    }

    #[test]
    fn command_run_request_projection_keeps_runtime_state_times_and_final_status() {
        let cases = [
            (
                "completed",
                serde_json::json!({"success": true}),
                "completed",
            ),
            ("error", serde_json::json!({"success": false}), "failed"),
            (
                "running",
                serde_json::json!({"status": "running"}),
                "running",
            ),
        ];

        for (index, (final_status, result_status, command_status)) in cases.into_iter().enumerate()
        {
            let runtime_start = 1_781_514_293_670_i64 + index as i64 * 10_000;
            let runtime_end = runtime_start + 4_321;
            let conflicting_event_start = runtime_start + 7;
            let conflicting_event_end = runtime_end - 5;
            let result = serde_json::json!({
                "step": 1,
                "command_type": "shell_command",
                "command_line": "npm test",
                "status": result_status.get("status").cloned().unwrap_or(serde_json::Value::Null),
                "success": result_status.get("success").cloned().unwrap_or(serde_json::Value::Null),
            });
            let state = serde_json::json!({
                "status": final_status,
                "input": {
                    "commands": [{
                        "step": 1,
                        "command_type": "shell_command",
                        "command_line": "npm test",
                        "started_at": runtime_start,
                    }]
                },
                "output": {
                    "streamed_command_run_result": {
                        "results": [result]
                    }
                },
                "streamed_command_run_result": {
                    "command_events": [
                        {
                            "status": "running",
                            "timestamp": conflicting_event_start,
                            "command_line": "npm test"
                        },
                        {
                            "status": "error",
                            "timestamp": conflicting_event_end,
                            "command_line": "npm test"
                        }
                    ],
                    "results": [result]
                },
                "time": {
                    "start": runtime_start,
                    "end": runtime_end
                }
            });
            let metadata = serde_json::json!({
                "kind": "mano_tool_call",
                "transient": true,
                "streaming_partial": final_status == "running",
                "output": {
                    "streamed_command_run_result": {
                        "command_events": [
                            {
                                "status": "ready",
                                "timestamp": conflicting_event_start,
                                "command_line": "npm test"
                            },
                            {
                                "status": "completed",
                                "timestamp": conflicting_event_end,
                                "command_line": "npm test"
                            }
                        ]
                    }
                }
            });
            let message = crate::session::store::Message {
                id: format!("message-{index}"),
                session_id: "session-command-run-times".to_string(),
                role: crate::session::store::MessageRole::Assistant,
                parent_id: None,
                parts: vec![store_part(
                    "tool",
                    Some("command_run"),
                    None,
                    None,
                    Some(metadata),
                    Some(state),
                )],
                created_at: runtime_start,
                updated_at: runtime_end,
            };

            let value =
                serde_json::to_value(message_with_parts_from_store(message)).expect("message json");
            let state = &value["parts"][0]["state"];

            assert_eq!(state["status"], final_status);
            assert_eq!(state["time"]["start"], runtime_start);
            assert_eq!(state["time"]["end"], runtime_end);
            assert_eq!(state["commands"][0]["status"], command_status);
        }
    }

    fn store_part(
        part_type: &str,
        tool: Option<&str>,
        content: Option<&str>,
        text: Option<&str>,
        metadata: Option<serde_json::Value>,
        state: Option<serde_json::Value>,
    ) -> crate::session::store::MessagePart {
        crate::session::store::MessagePart {
            id: format!("part-{part_type}-{}", tool.unwrap_or("none")),
            part_type: part_type.to_string(),
            content: content.map(ToString::to_string),
            text: text.map(ToString::to_string),
            metadata,
            call_id: Some("call-1".to_string()),
            tool: tool.map(ToString::to_string),
            state,
        }
    }
}
