#![deny(clippy::unwrap_used)]
#![forbid(unsafe_code)]

pub mod api;
pub mod channel;
pub mod contracts;
pub mod handler;
pub mod media;
pub mod mock;
pub mod process_lock;
pub mod router_client;
pub mod router_process;
pub mod runtime;
pub mod session;
pub mod session_db_client;
pub mod session_log_writer;
pub mod simple_runtime;
pub mod tray;
pub mod types;
pub mod web;

pub use channel::ChannelSender;
pub use handler::ProcessedMessageHandler;
pub use media::GatewayMediaProcessor;
pub use runtime::GatewayRuntime;
pub use session::{session_store, SessionInfo, SessionManager, SessionStatus, SessionStore};
pub use simple_runtime::{SimpleGatewayRuntime, SimpleMessageHandler};
pub use types::*;

#[cfg(test)]
pub(crate) mod test_support {
    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    static CURRENT_DIRECTORY_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    pub(crate) struct EnvRestore {
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
            for (key, value) in &self.keys {
                match value {
                    Some(value) => std::env::set_var(key, value),
                    None => std::env::remove_var(key),
                }
            }
        }
    }

    pub(crate) struct SessionDbTestService {
        _guard: std::sync::MutexGuard<'static, ()>,
        _env: EnvRestore,
        _root: tempfile::TempDir,
        handle: Option<std::thread::JoinHandle<anyhow::Result<()>>>,
    }

    impl SessionDbTestService {
        pub(crate) fn start() -> Self {
            let guard = env_lock();
            let env = EnvRestore::capture(&["TURA_HOME", "SESSION_LOG_DB_ROOT", "TURA_DB_ROOT"]);
            let root = tempfile::tempdir().expect("session db root");
            let home = root.path().join("home");
            std::fs::create_dir_all(&home).expect("session db home");
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
                        Err(_) => {
                            "service thread panicked before publishing service.addr".to_string()
                        }
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

    pub(crate) fn env_lock() -> std::sync::MutexGuard<'static, ()> {
        ENV_LOCK.lock().unwrap_or_else(|error| error.into_inner())
    }

    pub(crate) fn current_directory_lock() -> std::sync::MutexGuard<'static, ()> {
        CURRENT_DIRECTORY_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner())
    }
}
