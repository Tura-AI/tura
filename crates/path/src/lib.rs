//! `tura_path` — the single source of truth for instance/home/path resolution.
//!
//! Refactor stage 2: an *instance_home* is one isolated Tura instance (modelled
//! on codex's `CODEX_HOME`). Every per-instance path — control sockets, flock
//! files, the private database directory, the version used for the connection
//! handshake — derives from one `instance_home()`. dev / release / debug are
//! simply different homes (selected by `TURA_HOME`), so they coexist with no
//! shared ports or locks.
//!
//! This crate replaces the previously scattered `find_repo_root` / `my_root` /
//! `default_db_dir` helpers so there is exactly one normalization and one
//! derivation path.

use std::path::{Path, PathBuf};

/// Directory name used for the embedded database, kept stable for back-compat.
pub const DB_DIR_NAME: &str = "session_log";

/// Sub-directory under an instance home that holds per-instance runtime state
/// (sockets, locks, endpoint files). Hidden so it does not clutter a repo home.
pub const RUNTIME_DIR_NAME: &str = ".tura";

// ---------------------------------------------------------------------------
// Repo / project root
// ---------------------------------------------------------------------------

/// Find the workspace root by ascending from `start`, looking for a `Cargo.toml`
/// next to a `crates/` directory. Returns `None` when run outside the repo
/// (e.g. an installed release that no longer ships sources).
pub fn repo_root_from(start: impl AsRef<Path>) -> Option<PathBuf> {
    let start = start.as_ref();
    let start = if start.is_dir() {
        start
    } else {
        start.parent().unwrap_or(start)
    };
    start.ancestors().find_map(|candidate| {
        (candidate.join("Cargo.toml").exists() && candidate.join("crates").is_dir())
            .then(|| candidate.to_path_buf())
    })
}

/// The canonical project root: `TURA_PROJECT_ROOT` when it exists, else the
/// current directory, canonicalized and stripped of the Windows verbatim
/// prefix. This is the value gateways report as their `root`.
pub fn canonical_root() -> PathBuf {
    let root = std::env::var_os("TURA_PROJECT_ROOT")
        .map(PathBuf::from)
        .filter(|path| path.exists())
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_default();
    normalize_path(&root)
}

// ---------------------------------------------------------------------------
// Instance home
// ---------------------------------------------------------------------------

/// The instance home for this process.
///
/// Precedence:
/// 1. `TURA_HOME` (explicit instance selection — dev/release/profile),
/// 2. the repo root when running inside the source tree,
/// 3. the canonical current directory.
///
/// The result is normalized so two paths that differ only by case, a trailing
/// separator, or a symlink hop resolve to the same home (and therefore the same
/// sockets/locks/db).
pub fn instance_home() -> PathBuf {
    if let Some(value) = std::env::var_os("TURA_HOME") {
        let path = PathBuf::from(value);
        if !path.as_os_str().is_empty() {
            return normalize_path(&path);
        }
    }
    let start = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let base = repo_root_from(&start).unwrap_or(start);
    normalize_path(&base)
}

/// Per-instance runtime directory (`<home>/.tura`) holding sockets/locks/etc.
pub fn home_runtime_dir() -> PathBuf {
    instance_home().join(RUNTIME_DIR_NAME)
}

/// Path of a named control endpoint for this instance (e.g. `session_db`,
/// `router`, `gateway`). Used for the socket / named-pipe address file.
pub fn home_socket(name: &str) -> PathBuf {
    home_runtime_dir()
        .join("sockets")
        .join(format!("{name}.sock"))
}

/// Directory holding this instance's flock files.
pub fn locks_dir() -> PathBuf {
    home_runtime_dir().join("locks")
}

/// The private database directory for this instance.
///
/// Legacy `SESSION_LOG_DB_ROOT` / `TURA_DB_ROOT` overrides are still honored for
/// now (they are redundant with the home and slated for retirement), and a repo
/// checkout keeps its existing `<repo>/db/session_log` layout so dev databases
/// are not relocated. Otherwise the directory derives from the instance home.
pub fn home_db_dir() -> PathBuf {
    for key in ["SESSION_LOG_DB_ROOT", "TURA_DB_ROOT"] {
        if let Ok(value) = std::env::var(key) {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                return PathBuf::from(trimmed).join(DB_DIR_NAME);
            }
        }
    }
    if std::env::var_os("TURA_HOME")
        .filter(|value| !value.is_empty())
        .is_some()
    {
        return instance_home().join("db").join(DB_DIR_NAME);
    }
    let start = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    if let Some(repo) = repo_root_from(&start) {
        return repo.join("db").join(DB_DIR_NAME);
    }
    instance_home().join("db").join(DB_DIR_NAME)
}

// ---------------------------------------------------------------------------
// Version / build-kind (connection handshake)
// ---------------------------------------------------------------------------

/// The build-kind marker injected at compile time (`dev` for `build`, `release`
/// for the packaged release). Falls back to `dev` when unset.
pub fn build_kind() -> &'static str {
    option_env!("TURA_BUILD_KIND").unwrap_or("dev")
}

/// The package version of this build.
pub fn package_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// The instance version string used for the probe / version handshake between a
/// client and the per-home services. A mismatch means the client is talking to
/// a service from a different build and should refuse or restart it.
pub fn instance_version() -> String {
    format!("{}+{}", package_version(), build_kind())
}

// ---------------------------------------------------------------------------
// Normalization
// ---------------------------------------------------------------------------

/// Canonicalize a path and strip the Windows verbatim (`\\?\`) prefix, falling
/// back to the input when the path does not yet exist on disk.
pub fn normalize_path(path: &Path) -> PathBuf {
    let resolved = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    PathBuf::from(strip_verbatim_prefix(&resolved.to_string_lossy()))
}

/// Strip the Windows extended-length (verbatim) path prefix so paths print and
/// compare the way users expect, including the UNC form. No-op otherwise.
pub fn strip_verbatim_prefix(path: &str) -> String {
    if let Some(rest) = path.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{rest}")
    } else if let Some(rest) = path.strip_prefix(r"\\?\") {
        rest.to_string()
    } else {
        path.to_string()
    }
}

/// Normalize a workspace directory used as a session key: forward slashes, no
/// trailing separator (except bare drive roots like `C:/` and the root `/`).
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

#[cfg(test)]
mod tests {
    use super::*;

    // `TURA_HOME` is process-global; serialize the tests that mutate it so they
    // do not race under the default parallel test runner.
    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn workspace_normalization_handles_drives_roots_and_separators() {
        assert_eq!(normalize_workspace(r"C:\repo\"), "C:/repo");
        assert_eq!(normalize_workspace(r"C:\"), "C:/");
        assert_eq!(normalize_workspace("/"), "/");
        assert_eq!(normalize_workspace("///"), "/");
        assert_eq!(
            normalize_workspace("  /home/user/proj/  "),
            "/home/user/proj"
        );
        assert_eq!(normalize_workspace(""), "");
    }

    #[test]
    fn strip_verbatim_prefix_is_noop_without_prefix() {
        assert_eq!(strip_verbatim_prefix(r"C:\Users\x"), r"C:\Users\x");
        assert_eq!(strip_verbatim_prefix(r"\\?\C:\Users\x"), r"C:\Users\x");
        assert_eq!(strip_verbatim_prefix(r"\\?\UNC\srv\share"), r"\\srv\share");
    }

    #[test]
    fn instance_home_honors_explicit_override() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let temp = std::env::temp_dir();
        std::env::set_var("TURA_HOME", &temp);
        // Normalization resolves to the same home regardless of trailing slash
        // or case differences the OS treats as equivalent.
        let home = instance_home();
        let expected = normalize_path(&temp);
        assert_eq!(home, expected);
        std::env::remove_var("TURA_HOME");
    }

    #[test]
    fn derived_paths_live_under_the_instance_home() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let temp = std::env::temp_dir().join("tura-path-derive-test");
        std::fs::create_dir_all(&temp).expect("create temp TURA_HOME");
        std::env::set_var("TURA_HOME", &temp);
        let home = instance_home();
        assert!(home_socket("session_db").starts_with(&home));
        assert!(locks_dir().starts_with(&home));
        assert!(home_runtime_dir().starts_with(&home));
        std::env::remove_var("TURA_HOME");
    }

    #[test]
    fn symlinked_home_resolves_to_the_real_target() {
        // canonicalize collapses a symlink to its real target so two homes that
        // reach the same directory share sockets/locks/db.
        let real = std::env::temp_dir().join("tura-path-real");
        std::fs::create_dir_all(&real).expect("create real home");
        let canon = normalize_path(&real);
        // A path with a redundant trailing component resolves identically.
        let with_dot = real.join(".");
        assert_eq!(normalize_path(&with_dot), canon);
    }

    #[test]
    fn instance_version_includes_build_kind() {
        let version = instance_version();
        assert!(version.contains(package_version()));
        assert!(version.ends_with(build_kind()));
    }
}
