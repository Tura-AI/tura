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

#[cfg(test)]
mod tests {
    use super::{SessionWriteQueueItem, SESSION_WRITE_QUEUE_TABLE};
    use serde_json::json;

    #[test]
    fn queue_item_round_trips_all_idempotency_and_retry_fields() {
        let item = SessionWriteQueueItem {
            id: Some(42),
            idempotency_key: "session:turn:worker:run:cmd:1:status".to_string(),
            session_id: "session".to_string(),
            turn_id: Some("turn".to_string()),
            runtime_worker_id: Some("worker".to_string()),
            command_run_id: Some("run".to_string()),
            command_id: Some("cmd".to_string()),
            event_seq: Some(1),
            event_type: "checkpoint.apply".to_string(),
            payload_json: json!({ "status": "command_finished" }),
            status: "pending".to_string(),
            retry_count: 2,
            created_at: Some("2026-06-11T00:00:00Z".to_string()),
            applied_at: None,
            last_error: Some("locked".to_string()),
        };

        let encoded = serde_json::to_string(&item).expect("encode queue item");
        let decoded: SessionWriteQueueItem =
            serde_json::from_str(&encoded).expect("decode queue item");

        assert_eq!(decoded.id, Some(42));
        assert_eq!(decoded.idempotency_key, item.idempotency_key);
        assert_eq!(decoded.event_type, "checkpoint.apply");
        assert_eq!(decoded.payload_json["status"], "command_finished");
        assert_eq!(decoded.retry_count, 2);
        assert_eq!(decoded.last_error.as_deref(), Some("locked"));
    }

    #[test]
    fn queue_table_name_is_stable_for_migrations_and_diagnostics() {
        assert_eq!(SESSION_WRITE_QUEUE_TABLE, "session_write_queue");
    }
}
