use anyhow::{anyhow, Context, Result};
use lifecycle::{SessionCommand, SessionEvent};
use serde_json::{json, Value};
use session_log_contract::{ExecuteSessionCommandRequest, SessionLogCommand, SessionLogResponse};

pub fn take(input: &Value) -> Result<Value> {
    let session_id = normalized_session_id(input)?;
    let response = session_log::ipc::call_service(&SessionLogCommand::ExecuteSessionCommand(
        ExecuteSessionCommandRequest {
            session_id: session_id.clone(),
            session_command: SessionCommand::ConsumeQueuedUserInputs,
        },
    ))
    .with_context(|| format!("failed to consume queued inputs for session {session_id}"))?;
    consumed_inputs_response(session_id, response)
}

fn normalized_session_id(input: &Value) -> Result<String> {
    input
        .get("root_session_id")
        .or_else(|| input.get("session_id"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .ok_or_else(|| anyhow!("session.take_user_commands requires a non-empty session_id"))
}

fn consumed_inputs_response(session_id: String, response: SessionLogResponse) -> Result<Value> {
    match response {
        SessionLogResponse::SessionCommandApplied { result } => match result.event {
            SessionEvent::QueuedUserInputsConsumed { inputs } => Ok(json!({
                "ok": true,
                "session_id": session_id,
                "commands": inputs,
            })),
            event => Err(anyhow!(
                "session service returned unexpected queued-input event: {event:?}"
            )),
        },
        SessionLogResponse::Error { error } => Err(anyhow!(
            "session_db consume_queued_user_inputs failed: {error}"
        )),
        response => Err(anyhow!(
            "unexpected session_db response for consume_queued_user_inputs: {response:?}"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::{consumed_inputs_response, normalized_session_id};
    use lifecycle::{SessionAggregate, SessionEvent, SessionQuery};
    use serde_json::json;
    use session_log_contract::{SessionCommandResult, SessionLogResponse};

    #[test]
    fn root_session_id_is_required_and_takes_precedence() {
        assert_eq!(
            normalized_session_id(&json!({
                "session_id": "child",
                "root_session_id": "root"
            }))
            .expect("root id"),
            "root"
        );
        assert!(normalized_session_id(&json!({})).is_err());
    }

    #[test]
    fn consume_response_preserves_fifo_inputs() {
        let projection = SessionAggregate::new("root".to_string()).query(SessionQuery::Lifecycle);
        let response = consumed_inputs_response(
            "root".to_string(),
            SessionLogResponse::SessionCommandApplied {
                result: Box::new(SessionCommandResult {
                    event: SessionEvent::QueuedUserInputsConsumed {
                        inputs: vec!["first".to_string(), "second".to_string()],
                    },
                    projection,
                    session_name: None,
                }),
            },
        )
        .expect("consume response");
        assert_eq!(response["commands"], json!(["first", "second"]));
    }
}
