use std::path::{Path, PathBuf};

pub const DB_DIR_NAME: &str = "session_log";
pub fn repo_root_from(start: impl AsRef<Path>) -> Option<PathBuf> {
    start.as_ref().ancestors().find_map(|candidate| {
        (candidate.join("Cargo.toml").exists()
            && candidate.join("crates").join("session_log").exists())
        .then(|| candidate.to_path_buf())
    })
}

pub fn default_db_dir() -> PathBuf {
    for key in ["SESSION_LOG_DB_ROOT", "TURA_DB_ROOT"] {
        if let Ok(value) = std::env::var(key) {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                return PathBuf::from(trimmed).join(DB_DIR_NAME);
            }
        }
    }
    let start = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    repo_root_from(start)
        .unwrap_or_else(|| PathBuf::from("."))
        .join("db")
        .join(DB_DIR_NAME)
}

pub fn normalize_workspace(directory: &str) -> String {
    let value = directory.trim().replace('\\', "/");
    if value.is_empty() {
        return String::new();
    }
    if value.len() == 3
        && value.as_bytes()[1] == b':'
        && value.ends_with('/')
        && value.as_bytes()[0].is_ascii_alphabetic()
    {
        return value;
    }
    if value.chars().all(|ch| ch == '/') {
        return "/".to_string();
    }
    value.trim_end_matches('/').to_string()
}
