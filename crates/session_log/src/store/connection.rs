use super::SessionLogStore;
use anyhow::{Context, Result};
use fs2::FileExt;
use rusqlite::Connection;
use std::collections::BTreeSet;
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
    require_canonical_schema(conn, "index", INDEX_SCHEMA)?;
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS sessions (
            session_id TEXT PRIMARY KEY,
            workspace TEXT NOT NULL,
            workspace_db_path TEXT NOT NULL,
            updated_at INTEGER NOT NULL,
            last_user_message_at INTEGER,
            state TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_sessions_workspace_updated
            ON sessions(workspace, updated_at DESC, session_id);
        CREATE TABLE IF NOT EXISTS runtime_locations (
            runtime_id TEXT PRIMARY KEY,
            session_id TEXT NOT NULL,
            workspace_db_path TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_runtime_locations_session
            ON runtime_locations(session_id, runtime_id);
        CREATE TABLE IF NOT EXISTS command_checkpoints (
            idempotency_key TEXT PRIMARY KEY,
            session_id TEXT NOT NULL,
            runtime_id TEXT NOT NULL,
            runtime_worker_id TEXT,
            provider_call_id TEXT,
            command_run_id TEXT,
            command_id TEXT,
            event_seq INTEGER,
            checkpoint_type TEXT NOT NULL,
            command_type TEXT,
            command_line TEXT,
            output_summary TEXT,
            changes_json TEXT NOT NULL,
            started_at TEXT,
            finished_at TEXT,
            applied_at INTEGER NOT NULL
        );
        ",
    )?;
    conn.execute_batch(
        "
        CREATE INDEX IF NOT EXISTS idx_sessions_workspace_last_user_message
            ON sessions(workspace, last_user_message_at DESC, session_id);
        ",
    )?;
    Ok(())
}

pub(super) fn init_workspace_db(conn: &Connection) -> Result<()> {
    require_canonical_schema(conn, "workspace", WORKSPACE_SCHEMA)?;
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
            todos_json TEXT NOT NULL DEFAULT '[]',
            next_context_sequence INTEGER NOT NULL DEFAULT 0,
            retained_from_sequence INTEGER NOT NULL DEFAULT 0,
            next_management_sequence INTEGER NOT NULL DEFAULT 0
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
        CREATE TABLE IF NOT EXISTS session_context_records (
            session_id TEXT NOT NULL,
            sequence INTEGER NOT NULL,
            record_json TEXT NOT NULL,
            projection_json TEXT NOT NULL,
            PRIMARY KEY(session_id, sequence),
            FOREIGN KEY(session_id) REFERENCES sessions(session_id) ON DELETE CASCADE
        );
        CREATE INDEX IF NOT EXISTS idx_context_records_session_sequence
            ON session_context_records(session_id, sequence);
        CREATE TABLE IF NOT EXISTS session_events (
            session_id TEXT NOT NULL,
            event_seq INTEGER NOT NULL,
            event_json TEXT NOT NULL,
            PRIMARY KEY(session_id, event_seq),
            FOREIGN KEY(session_id) REFERENCES sessions(session_id) ON DELETE CASCADE
        );
        CREATE TABLE IF NOT EXISTS session_command_receipts (
            command_id TEXT PRIMARY KEY,
            session_id TEXT NOT NULL,
            request_json TEXT NOT NULL,
            result_json TEXT NOT NULL,
            FOREIGN KEY(session_id) REFERENCES sessions(session_id) ON DELETE CASCADE
        );
        CREATE TABLE IF NOT EXISTS management_deltas (
            session_id TEXT NOT NULL,
            sequence INTEGER NOT NULL,
            delta_json TEXT NOT NULL,
            PRIMARY KEY(session_id, sequence),
            FOREIGN KEY(session_id) REFERENCES sessions(session_id) ON DELETE CASCADE
        );
        ",
    )?;
    conn.execute_batch(
        "
        CREATE INDEX IF NOT EXISTS idx_workspace_sessions_last_user_message
            ON sessions(workspace, last_user_message_at DESC, session_id);
        CREATE TABLE IF NOT EXISTS runtimes (
            runtime_id TEXT PRIMARY KEY,
            session_id TEXT NOT NULL,
            fallback_from_id TEXT,
            lease_id TEXT,
            lease_active INTEGER NOT NULL DEFAULT 0 CHECK(lease_active IN (0, 1)),
            revision INTEGER NOT NULL DEFAULT 0,
            last_event_seq INTEGER NOT NULL DEFAULT 0,
            terminal INTEGER NOT NULL DEFAULT 0 CHECK(terminal IN (0, 1)),
            FOREIGN KEY(session_id) REFERENCES sessions(session_id) ON DELETE CASCADE
        );
        CREATE INDEX IF NOT EXISTS idx_runtimes_session
            ON runtimes(session_id, runtime_id);
        CREATE TABLE IF NOT EXISTS runtime_events (
            runtime_id TEXT NOT NULL,
            event_seq INTEGER NOT NULL,
            revision INTEGER NOT NULL,
            idempotency_key TEXT NOT NULL UNIQUE,
            event_json TEXT NOT NULL,
            PRIMARY KEY(runtime_id, event_seq),
            FOREIGN KEY(runtime_id) REFERENCES runtimes(runtime_id) ON DELETE CASCADE
        );
        CREATE TABLE IF NOT EXISTS session_feed_events (
            session_id TEXT NOT NULL,
            cursor INTEGER NOT NULL,
            runtime_id TEXT,
            event_id TEXT NOT NULL UNIQUE,
            event_json TEXT NOT NULL,
            PRIMARY KEY(session_id, cursor),
            FOREIGN KEY(session_id) REFERENCES sessions(session_id) ON DELETE CASCADE,
            FOREIGN KEY(runtime_id) REFERENCES runtimes(runtime_id) ON DELETE CASCADE
        );
        CREATE INDEX IF NOT EXISTS idx_session_feed_runtime
            ON session_feed_events(runtime_id, cursor);
        ",
    )?;
    Ok(())
}

fn require_canonical_schema(
    conn: &Connection,
    database: &str,
    expected: &[(&str, &[&str])],
) -> Result<()> {
    let actual_tables = conn
        .prepare(
            "SELECT name FROM sqlite_master
             WHERE type = 'table' AND name NOT LIKE 'sqlite_%'",
        )?
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<std::result::Result<BTreeSet<_>, _>>()?;
    if actual_tables.is_empty() {
        return Ok(());
    }
    let expected_tables = expected
        .iter()
        .map(|(table, _)| (*table).to_string())
        .collect::<BTreeSet<_>>();
    if actual_tables != expected_tables {
        anyhow::bail!(
            "incompatible {database} session database schema: expected tables {expected_tables:?}, found {actual_tables:?}; start with a clean canonical database"
        );
    }
    for (table, expected_columns) in expected {
        let actual_columns = conn
            .prepare(&format!("PRAGMA table_info({table})"))?
            .query_map([], |row| row.get::<_, String>(1))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        if actual_columns
            != expected_columns
                .iter()
                .map(|column| (*column).to_string())
                .collect::<Vec<_>>()
        {
            anyhow::bail!(
                "incompatible {database} session database schema: table {table} has columns {actual_columns:?}, expected {expected_columns:?}; start with a clean canonical database"
            );
        }
    }
    Ok(())
}

const INDEX_SCHEMA: &[(&str, &[&str])] = &[
    (
        "sessions",
        &[
            "session_id",
            "workspace",
            "workspace_db_path",
            "updated_at",
            "last_user_message_at",
            "state",
        ],
    ),
    (
        "runtime_locations",
        &["runtime_id", "session_id", "workspace_db_path"],
    ),
    (
        "command_checkpoints",
        &[
            "idempotency_key",
            "session_id",
            "runtime_id",
            "runtime_worker_id",
            "provider_call_id",
            "command_run_id",
            "command_id",
            "event_seq",
            "checkpoint_type",
            "command_type",
            "command_line",
            "output_summary",
            "changes_json",
            "started_at",
            "finished_at",
            "applied_at",
        ],
    ),
];

const WORKSPACE_SCHEMA: &[(&str, &[&str])] = &[
    (
        "sessions",
        &[
            "session_id",
            "workspace",
            "name",
            "parent_id",
            "created_at",
            "updated_at",
            "last_user_message_at",
            "state",
            "status",
            "message_count",
            "task_management_json",
            "management_json",
            "session_json",
            "todos_json",
            "next_context_sequence",
            "retained_from_sequence",
            "next_management_sequence",
        ],
    ),
    (
        "session_records",
        &[
            "id",
            "session_id",
            "message_id",
            "role",
            "created_at",
            "updated_at",
            "record_json",
        ],
    ),
    (
        "session_context_records",
        &["session_id", "sequence", "record_json", "projection_json"],
    ),
    ("session_events", &["session_id", "event_seq", "event_json"]),
    (
        "session_command_receipts",
        &["command_id", "session_id", "request_json", "result_json"],
    ),
    (
        "management_deltas",
        &["session_id", "sequence", "delta_json"],
    ),
    (
        "runtimes",
        &[
            "runtime_id",
            "session_id",
            "fallback_from_id",
            "lease_id",
            "lease_active",
            "revision",
            "last_event_seq",
            "terminal",
        ],
    ),
    (
        "runtime_events",
        &[
            "runtime_id",
            "event_seq",
            "revision",
            "idempotency_key",
            "event_json",
        ],
    ),
    (
        "session_feed_events",
        &[
            "session_id",
            "cursor",
            "runtime_id",
            "event_id",
            "event_json",
        ],
    ),
];
