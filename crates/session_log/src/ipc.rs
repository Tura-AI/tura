//! Session DB service socket IPC.
//!
//! Stage 1 of the single-DB-owner refactor: the `tura_session_db` process is the
//! owner of session-log SQLite writes. Every other process (runtime, gateway,
//! CLI front) reaches the store through this socket instead of opening its own
//! store.
//!
//! Transport: loopback TCP on an ephemeral port published to an address file
//! under the db directory. Each client opens its own short-lived connection, so
//! concurrent callers never serialize behind a single pipe (no head-of-line
//! blocking). A later stage migrates the address to `instance_home` and may swap
//! the transport for a Unix socket / Windows named pipe; callers only depend on
//! [`call_service`] / [`serve_blocking`].

use std::io::{BufRead, BufReader, Write};
use std::io::{Error as IoError, ErrorKind};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use crate::path::default_db_dir;
use crate::SessionLogStore;
use anyhow::{anyhow, Context, Result};
use session_log_contract::{ServiceEndpoint, SessionLogCommand, SessionLogResponse};

const ADDR_FILE: &str = "service.addr";
const CONNECT_TIMEOUT: Duration = Duration::from_secs(2);
const PROBE_CONNECT_TIMEOUT: Duration = Duration::from_millis(100);
const PROBE_RESPONSE_TIMEOUT: Duration = Duration::from_millis(500);
const PROBE_RESPONSE_ATTEMPTS: usize = 3;
const PROBE_RETRY_DELAY: Duration = Duration::from_millis(50);
const READ_TIMEOUT: Duration = Duration::from_secs(60);
const EMPTY_RESPONSE_RETRIES: usize = 3;
static SHUTDOWN_REQUESTED: AtomicBool = AtomicBool::new(false);

/// Path to the file that records the running service's endpoint.
pub fn service_addr_path() -> PathBuf {
    default_db_dir().join(ADDR_FILE)
}

fn read_endpoint() -> Result<ServiceEndpoint> {
    let path = service_addr_path();
    let raw = std::fs::read_to_string(&path).with_context(|| {
        format!(
            "session_db service address file {} not found",
            path.display()
        )
    })?;
    serde_json::from_str::<ServiceEndpoint>(raw.trim())
        .with_context(|| format!("invalid session_db endpoint record {}", path.display()))
}

fn parse_addr(endpoint: &ServiceEndpoint) -> Result<SocketAddr> {
    endpoint
        .addr
        .parse()
        .with_context(|| format!("invalid session_db service address {:?}", endpoint.addr))
}

/// Refuse to use a service from a different build.
fn ensure_version_compatible(endpoint: &ServiceEndpoint) -> Result<()> {
    let expected = tura_path::instance_version();
    if endpoint.version != expected {
        return Err(anyhow!(
            "session_db service version {} does not match client {}; refusing to use a service \
             from a different build",
            endpoint.version,
            expected
        ));
    }
    Ok(())
}

/// The version reported by the running service, if any is published.
pub fn service_version() -> Option<String> {
    read_endpoint().ok().map(|endpoint| endpoint.version)
}

/// True when a session_db service is reachable on the published address and
/// responds to the session_db protocol health command.
pub fn service_is_running() -> bool {
    let endpoint = match read_endpoint() {
        Ok(endpoint) => endpoint,
        Err(_) => return false,
    };
    if ensure_version_compatible(&endpoint).is_err() {
        let _ = std::fs::remove_file(service_addr_path());
        return false;
    }
    let addr = match parse_addr(&endpoint) {
        Ok(addr) => addr,
        Err(_) => {
            let _ = std::fs::remove_file(service_addr_path());
            return false;
        }
    };
    if probe_session_db(&addr) {
        true
    } else {
        let _ = std::fs::remove_file(service_addr_path());
        false
    }
}

fn probe_session_db(addr: &SocketAddr) -> bool {
    for attempt in 0..PROBE_RESPONSE_ATTEMPTS {
        match call_service_addr(
            addr,
            &SessionLogCommand::Health,
            probe_connect_timeout(),
            probe_response_timeout(),
        ) {
            Ok(SessionLogResponse::Ok) => return true,
            Ok(_) => return false,
            Err(error) if is_retryable_probe_error(&error) => {
                if attempt + 1 < PROBE_RESPONSE_ATTEMPTS {
                    std::thread::sleep(PROBE_RETRY_DELAY);
                    continue;
                }
                return false;
            }
            Err(_) => return false,
        }
    }
    false
}

fn probe_connect_timeout() -> Duration {
    std::env::var("TURA_SESSION_DB_PROBE_TIMEOUT_MS")
        .ok()
        .and_then(|value| value.trim().parse::<u64>().ok())
        .filter(|millis| *millis > 0)
        .map(Duration::from_millis)
        .unwrap_or(PROBE_CONNECT_TIMEOUT)
}

fn probe_response_timeout() -> Duration {
    std::env::var("TURA_SESSION_DB_PROBE_RESPONSE_TIMEOUT_MS")
        .ok()
        .and_then(|value| value.trim().parse::<u64>().ok())
        .filter(|millis| *millis > 0)
        .map(Duration::from_millis)
        .unwrap_or(PROBE_RESPONSE_TIMEOUT)
}

/// Send a single command to the running session_db service and await the
/// response. Returns `Err` when no service is reachable or its version does not
/// match this build.
pub fn call_service(command: &SessionLogCommand) -> Result<SessionLogResponse> {
    let mut last_transient_response_error = None;
    for attempt in 0..EMPTY_RESPONSE_RETRIES {
        match call_service_once(command) {
            Ok(response) => return Ok(response),
            Err(error) if is_transient_response_error(&error) => {
                last_transient_response_error = Some(error);
                if attempt + 1 < EMPTY_RESPONSE_RETRIES {
                    std::thread::sleep(Duration::from_millis(20));
                    continue;
                }
            }
            Err(error) => return Err(error),
        }
    }
    Err(last_transient_response_error
        .unwrap_or_else(|| anyhow!("session_db service call did not run")))
}

fn call_service_once(command: &SessionLogCommand) -> Result<SessionLogResponse> {
    let endpoint = read_endpoint()?;
    ensure_version_compatible(&endpoint)?;
    let addr = parse_addr(&endpoint)?;
    call_service_addr(&addr, command, CONNECT_TIMEOUT, READ_TIMEOUT)
}

fn call_service_addr(
    addr: &SocketAddr,
    command: &SessionLogCommand,
    connect_timeout: Duration,
    read_timeout: Duration,
) -> Result<SessionLogResponse> {
    let stream = TcpStream::connect_timeout(addr, connect_timeout)
        .with_context(|| format!("failed to connect to session_db service at {addr}"))?;
    stream.set_read_timeout(Some(read_timeout))?;
    stream.set_write_timeout(Some(read_timeout))?;
    let mut writer = stream.try_clone()?;
    let line = serde_json::to_string(command)?;
    writer.write_all(line.as_bytes())?;
    writer.write_all(b"\n")?;
    writer.flush()?;

    let mut reader = BufReader::new(stream);
    let mut response_line = String::new();
    let read = reader.read_line(&mut response_line)?;
    if read == 0 || response_line.trim().is_empty() {
        return Err(anyhow!(
            "session_db service at {addr} closed without a response"
        ));
    }
    serde_json::from_str(response_line.trim())
        .with_context(|| "failed to decode session_db service response")
}

fn is_transient_response_error(error: &anyhow::Error) -> bool {
    error.chain().any(|cause| {
        cause.to_string().contains("closed without a response")
            || cause.downcast_ref::<IoError>().is_some_and(|io_error| {
                matches!(
                    io_error.kind(),
                    ErrorKind::ConnectionAborted
                        | ErrorKind::ConnectionReset
                        | ErrorKind::UnexpectedEof
                        | ErrorKind::BrokenPipe
                )
            })
    })
}

fn is_retryable_probe_error(error: &anyhow::Error) -> bool {
    error.chain().any(|cause| {
        cause.to_string().contains("closed without a response")
            || cause.downcast_ref::<IoError>().is_some_and(|io_error| {
                matches!(
                    io_error.kind(),
                    ErrorKind::ConnectionAborted
                        | ErrorKind::ConnectionReset
                        | ErrorKind::UnexpectedEof
                        | ErrorKind::BrokenPipe
                        | ErrorKind::TimedOut
                        | ErrorKind::WouldBlock
                )
            })
    })
}

/// Execute a command against an owned store. Shared by the socket server and the
/// `tura_session_db` admin CLI so the data path has one implementation.
pub fn dispatch_command(store: &SessionLogStore, command: SessionLogCommand) -> SessionLogResponse {
    match dispatch_inner(store, command) {
        Ok(response) => response,
        Err(error) => SessionLogResponse::Error {
            error: error.to_string(),
        },
    }
}

fn dispatch_inner(
    store: &SessionLogStore,
    command: SessionLogCommand,
) -> Result<SessionLogResponse> {
    Ok(match command {
        SessionLogCommand::Health => SessionLogResponse::Ok,
        SessionLogCommand::UpsertSession(payload) => {
            store.upsert_session(payload)?;
            SessionLogResponse::Ok
        }
        SessionLogCommand::ApplyCommandCheckpoint(payload) => {
            store.apply_command_checkpoint(*payload)?;
            SessionLogResponse::Ok
        }
        SessionLogCommand::ListWorkspaces => SessionLogResponse::Workspaces {
            workspaces: store.list_workspaces()?,
        },
        SessionLogCommand::GetSession(payload) => SessionLogResponse::Session {
            session: store.get_session(payload)?.map(Box::new),
        },
        SessionLogCommand::ListSessions(payload) => {
            let (page, sessions) = store.list_sessions(payload)?;
            SessionLogResponse::Sessions { page, sessions }
        }
        SessionLogCommand::ListSessionSummaries(payload) => {
            let (page, sessions) = store.list_session_summaries(payload)?;
            SessionLogResponse::SessionSummaries { page, sessions }
        }
        SessionLogCommand::ListSessionRecords(payload) => {
            let (page, records) = store.list_session_records(payload)?;
            SessionLogResponse::Records { page, records }
        }
        SessionLogCommand::MarkSessionInterrupted(payload) => {
            store.mark_session_interrupted(payload)?;
            SessionLogResponse::Ok
        }
        SessionLogCommand::DeleteSession(payload) => {
            store.delete_session(payload)?;
            SessionLogResponse::Ok
        }
        SessionLogCommand::DeleteWorkspace(payload) => {
            store.delete_workspace(payload)?;
            SessionLogResponse::Ok
        }
        SessionLogCommand::Shutdown => SessionLogResponse::Ok,
    })
}

/// Bind the service socket, publish its address, and serve commands until the
/// process exits. One thread per accepted connection; the store clone shares the
/// underlying connection pool, so concurrent clients run in parallel.
pub fn serve_blocking(store: SessionLogStore) -> Result<()> {
    SHUTDOWN_REQUESTED.store(false, Ordering::SeqCst);
    let listener =
        TcpListener::bind(("127.0.0.1", 0)).context("failed to bind session_db service socket")?;
    listener.set_nonblocking(true)?;
    let addr = listener.local_addr()?;
    publish_addr(&addr)?;
    tracing::info!(address = %addr, "session_db service listening");

    while !SHUTDOWN_REQUESTED.load(Ordering::SeqCst) {
        match listener.accept() {
            Ok((stream, _peer_addr)) => {
                let store = store.clone();
                std::thread::spawn(move || {
                    if let Err(error) = handle_connection(store, stream) {
                        tracing::warn!(error = %error, "session_db connection ended with error");
                    }
                });
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(25));
            }
            Err(error) => {
                tracing::warn!(error = %error, "session_db accept failed");
            }
        }
    }
    let _ = std::fs::remove_file(service_addr_path());
    Ok(())
}

fn publish_addr(addr: &SocketAddr) -> Result<()> {
    let path = service_addr_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    let endpoint = ServiceEndpoint {
        addr: addr.to_string(),
        version: tura_path::instance_version(),
    };
    let tmp = path.with_extension("addr.tmp");
    std::fs::write(&tmp, serde_json::to_string(&endpoint)?)
        .with_context(|| format!("failed to write {}", tmp.display()))?;
    std::fs::rename(&tmp, &path)
        .with_context(|| format!("failed to publish {}", path.display()))?;
    Ok(())
}

fn handle_connection(store: SessionLogStore, stream: TcpStream) -> Result<()> {
    stream.set_read_timeout(Some(READ_TIMEOUT))?;
    let mut writer = stream.try_clone()?;
    let reader = BufReader::new(stream);
    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let response = match serde_json::from_str::<SessionLogCommand>(line.trim()) {
            Ok(command) => {
                let shutdown = matches!(command, SessionLogCommand::Shutdown);
                let response = dispatch_command(&store, command);
                if shutdown {
                    request_shutdown();
                }
                response
            }
            Err(error) => SessionLogResponse::Error {
                error: format!("invalid session_db request: {error}"),
            },
        };
        let encoded = serde_json::to_string(&response)?;
        writer.write_all(encoded.as_bytes())?;
        writer.write_all(b"\n")?;
        writer.flush()?;
    }
    Ok(())
}

fn request_shutdown() {
    SHUTDOWN_REQUESTED.store(true, Ordering::SeqCst);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;
    use std::time::Instant;

    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    struct EnvGuard {
        previous: Vec<(&'static str, Option<std::ffi::OsString>)>,
    }

    impl EnvGuard {
        fn set(values: &[(&'static str, Option<&std::path::Path>)]) -> Self {
            let previous = values
                .iter()
                .map(|(key, _)| (*key, std::env::var_os(key)))
                .collect::<Vec<_>>();
            for (key, value) in values {
                match value {
                    Some(value) => std::env::set_var(key, value),
                    None => std::env::remove_var(key),
                }
            }
            Self { previous }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            for (key, value) in &self.previous {
                match value {
                    Some(value) => std::env::set_var(key, value),
                    None => std::env::remove_var(key),
                }
            }
        }
    }

    #[test]
    fn version_handshake_refuses_a_different_build() {
        let matching = ServiceEndpoint {
            addr: "127.0.0.1:1234".to_string(),
            version: tura_path::instance_version(),
        };
        assert!(ensure_version_compatible(&matching).is_ok());

        let mismatched = ServiceEndpoint {
            addr: "127.0.0.1:1234".to_string(),
            version: "0.0.0-other+release".to_string(),
        };
        let error =
            ensure_version_compatible(&mismatched).expect_err("a different build must be refused");
        assert!(error.to_string().contains("different build"));
    }

    #[test]
    fn service_probe_removes_unreachable_addr_file_quickly() {
        let _lock = ENV_LOCK.lock().expect("env lock");
        let root = tempfile::tempdir().expect("temp db root");
        let _env = EnvGuard::set(&[
            ("SESSION_LOG_DB_ROOT", Some(root.path())),
            ("TURA_DB_ROOT", None),
        ]);

        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("reserve loopback port");
        let addr = listener.local_addr().expect("reserved address");
        drop(listener);

        let path = service_addr_path();
        std::fs::create_dir_all(path.parent().expect("addr parent")).expect("addr dir");
        std::fs::write(
            &path,
            serde_json::to_string(&ServiceEndpoint {
                addr: addr.to_string(),
                version: tura_path::instance_version(),
            })
            .expect("endpoint json"),
        )
        .expect("write stale addr");

        let started = Instant::now();
        assert!(!service_is_running());
        assert!(
            started.elapsed() < Duration::from_secs(1),
            "stale session_db probe should not wait for the service connect timeout"
        );
        assert!(
            !path.exists(),
            "unreachable session_db addr should be removed"
        );
    }

    #[test]
    fn service_probe_rejects_socket_that_does_not_speak_session_db_protocol() {
        let _lock = ENV_LOCK.lock().expect("env lock");
        let root = tempfile::tempdir().expect("temp db root");
        let _env = EnvGuard::set(&[
            ("SESSION_LOG_DB_ROOT", Some(root.path())),
            ("TURA_DB_ROOT", None),
        ]);

        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind abortive endpoint");
        let addr = listener.local_addr().expect("abortive endpoint addr");
        let server = std::thread::spawn(move || {
            let (stream, _) = listener.accept().expect("accept health probe");
            drop(stream);
        });

        let path = service_addr_path();
        std::fs::create_dir_all(path.parent().expect("addr parent")).expect("addr dir");
        std::fs::write(
            &path,
            serde_json::to_string(&ServiceEndpoint {
                addr: addr.to_string(),
                version: tura_path::instance_version(),
            })
            .expect("endpoint json"),
        )
        .expect("write abortive addr");

        assert!(
            !service_is_running(),
            "a raw TCP accept without a session_db health response must not be adopted"
        );
        assert!(
            !path.exists(),
            "non-protocol session_db addr should be removed"
        );
        server.join().expect("abortive endpoint thread");
    }

    #[test]
    fn service_probe_retries_transient_health_disconnect_before_replacing_endpoint() {
        let _lock = ENV_LOCK.lock().expect("env lock");
        let root = tempfile::tempdir().expect("temp db root");
        let _env = EnvGuard::set(&[
            ("SESSION_LOG_DB_ROOT", Some(root.path())),
            ("TURA_DB_ROOT", None),
        ]);

        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind flaky endpoint");
        let addr = listener.local_addr().expect("flaky endpoint addr");
        let server = std::thread::spawn(move || {
            let (stream, _) = listener.accept().expect("accept transient probe");
            drop(stream);

            let (mut stream, _) = listener.accept().expect("accept retry probe");
            let mut request = String::new();
            let _ = BufReader::new(stream.try_clone().expect("clone retry stream"))
                .read_line(&mut request)
                .expect("read retry probe");
            assert!(request.contains("\"health\""));
            stream
                .write_all(
                    serde_json::to_string(&SessionLogResponse::Ok)
                        .expect("health retry response json")
                        .as_bytes(),
                )
                .expect("write health retry response");
            stream.write_all(b"\n").expect("write retry newline");
            stream.flush().expect("flush retry response");
        });

        let path = service_addr_path();
        std::fs::create_dir_all(path.parent().expect("addr parent")).expect("addr dir");
        std::fs::write(
            &path,
            serde_json::to_string(&ServiceEndpoint {
                addr: addr.to_string(),
                version: tura_path::instance_version(),
            })
            .expect("endpoint json"),
        )
        .expect("write flaky addr");

        assert!(
            service_is_running(),
            "a transient health disconnect should be retried before replacing a live endpoint"
        );
        assert!(
            path.exists(),
            "endpoint should be kept after a successful retry"
        );
        server.join().expect("flaky endpoint thread");
    }

    #[test]
    fn service_probe_keeps_live_endpoint_when_connect_timeout_is_short() {
        let _lock = ENV_LOCK.lock().expect("env lock");
        let root = tempfile::tempdir().expect("temp db root");
        let _env = EnvGuard::set(&[
            ("SESSION_LOG_DB_ROOT", Some(root.path())),
            ("TURA_DB_ROOT", None),
        ]);
        let _probe_env = ProbeEnvGuard::set("20", None);

        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind slow health endpoint");
        let addr = listener.local_addr().expect("slow health endpoint addr");
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept slow health probe");
            let mut request = String::new();
            let _ = BufReader::new(stream.try_clone().expect("clone slow health stream"))
                .read_line(&mut request)
                .expect("read slow health probe");
            assert!(request.contains("\"health\""));
            std::thread::sleep(Duration::from_millis(150));
            stream
                .write_all(
                    serde_json::to_string(&SessionLogResponse::Ok)
                        .expect("health response json")
                        .as_bytes(),
                )
                .expect("write slow health response");
            stream.write_all(b"\n").expect("write slow health newline");
            stream.flush().expect("flush slow health response");
        });

        let path = service_addr_path();
        std::fs::create_dir_all(path.parent().expect("addr parent")).expect("addr dir");
        std::fs::write(
            &path,
            serde_json::to_string(&ServiceEndpoint {
                addr: addr.to_string(),
                version: tura_path::instance_version(),
            })
            .expect("endpoint json"),
        )
        .expect("write slow health addr");

        assert!(
            service_is_running(),
            "short connect probes must still allow a reasonable protocol response window"
        );
        assert!(path.exists(), "live endpoint should not be removed");
        server.join().expect("slow health endpoint thread");
    }

    #[test]
    fn service_probe_removes_foreign_version_addr_file() {
        let _lock = ENV_LOCK.lock().expect("env lock");
        let root = tempfile::tempdir().expect("temp db root");
        let _env = EnvGuard::set(&[
            ("SESSION_LOG_DB_ROOT", Some(root.path())),
            ("TURA_DB_ROOT", None),
        ]);

        let path = service_addr_path();
        std::fs::create_dir_all(path.parent().expect("addr parent")).expect("addr dir");
        std::fs::write(
            &path,
            serde_json::to_string(&ServiceEndpoint {
                addr: "127.0.0.1:1".to_string(),
                version: "0.0.0-foreign+release".to_string(),
            })
            .expect("endpoint json"),
        )
        .expect("write foreign addr");

        assert!(!service_is_running());
        assert!(
            !path.exists(),
            "foreign-version session_db addr should be removed"
        );
    }

    #[test]
    fn transient_response_retry_includes_windows_connection_aborted() {
        for kind in [
            ErrorKind::ConnectionAborted,
            ErrorKind::ConnectionReset,
            ErrorKind::UnexpectedEof,
            ErrorKind::BrokenPipe,
        ] {
            let error = anyhow::Error::new(IoError::from(kind));
            assert!(
                is_transient_response_error(&error),
                "{kind:?} should be retried as a transient session_db response error"
            );
        }
    }

    #[test]
    fn call_service_reports_version_mismatch_before_connecting() {
        let _lock = ENV_LOCK.lock().expect("env lock");
        let root = tempfile::tempdir().expect("temp db root");
        let _env = EnvGuard::set(&[
            ("SESSION_LOG_DB_ROOT", Some(root.path())),
            ("TURA_DB_ROOT", None),
        ]);

        let path = service_addr_path();
        std::fs::create_dir_all(path.parent().expect("addr parent")).expect("addr dir");
        std::fs::write(
            &path,
            serde_json::to_string(&ServiceEndpoint {
                addr: "127.0.0.1:1".to_string(),
                version: "0.0.0-foreign+release".to_string(),
            })
            .expect("endpoint json"),
        )
        .expect("write foreign addr");

        let error = call_service(&SessionLogCommand::ListWorkspaces)
            .expect_err("foreign version must be refused");
        assert!(
            error.to_string().contains("different build"),
            "unexpected error: {error:#}"
        );
    }

    struct ProbeEnvGuard {
        previous_connect: Option<std::ffi::OsString>,
        previous_response: Option<std::ffi::OsString>,
    }

    impl ProbeEnvGuard {
        fn set(connect_ms: &str, response_ms: Option<&str>) -> Self {
            let previous_connect = std::env::var_os("TURA_SESSION_DB_PROBE_TIMEOUT_MS");
            let previous_response = std::env::var_os("TURA_SESSION_DB_PROBE_RESPONSE_TIMEOUT_MS");
            std::env::set_var("TURA_SESSION_DB_PROBE_TIMEOUT_MS", connect_ms);
            match response_ms {
                Some(value) => {
                    std::env::set_var("TURA_SESSION_DB_PROBE_RESPONSE_TIMEOUT_MS", value)
                }
                None => std::env::remove_var("TURA_SESSION_DB_PROBE_RESPONSE_TIMEOUT_MS"),
            }
            Self {
                previous_connect,
                previous_response,
            }
        }
    }

    impl Drop for ProbeEnvGuard {
        fn drop(&mut self) {
            match &self.previous_connect {
                Some(value) => std::env::set_var("TURA_SESSION_DB_PROBE_TIMEOUT_MS", value),
                None => std::env::remove_var("TURA_SESSION_DB_PROBE_TIMEOUT_MS"),
            }
            match &self.previous_response {
                Some(value) => {
                    std::env::set_var("TURA_SESSION_DB_PROBE_RESPONSE_TIMEOUT_MS", value)
                }
                None => std::env::remove_var("TURA_SESSION_DB_PROBE_RESPONSE_TIMEOUT_MS"),
            }
        }
    }
}
