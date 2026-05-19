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
        .map_err(|e| format!("failed to create redis client: {}", e))?;

    let mut con = client
        .get_multiplexed_async_connection()
        .await
        .map_err(|e| format!("failed to get redis connection: {}", e))?;

    let queue_key = format!("calldata:queue:{}", call_data.session_id);

    let payload = serde_json::to_string(&call_data)
        .map_err(|e| format!("failed to serialize calldata: {}", e))?;

    redis::cmd("RPUSH")
        .arg(&queue_key)
        .arg(&payload)
        .query_async::<_, ()>(&mut con)
        .await
        .map_err(|e| format!("failed to enqueue calldata: {}", e))?;

    info!(
        session_id = %call_data.session_id,
        tool_name = %call_data.tool_name,
        "calldata sent"
    );

    Ok(())
}

pub async fn send_callback(callback_data: CallbackData, redis_url: &str) -> Result<(), String> {
    let client = redis::Client::open(redis_url)
        .map_err(|e| format!("failed to create redis client: {}", e))?;

    let mut con = client
        .get_multiplexed_async_connection()
        .await
        .map_err(|e| format!("failed to get redis connection: {}", e))?;

    let queue_key = format!("callback:queue:{}", callback_data.session_id);

    let payload = serde_json::to_string(&callback_data)
        .map_err(|e| format!("failed to serialize callback data: {}", e))?;

    redis::cmd("RPUSH")
        .arg(&queue_key)
        .arg(&payload)
        .query_async::<_, ()>(&mut con)
        .await
        .map_err(|e| format!("failed to enqueue callback: {}", e))?;

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
        .map_err(|e| format!("failed to create redis client: {}", e))?;

    let mut con = client
        .get_multiplexed_async_connection()
        .await
        .map_err(|e| format!("failed to get redis connection: {}", e))?;

    let queue_key = format!("calldata:queue:{}", session_id);

    let result: Option<String> = redis::cmd("LPOP")
        .arg(&queue_key)
        .query_async(&mut con)
        .await
        .map_err(|e| format!("failed to dequeue calldata: {}", e))?;

    match result {
        Some(payload) => {
            let item: CallData = serde_json::from_str(&payload)
                .map_err(|e| format!("failed to deserialize calldata: {}", e))?;
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
        .map_err(|e| format!("failed to create redis client: {}", e))?;

    let mut con = client
        .get_multiplexed_async_connection()
        .await
        .map_err(|e| format!("failed to get redis connection: {}", e))?;

    let queue_key = format!("callback:queue:{}", session_id);

    let result: Option<String> = redis::cmd("LPOP")
        .arg(&queue_key)
        .query_async(&mut con)
        .await
        .map_err(|e| format!("failed to dequeue callback: {}", e))?;

    match result {
        Some(payload) => {
            let item: CallbackData = serde_json::from_str(&payload)
                .map_err(|e| format!("failed to deserialize callback: {}", e))?;
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
