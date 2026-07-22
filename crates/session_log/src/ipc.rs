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
//! [`session_log_contract::client::call_service`] / [`serve_blocking`].

use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;

use crate::SessionLogStore;
use anyhow::{Context, Result};
use session_log_contract::client::service_addr_path;
use session_log_contract::{
    ServiceEndpoint, SessionFeedAppendOutcome, SessionFeedEntry, SessionLogCommand,
    SessionLogResponse,
};

const READ_TIMEOUT: Duration = Duration::from_secs(60);
static SHUTDOWN_REQUESTED: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Default)]
pub(crate) struct SessionFeedHub {
    subscribers: Arc<Mutex<Vec<mpsc::Sender<SessionFeedEntry>>>>,
}

impl SessionFeedHub {
    fn subscribe(&self) -> mpsc::Receiver<SessionFeedEntry> {
        let (sender, receiver) = mpsc::channel();
        self.subscribers
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .push(sender);
        receiver
    }

    pub(crate) fn publish(&self, entry: SessionFeedEntry) {
        self.subscribers
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .retain(|subscriber| subscriber.send(entry.clone()).is_ok());
    }
}

pub(crate) struct CommandDispatchOutcome {
    pub(crate) response: SessionLogResponse,
    pub(crate) committed_feed_entries: Vec<SessionFeedEntry>,
}

/// Execute a command against an owned store. Shared by the socket server and the
/// `tura_session_db` admin CLI so the data path has one implementation.
pub fn dispatch_command(store: &SessionLogStore, command: SessionLogCommand) -> SessionLogResponse {
    dispatch_command_with_feed(store, command).response
}

fn dispatch_command_with_feed(
    store: &SessionLogStore,
    command: SessionLogCommand,
) -> CommandDispatchOutcome {
    match execute_command_with_feed(store, command) {
        Ok(outcome) => outcome,
        Err(error) => SessionLogResponse::Error {
            error: error.to_string(),
        }
        .into(),
    }
}

pub(crate) fn execute_command_with_feed(
    store: &SessionLogStore,
    command: SessionLogCommand,
) -> Result<CommandDispatchOutcome> {
    let mut committed_feed_entries = Vec::new();
    let response = match command {
        SessionLogCommand::Health => SessionLogResponse::Ok,
        SessionLogCommand::CreateSession(payload) => {
            let outcome = store.create_session_with_feed(payload)?;
            committed_feed_entries.extend(outcome.feed_entries);
            SessionLogResponse::SessionCommandApplied {
                result: Box::new(outcome.result),
            }
        }
        SessionLogCommand::ExecuteSessionCommand(payload) => {
            let outcome = store.execute_session_command_with_feed(payload)?;
            committed_feed_entries.extend(outcome.feed_entries);
            SessionLogResponse::SessionCommandApplied {
                result: Box::new(outcome.result),
            }
        }
        SessionLogCommand::UpdateSession(payload) => {
            let outcome = store.update_session_with_feed(payload)?;
            committed_feed_entries.extend(outcome.feed_entries);
            SessionLogResponse::SessionUpdated {
                session: Box::new(outcome.snapshot),
            }
        }
        SessionLogCommand::UpdateSessionTodos(payload) => {
            let outcome = store.update_session_todos_with_feed(payload)?;
            if let Some(entry) = outcome.feed_entry {
                committed_feed_entries.push(entry);
            }
            SessionLogResponse::SessionTodosUpdated {
                todos: outcome.todos,
                cursor: outcome.cursor,
            }
        }
        SessionLogCommand::RegisterRuntime(payload) => {
            let runtime_id = payload.runtime_id.clone();
            let projection_event_id = format!("{runtime_id}:session-projection:registered");
            let result = store.register_runtime(payload)?;
            if matches!(
                &result,
                session_log_contract::RuntimeRegistrationOutcome::Registered { .. }
            ) {
                if let Some(entry) =
                    store.session_feed_entry_by_event_id(&runtime_id, &projection_event_id)?
                {
                    committed_feed_entries.push(entry);
                }
            }
            SessionLogResponse::RuntimeRegistered { result }
        }
        SessionLogCommand::ActivateRuntimeLease(payload) => {
            SessionLogResponse::RuntimeLeaseActivated {
                result: store.activate_runtime_lease(payload)?,
            }
        }
        SessionLogCommand::CommitRuntimeEvent(payload) => {
            let runtime_id = payload.runtime_id.clone();
            let projection_event_id = format!("{}:session-projection", payload.idempotency_key);
            let result = store.commit_runtime_event(payload)?;
            if matches!(
                &result,
                session_log_contract::RuntimeEventCommitOutcome::Applied { projection, .. }
                    if projection.state.is_terminal()
            ) {
                if let Some(entry) =
                    store.session_feed_entry_by_event_id(&runtime_id, &projection_event_id)?
                {
                    committed_feed_entries.push(entry);
                }
            }
            SessionLogResponse::RuntimeEventCommitted { result }
        }
        SessionLogCommand::AppendSessionFeedEvent(payload) => {
            let entry = SessionFeedEntry {
                session_id: payload.target_session_id.clone(),
                cursor: 0,
                runtime_id: Some(payload.runtime_id.clone()),
                event_id: payload.event_id.clone(),
                event: payload.event.clone(),
            };
            let result = store.append_session_feed_event(payload)?;
            if let SessionFeedAppendOutcome::Applied { cursor } = &result {
                committed_feed_entries.push(SessionFeedEntry {
                    cursor: *cursor,
                    ..entry
                });
            }
            SessionLogResponse::SessionFeedEventAppended { result }
        }
        SessionLogCommand::ReadSessionFeed(payload) => {
            let (entries, next_cursor) = store.read_session_feed(payload)?;
            SessionLogResponse::SessionFeed {
                entries,
                next_cursor,
            }
        }
        SessionLogCommand::SubscribeSessionFeed => {
            anyhow::bail!("session feed subscription requires a streaming socket")
        }
        SessionLogCommand::ReplayRuntime(payload) => SessionLogResponse::RuntimeReplayed {
            runtime: store.replay_runtime(payload)?.map(Box::new),
        },
        SessionLogCommand::PersistSessionDelta(payload) => {
            let outcome = store.persist_session_delta_with_feed(*payload)?;
            committed_feed_entries.extend(outcome.feed_entries);
            SessionLogResponse::SessionDeltaPersisted {
                next_sequence: outcome.next_sequence,
                next_management_sequence: outcome.next_management_sequence,
            }
        }
        SessionLogCommand::ReadContextSlice(payload) => SessionLogResponse::ContextSlice {
            context: store.read_context_slice(payload)?,
        },
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
            let outcome = store.mark_session_interrupted_with_feed(payload)?;
            committed_feed_entries.extend(outcome.feed_entries);
            SessionLogResponse::Ok
        }
        SessionLogCommand::DeleteSession(payload) => {
            let outcome = store.delete_session_with_feed(payload)?;
            committed_feed_entries.extend(outcome.feed_entries);
            SessionLogResponse::Ok
        }
        SessionLogCommand::DeleteWorkspace(payload) => {
            let outcome = store.delete_workspace_with_feed(payload)?;
            committed_feed_entries.extend(outcome.feed_entries);
            SessionLogResponse::Ok
        }
        SessionLogCommand::Shutdown => SessionLogResponse::Ok,
    };
    Ok(CommandDispatchOutcome {
        response,
        committed_feed_entries,
    })
}

impl From<SessionLogResponse> for CommandDispatchOutcome {
    fn from(response: SessionLogResponse) -> Self {
        Self {
            response,
            committed_feed_entries: Vec::new(),
        }
    }
}

/// Bind the service socket, publish its address, and serve commands until the
/// process exits. One thread per accepted connection; the store clone shares the
/// underlying connection pool, so concurrent clients run in parallel.
pub fn serve_blocking(store: SessionLogStore) -> Result<()> {
    serve_blocking_with_feed_hub(store, SessionFeedHub::default())
}

pub(crate) fn serve_blocking_with_feed_hub(
    store: SessionLogStore,
    feed_hub: SessionFeedHub,
) -> Result<()> {
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
                let feed_hub = feed_hub.clone();
                std::thread::spawn(move || {
                    if let Err(error) = handle_connection(store, stream, feed_hub) {
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

fn handle_connection(
    store: SessionLogStore,
    stream: TcpStream,
    feed_hub: SessionFeedHub,
) -> Result<()> {
    // BSD/macOS accepted sockets inherit O_NONBLOCK from the listener. Restore
    // blocking I/O so large NDJSON frames cannot be truncated by WouldBlock.
    stream.set_nonblocking(false)?;
    stream.set_read_timeout(Some(READ_TIMEOUT))?;
    let mut writer = stream.try_clone()?;
    let reader = BufReader::new(stream);
    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let outcome = match serde_json::from_str::<SessionLogCommand>(line.trim()) {
            Ok(SessionLogCommand::SubscribeSessionFeed) => {
                let receiver = feed_hub.subscribe();
                write_response(&mut writer, &SessionLogResponse::SessionFeedSubscribed)?;
                for entry in receiver {
                    if let Err(error) = write_response(
                        &mut writer,
                        &SessionLogResponse::SessionFeedEvent {
                            entry: Box::new(entry),
                        },
                    ) {
                        if is_subscription_disconnect(&error) {
                            return Ok(());
                        }
                        return Err(error);
                    }
                }
                return Ok(());
            }
            Ok(command) => {
                let shutdown = matches!(command, SessionLogCommand::Shutdown);
                let outcome = dispatch_command_with_feed(&store, command);
                if shutdown {
                    request_shutdown();
                }
                outcome
            }
            Err(error) => SessionLogResponse::Error {
                error: format!("invalid session_db request: {error}"),
            }
            .into(),
        };
        let response_result = write_response(&mut writer, &outcome.response);
        for entry in outcome.committed_feed_entries {
            feed_hub.publish(entry);
        }
        response_result?;
    }

    Ok(())
}

fn write_response(writer: &mut TcpStream, response: &SessionLogResponse) -> Result<()> {
    writer.write_all(serde_json::to_string(response)?.as_bytes())?;
    writer.write_all(b"\n")?;
    writer.flush()?;
    Ok(())
}

fn is_subscription_disconnect(error: &anyhow::Error) -> bool {
    error.chain().any(|cause| {
        cause.downcast_ref::<std::io::Error>().is_some_and(|error| {
            matches!(
                error.kind(),
                std::io::ErrorKind::BrokenPipe
                    | std::io::ErrorKind::ConnectionAborted
                    | std::io::ErrorKind::ConnectionReset
                    | std::io::ErrorKind::UnexpectedEof
            )
        })
    })
}

fn request_shutdown() {
    SHUTDOWN_REQUESTED.store(true, Ordering::SeqCst);
}

#[cfg(test)]
mod tests {
    use super::*;
    use lifecycle::{SessionCommand, TaskPlan};
    use session_log_contract::client::{call_service, service_is_running};
    use session_log_contract::{
        CreateSessionRequest, DeleteSessionRequest, ExecuteSessionCommandRequest,
        GetSessionRequest, MarkSessionInterruptedRequest, SessionFeedEvent,
        UpdateSessionTodosRequest,
    };
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

    #[test]
    fn nonblocking_listener_serves_responses_larger_than_the_socket_buffer() {
        let root = tempfile::tempdir().expect("temp db root");
        let store = SessionLogStore::open(root.path()).expect("open session store");
        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind loopback listener");
        listener
            .set_nonblocking(true)
            .expect("set listener nonblocking");
        let addr = listener.local_addr().expect("listener address");
        let mut client = TcpStream::connect(addr).expect("connect client");
        client
            .set_read_timeout(Some(Duration::from_secs(10)))
            .expect("set client read timeout");
        client
            .set_write_timeout(Some(Duration::from_secs(10)))
            .expect("set client write timeout");

        let accept_started = Instant::now();
        let server_stream = loop {
            match listener.accept() {
                Ok((stream, _)) => break stream,
                Err(error)
                    if error.kind() == std::io::ErrorKind::WouldBlock
                        && accept_started.elapsed() < Duration::from_secs(5) =>
                {
                    std::thread::sleep(Duration::from_millis(10));
                }
                Err(error) => panic!("accept queued client: {error}"),
            }
        };
        let server = std::thread::spawn(move || {
            handle_connection(store, server_stream, SessionFeedHub::default())
        });

        let oversized_variant = "x".repeat(1_000_000);
        let request = serde_json::json!({ "command": oversized_variant }).to_string();
        client
            .write_all(request.as_bytes())
            .expect("write oversized request");
        client.write_all(b"\n").expect("terminate request");
        client.flush().expect("flush request");

        let mut response_line = String::new();
        BufReader::new(client.try_clone().expect("clone client"))
            .read_line(&mut response_line)
            .expect("read oversized response");
        assert!(response_line.ends_with('\n'));
        assert!(response_line.len() > 1_000_000);
        assert!(matches!(
            serde_json::from_str::<SessionLogResponse>(response_line.trim())
                .expect("decode complete oversized response"),
            SessionLogResponse::Error { .. }
        ));

        drop(client);
        server
            .join()
            .expect("session_db connection thread")
            .expect("serve oversized response");
    }

    #[test]
    fn subscriptions_receive_each_applied_feed_entry_once_in_identical_order() {
        let root = tempfile::tempdir().expect("temp db root");
        let store = SessionLogStore::open(root.path()).expect("open session store");
        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind loopback listener");
        let addr = listener.local_addr().expect("listener address");
        let hub = SessionFeedHub::default();
        let server_hub = hub.clone();
        let server = std::thread::spawn(move || {
            let mut connections = Vec::new();
            for _ in 0..2 {
                let (stream, _) = listener.accept().expect("accept subscriber");
                let connection_store = store.clone();
                let connection_hub = server_hub.clone();
                connections.push(std::thread::spawn(move || {
                    handle_connection(connection_store, stream, connection_hub)
                }));
            }
            for connection in connections {
                connection.join().expect("subscription connection thread")?;
            }
            Ok::<_, anyhow::Error>(())
        });

        let mut clients = Vec::new();
        let mut readers = Vec::new();
        for _ in 0..2 {
            let mut client = TcpStream::connect(addr).expect("connect subscriber");
            client
                .set_read_timeout(Some(Duration::from_secs(2)))
                .expect("set subscriber timeout");
            write_response_command(&mut client, &SessionLogCommand::SubscribeSessionFeed);
            let mut reader = BufReader::new(client.try_clone().expect("clone subscriber"));
            assert!(matches!(
                read_response(&mut reader),
                SessionLogResponse::SessionFeedSubscribed
            ));
            clients.push(client);
            readers.push(reader);
        }

        let entry = SessionFeedEntry {
            session_id: "session-1".to_string(),
            cursor: 1,
            runtime_id: Some("runtime-1".to_string()),
            event_id: "event-1".to_string(),
            event: session_log_contract::SessionFeedEvent::TodosUpdated {
                todos: Vec::new(),
                updated_at: 1,
            },
        };
        let second_entry = SessionFeedEntry {
            cursor: 2,
            event_id: "event-2".to_string(),
            ..entry.clone()
        };
        hub.publish(entry.clone());
        hub.publish(second_entry.clone());
        for reader in &mut readers {
            assert!(matches!(
                read_response(reader),
                SessionLogResponse::SessionFeedEvent { entry: actual }
                    if actual.as_ref() == &entry
            ));
            assert!(matches!(
                read_response(reader),
                SessionLogResponse::SessionFeedEvent { entry: actual }
                    if actual.as_ref() == &second_entry
            ));
        }

        drop(readers);
        drop(clients);
        hub.subscribers
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clear();
        server
            .join()
            .expect("subscription server thread")
            .expect("subscription connection");
    }

    #[test]
    fn command_dispatch_replay_returns_no_committed_feed_entries() {
        let root = tempfile::tempdir().expect("temp db root");
        let workspace = tempfile::tempdir().expect("temp workspace");
        let store = SessionLogStore::open(root.path()).expect("open session store");
        let command = SessionLogCommand::CreateSession(test_create_request(
            workspace.path(),
            "dispatch-replay-session",
        ));

        let first = execute_command_with_feed(&store, command.clone()).expect("first dispatch");
        assert_eq!(first.committed_feed_entries.len(), 1);
        assert!(matches!(
            first.committed_feed_entries[0].event,
            SessionFeedEvent::SessionSnapshotCreated { .. }
        ));

        let replay = execute_command_with_feed(&store, command).expect("replayed dispatch");
        assert!(
            replay.committed_feed_entries.is_empty(),
            "a queued command replay after commit must not publish its durable feed again"
        );
    }

    #[test]
    fn todo_command_replay_returns_latest_canonical_projection_and_cursor() {
        let root = tempfile::tempdir().expect("temp db root");
        let workspace = tempfile::tempdir().expect("temp workspace");
        let store = SessionLogStore::open(root.path()).expect("open session store");
        let session_id = "todo-command-replay-session".to_string();
        store
            .create_session(test_create_request(workspace.path(), &session_id))
            .expect("create todo command fixture");
        let first = SessionLogCommand::UpdateSessionTodos(UpdateSessionTodosRequest {
            command_id: "todo-command-first".to_string(),
            session_id: session_id.clone(),
            todos: vec![serde_json::json!({"id": "first"})],
            updated_at: 2,
        });
        let second = SessionLogCommand::UpdateSessionTodos(UpdateSessionTodosRequest {
            command_id: "todo-command-second".to_string(),
            session_id,
            todos: vec![serde_json::json!({"id": "second"})],
            updated_at: 3,
        });

        let first_result =
            execute_command_with_feed(&store, first.clone()).expect("first todo dispatch");
        assert_eq!(first_result.committed_feed_entries.len(), 1);
        let second_result =
            execute_command_with_feed(&store, second).expect("second todo dispatch");
        assert_eq!(second_result.committed_feed_entries.len(), 1);
        let replay = execute_command_with_feed(&store, first).expect("replay first todo dispatch");
        assert!(replay.committed_feed_entries.is_empty());
        assert!(matches!(
            replay.response,
            SessionLogResponse::SessionTodosUpdated { todos, cursor }
                if todos == vec![serde_json::json!({"id": "second"})] && cursor == 3
        ));
    }

    #[test]
    fn mark_session_interrupted_broadcasts_the_committed_projection_once() {
        let root = tempfile::tempdir().expect("temp db root");
        let workspace = tempfile::tempdir().expect("temp workspace");
        let store = SessionLogStore::open(root.path()).expect("open session store");
        let session_id = "ipc-interrupt-session".to_string();
        store
            .create_session(test_create_request(workspace.path(), &session_id))
            .expect("create interrupt fixture");
        store
            .execute_session_command(ExecuteSessionCommandRequest {
                command_id: "ipc-interrupt-start".to_string(),
                session_id: session_id.clone(),
                session_command: SessionCommand::StartUserTurn,
                message_projection: None,
            })
            .expect("start interrupt fixture");

        let hub = SessionFeedHub::default();
        let receiver = hub.subscribe();
        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind interrupt listener");
        let addr = listener.local_addr().expect("interrupt listener address");
        let mut client = TcpStream::connect(addr).expect("connect interrupt client");
        client
            .set_read_timeout(Some(Duration::from_secs(2)))
            .expect("set interrupt client timeout");
        let (server_stream, _) = listener.accept().expect("accept interrupt client");
        let server_store = store;
        let server =
            std::thread::spawn(move || handle_connection(server_store, server_stream, hub));
        let mut reader = BufReader::new(client.try_clone().expect("clone interrupt client"));

        write_response_command(
            &mut client,
            &SessionLogCommand::MarkSessionInterrupted(MarkSessionInterruptedRequest {
                session_id: session_id.clone(),
            }),
        );
        assert!(matches!(read_response(&mut reader), SessionLogResponse::Ok));
        let entry = receiver
            .recv_timeout(Duration::from_secs(2))
            .expect("committed interrupted projection");
        assert_eq!(entry.session_id, session_id);
        assert_eq!(entry.cursor, 3);
        assert!(matches!(
            entry.event,
            SessionFeedEvent::SessionProjectionUpdated { projection, .. }
                if projection.state == lifecycle::SessionState::Interrupted
        ));
        assert!(receiver.recv_timeout(Duration::from_millis(50)).is_err());

        drop(reader);
        drop(client);
        server
            .join()
            .expect("interrupt server thread")
            .expect("serve interrupt command");
    }

    #[test]
    fn delete_session_broadcasts_one_committed_tombstone_and_replay_is_silent() {
        let root = tempfile::tempdir().expect("temp db root");
        let workspace = tempfile::tempdir().expect("temp workspace");
        let store = SessionLogStore::open(root.path()).expect("open session store");
        let session_id = "ipc-delete-session".to_string();
        store
            .create_session(test_create_request(workspace.path(), &session_id))
            .expect("create deletion fixture");
        let hub = SessionFeedHub::default();
        let receiver = hub.subscribe();
        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind delete listener");
        let addr = listener.local_addr().expect("delete listener address");
        let mut client = TcpStream::connect(addr).expect("connect delete client");
        client
            .set_read_timeout(Some(Duration::from_secs(2)))
            .expect("set delete client timeout");
        let (server_stream, _) = listener.accept().expect("accept delete client");
        let server_store = store.clone();
        let server =
            std::thread::spawn(move || handle_connection(server_store, server_stream, hub));
        let mut reader = BufReader::new(client.try_clone().expect("clone delete client"));
        let command = SessionLogCommand::DeleteSession(DeleteSessionRequest {
            session_id: session_id.clone(),
        });

        write_response_command(&mut client, &command);
        assert!(matches!(read_response(&mut reader), SessionLogResponse::Ok));
        let entry = receiver
            .recv_timeout(Duration::from_secs(2))
            .expect("committed deletion tombstone");
        assert_eq!(entry.session_id, session_id);
        assert_eq!(entry.cursor, 2);
        assert!(matches!(entry.event, SessionFeedEvent::SessionDeleted {}));

        write_response_command(&mut client, &command);
        assert!(matches!(read_response(&mut reader), SessionLogResponse::Ok));
        assert!(receiver.recv_timeout(Duration::from_millis(50)).is_err());
        assert!(store
            .get_session(GetSessionRequest { session_id })
            .expect("read deleted session")
            .is_none());
        drop(reader);
        drop(client);
        server
            .join()
            .expect("delete server thread")
            .expect("serve delete commands");
    }

    #[test]
    fn failed_delete_transaction_does_not_broadcast_a_tombstone() {
        let root = tempfile::tempdir().expect("temp db root");
        let workspace = tempfile::tempdir().expect("temp workspace");
        let store = SessionLogStore::open(root.path()).expect("open session store");
        let session_id = "ipc-delete-rollback".to_string();
        store
            .create_session(test_create_request(workspace.path(), &session_id))
            .expect("create rollback fixture");
        let workspace_db = workspace.path().join(".tura").join("session_log.sqlite3");
        rusqlite::Connection::open(workspace_db)
            .expect("open rollback workspace db")
            .execute_batch(
                "CREATE TRIGGER reject_session_delete
                 BEFORE DELETE ON sessions
                 BEGIN SELECT RAISE(ABORT, 'session delete rejected'); END;",
            )
            .expect("install deletion failure trigger");
        let hub = SessionFeedHub::default();
        let receiver = hub.subscribe();
        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind rollback listener");
        let addr = listener.local_addr().expect("rollback listener address");
        let mut client = TcpStream::connect(addr).expect("connect rollback client");
        client
            .set_read_timeout(Some(Duration::from_secs(2)))
            .expect("set rollback client timeout");
        let (server_stream, _) = listener.accept().expect("accept rollback client");
        let server_store = store.clone();
        let server =
            std::thread::spawn(move || handle_connection(server_store, server_stream, hub));
        let mut reader = BufReader::new(client.try_clone().expect("clone rollback client"));

        write_response_command(
            &mut client,
            &SessionLogCommand::DeleteSession(DeleteSessionRequest {
                session_id: session_id.clone(),
            }),
        );
        assert!(matches!(
            read_response(&mut reader),
            SessionLogResponse::Error { error } if error.contains("session delete rejected")
        ));
        assert!(receiver.recv_timeout(Duration::from_millis(50)).is_err());
        assert!(store
            .get_session(GetSessionRequest { session_id })
            .expect("read rolled back session")
            .is_some());
        drop(reader);
        drop(client);
        server
            .join()
            .expect("rollback server thread")
            .expect("serve rollback command");
    }

    #[test]
    fn delete_workspace_broadcasts_each_committed_session_tombstone() {
        let root = tempfile::tempdir().expect("temp db root");
        let workspace = tempfile::tempdir().expect("temp workspace");
        let store = SessionLogStore::open(root.path()).expect("open session store");
        for session_id in ["ipc-workspace-delete-a", "ipc-workspace-delete-b"] {
            store
                .create_session(test_create_request(workspace.path(), session_id))
                .expect("create workspace deletion fixture");
        }
        let hub = SessionFeedHub::default();
        let receiver = hub.subscribe();
        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind workspace listener");
        let addr = listener.local_addr().expect("workspace listener address");
        let mut client = TcpStream::connect(addr).expect("connect workspace client");
        client
            .set_read_timeout(Some(Duration::from_secs(2)))
            .expect("set workspace client timeout");
        let (server_stream, _) = listener.accept().expect("accept workspace client");
        let server_store = store;
        let server =
            std::thread::spawn(move || handle_connection(server_store, server_stream, hub));
        let mut reader = BufReader::new(client.try_clone().expect("clone workspace client"));

        write_response_command(
            &mut client,
            &SessionLogCommand::DeleteWorkspace(session_log_contract::DeleteWorkspaceRequest {
                workspace: workspace.path().to_string_lossy().replace('\\', "/"),
            }),
        );
        assert!(matches!(read_response(&mut reader), SessionLogResponse::Ok));
        let mut deleted = [
            receiver
                .recv_timeout(Duration::from_secs(2))
                .expect("first workspace tombstone"),
            receiver
                .recv_timeout(Duration::from_secs(2))
                .expect("second workspace tombstone"),
        ];
        deleted.sort_by(|left, right| left.session_id.cmp(&right.session_id));
        assert_eq!(
            deleted.map(|entry| (entry.session_id, entry.cursor, entry.event)),
            [
                (
                    "ipc-workspace-delete-a".to_string(),
                    2,
                    SessionFeedEvent::SessionDeleted {},
                ),
                (
                    "ipc-workspace-delete-b".to_string(),
                    2,
                    SessionFeedEvent::SessionDeleted {},
                ),
            ]
        );
        assert!(receiver.recv_timeout(Duration::from_millis(50)).is_err());
        drop(reader);
        drop(client);
        server
            .join()
            .expect("workspace server thread")
            .expect("serve workspace delete command");
    }

    fn test_create_request(workspace: &std::path::Path, session_id: &str) -> CreateSessionRequest {
        let workspace = workspace.to_string_lossy().replace('\\', "/");
        CreateSessionRequest {
            command_id: format!("create:{session_id}"),
            session_id: session_id.to_string(),
            creation_command: SessionCommand::CreateSession {
                task_plan: TaskPlan::default(),
            },
            copy_context: false,
            workspace: workspace.clone(),
            session_directory: workspace,
            name: "IPC deletion fixture".to_string(),
            created_at: 1,
            model: None,
            agent: None,
            session_type: "coding".to_string(),
            kill_processes_on_start: false,
            validator_enabled: false,
            force_planning: false,
            model_variant: None,
            model_acceleration_enabled: false,
            disable_permission_restrictions: false,
            use_last_tool_call_response: false,
            auto_session_name: false,
            initial_task_plan_patch: None,
        }
    }

    fn write_response_command(stream: &mut TcpStream, command: &SessionLogCommand) {
        stream
            .write_all(
                serde_json::to_string(command)
                    .expect("encode command")
                    .as_bytes(),
            )
            .expect("write command");
        stream.write_all(b"\n").expect("terminate command");
        stream.flush().expect("flush command");
    }

    fn read_response(reader: &mut BufReader<TcpStream>) -> SessionLogResponse {
        let mut line = String::new();
        reader.read_line(&mut line).expect("read response");
        serde_json::from_str(line.trim()).expect("decode response")
    }
}
