use super::SessionLogStore;
use anyhow::{Context, Result};
use fs2::FileExt;
use rusqlite::Connection;
use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

impl SessionLogStore {
    pub(super) fn with_index_connection<T>(
        &self,
        f: impl FnOnce(&mut Connection) -> Result<T>,
    ) -> Result<T> {
        with_connection(&self.index_db_path, init_index_db, f)
    }

    pub(super) fn with_workspace_connection<T>(
        &self,
        path: &Path,
        f: impl FnOnce(&mut Connection) -> Result<T>,
    ) -> Result<T> {
        with_connection(path, init_workspace_db, f)
    }
}

pub(super) fn with_connection<T>(
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

pub(super) fn init_index_db(conn: &Connection) -> Result<()> {
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
            last_user_message_at INTEGER,
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
    ensure_column(conn, "sessions", "last_user_message_at", "INTEGER")?;
    conn.execute_batch(
        "
        CREATE INDEX IF NOT EXISTS idx_sessions_workspace_last_user_message
            ON sessions(workspace, last_user_message_at DESC, session_id);
        ",
    )?;
    Ok(())
}

pub(super) fn init_workspace_db(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS sessions (
            session_id TEXT PRIMARY KEY,
            workspace TEXT NOT NULL,
            name TEXT,
            parent_id TEXT,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL,
            last_user_message_at INTEGER,
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
        DELETE FROM session_records
            WHERE id NOT IN (
                SELECT MAX(id)
                FROM session_records
                GROUP BY session_id, message_id
            );
        CREATE UNIQUE INDEX IF NOT EXISTS idx_records_session_message
            ON session_records(session_id, message_id);
        ",
    )?;
    ensure_column(conn, "sessions", "last_user_message_at", "INTEGER")?;
    conn.execute_batch(
        "
        CREATE INDEX IF NOT EXISTS idx_workspace_sessions_last_user_message
            ON sessions(workspace, last_user_message_at DESC, session_id);
        ",
    )?;
    Ok(())
}

fn ensure_column(conn: &Connection, table: &str, column: &str, definition: &str) -> Result<()> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({table})"))?;
    let exists = stmt
        .query_map([], |row| row.get::<_, String>(1))?
        .collect::<std::result::Result<Vec<_>, _>>()?
        .into_iter()
        .any(|name| name == column);
    if !exists {
        conn.execute_batch(&format!(
            "ALTER TABLE {table} ADD COLUMN {column} {definition};"
        ))?;
    }
    Ok(())
}
