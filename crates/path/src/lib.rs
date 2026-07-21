#![deny(clippy::unwrap_used)]
#![deny(unsafe_code)]

//! `tura_path` — the single source of truth for instance/home/path resolution.
//!
//! An instance home is one isolated Tura runtime root. Control sockets, locks,
//! endpoint files, database paths, and versioned handshakes derive from that
//! root so debug, release, and development instances do not share process state.

use std::path::{Path, PathBuf};

pub mod process_hardening;
pub mod shell_fallback;
pub mod workspace_git;

/// Directory name used for the embedded database, kept stable for back-compat.
pub const DB_DIR_NAME: &str = "session_log";

/// Sub-directory under an instance home that holds per-instance runtime state
/// (sockets, locks, endpoint files). Hidden so it does not clutter a repo home.
pub const RUNTIME_DIR_NAME: &str = ".tura";

pub const DEBUG_GATEWAY_PORT: u16 = 4125;
pub const RELEASE_GATEWAY_PORT: u16 = 4126;
pub const ACTIVE_GATEWAY_ENV_FILE: &str = "gateway-active.env";
pub const TURA_GATEWAY_URL_ENV: &str = "TURA_GATEWAY_URL";
pub const TURA_GATEWAY_PORT_ENV: &str = "TURA_GATEWAY_PORT";
pub const TURA_GATEWAY_PID_ENV: &str = "TURA_GATEWAY_PID";
pub const TURA_GATEWAY_PROCESS_START_TIME_ENV: &str = "TURA_GATEWAY_PROCESS_START_TIME";

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

pub fn default_gateway_port_for_build_kind(build_kind: &str) -> u16 {
    if build_kind == "release" {
        RELEASE_GATEWAY_PORT
    } else {
        DEBUG_GATEWAY_PORT
    }
}

pub fn default_gateway_url_for_build_kind(build_kind: &str) -> String {
    format!(
        "http://127.0.0.1:{}",
        default_gateway_port_for_build_kind(build_kind)
    )
}

pub fn active_gateway_env_path_for_home(home: impl AsRef<Path>) -> PathBuf {
    home.as_ref()
        .join(RUNTIME_DIR_NAME)
        .join(ACTIVE_GATEWAY_ENV_FILE)
}

pub fn read_active_gateway_url_for_home(home: impl AsRef<Path>) -> Option<String> {
    let path = active_gateway_env_path_for_home(home);
    let raw = std::fs::read_to_string(path).ok()?;
    parse_active_gateway_url(&raw)
}

pub fn write_active_gateway_url_for_home(
    home: impl AsRef<Path>,
    gateway_url: &str,
) -> std::io::Result<()> {
    let path = active_gateway_env_path_for_home(home);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, format!("{TURA_GATEWAY_URL_ENV}={gateway_url}\n"))
}

pub fn write_active_gateway_process_for_home(
    home: impl AsRef<Path>,
    gateway_url: &str,
    pid: u32,
    process_start_time: Option<u64>,
) -> std::io::Result<()> {
    let path = active_gateway_env_path_for_home(home);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut content =
        format!("{TURA_GATEWAY_URL_ENV}={gateway_url}\n{TURA_GATEWAY_PID_ENV}={pid}\n");
    if let Some(start_time) = process_start_time {
        content.push_str(&format!(
            "{TURA_GATEWAY_PROCESS_START_TIME_ENV}={start_time}\n"
        ));
    }
    std::fs::write(path, content)
}

fn parse_active_gateway_url(raw: &str) -> Option<String> {
    raw.lines().find_map(|line| {
        let trimmed = line.trim();
        let value = trimmed.strip_prefix(&format!("{TURA_GATEWAY_URL_ENV}="))?;
        let value = value.trim().trim_matches('"').trim_matches('\'');
        (!value.is_empty()).then(|| value.to_string())
    })
}

/// The private database directory for this instance.
///
/// `SESSION_LOG_DB_ROOT` / `TURA_DB_ROOT` overrides are honored for explicit
/// test and tool runs. A repo checkout keeps its `<repo>/db/session_log` layout
/// so dev databases are not relocated. Otherwise the directory derives from the
/// instance home.
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

    #[test]
    fn repo_root_from_accepts_file_or_directory_start_points() {
        let temp = tempfile::tempdir().expect("temp repo");
        std::fs::write(temp.path().join("Cargo.toml"), "[workspace]\n").expect("cargo toml");
        std::fs::create_dir_all(temp.path().join("crates/demo/src")).expect("crate dir");
        let source = temp.path().join("crates/demo/src/lib.rs");
        std::fs::write(&source, "").expect("source file");

        assert_eq!(repo_root_from(temp.path()), Some(temp.path().to_path_buf()));
        assert_eq!(
            repo_root_from(temp.path().join("crates/demo")),
            Some(temp.path().to_path_buf())
        );
        assert_eq!(repo_root_from(&source), Some(temp.path().to_path_buf()));
        assert_eq!(
            repo_root_from(temp.path().join("missing/file.rs")),
            Some(temp.path().to_path_buf())
        );
    }

    #[test]
    fn repo_root_from_returns_none_outside_workspace_shape() {
        let temp = tempfile::tempdir().expect("temp");
        std::fs::write(temp.path().join("Cargo.toml"), "[package]\n").expect("cargo toml");

        assert_eq!(repo_root_from(temp.path()), None);
    }

    #[test]
    fn canonical_root_prefers_existing_project_root_env() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let temp = tempfile::tempdir().expect("temp root");
        let previous = std::env::var_os("TURA_PROJECT_ROOT");
        std::env::set_var("TURA_PROJECT_ROOT", temp.path());

        let root = canonical_root();

        assert_eq!(root, normalize_path(temp.path()));
        restore_env("TURA_PROJECT_ROOT", previous);
    }

    #[test]
    fn canonical_root_ignores_missing_project_root_env() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let previous = std::env::var_os("TURA_PROJECT_ROOT");
        std::env::set_var("TURA_PROJECT_ROOT", "Z:/definitely/missing/tura/root");

        let root = canonical_root();

        assert!(root.exists() || root.as_os_str().is_empty());
        assert_ne!(root, PathBuf::from("Z:/definitely/missing/tura/root"));
        restore_env("TURA_PROJECT_ROOT", previous);
    }

    #[test]
    fn home_socket_and_locks_have_stable_runtime_layout() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let temp = tempfile::tempdir().expect("temp home");
        let previous = std::env::var_os("TURA_HOME");
        std::env::set_var("TURA_HOME", temp.path());

        assert_eq!(
            home_runtime_dir(),
            normalize_path(temp.path()).join(".tura")
        );
        assert_eq!(
            home_socket("gateway"),
            normalize_path(temp.path()).join(".tura/sockets/gateway.sock")
        );
        assert_eq!(locks_dir(), normalize_path(temp.path()).join(".tura/locks"));

        restore_env("TURA_HOME", previous);
    }

    #[test]
    fn gateway_default_ports_match_build_kind_contract() {
        assert_eq!(default_gateway_port_for_build_kind("dev"), 4125);
        assert_eq!(default_gateway_port_for_build_kind("release"), 4126);
        assert_eq!(
            default_gateway_url_for_build_kind("release"),
            "http://127.0.0.1:4126"
        );
    }

    #[test]
    fn active_gateway_env_round_trips_project_url() {
        let temp = tempfile::tempdir().expect("temp home");

        write_active_gateway_url_for_home(temp.path(), "http://127.0.0.1:4777")
            .expect("write active gateway");

        assert_eq!(
            read_active_gateway_url_for_home(temp.path()).as_deref(),
            Some("http://127.0.0.1:4777")
        );
    }

    #[test]
    fn home_db_dir_prefers_session_log_override_then_tura_db_override() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let temp = tempfile::tempdir().expect("temp");
        let session_root = temp.path().join("session-root");
        let tura_root = temp.path().join("tura-root");
        let previous_session = std::env::var_os("SESSION_LOG_DB_ROOT");
        let previous_tura = std::env::var_os("TURA_DB_ROOT");
        std::env::set_var("SESSION_LOG_DB_ROOT", &session_root);
        std::env::set_var("TURA_DB_ROOT", &tura_root);

        assert_eq!(home_db_dir(), session_root.join(DB_DIR_NAME));

        std::env::remove_var("SESSION_LOG_DB_ROOT");
        assert_eq!(home_db_dir(), tura_root.join(DB_DIR_NAME));

        restore_env("SESSION_LOG_DB_ROOT", previous_session);
        restore_env("TURA_DB_ROOT", previous_tura);
    }

    #[test]
    fn home_db_dir_uses_instance_home_when_tura_home_is_explicit() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let temp = tempfile::tempdir().expect("temp home");
        let previous_home = std::env::var_os("TURA_HOME");
        let previous_session = std::env::var_os("SESSION_LOG_DB_ROOT");
        let previous_tura = std::env::var_os("TURA_DB_ROOT");
        std::env::remove_var("SESSION_LOG_DB_ROOT");
        std::env::remove_var("TURA_DB_ROOT");
        std::env::set_var("TURA_HOME", temp.path());

        assert_eq!(
            home_db_dir(),
            normalize_path(temp.path()).join("db").join(DB_DIR_NAME)
        );

        restore_env("TURA_HOME", previous_home);
        restore_env("SESSION_LOG_DB_ROOT", previous_session);
        restore_env("TURA_DB_ROOT", previous_tura);
    }

    #[test]
    fn normalize_workspace_preserves_unc_and_relative_paths_without_trailing_slash() {
        assert_eq!(
            normalize_workspace(r"\\server\share\project\"),
            "//server/share/project"
        );
        assert_eq!(normalize_workspace(r"relative\path\"), "relative/path");
        assert_eq!(normalize_workspace("relative/path///"), "relative/path");
        assert_eq!(normalize_workspace("  C:/Repo/Sub  "), "C:/Repo/Sub");
    }

    #[test]
    fn normalize_path_keeps_nonexistent_paths_comparable() {
        let temp = tempfile::tempdir().expect("temp");
        let missing = temp.path().join("missing").join("child");

        assert_eq!(
            normalize_path(&missing),
            PathBuf::from(strip_verbatim_prefix(&missing.to_string_lossy()))
        );
    }

    fn restore_env(key: &str, previous: Option<std::ffi::OsString>) {
        if let Some(value) = previous {
            std::env::set_var(key, value);
        } else {
            std::env::remove_var(key);
        }
    }
}
