//! Router-owned session DB service lifecycle.
//!
//! Router starts and health-checks the service, but it must not parse or proxy
//! normal session DB read/write payloads.

use anyhow::{anyhow, Result};
use serde_json::json;
use std::{
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

const SESSION_DB_STARTUP_TIMEOUT: Duration = Duration::from_secs(30);
const SESSION_DB_STARTUP_POLL: Duration = Duration::from_millis(50);

#[derive(Clone)]
pub struct SessionDbService {
    child: Arc<Mutex<Option<Child>>>,
}

impl SessionDbService {
    pub fn new() -> Self {
        Self {
            child: Arc::new(Mutex::new(None)),
        }
    }

    pub fn start(&self) -> Result<serde_json::Value> {
        if self.is_ready() {
            return Ok(self.status_payload("running"));
        }
        self.kill_managed_child();
        stop_unmanaged_session_db_service();
        let service_bin = session_db_binary()
            .ok_or_else(|| anyhow!("session_db service executable tura_session_db not found"))?;
        // tura_session_db owns the SQLite session-log write path. It serves a
        // socket and does not read stdin, so no stdio protocol is wired here.
        let mut command = Command::new(&service_bin);
        command
            .env("TURA_HOME", tura_path::instance_home())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        if let Some(root) = std::env::var_os("TURA_PROJECT_ROOT") {
            command.env("TURA_PROJECT_ROOT", root);
        }
        tura_path::process_hardening::hide_child_console_window(&mut command);
        let child = command.spawn()?;
        *self
            .child
            .lock()
            .map_err(|_| anyhow!("session_db lock poisoned"))? = Some(child);
        if let Err(error) = self.wait_until_running(SESSION_DB_STARTUP_TIMEOUT) {
            self.kill_managed_child();
            return Err(error);
        }
        Ok(self.status_payload("running"))
    }

    pub fn restart(&self) -> Result<serde_json::Value> {
        self.stop();
        self.start()
    }

    pub fn status(&self) -> serde_json::Value {
        self.status_payload(if self.is_ready() {
            "running"
        } else {
            "stopped"
        })
    }

    pub fn stop(&self) {
        let child = self.child.lock().ok().and_then(|mut guard| guard.take());
        if session_log::ipc::service_is_running() {
            let _ = session_log::ipc::call_service(&session_log::SessionLogCommand::Shutdown);
        }
        if let Some(mut child) = child {
            if !wait_for_child_exit(&mut child, std::time::Duration::from_secs(10)) {
                let _ = child.kill();
                let _ = child.wait();
            }
        }
        let _ = std::fs::remove_file(session_log::ipc::service_addr_path());
    }

    fn is_ready(&self) -> bool {
        let Ok(mut guard) = self.child.lock() else {
            return false;
        };
        match guard.as_mut() {
            Some(child) => match child.try_wait() {
                Ok(None) => session_log::ipc::service_is_running(),
                Ok(Some(_)) | Err(_) => {
                    *guard = None;
                    false
                }
            },
            None => false,
        }
    }

    fn status_payload(&self, status: &str) -> serde_json::Value {
        let pid = self
            .child
            .lock()
            .ok()
            .and_then(|guard| guard.as_ref().map(Child::id));
        json!({ "status": status, "pid": pid })
    }

    fn wait_until_running(&self, timeout: Duration) -> Result<()> {
        let started = Instant::now();
        loop {
            if session_log::ipc::service_is_running() {
                return Ok(());
            }
            {
                let mut guard = self
                    .child
                    .lock()
                    .map_err(|_| anyhow!("session_db lock poisoned"))?;
                if let Some(child) = guard.as_mut() {
                    if let Some(status) = child.try_wait()? {
                        *guard = None;
                        return Err(anyhow!(
                            "session_db service exited before publishing a reachable socket: {status}"
                        ));
                    }
                }
            }
            if started.elapsed() >= timeout {
                return Err(anyhow!(
                    "timed out waiting for session_db service to publish a reachable socket"
                ));
            }
            std::thread::sleep(SESSION_DB_STARTUP_POLL);
        }
    }

    fn kill_managed_child(&self) {
        let child = self.child.lock().ok().and_then(|mut guard| guard.take());
        if let Some(mut child) = child {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

fn wait_for_child_exit(child: &mut Child, timeout: Duration) -> bool {
    let started = Instant::now();
    while started.elapsed() < timeout {
        if matches!(child.try_wait(), Ok(Some(_))) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    false
}

fn stop_unmanaged_session_db_service() {
    if !session_log::ipc::service_is_running() {
        return;
    }
    let _ = session_log::ipc::call_service(&session_log::SessionLogCommand::Shutdown);
    let started = Instant::now();
    while started.elapsed() < Duration::from_secs(10) {
        if !session_log::ipc::service_is_running() {
            return;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    let _ = std::fs::remove_file(session_log::ipc::service_addr_path());
}

fn session_db_binary() -> Option<PathBuf> {
    resolve_binary(if cfg!(windows) {
        "tura_session_db.exe"
    } else {
        "tura_session_db"
    })
}

fn resolve_binary(executable: &str) -> Option<PathBuf> {
    let root = std::env::var("TURA_PROJECT_ROOT")
        .ok()
        .map(PathBuf::from)
        .or_else(|| {
            std::env::current_exe()
                .ok()
                .as_deref()
                .and_then(find_repo_root)
        })
        .or_else(|| {
            std::env::current_dir()
                .ok()
                .as_deref()
                .and_then(find_repo_root)
        })?;
    let mut candidates = Vec::new();
    if let Ok(current_exe) = std::env::current_exe() {
        candidates.push(current_exe.with_file_name(executable));
    }
    candidates.push(root.join("bin").join(executable));
    candidates.push(root.join("target").join("release").join(executable));
    candidates.push(root.join("target").join("debug").join(executable));
    candidates.into_iter().find(|path| path.exists())
}

fn find_repo_root(path: &Path) -> Option<PathBuf> {
    let start = if path.is_dir() {
        path
    } else {
        path.parent().unwrap_or(path)
    };
    start
        .ancestors()
        .find(|candidate| candidate.join("crates").join("router").is_dir())
        .map(Path::to_path_buf)
}

#[cfg(test)]
mod tests {
    use super::{find_repo_root, resolve_binary, SessionDbService};
    use std::net::TcpListener;
    use std::path::PathBuf;

    #[test]
    fn status_payload_reports_requested_status_without_child_pid() {
        let service = SessionDbService::new();
        let payload = service.status_payload("running");

        assert_eq!(payload["status"], "running");
        assert!(payload["pid"].is_null());
    }

    #[test]
    fn status_does_not_adopt_socket_that_fails_session_db_health() -> anyhow::Result<()> {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|error| error.into_inner());
        let home = tempfile::tempdir()?;
        let _env = EnvGuard::set_home(home.path());
        let listener = TcpListener::bind(("127.0.0.1", 0))?;
        let addr = listener.local_addr()?;
        let server = std::thread::spawn(move || -> anyhow::Result<()> {
            let (stream, _) = listener.accept()?;
            drop(stream);
            Ok(())
        });

        let path = session_log::ipc::service_addr_path();
        std::fs::create_dir_all(path.parent().expect("service addr parent"))?;
        std::fs::write(
            &path,
            serde_json::to_string(&session_log::ipc::ServiceEndpoint {
                addr: addr.to_string(),
                version: tura_path::instance_version(),
            })?,
        )?;

        let service = SessionDbService::new();
        let status = service.status();

        assert_eq!(
            status["status"], "stopped",
            "router must not adopt a socket that does not answer session_db health: {status}"
        );
        assert!(
            !path.exists(),
            "failed session_db health probes should remove the stale endpoint"
        );
        server
            .join()
            .map_err(|_| anyhow::anyhow!("abortive session_db endpoint panicked"))??;
        Ok(())
    }

    #[test]
    fn find_repo_root_walks_up_from_files_and_directories() {
        let crate_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let workspace_root = crate_root
            .parent()
            .and_then(std::path::Path::parent)
            .map(std::path::Path::to_path_buf)
            .expect("router crate should live under workspace/crates/router");
        let file_path = crate_root
            .join("src")
            .join("services")
            .join("session_db.rs");

        assert_eq!(find_repo_root(&file_path), Some(workspace_root.clone()));
        assert_eq!(
            find_repo_root(&crate_root.join("src")),
            Some(workspace_root)
        );
    }

    #[test]
    fn find_repo_root_returns_none_outside_repository_shape() {
        let outside = std::env::temp_dir().join("tura-router-session-db-test-outside");

        assert_eq!(find_repo_root(&outside), None);
    }

    #[test]
    fn resolve_binary_returns_none_for_unknown_executable() {
        assert_eq!(
            resolve_binary("definitely-missing-tura-session-db-test-binary"),
            None
        );
    }

    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    struct EnvGuard {
        previous: Vec<(&'static str, Option<std::ffi::OsString>)>,
    }

    impl EnvGuard {
        fn set_home(home: &std::path::Path) -> Self {
            let keys = [
                "TURA_HOME",
                "SESSION_LOG_DB_ROOT",
                "TURA_DB_ROOT",
                "TURA_SESSION_DB_PROBE_TIMEOUT_MS",
            ];
            let previous = keys
                .iter()
                .map(|key| (*key, std::env::var_os(key)))
                .collect::<Vec<_>>();
            std::env::set_var("TURA_HOME", home);
            std::env::remove_var("SESSION_LOG_DB_ROOT");
            std::env::remove_var("TURA_DB_ROOT");
            std::env::set_var("TURA_SESSION_DB_PROBE_TIMEOUT_MS", "25");
            Self { previous }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            for (key, value) in self.previous.drain(..) {
                match value {
                    Some(value) => std::env::set_var(key, value),
                    None => std::env::remove_var(key),
                }
            }
        }
    }
}
