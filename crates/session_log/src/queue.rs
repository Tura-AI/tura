//! Durable session write queue schema and replay entry points.
//!
//! Queue storage belongs in session_log/session_db, never router memory.

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionWriteQueueItem {
    pub id: Option<i64>,
    pub idempotency_key: String,
    pub session_id: String,
    pub turn_id: Option<String>,
    pub runtime_worker_id: Option<String>,
    pub command_run_id: Option<String>,
    pub command_id: Option<String>,
    pub event_seq: Option<i64>,
    pub event_type: String,
    pub payload_json: Value,
    pub status: String,
    pub retry_count: i32,
    pub created_at: Option<String>,
    pub applied_at: Option<String>,
    pub last_error: Option<String>,
}

pub const SESSION_WRITE_QUEUE_TABLE: &str = "session_write_queue";
