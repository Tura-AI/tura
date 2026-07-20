use std::sync::MutexGuard;

pub struct SessionDbTestService {
    _guard: MutexGuard<'static, ()>,
    _env: EnvRestore,
    _root: tempfile::TempDir,
    handle: Option<std::thread::JoinHandle<anyhow::Result<()>>>,
}

impl SessionDbTestService {
    pub fn start(lock: &'static std::sync::Mutex<()>) -> Self {
        let guard = lock.lock().unwrap_or_else(|error| error.into_inner());
        let env = EnvRestore::capture(&["TURA_HOME", "SESSION_LOG_DB_ROOT", "TURA_DB_ROOT"]);
        let root = tempfile::tempdir().expect("session_db root");
        let home = root.path().join("home");
        std::fs::create_dir_all(&home).expect("session_db home");
        std::env::set_var("TURA_HOME", &home);
        std::env::set_var("SESSION_LOG_DB_ROOT", root.path());
        std::env::remove_var("TURA_DB_ROOT");

        let handle = std::thread::spawn(session_log::service::run_socket_service);
        let started = std::time::Instant::now();
        while started.elapsed() < std::time::Duration::from_secs(10) {
            if handle.is_finished() {
                let detail = match handle.join() {
                    Ok(Ok(())) => "service exited without publishing service.addr".to_string(),
                    Ok(Err(error)) => format!("service exited with error: {error:#}"),
                    Err(_) => "service thread panicked before publishing service.addr".to_string(),
                };
                panic!("session_db test service did not become reachable: {detail}");
            }
            if session_log::ipc::service_is_running() {
                return Self {
                    _guard: guard,
                    _env: env,
                    _root: root,
                    handle: Some(handle),
                };
            }
            std::thread::sleep(std::time::Duration::from_millis(25));
        }
        panic!(
            "session_db test service did not become reachable within 10s; addr_path={}",
            session_log::ipc::service_addr_path().display()
        );
    }
}

impl Drop for SessionDbTestService {
    fn drop(&mut self) {
        let _ = session_log::ipc::call_service(&session_log_contract::SessionLogCommand::Shutdown);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

struct EnvRestore {
    previous: Vec<(&'static str, Option<std::ffi::OsString>)>,
}

impl EnvRestore {
    fn capture(keys: &[&'static str]) -> Self {
        Self {
            previous: keys
                .iter()
                .map(|key| (*key, std::env::var_os(key)))
                .collect(),
        }
    }
}

impl Drop for EnvRestore {
    fn drop(&mut self) {
        for (key, value) in &self.previous {
            match value {
                Some(value) => std::env::set_var(key, value),
                None => std::env::remove_var(key),
            }
        }
    }
}
