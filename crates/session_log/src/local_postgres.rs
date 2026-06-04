use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use fs2::FileExt;
use postgres::{Client, NoTls};
use postgresql_embedded::blocking::PostgreSQL;
use postgresql_embedded::{SettingsBuilder, Status, V16};

use crate::path::default_db_dir;

const DB_NAME: &str = "session_log";
const DB_USER: &str = "postgres";
const DB_PASSWORD: &str = "postgres";
const DEFAULT_PORT: u16 = 55432;
const STARTUP_LOCK_TIMEOUT: Duration = Duration::from_secs(90);
const READY_TIMEOUT: Duration = Duration::from_secs(30);
const RETRY_INITIAL_DELAY: Duration = Duration::from_millis(50);
const RETRY_MAX_DELAY: Duration = Duration::from_millis(750);

static LOCAL_POSTGRES: OnceLock<Mutex<Option<PostgreSQL>>> = OnceLock::new();

pub fn database_url() -> Result<String> {
    if let Ok(url) =
        std::env::var("session_log_DATABASE_URL").or_else(|_| std::env::var("DATABASE_URL"))
    {
        return Ok(url);
    }

    let guard = LOCAL_POSTGRES.get_or_init(|| Mutex::new(None));
    let mut postgres = guard
        .lock()
        .map_err(|_| anyhow::anyhow!("local PostgreSQL lock poisoned"))?;

    if postgres.is_none() {
        let base_dir = default_db_dir();
        std::fs::create_dir_all(&base_dir)
            .with_context(|| format!("failed to create {}", base_dir.display()))?;
        let settings = SettingsBuilder::new()
            .version((*V16).clone())
            .installation_dir(local_postgres_installation_dir())
            .data_dir(base_dir.join("data"))
            .password_file(base_dir.join("password"))
            .host("127.0.0.1")
            .port(session_log_port())
            .username(DB_USER)
            .password(DB_PASSWORD)
            .temporary(false)
            .timeout(Some(Duration::from_secs(60)))
            .config("max_connections", "100")
            .config("synchronous_commit", "off")
            .build();
        *postgres = Some(PostgreSQL::new(settings));
    }

    let postgres = postgres.as_mut().expect("local PostgreSQL should be set");
    let database_url = postgres.settings().url(DB_NAME);
    if wait_for_database_ready(&database_url, Duration::from_millis(500)) {
        return Ok(database_url);
    }

    let _startup_lock = acquire_startup_lock()?;
    if wait_for_database_ready(&database_url, Duration::from_secs(2)) {
        return Ok(database_url);
    }

    clear_stale_pid_file()?;
    if !matches!(postgres.status(), Status::Started)
        && !wait_for_server_ready(postgres, Duration::from_secs(2))
    {
        if let Err(err) = setup_and_start(postgres) {
            if wait_for_database_ready(&database_url, Duration::from_secs(10)) {
                return Ok(database_url);
            }
            return Err(err);
        }
    }
    ensure_database_exists(postgres)?;
    if !wait_for_database_ready(&database_url, READY_TIMEOUT) {
        anyhow::bail!(
            "local PostgreSQL did not become ready under {} (port={})",
            default_db_dir().display(),
            postgres.settings().port,
        );
    }

    Ok(database_url)
}

struct StartupLock {
    file: std::fs::File,
}

impl Drop for StartupLock {
    fn drop(&mut self) {
        let _ = self.file.unlock();
    }
}

fn acquire_startup_lock() -> Result<StartupLock> {
    let base_dir = default_db_dir();
    std::fs::create_dir_all(&base_dir)
        .with_context(|| format!("failed to create {}", base_dir.display()))?;
    let path = base_dir.join(".startup.lock");
    let file = std::fs::OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .open(&path)
        .with_context(|| format!("failed to open {}", path.display()))?;
    let locked = wait_with_backoff(STARTUP_LOCK_TIMEOUT, || {
        file.try_lock_exclusive().map(|_| true).or_else(|err| {
            if is_lock_contention(&err) {
                Ok(false)
            } else {
                Err(err)
            }
        })
    })
    .with_context(|| format!("failed to lock {}", path.display()))?;
    if !locked {
        anyhow::bail!(
            "timed out waiting for local PostgreSQL startup lock {}",
            path.display()
        );
    }
    file.set_len(0)
        .with_context(|| format!("failed to truncate {}", path.display()))?;
    use std::io::Write;
    writeln!(&file, "pid={}", std::process::id())
        .with_context(|| format!("failed to write {}", path.display()))?;
    Ok(StartupLock { file })
}

fn is_lock_contention(err: &std::io::Error) -> bool {
    err.kind() == std::io::ErrorKind::WouldBlock || matches!(err.raw_os_error(), Some(32 | 33))
}

fn setup_and_start(postgres: &mut PostgreSQL) -> Result<()> {
    let install_dir = postgres.settings().installation_dir.display().to_string();
    let data_dir = postgres.settings().data_dir.display().to_string();
    let port = postgres.settings().port;
    postgres.setup().with_context(|| {
        format!(
            "failed to set up local PostgreSQL under {} (install={}, data={}, port={})",
            default_db_dir().display(),
            install_dir,
            data_dir,
            port,
        )
    })?;
    postgres.start().with_context(|| {
        format!(
            "failed to start local PostgreSQL under {} (install={}, data={}, port={})",
            default_db_dir().display(),
            install_dir,
            data_dir,
            port,
        )
    })?;
    if !wait_for_server_ready(postgres, READY_TIMEOUT) {
        anyhow::bail!(
            "local PostgreSQL server did not become ready under {} (install={}, data={}, port={})",
            default_db_dir().display(),
            install_dir,
            data_dir,
            port,
        );
    }
    Ok(())
}

fn local_postgres_installation_dir() -> PathBuf {
    if let Ok(value) = std::env::var("session_log_POSTGRES_INSTALL_DIR") {
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
    }
    if let Ok(root) = std::env::var("TURA_PROJECT_ROOT") {
        let trimmed = root.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed)
                .join("target")
                .join("session_log")
                .join("postgresql");
        }
    }
    std::env::temp_dir()
        .join("tura")
        .join("session_log")
        .join("postgresql")
}

fn session_log_port() -> u16 {
    std::env::var("session_log_POSTGRES_PORT")
        .ok()
        .and_then(|value| value.parse().ok())
        .filter(|port| *port > 0)
        .unwrap_or(DEFAULT_PORT)
}

fn clear_stale_pid_file() -> Result<()> {
    let pid_file = default_db_dir().join("data").join("postmaster.pid");
    let Ok(content) = std::fs::read_to_string(&pid_file) else {
        return Ok(());
    };
    let Some(pid) = content
        .lines()
        .next()
        .and_then(|line| line.parse::<u32>().ok())
    else {
        return Ok(());
    };
    if process_exists(pid) {
        return Ok(());
    }
    std::fs::remove_file(&pid_file)
        .with_context(|| format!("failed to remove stale {}", pid_file.display()))
}

fn database_ready(database_url: &str) -> bool {
    Client::connect(database_url, NoTls).is_ok()
}

fn server_ready(postgres: &PostgreSQL) -> Result<bool> {
    let maintenance_url = postgres.settings().url("postgres");
    Ok(Client::connect(&maintenance_url, NoTls).is_ok())
}

fn wait_for_database_ready(database_url: &str, timeout: Duration) -> bool {
    wait_with_backoff(timeout, || {
        Ok::<bool, std::convert::Infallible>(database_ready(database_url))
    })
    .unwrap_or(false)
}

fn wait_for_server_ready(postgres: &PostgreSQL, timeout: Duration) -> bool {
    wait_with_backoff(timeout, || server_ready(postgres)).unwrap_or(false)
}

fn wait_with_backoff<E>(
    timeout: Duration,
    mut attempt: impl FnMut() -> std::result::Result<bool, E>,
) -> std::result::Result<bool, E> {
    let started = Instant::now();
    let mut delay = RETRY_INITIAL_DELAY;
    loop {
        if attempt()? {
            return Ok(true);
        }
        if started.elapsed() >= timeout {
            return Ok(false);
        }
        std::thread::sleep(delay);
        delay = (delay * 2).min(RETRY_MAX_DELAY);
    }
}

fn ensure_database_exists(postgres: &PostgreSQL) -> Result<()> {
    let maintenance_url = postgres.settings().url("postgres");
    if let Ok(mut client) = Client::connect(&maintenance_url, NoTls) {
        let exists = client
            .query_opt("SELECT 1 FROM pg_database WHERE datname = $1", &[&DB_NAME])
            .context("failed to check local PostgreSQL session_log database")?
            .is_some();
        if !exists {
            client
                .execute(&format!("CREATE DATABASE {DB_NAME}"), &[])
                .with_context(|| format!("failed to create local PostgreSQL database {DB_NAME}"))?;
        }
        return Ok(());
    }

    if !postgres.database_exists(DB_NAME)? {
        postgres
            .create_database(DB_NAME)
            .with_context(|| format!("failed to create local PostgreSQL database {DB_NAME}"))?;
    }
    Ok(())
}

#[cfg(windows)]
fn process_exists(pid: u32) -> bool {
    let Ok(output) = std::process::Command::new("tasklist")
        .args(["/FI", &format!("PID eq {pid}"), "/FO", "CSV", "/NH"])
        .output()
    else {
        return true;
    };
    String::from_utf8_lossy(&output.stdout).contains(&pid.to_string())
}

#[cfg(not(windows))]
fn process_exists(pid: u32) -> bool {
    std::process::Command::new("kill")
        .args(["-0", &pid.to_string()])
        .status()
        .map(|status| status.success())
        .unwrap_or(true)
}
