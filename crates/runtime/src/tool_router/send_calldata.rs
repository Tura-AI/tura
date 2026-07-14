use chrono::Utc;
use serde::{Deserialize, Serialize};
use tracing::info;

use crate::state_machine::agent_management::AgentId;
use crate::state_machine::runtime_management::RuntimeId;
use crate::state_machine::session_management::SessionId;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallData {
    pub session_id: SessionId,
    pub agent_id: AgentId,
    pub runtime_id: RuntimeId,
    pub tool_name: String,
    pub arguments: serde_json::Value,
    pub callback: Option<String>,
    pub created_at: chrono::DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallbackData {
    pub session_id: SessionId,
    pub agent_id: AgentId,
    pub runtime_id: RuntimeId,
    pub tool_name: String,
    pub result: serde_json::Value,
    pub success: bool,
    pub error: Option<String>,
    pub callback: Option<String>,
    pub created_at: chrono::DateTime<Utc>,
}

pub async fn send_calldata(call_data: CallData, redis_url: &str) -> Result<(), String> {
    let client = redis::Client::open(redis_url)
        .map_err(|e| format!("failed to create redis client: {e}"))?;

    let mut con = client
        .get_multiplexed_async_connection()
        .await
        .map_err(|e| format!("failed to get redis connection: {e}"))?;

    let queue_key = calldata_queue_key(&call_data.session_id);
    let payload = serialize_calldata(&call_data)?;

    redis::cmd("RPUSH")
        .arg(&queue_key)
        .arg(&payload)
        .query_async::<_, ()>(&mut con)
        .await
        .map_err(|e| format!("failed to enqueue calldata: {e}"))?;

    info!(
        session_id = %call_data.session_id,
        tool_name = %call_data.tool_name,
        "calldata sent"
    );

    Ok(())
}

pub async fn send_callback(callback_data: CallbackData, redis_url: &str) -> Result<(), String> {
    let client = redis::Client::open(redis_url)
        .map_err(|e| format!("failed to create redis client: {e}"))?;

    let mut con = client
        .get_multiplexed_async_connection()
        .await
        .map_err(|e| format!("failed to get redis connection: {e}"))?;

    let queue_key = callback_queue_key(&callback_data.session_id);
    let payload = serialize_callback_data(&callback_data)?;

    redis::cmd("RPUSH")
        .arg(&queue_key)
        .arg(&payload)
        .query_async::<_, ()>(&mut con)
        .await
        .map_err(|e| format!("failed to enqueue callback: {e}"))?;

    info!(
        session_id = %callback_data.session_id,
        tool_name = %callback_data.tool_name,
        success = callback_data.success,
        "callback sent"
    );

    Ok(())
}

pub async fn dequeue_calldata(
    session_id: &SessionId,
    redis_url: &str,
) -> Result<Option<CallData>, String> {
    let client = redis::Client::open(redis_url)
        .map_err(|e| format!("failed to create redis client: {e}"))?;

    let mut con = client
        .get_multiplexed_async_connection()
        .await
        .map_err(|e| format!("failed to get redis connection: {e}"))?;

    let queue_key = calldata_queue_key(session_id);

    let result: Option<String> = redis::cmd("LPOP")
        .arg(&queue_key)
        .query_async(&mut con)
        .await
        .map_err(|e| format!("failed to dequeue calldata: {e}"))?;

    match result {
        Some(payload) => {
            let item = deserialize_calldata(&payload)?;
            Ok(Some(item))
        }
        None => Ok(None),
    }
}

pub async fn dequeue_callback(
    session_id: &SessionId,
    redis_url: &str,
) -> Result<Option<CallbackData>, String> {
    let client = redis::Client::open(redis_url)
        .map_err(|e| format!("failed to create redis client: {e}"))?;

    let mut con = client
        .get_multiplexed_async_connection()
        .await
        .map_err(|e| format!("failed to get redis connection: {e}"))?;

    let queue_key = callback_queue_key(session_id);

    let result: Option<String> = redis::cmd("LPOP")
        .arg(&queue_key)
        .query_async(&mut con)
        .await
        .map_err(|e| format!("failed to dequeue callback: {e}"))?;

    match result {
        Some(payload) => {
            let item = deserialize_callback_data(&payload)?;
            Ok(Some(item))
        }
        None => Ok(None),
    }
}

pub fn build_callback_data(
    call_data: CallData,
    result: serde_json::Value,
    success: bool,
    error: Option<String>,
) -> CallbackData {
    CallbackData {
        session_id: call_data.session_id,
        agent_id: call_data.agent_id,
        runtime_id: call_data.runtime_id,
        tool_name: call_data.tool_name,
        result,
        success,
        error,
        callback: call_data.callback,
        created_at: Utc::now(),
    }
}

fn calldata_queue_key(session_id: &SessionId) -> String {
    format!("calldata:queue:{session_id}")
}

fn callback_queue_key(session_id: &SessionId) -> String {
    format!("callback:queue:{session_id}")
}

fn serialize_calldata(call_data: &CallData) -> Result<String, String> {
    serde_json::to_string(call_data).map_err(|e| format!("failed to serialize calldata: {e}"))
}

fn serialize_callback_data(callback_data: &CallbackData) -> Result<String, String> {
    serde_json::to_string(callback_data)
        .map_err(|e| format!("failed to serialize callback data: {e}"))
}

fn deserialize_calldata(payload: &str) -> Result<CallData, String> {
    serde_json::from_str(payload).map_err(|e| format!("failed to deserialize calldata: {e}"))
}

fn deserialize_callback_data(payload: &str) -> Result<CallbackData, String> {
    serde_json::from_str(payload).map_err(|e| format!("failed to deserialize callback: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn queue_keys_are_session_scoped_and_do_not_mix_calldata_with_callbacks() {
        let session_id = "session-abc".to_string();

        assert_eq!(
            calldata_queue_key(&session_id),
            "calldata:queue:session-abc"
        );
        assert_eq!(
            callback_queue_key(&session_id),
            "callback:queue:session-abc"
        );
        assert_ne!(
            calldata_queue_key(&session_id),
            callback_queue_key(&session_id)
        );
    }

    #[test]
    fn calldata_serialization_round_trips_nested_arguments_and_callback() {
        let call_data = call_fixture();

        let payload = serialize_calldata(&call_data).expect("serialize calldata");
        let decoded = deserialize_calldata(&payload).expect("deserialize calldata");

        assert_eq!(decoded.session_id, "session-1");
        assert_eq!(decoded.agent_id, "agent-1");
        assert_eq!(decoded.runtime_id, "runtime-1");
        assert_eq!(decoded.tool_name, "command_run");
        assert_eq!(decoded.callback.as_deref(), Some("callback://command"));
        assert_eq!(decoded.arguments["command"], "cargo test");
        assert_eq!(decoded.arguments["env"]["RUST_LOG"], "debug");
        assert_eq!(decoded.created_at, call_data.created_at);
    }

    #[test]
    fn callback_serialization_round_trips_success_and_error_shapes() {
        let callback = CallbackData {
            session_id: "session-1".to_string(),
            agent_id: "agent-1".to_string(),
            runtime_id: "runtime-1".to_string(),
            tool_name: "command_run".to_string(),
            result: serde_json::json!({ "stdout": "ok", "exit_code": 0 }),
            success: false,
            error: Some("command failed".to_string()),
            callback: Some("callback://command".to_string()),
            created_at: Utc::now(),
        };

        let payload = serialize_callback_data(&callback).expect("serialize callback");
        let decoded = deserialize_callback_data(&payload).expect("deserialize callback");

        assert_eq!(decoded.session_id, callback.session_id);
        assert_eq!(decoded.agent_id, callback.agent_id);
        assert_eq!(decoded.runtime_id, callback.runtime_id);
        assert_eq!(decoded.tool_name, callback.tool_name);
        assert!(!decoded.success);
        assert_eq!(decoded.error.as_deref(), Some("command failed"));
        assert_eq!(decoded.callback.as_deref(), Some("callback://command"));
        assert_eq!(decoded.result["stdout"], "ok");
    }

    #[test]
    fn deserializers_report_payload_shape_errors_with_operation_context() {
        assert!(deserialize_calldata("{not-json}")
            .expect_err("bad calldata")
            .starts_with("failed to deserialize calldata:"));
        assert!(deserialize_callback_data("{not-json}")
            .expect_err("bad callback")
            .starts_with("failed to deserialize callback:"));
    }

    #[test]
    fn build_callback_data_preserves_call_identity_callback_and_result() {
        let call_data = call_fixture();
        let call_created_at = call_data.created_at;

        let callback = build_callback_data(
            call_data,
            serde_json::json!({ "summary": "done" }),
            true,
            None,
        );

        assert_eq!(callback.session_id, "session-1");
        assert_eq!(callback.agent_id, "agent-1");
        assert_eq!(callback.runtime_id, "runtime-1");
        assert_eq!(callback.tool_name, "command_run");
        assert_eq!(callback.callback.as_deref(), Some("callback://command"));
        assert_eq!(callback.result["summary"], "done");
        assert!(callback.success);
        assert_eq!(callback.error, None);
        assert!(callback.created_at >= call_created_at);
    }

    #[tokio::test]
    async fn redis_entry_points_report_invalid_client_url_before_network_io() {
        let call_data = call_fixture();
        let callback_data = build_callback_data(
            call_data.clone(),
            serde_json::json!({}),
            false,
            Some("bad".to_string()),
        );

        let send_call = send_calldata(call_data, "not a redis url")
            .await
            .expect_err("invalid redis url");
        assert!(send_call.starts_with("failed to create redis client:"));

        let send_callback = send_callback(callback_data, "not a redis url")
            .await
            .expect_err("invalid redis url");
        assert!(send_callback.starts_with("failed to create redis client:"));

        let dequeue_call = dequeue_calldata(&"session-1".to_string(), "not a redis url")
            .await
            .expect_err("invalid redis url");
        assert!(dequeue_call.starts_with("failed to create redis client:"));

        let dequeue_callback = dequeue_callback(&"session-1".to_string(), "not a redis url")
            .await
            .expect_err("invalid redis url");
        assert!(dequeue_callback.starts_with("failed to create redis client:"));
    }

    fn call_fixture() -> CallData {
        CallData {
            session_id: "session-1".to_string(),
            agent_id: "agent-1".to_string(),
            runtime_id: "runtime-1".to_string(),
            tool_name: "command_run".to_string(),
            arguments: serde_json::json!({
                "command": "cargo test",
                "env": {
                    "RUST_LOG": "debug"
                },
                "args": ["-p", "runtime"]
            }),
            callback: Some("callback://command".to_string()),
            created_at: Utc::now(),
        }
    }
}
