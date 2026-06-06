//! Session DB migrations, including the durable write queue table.

pub const SESSION_WRITE_QUEUE_MIGRATION: &str = r#"
CREATE TABLE IF NOT EXISTS session_write_queue (
  id BIGSERIAL PRIMARY KEY,
  idempotency_key TEXT NOT NULL UNIQUE,
  session_id TEXT NOT NULL,
  turn_id TEXT NULL,
  runtime_worker_id TEXT NULL,
  command_run_id TEXT NULL,
  command_id TEXT NULL,
  event_seq BIGINT NULL,
  event_type TEXT NOT NULL,
  payload_json JSONB NOT NULL,
  status TEXT NOT NULL,
  retry_count INTEGER NOT NULL DEFAULT 0,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  applied_at TIMESTAMPTZ NULL,
  last_error TEXT NULL
);
"#;
