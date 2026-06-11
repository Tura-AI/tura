//! Path helpers for the session store.
//!
//! All instance/home/db-path resolution lives in the `tura_path` crate so there
//! is a single source of truth. These thin re-exports keep the historical
//! `session_log::path::*` call sites working.

use std::path::{Path, PathBuf};

pub use tura_path::DB_DIR_NAME;

/// The instance's private database directory (see [`tura_path::home_db_dir`]).
pub fn default_db_dir() -> PathBuf {
    tura_path::home_db_dir()
}

/// Find the workspace root by ascending from `start`.
pub fn repo_root_from(start: impl AsRef<Path>) -> Option<PathBuf> {
    tura_path::repo_root_from(start)
}

/// Normalize a workspace directory used as a session key.
pub fn normalize_workspace(directory: &str) -> String {
    tura_path::normalize_workspace(directory)
}

/// Directory that stores the durable session log for a workspace.
///
/// The session log follows the workspace, so dev and release builds use the
/// same `<workspace>/.tura` location for a given project.
pub fn workspace_session_log_dir(directory: &str) -> PathBuf {
    let workspace = normalize_workspace(directory);
    if workspace.is_empty() {
        return default_db_dir()
            .join("workspaces")
            .join("_unknown")
            .join(".tura");
    }
    PathBuf::from(workspace).join(".tura")
}

/// SQLite database that stores the full session log for a workspace.
pub fn workspace_session_log_db(directory: &str) -> PathBuf {
    workspace_session_log_dir(directory).join("session_log.sqlite3")
}

/// SQLite database that stores the global session state/index.
pub fn index_db_path() -> PathBuf {
    default_db_dir().join("index.sqlite3")
}
