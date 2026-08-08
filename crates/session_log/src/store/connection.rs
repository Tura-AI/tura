use super::SessionLogStore;
use anyhow::{Context, Result};
use fs2::FileExt;
use rusqlite::{ffi::ErrorCode, Connection};
use std::collections::{BTreeSet, HashMap};
use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock, Weak};
use std::time::{Duration, Instant};

const SQLITE_INIT_LOCK_OPEN_TIMEOUT: Duration = Duration::from_secs(30);
const SQLITE_INIT_LOCK_OPEN_RETRY_DELAY: Duration = Duration::from_millis(10);
const SQLITE_OPERATION_RETRY_DELAYS: &[Duration] = &[
    Duration::from_millis(10),
    Duration::from_millis(25),
    Duration::from_millis(50),
    Duration::from_millis(100),
    Duration::from_millis(200),
    Duration::from_millis(400),
];

impl SessionLogStore {
    pub(super) fn with_index_connection<T>(
        &self,
        f: impl FnMut(&mut Connection) -> Result<T>,
    ) -> Result<T> {
        with_connection(&self.index_db_path, init_index_db, f)
    }

    pub(super) fn with_workspace_connection<T>(
        &self,
        path: &Path,
        f: impl FnMut(&mut Connection) -> Result<T>,
    ) -> Result<T> {
        with_connection(path, init_workspace_db, f)
    }
}

pub(super) fn with_connection<T>(
    path: &Path,
    init: fn(&Connection) -> Result<()>,
    mut f: impl FnMut(&mut Connection) -> Result<T>,
) -> Result<T> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    let operation_lock = sqlite_operation_lock(path);
    let _operation_guard = operation_lock
        .lock()
        .unwrap_or_else(|error| error.into_inner());

    let mut attempt = 1_usize;
    loop {
        match with_connection_once(path, init, &mut f) {
            Ok(value) => return Ok(value),
            Err(error) => {
                let sqlite_error = sqlite_error_details(&error);
                let retry_delay = SQLITE_OPERATION_RETRY_DELAYS.get(attempt - 1).copied();
                if let (Some((code, extended_code)), Some(retry_delay)) =
                    (sqlite_error, retry_delay)
                    && sqlite_error_is_transient((code, extended_code))
                {
                    tracing::warn!(
                        database_path = %path.display(),
                        attempt,
                        max_attempts = SQLITE_OPERATION_RETRY_DELAYS.len() + 1,
                        sqlite_code = ?code,
                        sqlite_extended_code = extended_code,
                        retry_delay_ms = retry_delay.as_millis(),
                        error = %error,
                        "retrying transient SQLite operation failure"
                    );
                    std::thread::sleep(retry_delay);
                    attempt += 1;
                    continue;
                }

                let Some((code, extended_code)) = sqlite_error else {
                    return Err(error);
                };
                let context = format!(
                    "SQLite operation on {} failed after {attempt} attempt(s); SQLite code {code:?}, extended code {extended_code}: {error}",
                    path.display()
                );
                return Err(error).context(context);
            }
        }
    }
}

fn with_connection_once<T>(
    path: &Path,
    init: fn(&Connection) -> Result<()>,
    f: &mut impl FnMut(&mut Connection) -> Result<T>,
) -> Result<T> {
    let mut conn = {
        let _file_guard = sqlite_init_file_lock(path)?;
        let _guard = sqlite_init_lock()
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let conn =
            Connection::open(path).with_context(|| format!("failed to open {}", path.display()))?;
        conn.busy_timeout(Duration::from_secs(30))?;
        let journal_mode =
            conn.pragma_query_value(None, "journal_mode", |row| row.get::<_, String>(0))?;
        if !journal_mode.eq_ignore_ascii_case("wal") {
            conn.pragma_update(None, "journal_mode", "WAL")?;
        }
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        init(&conn)?;
        conn
    };
    f(&mut conn)
}

fn sqlite_operation_lock(path: &Path) -> Arc<Mutex<()>> {
    static LOCKS: OnceLock<Mutex<HashMap<PathBuf, Weak<Mutex<()>>>>> = OnceLock::new();
    let locks = LOCKS.get_or_init(|| Mutex::new(HashMap::new()));
    let mut locks = locks.lock().unwrap_or_else(|error| error.into_inner());
    locks.retain(|_, lock| lock.strong_count() > 0);
    let key = sqlite_operation_lock_key(path);
    if let Some(lock) = locks.get(&key).and_then(Weak::upgrade) {
        return lock;
    }
    let lock = Arc::new(Mutex::new(()));
    locks.insert(key, Arc::downgrade(&lock));
    lock
}

fn sqlite_operation_lock_key(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| {
        let Some(parent) = path.parent() else {
            return path.to_path_buf();
        };
        parent
            .canonicalize()
            .map(|parent| {
                path.file_name()
                    .map_or(parent.clone(), |name| parent.join(name))
            })
            .unwrap_or_else(|_| path.to_path_buf())
    })
}

fn sqlite_error_details(error: &anyhow::Error) -> Option<(ErrorCode, i32)> {
    error.chain().find_map(|cause| {
        cause
            .downcast_ref::<rusqlite::Error>()
            .and_then(rusqlite::Error::sqlite_error)
            .map(|error| (error.code, error.extended_code))
    })
}

fn sqlite_error_is_transient((code, extended_code): (ErrorCode, i32)) -> bool {
    match code {
        ErrorCode::DatabaseBusy | ErrorCode::DatabaseLocked => true,
        ErrorCode::ReadOnly => !matches!(
            extended_code,
            rusqlite::ffi::SQLITE_READONLY_DBMOVED | rusqlite::ffi::SQLITE_READONLY_DIRECTORY
        ),
        _ => false,
    }
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
    let started = Instant::now();
    let file = loop {
        match OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&lock_path)
        {
            Ok(file) => break file,
            Err(error)
                if sqlite_init_lock_open_error_is_transient(&error)
                    && started.elapsed() < SQLITE_INIT_LOCK_OPEN_TIMEOUT =>
            {
                std::thread::sleep(SQLITE_INIT_LOCK_OPEN_RETRY_DELAY);
            }
            Err(error) => {
                return Err(error).with_context(|| {
                    format!("failed to open SQLite init lock {}", lock_path.display())
                });
            }
        }
    };
    file.lock_exclusive()
        .with_context(|| format!("failed to lock SQLite init lock {}", lock_path.display()))?;
    Ok(SqliteInitFileLock { file })
}

fn sqlite_init_lock_open_error_is_transient(error: &std::io::Error) -> bool {
    #[cfg(windows)]
    {
        // ERROR_SHARING_VIOLATION / ERROR_LOCK_VIOLATION. Antivirus, file
        // indexers, and concurrent workspace maintenance can briefly hold the
        // persistent lock file with incompatible Windows sharing flags.
        matches!(error.raw_os_error(), Some(32 | 33))
    }
    #[cfg(not(windows))]
    {
        let _ = error;
        false
    }
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

#[cfg(all(test, windows))]
mod tests {
    use super::{sqlite_error_is_transient, sqlite_init_file_lock, with_connection};
    use anyhow::Result;
    use std::fs::OpenOptions;
    use std::os::windows::fs::OpenOptionsExt;
    use std::path::PathBuf;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn sqlite_init_lock_waits_for_transient_windows_share_violation() {
        let temp = tempfile::tempdir().expect("tempdir");
        let database_path = temp.path().join("session_log.sqlite3");
        let lock_path = PathBuf::from(format!("{}.init.lock", database_path.display()));
        std::fs::write(&lock_path, []).expect("create lock file");

        let blocker = OpenOptions::new()
            .read(true)
            .write(true)
            .share_mode(0)
            .open(&lock_path)
            .expect("open lock file without sharing");
        let release = thread::spawn(move || {
            thread::sleep(Duration::from_millis(150));
            drop(blocker);
        });

        let result = sqlite_init_file_lock(&database_path);
        release.join().expect("release blocker");
        let _guard = result.expect("transient sharing violation should be retried");
    }

    #[test]
    fn sqlite_connection_recovers_after_database_is_briefly_read_only() {
        let temp = tempfile::tempdir().expect("tempdir");
        let database_path = temp.path().join("session_log.sqlite3");
        with_connection(
            &database_path,
            |conn| {
                conn.execute_batch("CREATE TABLE IF NOT EXISTS events(id INTEGER PRIMARY KEY);")?;
                Ok(())
            },
            |_| Ok(()),
        )
        .expect("initialize database");

        let mut permissions = std::fs::metadata(&database_path)
            .expect("database metadata")
            .permissions();
        permissions.set_readonly(true);
        std::fs::set_permissions(&database_path, permissions).expect("make database read-only");

        let release_path = database_path.clone();
        let release = thread::spawn(move || {
            thread::sleep(Duration::from_millis(150));
            let mut permissions = std::fs::metadata(&release_path)
                .expect("database metadata")
                .permissions();
            permissions.set_readonly(false);
            std::fs::set_permissions(&release_path, permissions).expect("restore database writes");
        });

        let result = with_connection(
            &database_path,
            |_| Ok(()),
            |conn| -> Result<()> {
                conn.execute("INSERT INTO events DEFAULT VALUES", [])?;
                Ok(())
            },
        );
        release.join().expect("release read-only database");
        result.expect("brief read-only window should be retried");
    }

    #[test]
    fn sqlite_retry_classifier_excludes_permanent_readonly_failures() {
        assert!(sqlite_error_is_transient((
            rusqlite::ffi::ErrorCode::ReadOnly,
            rusqlite::ffi::SQLITE_READONLY
        )));
        assert!(sqlite_error_is_transient((
            rusqlite::ffi::ErrorCode::ReadOnly,
            rusqlite::ffi::SQLITE_READONLY_CANTINIT
        )));
        assert!(sqlite_error_is_transient((
            rusqlite::ffi::ErrorCode::DatabaseBusy,
            rusqlite::ffi::SQLITE_BUSY
        )));
        assert!(!sqlite_error_is_transient((
            rusqlite::ffi::ErrorCode::ReadOnly,
            rusqlite::ffi::SQLITE_READONLY_DBMOVED
        )));
        assert!(!sqlite_error_is_transient((
            rusqlite::ffi::ErrorCode::ReadOnly,
            rusqlite::ffi::SQLITE_READONLY_DIRECTORY
        )));
    }
}
