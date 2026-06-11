use crate::checkpoint::CommandCheckpoint;
use crate::path::{normalize_workspace, workspace_session_log_db};
use crate::protocol::{
    DeleteSessionRequest, DeleteWorkspaceRequest, GetSessionRequest, ListSessionRecordsRequest,
    ListSessionsRequest, Page, SessionRecord, SessionSnapshot, UpsertSessionRequest,
    WorkspaceSummary,
};
use anyhow::{Context, Result};
use fs2::FileExt;
use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

const INDEX_DB_FILE: &str = "index.sqlite3";

#[derive(Clone)]
pub struct SessionLogStore {
    data_dir: PathBuf,
    index_db_path: PathBuf,
}

impl SessionLogStore {
    pub fn open_default() -> Result<Self> {
        Self::open(crate::path::default_db_dir())
    }

    pub fn open(data_dir: impl AsRef<Path>) -> Result<Self> {
        let data_dir = data_dir.as_ref().to_path_buf();
        std::fs::create_dir_all(&data_dir)
            .with_context(|| format!("failed to create db directory {}", data_dir.display()))?;
        let store = Self {
            index_db_path: data_dir.join(INDEX_DB_FILE),
            data_dir,
        };
        store.with_index_connection(|conn| init_index_db(conn))?;
        Ok(store)
    }

    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    pub fn replay_pending_write_queue(&self) -> Result<u64> {
        let rows = self.with_index_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, event_type, payload_json
                 FROM session_write_queue
                 WHERE status = 'pending'
                 ORDER BY id
                 LIMIT 1000",
            )?;
            let rows = stmt
                .query_map([], |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                    ))
                })?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            Ok(rows)
        })?;

        let mut applied = 0;
        for (id, event_type, payload_json) in rows {
            match self.apply_queue_item(&event_type, &payload_json) {
                Ok(()) => {
                    let now_ms = chrono::Utc::now().timestamp_millis();
                    self.with_index_connection(|conn| {
                        conn.execute(
                            "UPDATE session_write_queue
                             SET status = 'applied', applied_at = ?2, last_error = NULL
                             WHERE id = ?1",
                            params![id, now_ms],
                        )?;
                        Ok(())
                    })?;
                    applied += 1;
                }
                Err(error) => {
                    self.with_index_connection(|conn| {
                        conn.execute(
                            "UPDATE session_write_queue
                             SET retry_count = retry_count + 1, last_error = ?2
                             WHERE id = ?1",
                            params![id, error.to_string()],
                        )?;
                        Ok(())
                    })?;
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
        let session_id = string_at(&request.session, &["id"])
            .or_else(|| string_at(&request.session, &["management", "session_id"]))
            .context("session id missing")?;
        let workspace = string_at(&request.session, &["directory"])
            .or_else(|| string_at(&request.session, &["management", "session_directory"]))
            .unwrap_or_default();
        let workspace = normalize_workspace(&workspace);
        let workspace_db = workspace_session_log_db(&workspace);
        let workspace_db_text = path_text(&workspace_db);

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

        self.with_workspace_connection(&workspace_db, |conn| {
            let tx = conn.transaction()?;
            tx.execute(
                "INSERT INTO sessions(
                    session_id, workspace, name, parent_id, created_at, updated_at,
                    state, status, message_count, task_management_json, management_json,
                    session_json, todos_json
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
                ON CONFLICT(session_id) DO UPDATE SET
                    workspace=excluded.workspace,
                    name=excluded.name,
                    parent_id=excluded.parent_id,
                    created_at=excluded.created_at,
                    updated_at=excluded.updated_at,
                    state=excluded.state,
                    status=excluded.status,
                    message_count=excluded.message_count,
                    task_management_json=excluded.task_management_json,
                    management_json=excluded.management_json,
                    session_json=excluded.session_json,
                    todos_json=excluded.todos_json",
                params![
                    session_id,
                    workspace,
                    name,
                    parent_id,
                    created_at,
                    updated_at,
                    state,
                    status,
                    message_count,
                    task_management_json,
                    management_json,
                    session_json,
                    todos_json,
                ],
            )?;

            tx.execute(
                "DELETE FROM session_records WHERE session_id = ?1",
                params![session_id],
            )?;
            {
                let mut stmt = tx.prepare(
                    "INSERT INTO session_records(
                        session_id, message_id, role, created_at, updated_at, record_json
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                )?;
                for message in request.messages {
                    let created = i64_at(&message, &["created_at"]).unwrap_or_default();
                    let message_id = string_at(&message, &["id"])
                        .unwrap_or_else(|| format!("{session_id}:{created}"));
                    let role = string_at(&message, &["role"]).unwrap_or_default();
                    let updated = i64_at(&message, &["updated_at"]).unwrap_or(created);
                    let record_json = serde_json::to_string(&message)?;
                    stmt.execute(params![
                        session_id,
                        message_id,
                        role,
                        created,
                        updated,
                        record_json,
                    ])?;
                }
            }
            tx.commit()?;
            Ok(())
        })?;

        self.with_index_connection(|conn| {
            conn.execute(
                "INSERT INTO sessions(
                    session_id, workspace, workspace_db_path, name, parent_id, created_at,
                    updated_at, state, status, message_count, task_management_json, management_json
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
                ON CONFLICT(session_id) DO UPDATE SET
                    workspace=excluded.workspace,
                    workspace_db_path=excluded.workspace_db_path,
                    name=excluded.name,
                    parent_id=excluded.parent_id,
                    created_at=excluded.created_at,
                    updated_at=excluded.updated_at,
                    state=excluded.state,
                    status=excluded.status,
                    message_count=excluded.message_count,
                    task_management_json=excluded.task_management_json,
                    management_json=excluded.management_json",
                params![
                    session_id,
                    workspace,
                    workspace_db_text,
                    name,
                    parent_id,
                    created_at,
                    updated_at,
                    state,
                    status,
                    message_count,
                    task_management_json,
                    management_json,
                ],
            )?;
            Ok(())
        })?;

        Ok(())
    }

    pub fn apply_command_checkpoint(&self, checkpoint: CommandCheckpoint) -> Result<()> {
        let idempotency_key = checkpoint.idempotency_key();
        let payload_json = serde_json::to_string(&checkpoint)?;
        let now_ms = chrono::Utc::now().timestamp_millis();
        self.with_index_connection(|conn| {
            conn.execute(
                "INSERT INTO session_write_queue(
                    idempotency_key, session_id, turn_id, runtime_worker_id,
                    command_run_id, command_id, event_seq, event_type, payload_json,
                    status, retry_count, created_at, applied_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 'applied', 0, ?10, ?10)
                ON CONFLICT(idempotency_key) DO UPDATE SET
                    payload_json=excluded.payload_json,
                    status='applied',
                    applied_at=excluded.applied_at,
                    last_error=NULL",
                params![
                    idempotency_key,
                    checkpoint.session_id,
                    checkpoint.turn_id,
                    checkpoint.runtime_worker_id,
                    checkpoint.command_run_id,
                    checkpoint.command_id,
                    checkpoint.event_seq,
                    checkpoint.status,
                    payload_json,
                    now_ms,
                ],
            )?;
            Ok(())
        })
    }

    pub fn mark_running_sessions_interrupted(&self) -> Result<u64> {
        let now_ms = chrono::Utc::now().timestamp_millis();
        let affected = self.with_index_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT session_id, workspace_db_path
                 FROM sessions
                 WHERE
                    COALESCE(status, '') IN ('busy', 'running')
                    OR COALESCE(state, '') IN ('busy', 'running', 'Running')",
            )?;
            let rows = stmt
                .query_map([], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                })?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            conn.execute(
                "UPDATE sessions
                 SET state = 'interrupted',
                     status = 'interrupted',
                     updated_at = MAX(updated_at, ?1)
                 WHERE
                    COALESCE(status, '') IN ('busy', 'running')
                    OR COALESCE(state, '') IN ('busy', 'running', 'Running')",
                params![now_ms],
            )?;
            Ok(rows)
        })?;

        for (session_id, workspace_db_path) in &affected {
            mark_workspace_session_interrupted(Path::new(workspace_db_path), session_id, now_ms)?;
        }
        Ok(affected.len() as u64)
    }

    pub fn delete_session(&self, request: DeleteSessionRequest) -> Result<()> {
        let workspace_db_path = self.with_index_connection(|conn| {
            conn.query_row(
                "SELECT workspace_db_path FROM sessions WHERE session_id = ?1",
                params![request.session_id],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(Into::into)
        })?;
        if let Some(path) = workspace_db_path.as_deref().map(Path::new) {
            if path.exists() {
                with_connection(path, init_workspace_db, |conn| {
                    conn.execute(
                        "DELETE FROM sessions WHERE session_id = ?1",
                        params![request.session_id],
                    )?;
                    Ok(())
                })?;
            }
        }
        self.delete_index_session(&request.session_id)
    }

    pub fn delete_workspace(&self, request: DeleteWorkspaceRequest) -> Result<()> {
        let workspace = normalize_workspace(&request.workspace);
        let db_paths = self.with_index_connection(|conn| {
            let mut stmt = conn
                .prepare("SELECT DISTINCT workspace_db_path FROM sessions WHERE workspace = ?1")?;
            let paths = stmt
                .query_map(params![workspace], |row| row.get::<_, String>(0))?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            conn.execute(
                "DELETE FROM sessions WHERE workspace = ?1",
                params![workspace],
            )?;
            Ok(paths)
        })?;
        for path in db_paths {
            remove_sqlite_files(Path::new(&path))?;
        }
        Ok(())
    }

    pub fn list_workspaces(&self) -> Result<Vec<WorkspaceSummary>> {
        self.sweep_missing_workspace_dbs()?;
        self.with_index_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT workspace, COUNT(*), COALESCE(MAX(updated_at), 0)
                 FROM sessions
                 WHERE workspace != ''
                 GROUP BY workspace
                 ORDER BY MAX(updated_at) DESC, workspace ASC",
            )?;
            let rows = stmt
                .query_map([], |row| {
                    Ok(WorkspaceSummary {
                        directory: row.get(0)?,
                        session_count: row.get::<_, i64>(1)? as u64,
                        last_updated_at: row.get(2)?,
                    })
                })?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            Ok(rows)
        })
    }

    pub fn list_sessions(
        &self,
        request: ListSessionsRequest,
    ) -> Result<(Page, Vec<SessionSnapshot>)> {
        self.sweep_missing_workspace_dbs()?;
        let workspace = normalize_workspace(&request.workspace);
        let page_size = request.page_size.clamp(1, 500);
        let (page, total, index_rows) = self.with_index_connection(|conn| {
            let total = conn.query_row(
                "SELECT COUNT(*) FROM sessions WHERE workspace = ?1",
                params![workspace],
                |row| row.get::<_, i64>(0),
            )? as u64;
            let page = bounded_page(request.page, page_size, total, false);
            let mut stmt = conn.prepare(
                "SELECT session_id, workspace, workspace_db_path, name, parent_id, created_at,
                        updated_at, state, status, message_count, task_management_json,
                        management_json
                 FROM sessions
                 WHERE workspace = ?1
                 ORDER BY updated_at DESC, session_id ASC
                 LIMIT ?2 OFFSET ?3",
            )?;
            let index_rows = stmt
                .query_map(
                    params![workspace, page_size as i64, (page * page_size) as i64],
                    index_session_from_row,
                )?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            Ok((page, total, index_rows))
        })?;
        let sessions = index_rows
            .into_iter()
            .filter_map(|row| match self.snapshot_from_index_row(row) {
                Ok(Some(snapshot)) => Some(Ok(snapshot)),
                Ok(None) => None,
                Err(error) => Some(Err(error)),
            })
            .collect::<Result<Vec<_>>>()?;
        Ok((
            Page {
                page,
                page_size,
                total,
            },
            sessions,
        ))
    }

    pub fn get_session(&self, request: GetSessionRequest) -> Result<Option<SessionSnapshot>> {
        let row = self.with_index_connection(|conn| {
            conn.query_row(
                "SELECT session_id, workspace, workspace_db_path, name, parent_id, created_at,
                        updated_at, state, status, message_count, task_management_json,
                        management_json
                 FROM sessions
                 WHERE session_id = ?1",
                params![request.session_id],
                index_session_from_row,
            )
            .optional()
            .map_err(Into::into)
        })?;
        row.map(|row| self.snapshot_from_index_row(row))
            .transpose()
            .map(Option::flatten)
    }

    pub fn list_session_records(
        &self,
        request: ListSessionRecordsRequest,
    ) -> Result<(Page, Vec<SessionRecord>)> {
        let workspace_db_path = self.with_index_connection(|conn| {
            conn.query_row(
                "SELECT workspace_db_path FROM sessions WHERE session_id = ?1",
                params![request.session_id],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(Into::into)
        })?;
        let Some(workspace_db_path) = workspace_db_path else {
            return Ok((Page::default(), Vec::new()));
        };
        if !Path::new(&workspace_db_path).exists() {
            self.delete_index_session(&request.session_id)?;
            return Ok((Page::default(), Vec::new()));
        }
        let page_size = request.page_size.clamp(1, 500);
        with_connection(Path::new(&workspace_db_path), init_workspace_db, |conn| {
            let total = conn.query_row(
                "SELECT COUNT(*) FROM session_records WHERE session_id = ?1",
                params![request.session_id],
                |row| row.get::<_, i64>(0),
            )? as u64;
            let page = bounded_page(request.page, page_size, total, true);
            let mut stmt = conn.prepare(
                "SELECT session_id, message_id, role, created_at, updated_at, record_json
                 FROM session_records
                 WHERE session_id = ?1
                 ORDER BY created_at ASC, id ASC
                 LIMIT ?2 OFFSET ?3",
            )?;
            let rows = stmt
                .query_map(
                    params![
                        request.session_id,
                        page_size as i64,
                        (page * page_size) as i64
                    ],
                    |row| {
                        let record_json: String = row.get(5)?;
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, String>(2)?,
                            row.get::<_, i64>(3)?,
                            row.get::<_, i64>(4)?,
                            record_json,
                        ))
                    },
                )?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            let records = rows
                .into_iter()
                .map(
                    |(session_id, message_id, role, created_at, updated_at, record_json)| {
                        Ok(SessionRecord {
                            record: parse_json_field(
                                &record_json,
                                "record_json",
                                Some(&session_id),
                            )?,
                            session_id,
                            message_id,
                            role,
                            created_at,
                            updated_at,
                        })
                    },
                )
                .collect::<Result<Vec<_>>>()?;
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

    fn with_index_connection<T>(&self, f: impl FnOnce(&mut Connection) -> Result<T>) -> Result<T> {
        with_connection(&self.index_db_path, init_index_db, f)
    }

    fn with_workspace_connection<T>(
        &self,
        path: &Path,
        f: impl FnOnce(&mut Connection) -> Result<T>,
    ) -> Result<T> {
        with_connection(path, init_workspace_db, f)
    }

    fn snapshot_from_index_row(&self, row: IndexSessionRow) -> Result<Option<SessionSnapshot>> {
        let workspace_payload =
            load_workspace_session_payload(&row.workspace_db_path, &row.session_id)?;
        let Some(workspace_payload) = workspace_payload else {
            self.delete_index_session(&row.session_id)?;
            return Ok(None);
        };
        let task_management = parse_json_field(
            &row.task_management_json,
            "task_management_json",
            Some(&row.session_id),
        )?;
        let management = parse_json_field(
            &row.management_json,
            "management_json",
            Some(&row.session_id),
        )?;
        Ok(Some(SessionSnapshot {
            session_id: row.session_id,
            workspace: row.workspace,
            name: row.name,
            parent_id: row.parent_id,
            created_at: row.created_at,
            updated_at: row.updated_at,
            state: row.state,
            status: row.status,
            message_count: row.message_count as u64,
            task_management,
            management,
            session: workspace_payload.session,
            todos: workspace_payload.todos,
        }))
    }

    fn delete_index_session(&self, session_id: &str) -> Result<()> {
        self.with_index_connection(|conn| {
            conn.execute(
                "DELETE FROM sessions WHERE session_id = ?1",
                params![session_id],
            )?;
            Ok(())
        })
    }

    fn sweep_missing_workspace_dbs(&self) -> Result<()> {
        let missing = self.with_index_connection(|conn| {
            let mut stmt = conn.prepare("SELECT DISTINCT workspace_db_path FROM sessions")?;
            let paths = stmt
                .query_map([], |row| row.get::<_, String>(0))?
                .collect::<std::result::Result<Vec<_>, _>>()?
                .into_iter()
                .filter(|path| !Path::new(path).exists())
                .collect::<Vec<_>>();
            for path in &paths {
                conn.execute(
                    "DELETE FROM sessions WHERE workspace_db_path = ?1",
                    params![path],
                )?;
            }
            Ok(paths)
        })?;
        for path in missing {
            tracing::warn!(
                path,
                "removed session index snapshots for missing workspace DB"
            );
        }
        Ok(())
    }
}

#[derive(Debug)]
struct IndexSessionRow {
    session_id: String,
    workspace: String,
    workspace_db_path: PathBuf,
    name: Option<String>,
    parent_id: Option<String>,
    created_at: i64,
    updated_at: i64,
    state: Option<String>,
    status: Option<String>,
    message_count: i64,
    task_management_json: String,
    management_json: String,
}

struct WorkspacePayload {
    session: Value,
    todos: Vec<Value>,
}

fn index_session_from_row(row: &Row<'_>) -> rusqlite::Result<IndexSessionRow> {
    Ok(IndexSessionRow {
        session_id: row.get(0)?,
        workspace: row.get(1)?,
        workspace_db_path: PathBuf::from(row.get::<_, String>(2)?),
        name: row.get(3)?,
        parent_id: row.get(4)?,
        created_at: row.get(5)?,
        updated_at: row.get(6)?,
        state: row.get(7)?,
        status: row.get(8)?,
        message_count: row.get(9)?,
        task_management_json: row.get(10)?,
        management_json: row.get(11)?,
    })
}

fn load_workspace_session_payload(
    workspace_db_path: &Path,
    session_id: &str,
) -> Result<Option<WorkspacePayload>> {
    if !workspace_db_path.exists() {
        return Ok(None);
    }
    let payload = with_connection(workspace_db_path, init_workspace_db, |conn| {
        conn.query_row(
            "SELECT session_json, todos_json FROM sessions WHERE session_id = ?1",
            params![session_id],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()
        .map_err(Into::into)
    })?;
    payload
        .map(|(session_json, todos_json)| {
            Ok(WorkspacePayload {
                session: parse_json_field(&session_json, "session_json", Some(session_id))?,
                todos: parse_json_field(&todos_json, "todos_json", Some(session_id))?,
            })
        })
        .transpose()
}

fn mark_workspace_session_interrupted(
    workspace_db_path: &Path,
    session_id: &str,
    now_ms: i64,
) -> Result<()> {
    if !workspace_db_path.exists() {
        return Ok(());
    }
    with_connection(workspace_db_path, init_workspace_db, |conn| {
        let payload = conn
            .query_row(
                "SELECT session_json, management_json FROM sessions WHERE session_id = ?1",
                params![session_id],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()?;
        let Some((session_json, management_json)) = payload else {
            return Ok(());
        };
        let mut session: Value = parse_json_field(&session_json, "session_json", Some(session_id))?;
        let mut management: Value =
            parse_json_field(&management_json, "management_json", Some(session_id))?;
        set_object_string(&mut session, "status", "interrupted");
        set_object_string(&mut management, "state", "interrupted");
        conn.execute(
            "UPDATE sessions
             SET state = 'interrupted',
                 status = 'interrupted',
                 updated_at = MAX(updated_at, ?2),
                 session_json = ?3,
                 management_json = ?4
             WHERE session_id = ?1",
            params![
                session_id,
                now_ms,
                serde_json::to_string(&session)?,
                serde_json::to_string(&management)?,
            ],
        )?;
        Ok(())
    })
}

fn with_connection<T>(
    path: &Path,
    init: fn(&Connection) -> Result<()>,
    f: impl FnOnce(&mut Connection) -> Result<T>,
) -> Result<T> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    let mut conn = {
        let _file_guard = sqlite_init_file_lock(path)?;
        let _guard = sqlite_init_lock()
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let conn =
            Connection::open(path).with_context(|| format!("failed to open {}", path.display()))?;
        conn.busy_timeout(Duration::from_secs(30))?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        init(&conn)?;
        conn
    };
    f(&mut conn)
}

fn sqlite_init_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

struct SqliteInitFileLock {
    file: File,
}

impl Drop for SqliteInitFileLock {
    fn drop(&mut self) {
        let _ = self.file.unlock();
    }
}

fn sqlite_init_file_lock(path: &Path) -> Result<SqliteInitFileLock> {
    let lock_path = PathBuf::from(format!("{}.init.lock", path.display()));
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&lock_path)
        .with_context(|| format!("failed to open SQLite init lock {}", lock_path.display()))?;
    file.lock_exclusive()
        .with_context(|| format!("failed to lock SQLite init lock {}", lock_path.display()))?;
    Ok(SqliteInitFileLock { file })
}

fn init_index_db(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS sessions (
            session_id TEXT PRIMARY KEY,
            workspace TEXT NOT NULL,
            workspace_db_path TEXT NOT NULL,
            name TEXT,
            parent_id TEXT,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL,
            state TEXT,
            status TEXT,
            message_count INTEGER NOT NULL DEFAULT 0,
            task_management_json TEXT NOT NULL,
            management_json TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_sessions_workspace_updated
            ON sessions(workspace, updated_at DESC, session_id);
        CREATE INDEX IF NOT EXISTS idx_sessions_parent
            ON sessions(parent_id);
        CREATE TABLE IF NOT EXISTS session_write_queue (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            idempotency_key TEXT NOT NULL UNIQUE,
            session_id TEXT NOT NULL,
            turn_id TEXT NULL,
            runtime_worker_id TEXT NULL,
            command_run_id TEXT NULL,
            command_id TEXT NULL,
            event_seq INTEGER NULL,
            event_type TEXT NOT NULL,
            payload_json TEXT NOT NULL,
            status TEXT NOT NULL,
            retry_count INTEGER NOT NULL DEFAULT 0,
            created_at INTEGER NOT NULL,
            applied_at INTEGER NULL,
            last_error TEXT NULL
        );
        ",
    )?;
    Ok(())
}

fn init_workspace_db(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS sessions (
            session_id TEXT PRIMARY KEY,
            workspace TEXT NOT NULL,
            name TEXT,
            parent_id TEXT,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL,
            state TEXT,
            status TEXT,
            message_count INTEGER NOT NULL DEFAULT 0,
            task_management_json TEXT NOT NULL,
            management_json TEXT NOT NULL,
            session_json TEXT NOT NULL,
            todos_json TEXT NOT NULL DEFAULT '[]'
        );
        CREATE INDEX IF NOT EXISTS idx_workspace_sessions_updated
            ON sessions(workspace, updated_at DESC, session_id);
        CREATE TABLE IF NOT EXISTS session_records (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            session_id TEXT NOT NULL,
            message_id TEXT NOT NULL,
            role TEXT NOT NULL,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL,
            record_json TEXT NOT NULL,
            FOREIGN KEY(session_id) REFERENCES sessions(session_id) ON DELETE CASCADE
        );
        CREATE INDEX IF NOT EXISTS idx_records_session_created
            ON session_records(session_id, created_at, id);
        ",
    )?;
    Ok(())
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

fn set_object_string(value: &mut Value, key: &str, next: &str) {
    if !value.is_object() {
        *value = serde_json::json!({});
    }
    if let Some(object) = value.as_object_mut() {
        object.insert(key.to_string(), Value::String(next.to_string()));
    }
}

fn path_text(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn parse_json_field<T: DeserializeOwned>(
    text: &str,
    field: &str,
    session_id: Option<&str>,
) -> Result<T> {
    serde_json::from_str(text).with_context(|| match session_id {
        Some(session_id) => format!("failed to parse {field} for session {session_id}"),
        None => format!("failed to parse {field}"),
    })
}

fn remove_sqlite_files(path: &Path) -> Result<()> {
    for suffix in ["", "-wal", "-shm"] {
        let target = PathBuf::from(format!("{}{}", path.display(), suffix));
        if target.exists() {
            std::fs::remove_file(&target)
                .with_context(|| format!("failed to remove {}", target.display()))?;
        }
    }
    Ok(())
}
