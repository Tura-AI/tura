use crate::checkpoint::CommandCheckpoint;
use crate::path::normalize_workspace;
use crate::protocol::{
    GetSessionRequest, ListSessionRecordsRequest, ListSessionsRequest, Page, SessionRecord,
    SessionSnapshot, UpsertSessionRequest, WorkspaceSummary,
};
use anyhow::{Context, Result};
use postgres::{Client, NoTls, Row};
use r2d2::Pool;
use r2d2_postgres::PostgresConnectionManager;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

type PgPool = Pool<PostgresConnectionManager<NoTls>>;

#[derive(Clone)]
pub struct SessionLogStore {
    pool: PgPool,
    data_dir: PathBuf,
}

impl SessionLogStore {
    pub fn open_default() -> Result<Self> {
        Self::open(
            crate::path::default_db_dir(),
            &crate::local_postgres::database_url()?,
        )
    }

    pub fn open(data_dir: impl AsRef<Path>, database_url: &str) -> Result<Self> {
        let data_dir = data_dir.as_ref().to_path_buf();
        std::fs::create_dir_all(&data_dir)
            .with_context(|| format!("failed to create db directory {}", data_dir.display()))?;

        let started = Instant::now();
        let mut delay = Duration::from_millis(50);
        loop {
            match Self::open_once(data_dir.clone(), database_url) {
                Ok(store) => return Ok(store),
                Err(err) if started.elapsed() < Duration::from_secs(30) => {
                    tracing::debug!(error = %err, "retrying PostgreSQL session_log open");
                    std::thread::sleep(delay);
                    delay = (delay * 2).min(Duration::from_millis(750));
                }
                Err(err) => return Err(err),
            }
        }
    }

    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    pub fn replay_pending_write_queue(&self) -> Result<u64> {
        let mut client = self.pool.get()?;
        let rows = client.query(
            "SELECT id, event_type, payload_json::TEXT
             FROM session_write_queue
             WHERE status = 'pending'
             ORDER BY id
             LIMIT 1000",
            &[],
        )?;
        drop(client);

        let mut applied = 0;
        for row in rows {
            let id: i64 = row.get(0);
            let event_type: String = row.get(1);
            let payload_json: String = row.get(2);
            match self.apply_queue_item(&event_type, &payload_json) {
                Ok(()) => {
                    let mut client = self.pool.get()?;
                    client.execute(
                        "UPDATE session_write_queue
                         SET status = 'applied', applied_at = NOW(), last_error = NULL
                         WHERE id = $1",
                        &[&id],
                    )?;
                    applied += 1;
                }
                Err(error) => {
                    let mut client = self.pool.get()?;
                    client.execute(
                        "UPDATE session_write_queue
                         SET retry_count = retry_count + 1, last_error = $2
                         WHERE id = $1",
                        &[&id, &error.to_string()],
                    )?;
                    return Err(error);
                }
            }
        }
        Ok(applied)
    }

    fn apply_queue_item(&self, event_type: &str, payload_json: &str) -> Result<()> {
        match event_type {
            "upsert_session" | "session.upsert" => {
                let request: UpsertSessionRequest = serde_json::from_str(payload_json)?;
                self.upsert_session(request)
            }
            other => anyhow::bail!("unsupported session_write_queue event_type: {other}"),
        }
    }

    pub fn upsert_session(&self, request: UpsertSessionRequest) -> Result<()> {
        let mut client = self.pool.get()?;
        let mut tx = client.transaction()?;

        let session_id = string_at(&request.session, &["id"])
            .or_else(|| string_at(&request.session, &["management", "session_id"]))
            .context("session id missing")?;
        let workspace = string_at(&request.session, &["directory"])
            .or_else(|| string_at(&request.session, &["management", "session_directory"]))
            .unwrap_or_default();
        let workspace = normalize_workspace(&workspace);
        let management = request
            .session
            .get("management")
            .cloned()
            .unwrap_or(Value::Null);
        let task_management = request
            .session
            .get("task_management")
            .cloned()
            .or_else(|| management_task_management(&management))
            .unwrap_or_else(|| serde_json::json!({}));
        let created_at = i64_at(&request.session, &["created_at"])
            .or_else(|| millis_at(&management, &["session_created_at"]))
            .unwrap_or_default();
        let updated_at = i64_at(&request.session, &["updated_at"])
            .or_else(|| millis_at(&management, &["session_last_update_at"]))
            .unwrap_or(created_at);
        let message_count = request.messages.len() as i64;
        let name = string_at(&request.session, &["name"])
            .or_else(|| string_at(&management, &["session_name"]));
        let parent_id = request
            .parent_id
            .or_else(|| string_at(&request.session, &["parent_id"]));
        let state = string_at(&management, &["state"]);
        let status = string_at(&request.session, &["status"]);
        let task_management_json = serde_json::to_string(&task_management)?;
        let management_json = serde_json::to_string(&management)?;
        let session_json = serde_json::to_string(&request.session)?;
        let todos_json = serde_json::to_string(&request.todos)?;

        tx.execute(
            "INSERT INTO sessions(
                session_id, workspace, name, parent_id, created_at, updated_at,
                state, status, message_count, task_management_json, management_json, session_json,
                todos_json
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
            ON CONFLICT(session_id) DO UPDATE SET
                workspace=EXCLUDED.workspace,
                name=EXCLUDED.name,
                parent_id=EXCLUDED.parent_id,
                created_at=EXCLUDED.created_at,
                updated_at=EXCLUDED.updated_at,
                state=EXCLUDED.state,
                status=EXCLUDED.status,
                message_count=EXCLUDED.message_count,
                task_management_json=EXCLUDED.task_management_json,
                management_json=EXCLUDED.management_json,
                session_json=EXCLUDED.session_json,
                todos_json=EXCLUDED.todos_json",
            &[
                &session_id,
                &workspace,
                &name,
                &parent_id,
                &created_at,
                &updated_at,
                &state,
                &status,
                &message_count,
                &task_management_json,
                &management_json,
                &session_json,
                &todos_json,
            ],
        )?;

        tx.execute(
            "DELETE FROM session_records WHERE session_id = $1",
            &[&session_id],
        )?;
        let insert_record = tx.prepare(
            "INSERT INTO session_records(
                session_id, message_id, role, created_at, updated_at, record_json
            ) VALUES ($1, $2, $3, $4, $5, $6)",
        )?;
        for message in request.messages {
            let created = i64_at(&message, &["created_at"]).unwrap_or_default();
            let message_id =
                string_at(&message, &["id"]).unwrap_or_else(|| format!("{session_id}:{created}"));
            let role = string_at(&message, &["role"]).unwrap_or_default();
            let updated = i64_at(&message, &["updated_at"]).unwrap_or(created);
            let record_json = serde_json::to_string(&message)?;
            tx.execute(
                &insert_record,
                &[
                    &session_id,
                    &message_id,
                    &role,
                    &created,
                    &updated,
                    &record_json,
                ],
            )?;
        }

        tx.commit()?;
        Ok(())
    }

    pub fn apply_command_checkpoint(&self, checkpoint: CommandCheckpoint) -> Result<()> {
        let mut client = self.pool.get()?;
        let idempotency_key = checkpoint.idempotency_key();
        let payload_json = serde_json::to_string(&checkpoint)?;
        client.execute(
            "INSERT INTO session_write_queue(
                idempotency_key, session_id, turn_id, runtime_worker_id,
                command_run_id, command_id, event_seq, event_type, payload_json,
                status, retry_count, applied_at
            ) VALUES (
                $1::TEXT, $2::TEXT, $3::TEXT, $4::TEXT, $5::TEXT,
                $6::TEXT, $7::BIGINT, $8::TEXT, $9::TEXT::JSONB,
                'applied', 0, NOW()
            )
            ON CONFLICT(idempotency_key) DO UPDATE SET
                payload_json=EXCLUDED.payload_json,
                status='applied',
                applied_at=NOW(),
                last_error=NULL",
            &[
                &idempotency_key,
                &checkpoint.session_id,
                &checkpoint.turn_id,
                &checkpoint.runtime_worker_id,
                &checkpoint.command_run_id,
                &checkpoint.command_id,
                &checkpoint.event_seq,
                &checkpoint.status,
                &payload_json,
            ],
        )?;
        Ok(())
    }

    pub fn mark_running_sessions_interrupted(&self) -> Result<u64> {
        let mut client = self.pool.get()?;
        let now_ms = chrono::Utc::now().timestamp_millis();
        let updated = client.execute(
            "UPDATE sessions
             SET
                state = 'interrupted',
                status = 'interrupted',
                updated_at = GREATEST(updated_at, $1),
                session_json = jsonb_set(
                    COALESCE(session_json::JSONB, '{}'::JSONB),
                    '{status}',
                    '\"interrupted\"'::JSONB,
                    true
                )::TEXT,
                management_json = jsonb_set(
                    COALESCE(management_json::JSONB, '{}'::JSONB),
                    '{state}',
                    '\"interrupted\"'::JSONB,
                    true
                )::TEXT
             WHERE
                COALESCE(status, '') IN ('busy', 'running')
                OR COALESCE(state, '') IN ('busy', 'running', 'Running')",
            &[&now_ms],
        )?;
        Ok(updated)
    }

    pub fn list_workspaces(&self) -> Result<Vec<WorkspaceSummary>> {
        self.with_client(|client| {
            let rows = client.query(
                "SELECT workspace, COUNT(*)::BIGINT, COALESCE(MAX(updated_at), 0)
                 FROM sessions
                 WHERE workspace != ''
                 GROUP BY workspace
                 ORDER BY MAX(updated_at) DESC, workspace ASC",
                &[],
            )?;
            Ok(rows
                .into_iter()
                .map(|row| WorkspaceSummary {
                    directory: row.get(0),
                    session_count: row.get::<_, i64>(1) as u64,
                    last_updated_at: row.get(2),
                })
                .collect())
        })
    }

    pub fn list_sessions(
        &self,
        request: ListSessionsRequest,
    ) -> Result<(Page, Vec<SessionSnapshot>)> {
        let workspace = normalize_workspace(&request.workspace);
        let page_size = request.page_size.clamp(1, 500);
        self.with_client(|client| {
            let total = client.query_one(
                "SELECT COUNT(*)::BIGINT FROM sessions WHERE workspace = $1",
                &[&workspace],
            )?;
            let total = total.get::<_, i64>(0) as u64;
            let page = bounded_page(request.page, page_size, total, false);
            let rows = client.query(
                "SELECT session_id, workspace, name, parent_id, created_at, updated_at,
                        state, status, message_count, task_management_json, management_json,
                        session_json, todos_json
                 FROM sessions
                 WHERE workspace = $1
                 ORDER BY updated_at DESC, session_id ASC
                 LIMIT $2 OFFSET $3",
                &[
                    &workspace,
                    &(page_size as i64),
                    &((page * page_size) as i64),
                ],
            )?;
            let sessions = rows.into_iter().map(session_snapshot_from_row).collect();
            Ok((
                Page {
                    page,
                    page_size,
                    total,
                },
                sessions,
            ))
        })
    }

    pub fn get_session(&self, request: GetSessionRequest) -> Result<Option<SessionSnapshot>> {
        self.with_client(|client| {
            let row = client.query_opt(
                "SELECT session_id, workspace, name, parent_id, created_at, updated_at,
                        state, status, message_count, task_management_json, management_json,
                        session_json, todos_json
                 FROM sessions
                 WHERE session_id = $1",
                &[&request.session_id],
            )?;
            Ok(row.map(session_snapshot_from_row))
        })
    }

    pub fn list_session_records(
        &self,
        request: ListSessionRecordsRequest,
    ) -> Result<(Page, Vec<SessionRecord>)> {
        let page_size = request.page_size.clamp(1, 500);
        self.with_client(|client| {
            let total = client.query_one(
                "SELECT COUNT(*)::BIGINT FROM session_records WHERE session_id = $1",
                &[&request.session_id],
            )?;
            let total = total.get::<_, i64>(0) as u64;
            let page = bounded_page(request.page, page_size, total, true);
            let rows = client.query(
                "SELECT session_id, message_id, role, created_at, updated_at, record_json
                 FROM session_records
                 WHERE session_id = $1
                 ORDER BY created_at ASC, id ASC
                 LIMIT $2 OFFSET $3",
                &[
                    &request.session_id,
                    &(page_size as i64),
                    &((page * page_size) as i64),
                ],
            )?;
            let records = rows
                .into_iter()
                .map(|row| {
                    let record_json: String = row.get(5);
                    SessionRecord {
                        session_id: row.get(0),
                        message_id: row.get(1),
                        role: row.get(2),
                        created_at: row.get(3),
                        updated_at: row.get(4),
                        record: serde_json::from_str(&record_json).unwrap_or(Value::Null),
                    }
                })
                .collect();
            Ok((
                Page {
                    page,
                    page_size,
                    total,
                },
                records,
            ))
        })
    }

    fn with_client<T>(&self, f: impl FnOnce(&mut Client) -> Result<T>) -> Result<T> {
        let mut client = self.pool.get()?;
        f(&mut client)
    }

    fn open_once(data_dir: PathBuf, database_url: &str) -> Result<Self> {
        let manager = PostgresConnectionManager::new(database_url.parse()?, NoTls);
        let pool = Pool::builder()
            .max_size(16)
            .min_idle(Some(1))
            .connection_timeout(Duration::from_secs(5))
            .build(manager)
            .context("failed to create PostgreSQL session_log pool")?;

        let store = Self { pool, data_dir };
        store.with_client(init)?;
        Ok(store)
    }
}

fn init(client: &mut Client) -> Result<()> {
    client.batch_execute(
        "
        CREATE TABLE IF NOT EXISTS sessions (
            session_id TEXT PRIMARY KEY,
            workspace TEXT NOT NULL,
            name TEXT,
            parent_id TEXT,
            created_at BIGINT NOT NULL,
            updated_at BIGINT NOT NULL,
            state TEXT,
            status TEXT,
            message_count BIGINT NOT NULL DEFAULT 0,
            task_management_json TEXT NOT NULL,
            management_json TEXT NOT NULL,
            session_json TEXT NOT NULL
        );
        ALTER TABLE sessions
            ADD COLUMN IF NOT EXISTS todos_json TEXT NOT NULL DEFAULT '[]';
        CREATE INDEX IF NOT EXISTS idx_sessions_workspace_updated
            ON sessions(workspace, updated_at DESC, session_id);
        CREATE INDEX IF NOT EXISTS idx_sessions_parent
            ON sessions(parent_id);
        CREATE TABLE IF NOT EXISTS session_records (
            id BIGSERIAL PRIMARY KEY,
            session_id TEXT NOT NULL REFERENCES sessions(session_id) ON DELETE CASCADE,
            message_id TEXT NOT NULL,
            role TEXT NOT NULL,
            created_at BIGINT NOT NULL,
            updated_at BIGINT NOT NULL,
            record_json TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_records_session_created
            ON session_records(session_id, created_at, id);
        ",
    )?;
    client.batch_execute(crate::migrations::SESSION_WRITE_QUEUE_MIGRATION)?;
    Ok(())
}

fn session_snapshot_from_row(row: Row) -> SessionSnapshot {
    let task_json: String = row.get(9);
    let management_json: String = row.get(10);
    let session_json: String = row.get(11);
    let todos_json: String = row.get(12);
    SessionSnapshot {
        session_id: row.get(0),
        workspace: row.get(1),
        name: row.get(2),
        parent_id: row.get(3),
        created_at: row.get(4),
        updated_at: row.get(5),
        state: row.get(6),
        status: row.get(7),
        message_count: row.get::<_, i64>(8) as u64,
        task_management: serde_json::from_str(&task_json).unwrap_or(Value::Null),
        management: serde_json::from_str(&management_json).unwrap_or(Value::Null),
        session: serde_json::from_str(&session_json).unwrap_or(Value::Null),
        todos: serde_json::from_str(&todos_json).unwrap_or_default(),
    }
}

fn bounded_page(requested: u64, page_size: u64, total: u64, zero_means_last: bool) -> u64 {
    if total == 0 {
        return 0;
    }
    let last = (total - 1) / page_size;
    if zero_means_last && requested == 0 {
        return last;
    }
    requested.min(last)
}

fn string_at(value: &Value, path: &[&str]) -> Option<String> {
    path.iter()
        .try_fold(value, |current, key| current.get(*key))
        .and_then(Value::as_str)
        .map(ToString::to_string)
}

fn i64_at(value: &Value, path: &[&str]) -> Option<i64> {
    path.iter()
        .try_fold(value, |current, key| current.get(*key))
        .and_then(Value::as_i64)
}

fn millis_at(value: &Value, path: &[&str]) -> Option<i64> {
    string_at(value, path).and_then(|text| {
        chrono::DateTime::parse_from_rfc3339(&text)
            .ok()
            .map(|value| value.timestamp_millis())
    })
}

fn management_task_management(management: &Value) -> Option<Value> {
    let task_plan = management.get("task_plan")?;
    let tasks = task_plan
        .get("detailed_tasks")
        .cloned()
        .unwrap_or(Value::Null);
    Some(serde_json::json!({
        "plan_summary": task_plan.get("plan_summary").cloned().unwrap_or(Value::String(String::new())),
        "tasks": tasks,
    }))
}
