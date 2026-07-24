//! Path helpers for the session store.
//!
//! All instance/home/db-path resolution lives in the `tura_path` crate so there
//! is a single source of truth for session_log call sites.

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
/// Source checkouts keep the historical `<workspace>/.tura` location. An
/// explicit `TURA_HOME` relocates workspace state under that instance home.
pub fn workspace_session_log_dir(directory: &str) -> PathBuf {
    let workspace = normalize_workspace(directory);
    if workspace.is_empty() {
        return default_db_dir()
            .join("workspaces")
            .join("_unknown")
            .join(".tura");
    }
    tura_path::workspace_runtime_dir(workspace)
}

/// SQLite database that stores the full session log for a workspace.
pub fn workspace_session_log_db(directory: &str) -> PathBuf {
    workspace_session_log_dir(directory).join("session_log.sqlite3")
}

/// SQLite database that stores the global session state/index.
pub fn index_db_path() -> PathBuf {
    default_db_dir().join("index.sqlite3")
}

#[cfg(test)]
mod tests {
    use super::{normalize_workspace, workspace_session_log_db, workspace_session_log_dir};

    #[test]
    fn workspace_session_log_lives_under_workspace_tura_directory() {
        let dir = tempfile::tempdir().expect("tempdir");
        let workspace = dir.path().join("project");
        std::fs::create_dir_all(&workspace).expect("workspace");
        let workspace_text = workspace.display().to_string();

        assert_eq!(
            workspace_session_log_dir(&workspace_text),
            workspace.join(".tura")
        );
        assert_eq!(
            workspace_session_log_db(&workspace_text),
            workspace.join(".tura").join("session_log.sqlite3")
        );
    }

    #[test]
    fn empty_workspace_uses_unknown_workspace_bucket_in_instance_db() {
        let dir = workspace_session_log_dir("");

        assert!(dir.ends_with(
            std::path::Path::new("workspaces")
                .join("_unknown")
                .join(".tura")
        ));
        assert!(workspace_session_log_db("").ends_with(
            std::path::Path::new("workspaces")
                .join("_unknown")
                .join(".tura")
                .join("session_log.sqlite3")
        ));
    }

    #[test]
    fn normalize_workspace_trims_and_normalizes_separators() {
        let normalized = normalize_workspace("  C:\\Users\\liuliu\\Documents\\tura  ");

        assert!(!normalized.starts_with(' '));
        assert!(!normalized.ends_with(' '));
        assert!(
            normalized.contains('/'),
            "normalized workspace should use stable slash separators: {normalized}"
        );
    }
}
