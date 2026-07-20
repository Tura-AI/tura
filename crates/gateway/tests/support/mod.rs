use session_log::SessionLogStore;
use session_log_contract::SessionLogCommand;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

pub struct TestSessionDb {
    _env: EnvRestore,
    _root: tempfile::TempDir,
    _guard: std::sync::MutexGuard<'static, ()>,
    workspace: PathBuf,
    handle: Option<std::thread::JoinHandle<anyhow::Result<()>>>,
}

impl TestSessionDb {
    pub fn start() -> anyhow::Result<Self> {
        let guard = ENV_LOCK.lock().unwrap_or_else(|error| error.into_inner());
        let env = EnvRestore::capture(&["TURA_HOME", "TURA_DB_ROOT", "SESSION_LOG_DB_ROOT"]);
        let root = tempfile::tempdir()?;
        let home = root.path().join("home");
        let workspace = root.path().join("workspace");
        std::fs::create_dir_all(&home)?;
        std::fs::create_dir_all(&workspace)?;
        std::env::set_var("TURA_HOME", &home);
        std::env::remove_var("TURA_DB_ROOT");
        std::env::remove_var("SESSION_LOG_DB_ROOT");

        let store = SessionLogStore::open_default()?;
        let handle = std::thread::spawn(move || session_log::ipc::serve_blocking(store));
        let started = Instant::now();
        while started.elapsed() < Duration::from_secs(10) {
            if handle.is_finished() {
                let detail = match handle.join() {
                    Ok(Ok(())) => "service exited before becoming reachable".to_string(),
                    Ok(Err(error)) => format!("service exited with error: {error:#}"),
                    Err(_) => "service thread panicked before becoming reachable".to_string(),
                };
                anyhow::bail!(detail);
            }
            if session_log::ipc::service_is_running() {
                return Ok(Self {
                    _env: env,
                    _root: root,
                    _guard: guard,
                    workspace,
                    handle: Some(handle),
                });
            }
            std::thread::sleep(Duration::from_millis(25));
        }
        anyhow::bail!("session DB service did not become reachable within 10s")
    }

    pub fn workspace(&self) -> &Path {
        &self.workspace
    }
}

impl Drop for TestSessionDb {
    fn drop(&mut self) {
        let _ = session_log::ipc::call_service(&SessionLogCommand::Shutdown);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

struct EnvRestore {
    keys: Vec<(&'static str, Option<std::ffi::OsString>)>,
}

impl EnvRestore {
    fn capture(keys: &[&'static str]) -> Self {
        Self {
            keys: keys
                .iter()
                .map(|key| (*key, std::env::var_os(key)))
                .collect(),
        }
    }
}

impl Drop for EnvRestore {
    fn drop(&mut self) {
        for (key, value) in self.keys.drain(..).rev() {
            match value {
                Some(value) => std::env::set_var(key, value),
                None => std::env::remove_var(key),
            }
        }
    }
}
