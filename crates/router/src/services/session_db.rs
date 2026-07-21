//! Router-owned session DB service lifecycle.
//!
//! Router starts and health-checks the service, but it must not parse or proxy
//! normal session DB read/write payloads.

use anyhow::{anyhow, Result};
use serde_json::json;
use session_log_contract::client::{
    call_service, service_addr_path, service_is_running, unreachable_owner_lock_message,
};
use std::{
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};

const SESSION_DB_STARTUP_TIMEOUT: Duration = Duration::from_secs(30);
const SESSION_DB_STARTUP_POLL: Duration = Duration::from_millis(50);

#[derive(Clone)]
pub struct SessionDbService {
    child: Arc<Mutex<Option<Child>>>,
    lifecycle: Arc<Mutex<()>>,
    shutdown_requested: Arc<AtomicBool>,
}

impl SessionDbService {
    pub fn new() -> Self {
        Self {
            child: Arc::new(Mutex::new(None)),
            lifecycle: Arc::new(Mutex::new(())),
            shutdown_requested: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn start(&self) -> Result<serde_json::Value> {
        let _lifecycle = self
            .lifecycle
            .lock()
            .map_err(|_| anyhow!("session_db lifecycle lock poisoned"))?;
        self.start_locked()
    }

    fn start_locked(&self) -> Result<serde_json::Value> {
        if self.shutdown_requested.load(Ordering::SeqCst) {
            return Err(anyhow!("session_db lifecycle is shutting down"));
        }
        if self.is_ready() {
            return Ok(self.status_payload("running"));
        }
        self.kill_managed_child();
        if service_is_running() {
            return Ok(self.status_payload("running"));
        }
        if let Some(message) = unreachable_owner_lock_message() {
            return Err(anyhow!(message));
        }
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
        let _lifecycle = self
            .lifecycle
            .lock()
            .map_err(|_| anyhow!("session_db lifecycle lock poisoned"))?;
        if self.shutdown_requested.load(Ordering::SeqCst) {
            return Err(anyhow!("session_db lifecycle is shutting down"));
        }
        self.stop_locked();
        self.start_locked()
    }

    pub fn status(&self) -> serde_json::Value {
        self.status_payload(if self.is_ready() {
            "running"
        } else {
            "stopped"
        })
    }

    pub fn stop(&self) {
        let Ok(_lifecycle) = self.lifecycle.lock() else {
            return;
        };
        self.stop_locked();
    }

    pub fn shutdown(&self) {
        self.shutdown_requested.store(true, Ordering::SeqCst);
        self.stop();
    }

    fn stop_locked(&self) {
        let child = self.child.lock().ok().and_then(|mut guard| guard.take());
        if service_is_running() {
            let _ = call_service(&session_log_contract::SessionLogCommand::Shutdown);
        }
        if let Some(mut child) = child {
            if !wait_for_child_exit(&mut child, std::time::Duration::from_secs(10)) {
                let _ = child.kill();
                let _ = child.wait();
            }
        }
        let _ = std::fs::remove_file(service_addr_path());
    }

    fn is_ready(&self) -> bool {
        let Ok(mut guard) = self.child.lock() else {
            return false;
        };
        match guard.as_mut() {
            Some(child) => match child.try_wait() {
                Ok(None) => service_is_running(),
                Ok(Some(_)) | Err(_) => {
                    *guard = None;
                    service_is_running()
                }
            },
            None => service_is_running(),
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
            if service_is_running() {
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
                        let detail = unreachable_owner_lock_message()
                            .map(|message| format!("; {message}"))
                            .unwrap_or_default();
                        return Err(anyhow!(
                            "session_db service exited before publishing a reachable socket: {status}{detail}"
                        ));
                    }
                }
            }
            if started.elapsed() >= timeout {
                let detail = unreachable_owner_lock_message()
                    .map(|message| format!("; {message}"))
                    .unwrap_or_default();
                return Err(anyhow!(
                    "timed out waiting for session_db service to publish a reachable socket{detail}"
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
    use session_log_contract::client::service_addr_path;
    use std::io::{BufRead, Write};
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
    fn shutdown_is_terminal_for_later_start_and_restart_requests() -> anyhow::Result<()> {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|error| error.into_inner());
        let home = tempfile::tempdir()?;
        let _env = EnvGuard::set_home(home.path());
        let service = SessionDbService::new();

        service.shutdown();

        let error = service
            .start()
            .expect_err("shutdown session_db service must not restart");
        assert!(error.to_string().contains("lifecycle is shutting down"));
        let error = service
            .restart()
            .expect_err("shutdown session_db service must reject explicit restart");
        assert!(error.to_string().contains("lifecycle is shutting down"));
        Ok(())
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

        let path = service_addr_path();
        std::fs::create_dir_all(path.parent().expect("service addr parent"))?;
        std::fs::write(
            &path,
            serde_json::to_string(&session_log_contract::ServiceEndpoint {
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
    fn start_adopts_reachable_existing_session_db_endpoint() -> anyhow::Result<()> {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|error| error.into_inner());
        let home = tempfile::tempdir()?;
        let _env = EnvGuard::set_home(home.path());
        let listener = TcpListener::bind(("127.0.0.1", 0))?;
        let addr = listener.local_addr()?;
        let server = std::thread::spawn(move || -> anyhow::Result<()> {
            let (mut stream, _) = listener.accept()?;
            let mut request = String::new();
            std::io::BufReader::new(stream.try_clone()?).read_line(&mut request)?;
            assert!(request.contains("\"health\""));
            stream.write_all(
                serde_json::to_string(&session_log_contract::SessionLogResponse::Ok)?.as_bytes(),
            )?;
            stream.write_all(b"\n")?;
            stream.flush()?;
            Ok(())
        });

        let path = service_addr_path();
        std::fs::create_dir_all(path.parent().expect("service addr parent"))?;
        std::fs::write(
            &path,
            serde_json::to_string(&session_log_contract::ServiceEndpoint {
                addr: addr.to_string(),
                version: tura_path::instance_version(),
            })?,
        )?;

        let service = SessionDbService::new();
        let status = service.start()?;

        assert_eq!(
            status["status"], "running",
            "router should adopt an already reachable same-version session_db: {status}"
        );
        assert!(
            status["pid"].is_null(),
            "adopting an external session_db should not fabricate a child pid: {status}"
        );
        assert!(
            path.exists(),
            "adopted session_db endpoint should remain published"
        );
        server
            .join()
            .map_err(|_| anyhow::anyhow!("adopted session_db endpoint panicked"))??;
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
