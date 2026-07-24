//! Session DB transport and durable handoff client.

use std::fs;
use std::io::{BufRead, BufReader, Error as IoError, ErrorKind, Write};
use std::net::{Shutdown, SocketAddr, TcpStream};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{mpsc, Arc};
use std::task::Poll;
use std::time::{Duration, Instant};

use anyhow::{anyhow, Context, Result};
use fs2::FileExt;

use crate::{ServiceEndpoint, SessionFeedEntry, SessionLogCommand, SessionLogResponse};

const ADDR_FILE: &str = "service.addr";
const QUEUE_DIR: &str = "message_queue";
const PENDING_DIR: &str = "pending";
const PROCESSING_DIR: &str = "processing";
const FAILED_DIR: &str = "failed";
const CONNECT_TIMEOUT: Duration = Duration::from_secs(2);
const PROBE_CONNECT_TIMEOUT: Duration = Duration::from_millis(100);
const PROBE_RESPONSE_TIMEOUT: Duration = Duration::from_millis(500);
const PROBE_OWNED_RESPONSE_TIMEOUT: Duration = Duration::from_secs(5);
const PROBE_RETRY_DELAY: Duration = Duration::from_millis(50);
const READ_TIMEOUT: Duration = Duration::from_secs(60);
const SUBSCRIPTION_BLOCKING_POLL_INTERVAL: Duration = Duration::from_millis(100);
const EMPTY_RESPONSE_RETRIES: usize = 3;
static NEXT_QUEUE_ID: AtomicU64 = AtomicU64::new(0);

pub fn default_db_dir() -> PathBuf {
    tura_path::home_db_dir()
}

pub fn service_addr_path() -> PathBuf {
    default_db_dir().join(ADDR_FILE)
}

pub fn message_queue_root() -> PathBuf {
    default_db_dir().join(QUEUE_DIR)
}

pub fn pending_queue_dir() -> PathBuf {
    message_queue_root().join(PENDING_DIR)
}

pub fn processing_queue_dir() -> PathBuf {
    message_queue_root().join(PROCESSING_DIR)
}

pub fn failed_queue_dir() -> PathBuf {
    message_queue_root().join(FAILED_DIR)
}

pub fn service_version() -> Option<String> {
    read_endpoint().ok().map(|endpoint| endpoint.version)
}

pub fn service_is_running() -> bool {
    let endpoint = match read_endpoint() {
        Ok(endpoint) => endpoint,
        Err(_) => return false,
    };
    let owner_lock_held = endpoint_owner_lock_is_held(&endpoint);
    if ensure_version_compatible(&endpoint).is_err() {
        if !owner_lock_held {
            remove_endpoint_if_unchanged(&endpoint);
        }
        return false;
    }
    let addr = match parse_addr(&endpoint) {
        Ok(addr) => addr,
        Err(_) => {
            if !owner_lock_held {
                remove_endpoint_if_unchanged(&endpoint);
            }
            return false;
        }
    };
    if probe_session_db(&addr, probe_response_timeout(owner_lock_held)) {
        true
    } else {
        remove_endpoint_if_unchanged(&endpoint);
        false
    }
}

pub fn call_service(command: &SessionLogCommand) -> Result<SessionLogResponse> {
    let mut last_transient_response_error = None;
    for attempt in 0..EMPTY_RESPONSE_RETRIES {
        match call_service_once(command) {
            Ok(response) => return Ok(response),
            Err(error) if is_transient_response_error(&error) => {
                last_transient_response_error = Some(error);
                if attempt + 1 < EMPTY_RESPONSE_RETRIES {
                    std::thread::sleep(Duration::from_millis(20));
                }
            }
            Err(error) => return Err(error),
        }
    }
    Err(last_transient_response_error
        .unwrap_or_else(|| anyhow!("session_db service call did not run")))
}

pub struct SessionFeedSubscription {
    reader: BufReader<TcpStream>,
    pending_frame: Vec<u8>,
    cancelled: Arc<AtomicBool>,
}

pub struct SessionFeedSubscriptionCancellation {
    stream: TcpStream,
    cancelled: Arc<AtomicBool>,
}

impl SessionFeedSubscriptionCancellation {
    pub fn cancel(self) -> Result<()> {
        self.cancelled.store(true, Ordering::SeqCst);
        match self.stream.shutdown(Shutdown::Both) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == ErrorKind::NotConnected => Ok(()),
            Err(error) => Err(error).context("failed to close session feed subscription"),
        }
    }
}

impl SessionFeedSubscription {
    pub fn cancellation_handle(&self) -> Result<SessionFeedSubscriptionCancellation> {
        Ok(SessionFeedSubscriptionCancellation {
            stream: self.reader.get_ref().try_clone()?,
            cancelled: Arc::clone(&self.cancelled),
        })
    }

    pub fn next_entry(&mut self) -> Result<Option<SessionFeedEntry>> {
        loop {
            match self.poll_next_entry(SUBSCRIPTION_BLOCKING_POLL_INTERVAL)? {
                Poll::Ready(entry) => return Ok(entry),
                Poll::Pending => {}
            }
        }
    }

    pub fn poll_next_entry(&mut self, timeout: Duration) -> Result<Poll<Option<SessionFeedEntry>>> {
        if self.cancelled.load(Ordering::SeqCst) {
            return Ok(Poll::Ready(None));
        }
        self.reader
            .get_ref()
            .set_read_timeout(Some(timeout))
            .context("failed to configure session feed subscription read timeout")?;
        match self.read_next_entry() {
            Ok(_) | Err(_) if self.cancelled.load(Ordering::SeqCst) => Ok(Poll::Ready(None)),
            result => result,
        }
    }

    fn read_next_entry(&mut self) -> Result<Poll<Option<SessionFeedEntry>>> {
        let read = match self.reader.read_until(b'\n', &mut self.pending_frame) {
            Ok(read) => read,
            Err(error) if matches!(error.kind(), ErrorKind::TimedOut | ErrorKind::WouldBlock) => {
                return Ok(Poll::Pending);
            }
            Err(error) => return Err(error.into()),
        };
        if read == 0 && self.pending_frame.is_empty() {
            return Ok(Poll::Ready(None));
        }
        let frame = std::mem::take(&mut self.pending_frame);
        match serde_json::from_slice::<SessionLogResponse>(&frame)
            .context("failed to decode session feed subscription frame")?
        {
            SessionLogResponse::SessionFeedEvent { entry } => Ok(Poll::Ready(Some(*entry))),
            SessionLogResponse::Error { error } => Err(anyhow!(
                "session_db session feed subscription failed: {error}"
            )),
            other => Err(anyhow!(
                "unexpected session feed subscription frame: {other:?}"
            )),
        }
    }
}

pub fn open_session_feed_subscription() -> Result<SessionFeedSubscription> {
    let endpoint = read_endpoint()?;
    ensure_version_compatible(&endpoint)?;
    let addr = parse_addr(&endpoint)?;
    open_session_feed_subscription_addr(&addr)
}

pub fn subscribe_session_feed(sender: mpsc::Sender<SessionFeedEntry>) -> Result<()> {
    let mut subscription = open_session_feed_subscription()?;
    while let Some(entry) = subscription.next_entry()? {
        if sender.send(entry).is_err() {
            return Ok(());
        }
    }
    Ok(())
}

fn open_session_feed_subscription_addr(addr: &SocketAddr) -> Result<SessionFeedSubscription> {
    let stream = TcpStream::connect_timeout(addr, CONNECT_TIMEOUT)
        .with_context(|| format!("failed to connect to session_db service at {addr}"))?;
    stream.set_read_timeout(None)?;
    stream.set_write_timeout(Some(READ_TIMEOUT))?;
    let mut writer = stream.try_clone()?;
    writer
        .write_all(serde_json::to_string(&SessionLogCommand::SubscribeSessionFeed)?.as_bytes())?;
    writer.write_all(b"\n")?;
    writer.flush()?;

    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    let read = reader.read_line(&mut line)?;
    if read == 0 {
        return Err(anyhow!(
            "session_db service at {addr} closed before subscription acknowledgement"
        ));
    }
    match serde_json::from_str::<SessionLogResponse>(line.trim())
        .context("failed to decode session feed subscription acknowledgement")?
    {
        SessionLogResponse::SessionFeedSubscribed => Ok(SessionFeedSubscription {
            reader,
            pending_frame: Vec::new(),
            cancelled: Arc::new(AtomicBool::new(false)),
        }),
        SessionLogResponse::Error { error } => Err(anyhow!(
            "session_db session feed subscription failed: {error}"
        )),
        other => Err(anyhow!(
            "unexpected session feed subscription acknowledgement: {other:?}"
        )),
    }
}

pub fn is_async_write(command: &SessionLogCommand) -> bool {
    matches!(
        command,
        SessionLogCommand::CreateSession(_)
            | SessionLogCommand::ExecuteSessionCommand(_)
            | SessionLogCommand::UpdateSession(_)
            | SessionLogCommand::UpdateSessionTodos(_)
            | SessionLogCommand::ApplyCommandCheckpoint(_)
            | SessionLogCommand::MarkSessionInterrupted(_)
            | SessionLogCommand::DeleteSession(_)
            | SessionLogCommand::DeleteWorkspace(_)
    )
}

pub fn enqueue_command(command: &SessionLogCommand) -> Result<PathBuf> {
    enqueue_serialized_command(&serde_json::to_vec(command)?)
}

pub fn enqueue_serialized_command(payload: &[u8]) -> Result<PathBuf> {
    let pending = pending_queue_dir();
    fs::create_dir_all(&pending)
        .with_context(|| format!("failed to create session queue {}", pending.display()))?;
    let id = NEXT_QUEUE_ID.fetch_add(1, Ordering::Relaxed);
    let now = chrono::Utc::now()
        .timestamp_nanos_opt()
        .unwrap_or_else(|| chrono::Utc::now().timestamp_micros() * 1000);
    let name = queue_item_name(now, std::process::id(), id);
    let tmp_path = pending.join(format!("{name}.tmp"));
    let final_path = pending.join(name);
    fs::write(&tmp_path, payload)
        .with_context(|| format!("failed to write session queue item {}", tmp_path.display()))?;
    fs::rename(&tmp_path, &final_path).with_context(|| {
        format!(
            "failed to publish session queue item {}",
            final_path.display()
        )
    })?;
    Ok(final_path)
}

fn queue_item_name(now: i64, pid: u32, id: u64) -> String {
    format!("{now:020}-{pid}-{id:020}.json")
}

pub fn session_db_owner_lock_path() -> PathBuf {
    session_db_owner_lock_path_for_build_kind(tura_path::build_kind())
}

pub fn unreachable_owner_lock_message() -> Option<String> {
    if service_is_running() {
        return None;
    }
    let path = session_db_owner_lock_path();
    if !path.exists() {
        return None;
    }
    let file = match fs::OpenOptions::new().read(true).write(true).open(&path) {
        Ok(file) => file,
        Err(error) => {
            return Some(format_owner_lock_message(
                &path,
                read_owner_lock_record(&path).as_ref(),
                &error.to_string(),
            ))
        }
    };
    match file.try_lock_exclusive() {
        Ok(()) => {
            let _ = file.unlock();
            None
        }
        Err(error) => Some(format_owner_lock_message(
            &path,
            read_owner_lock_record(&path).as_ref(),
            &error.to_string(),
        )),
    }
}

fn read_endpoint() -> Result<ServiceEndpoint> {
    let path = service_addr_path();
    let raw = fs::read_to_string(&path).with_context(|| {
        format!(
            "session_db service address file {} not found",
            path.display()
        )
    })?;
    serde_json::from_str(raw.trim())
        .with_context(|| format!("invalid session_db endpoint record {}", path.display()))
}

fn parse_addr(endpoint: &ServiceEndpoint) -> Result<SocketAddr> {
    endpoint
        .addr
        .parse()
        .with_context(|| format!("invalid session_db service address {:?}", endpoint.addr))
}

fn ensure_version_compatible(endpoint: &ServiceEndpoint) -> Result<()> {
    let expected = tura_path::instance_version();
    if endpoint.version != expected {
        return Err(anyhow!(
            "session_db service version {} does not match client {}; refusing to use a service from a different build",
            endpoint.version,
            expected
        ));
    }
    Ok(())
}

fn probe_session_db(addr: &SocketAddr, total_timeout: Duration) -> bool {
    let started = Instant::now();
    loop {
        let remaining = total_timeout.saturating_sub(started.elapsed());
        if remaining.is_zero() {
            return false;
        }
        match call_service_addr(
            addr,
            &SessionLogCommand::Health,
            probe_connect_timeout().min(remaining),
            remaining,
        ) {
            Ok(SessionLogResponse::Ok) => return true,
            Ok(_) => return false,
            Err(error) if is_retryable_probe_error(&error) => {
                let remaining = total_timeout.saturating_sub(started.elapsed());
                if remaining.is_zero() {
                    return false;
                }
                std::thread::sleep(PROBE_RETRY_DELAY.min(remaining));
            }
            Err(_) => return false,
        }
    }
}

fn probe_connect_timeout() -> Duration {
    timeout_from_env("TURA_SESSION_DB_PROBE_TIMEOUT_MS", PROBE_CONNECT_TIMEOUT)
}

fn probe_response_timeout(owner_lock_held: bool) -> Duration {
    timeout_from_env(
        "TURA_SESSION_DB_PROBE_RESPONSE_TIMEOUT_MS",
        if owner_lock_held {
            PROBE_OWNED_RESPONSE_TIMEOUT
        } else {
            PROBE_RESPONSE_TIMEOUT
        },
    )
}

fn endpoint_owner_lock_is_held(endpoint: &ServiceEndpoint) -> bool {
    let endpoint_build_kind = endpoint
        .version
        .rsplit_once('+')
        .map(|(_, build_kind)| build_kind)
        .filter(|build_kind| {
            !build_kind.is_empty()
                && build_kind
                    .chars()
                    .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_'))
        });
    let path = endpoint_build_kind
        .map(session_db_owner_lock_path_for_build_kind)
        .unwrap_or_else(session_db_owner_lock_path);
    owner_lock_is_held(&path)
}

fn session_db_owner_lock_path_for_build_kind(build_kind: &str) -> PathBuf {
    tura_path::locks_dir().join(format!("session-db-{build_kind}.lock"))
}

fn owner_lock_is_held(path: &std::path::Path) -> bool {
    if !path.exists() {
        return false;
    }
    let file = match fs::OpenOptions::new().read(true).write(true).open(path) {
        Ok(file) => file,
        Err(_) => return true,
    };
    match file.try_lock_exclusive() {
        Ok(()) => {
            let _ = file.unlock();
            false
        }
        Err(_) => true,
    }
}

fn remove_endpoint_if_unchanged(expected: &ServiceEndpoint) {
    if read_endpoint().ok().as_ref() == Some(expected) {
        let _ = fs::remove_file(service_addr_path());
    }
}

fn timeout_from_env(name: &str, fallback: Duration) -> Duration {
    std::env::var(name)
        .ok()
        .and_then(|value| value.trim().parse::<u64>().ok())
        .filter(|millis| *millis > 0)
        .map(Duration::from_millis)
        .unwrap_or(fallback)
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
    writer.write_all(serde_json::to_string(command)?.as_bytes())?;
    writer.write_all(b"\n")?;
    writer.flush()?;
    let mut response_line = String::new();
    let read = BufReader::new(stream).read_line(&mut response_line)?;
    if read == 0 || response_line.trim().is_empty() {
        return Err(anyhow!(
            "session_db service at {addr} closed without a response"
        ));
    }
    serde_json::from_str(response_line.trim())
        .with_context(|| "failed to decode session_db service response")
}

fn is_transient_response_error(error: &anyhow::Error) -> bool {
    has_io_error(error, false)
}

fn is_retryable_probe_error(error: &anyhow::Error) -> bool {
    has_io_error(error, true)
}

fn has_io_error(error: &anyhow::Error, include_timeout: bool) -> bool {
    error.chain().any(|cause| {
        cause.to_string().contains("closed without a response")
            || cause.downcast_ref::<IoError>().is_some_and(|io_error| {
                matches!(
                    io_error.kind(),
                    ErrorKind::ConnectionAborted
                        | ErrorKind::ConnectionRefused
                        | ErrorKind::ConnectionReset
                        | ErrorKind::NotConnected
                        | ErrorKind::UnexpectedEof
                        | ErrorKind::BrokenPipe
                ) || include_timeout
                    && matches!(io_error.kind(), ErrorKind::TimedOut | ErrorKind::WouldBlock)
            })
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct OwnerLockRecord {
    pid: Option<u32>,
    kind: Option<String>,
    build_kind: Option<String>,
    home: Option<String>,
}

fn read_owner_lock_record(path: &std::path::Path) -> Option<OwnerLockRecord> {
    let raw = fs::read_to_string(path).ok()?;
    let mut record = OwnerLockRecord {
        pid: None,
        kind: None,
        build_kind: None,
        home: None,
    };
    for line in raw.lines() {
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let value = value.trim();
        match key.trim() {
            "pid" => record.pid = value.parse().ok(),
            "kind" => record.kind = Some(value.to_string()),
            "build_kind" => record.build_kind = Some(value.to_string()),
            "home" => record.home = Some(value.to_string()),
            _ => {}
        }
    }
    Some(record)
}

fn format_owner_lock_message(
    path: &std::path::Path,
    record: Option<&OwnerLockRecord>,
    lock_error: &str,
) -> String {
    let owner = record
        .map(|record| {
            let mut parts = Vec::new();
            if let Some(pid) = record.pid {
                parts.push(format!("pid {pid}"));
            }
            if let Some(kind) = record.kind.as_deref() {
                parts.push(format!("kind {kind}"));
            }
            if let Some(build_kind) = record.build_kind.as_deref() {
                parts.push(format!("build {build_kind}"));
            }
            if let Some(home) = record.home.as_deref() {
                parts.push(format!("home {home}"));
            }
            parts.join(", ")
        })
        .filter(|owner| !owner.is_empty())
        .unwrap_or_else(|| "owner details unavailable".to_string());
    let kill_hint = record
        .and_then(|record| record.pid)
        .map(|pid| {
            if cfg!(windows) {
                format!(
                    "Kill the stale process and retry. PowerShell: Stop-Process -Id {pid} -Force"
                )
            } else {
                format!("Kill the stale process and retry. Shell: kill {pid}")
            }
        })
        .unwrap_or_else(|| {
            "Close other Tura windows or kill the stale tura_session_db process, then retry."
                .to_string()
        });
    format!(
        "Process lock error: session_db is not reachable, but its owner lock is held at {} ({owner}; lock error: {lock_error}). {kill_hint}",
        path.display()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_handshake_refuses_a_different_build() {
        let matching = ServiceEndpoint {
            addr: "127.0.0.1:1234".to_string(),
            version: tura_path::instance_version(),
        };
        assert!(ensure_version_compatible(&matching).is_ok());
        let mismatched = ServiceEndpoint {
            addr: matching.addr,
            version: "0.0.0-other+release".to_string(),
        };
        assert!(ensure_version_compatible(&mismatched)
            .expect_err("foreign builds must be refused")
            .to_string()
            .contains("different build"));
    }

    #[test]
    fn transient_response_classifier_covers_cross_platform_disconnects() {
        for kind in [
            ErrorKind::ConnectionAborted,
            ErrorKind::ConnectionReset,
            ErrorKind::UnexpectedEof,
            ErrorKind::BrokenPipe,
        ] {
            assert!(is_transient_response_error(&anyhow::Error::new(
                IoError::from(kind)
            )));
        }
    }

    #[test]
    fn owned_session_db_health_window_defaults_to_five_seconds() {
        assert_eq!(PROBE_OWNED_RESPONSE_TIMEOUT, Duration::from_secs(5));
        assert_eq!(probe_response_timeout(true), Duration::from_secs(5));
        assert_eq!(probe_response_timeout(false), Duration::from_millis(500));
    }

    #[test]
    fn subscription_cancellation_unblocks_a_pending_read() {
        let listener =
            std::net::TcpListener::bind(("127.0.0.1", 0)).expect("bind subscription test listener");
        let addr = listener.local_addr().expect("subscription test address");
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept subscription client");
            let mut command = String::new();
            BufReader::new(stream.try_clone().expect("clone subscription stream"))
                .read_line(&mut command)
                .expect("read subscription command");
            assert!(matches!(
                serde_json::from_str::<SessionLogCommand>(command.trim())
                    .expect("decode subscription command"),
                SessionLogCommand::SubscribeSessionFeed
            ));
            stream
                .write_all(
                    serde_json::to_string(&SessionLogResponse::SessionFeedSubscribed)
                        .expect("encode subscription acknowledgement")
                        .as_bytes(),
                )
                .expect("write subscription acknowledgement");
            stream
                .write_all(b"\n")
                .expect("terminate subscription acknowledgement");
            stream.flush().expect("flush subscription acknowledgement");

            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .expect("bound cancelled subscription observation");
            let mut closed = String::new();
            let read = BufReader::new(stream)
                .read_line(&mut closed)
                .expect("observe cancelled subscription");
            assert_eq!(read, 0, "cancelled subscription should close its socket");
        });

        let mut subscription =
            open_session_feed_subscription_addr(&addr).expect("open subscription");
        let cancellation = subscription
            .cancellation_handle()
            .expect("create cancellation handle");
        let (sender, receiver) = mpsc::channel();
        let reader = std::thread::spawn(move || {
            sender
                .send(subscription.next_entry())
                .expect("report subscription read result");
        });

        cancellation.cancel().expect("cancel subscription");
        let read_result = receiver
            .recv_timeout(Duration::from_secs(2))
            .expect("subscription read should unblock after cancellation");
        assert!(matches!(read_result, Ok(None)));
        reader.join().expect("subscription reader thread");
        server.join().expect("subscription server thread");
    }

    #[test]
    fn subscription_poll_timeout_preserves_a_partial_frame() {
        let listener =
            std::net::TcpListener::bind(("127.0.0.1", 0)).expect("bind subscription test listener");
        let addr = listener.local_addr().expect("subscription test address");
        let (prefix_sender, prefix_receiver) = mpsc::channel();
        let (release_sender, release_receiver) = mpsc::channel();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept subscription client");
            let mut command = String::new();
            BufReader::new(stream.try_clone().expect("clone subscription stream"))
                .read_line(&mut command)
                .expect("read subscription command");
            stream
                .write_all(
                    serde_json::to_string(&SessionLogResponse::SessionFeedSubscribed)
                        .expect("encode subscription acknowledgement")
                        .as_bytes(),
                )
                .expect("write subscription acknowledgement");
            stream
                .write_all(b"\n")
                .expect("terminate subscription acknowledgement");
            stream.flush().expect("flush subscription acknowledgement");

            let frame = serde_json::to_string(&SessionLogResponse::Error {
                error: "split frame preserved".to_string(),
            })
            .expect("encode split subscription frame");
            let split = frame.len() / 2;
            stream
                .write_all(&frame.as_bytes()[..split])
                .expect("write split subscription prefix");
            stream.flush().expect("flush split subscription prefix");
            prefix_sender.send(()).expect("report split prefix");
            release_receiver.recv().expect("release split frame server");
            stream
                .write_all(&frame.as_bytes()[split..])
                .expect("write split subscription suffix");
            stream
                .write_all(b"\n")
                .expect("terminate split subscription frame");
            stream.flush().expect("flush split subscription suffix");
        });

        let mut subscription =
            open_session_feed_subscription_addr(&addr).expect("open subscription");
        prefix_receiver.recv().expect("wait for split prefix");
        assert!(matches!(
            subscription
                .poll_next_entry(Duration::from_millis(100))
                .expect("poll partial subscription frame"),
            Poll::Pending
        ));
        release_sender.send(()).expect("release split frame server");
        let error = subscription
            .next_entry()
            .expect_err("split error frame should remain decodable");
        assert!(format!("{error:#}").contains("split frame preserved"));
        server.join().expect("subscription server thread");
    }

    #[test]
    fn owner_lock_message_names_pid_and_kill_command() {
        let record = OwnerLockRecord {
            pid: Some(29816),
            kind: Some("session_db".to_string()),
            build_kind: Some("release".to_string()),
            home: Some("C:/workspace/tura".to_string()),
        };
        let message = format_owner_lock_message(
            std::path::Path::new("C:/workspace/tura/.tura/locks/session-db-release.lock"),
            Some(&record),
            "file is locked",
        );
        assert!(message.contains("Process lock error"));
        assert!(message.contains("pid 29816"));
        assert!(message.contains("Kill the stale process"));
    }

    #[test]
    fn queue_item_names_keep_same_tick_ids_in_numeric_order() {
        let mut names = vec![
            queue_item_name(42, 7, 10),
            queue_item_name(42, 7, 2),
            queue_item_name(42, 7, 1),
        ];
        names.sort();
        assert_eq!(
            names,
            vec![
                queue_item_name(42, 7, 1),
                queue_item_name(42, 7, 2),
                queue_item_name(42, 7, 10),
            ]
        );
    }

    #[test]
    fn async_write_classifier_rejects_queries() {
        assert!(is_async_write(&SessionLogCommand::UpdateSessionTodos(
            crate::UpdateSessionTodosRequest {
                command_id: "todos-1".to_string(),
                session_id: "session-1".to_string(),
                todos: vec![serde_json::json!({"id": "todo-1"})],
                updated_at: 1,
            }
        )));
        assert!(is_async_write(&SessionLogCommand::DeleteWorkspace(
            crate::DeleteWorkspaceRequest {
                workspace: "workspace".to_string(),
            }
        )));
        assert!(!is_async_write(&SessionLogCommand::ListWorkspaces));
        assert!(!is_async_write(&SessionLogCommand::ReplayRuntime(
            crate::ReplayRuntimeRequest {
                runtime_id: "runtime".to_string(),
            }
        )));
        assert!(!is_async_write(&SessionLogCommand::CommitRuntimeEvent(
            crate::CommitRuntimeEventRequest {
                runtime_id: "runtime".to_string(),
                event_seq: 1,
                expected_revision: 0,
                lease_id: "lease".to_string(),
                idempotency_key: "runtime:1".to_string(),
                event: lifecycle::RuntimeEvent::TextAppended {
                    chunk: "delta".to_string(),
                },
            }
        )));
        assert!(!is_async_write(&SessionLogCommand::AppendSessionFeedEvent(
            crate::AppendSessionFeedEventRequest {
                runtime_id: "runtime".to_string(),
                target_session_id: "session".to_string(),
                lease_id: "lease".to_string(),
                event_id: "event".to_string(),
                event: crate::SessionFeedEvent::AssistantTextDelta {
                    message_id: "message".to_string(),
                    part_id: "part".to_string(),
                    delta: "delta".to_string(),
                    created_at: 1,
                    updated_at: 2,
                },
            }
        )));
        assert!(!is_async_write(&SessionLogCommand::ReadSessionFeed(
            crate::ReadSessionFeedRequest {
                session_id: "session".to_string(),
                after_cursor: 0,
                limit: 10,
            }
        )));
        assert!(!is_async_write(&SessionLogCommand::SubscribeSessionFeed));
        assert!(!is_async_write(&SessionLogCommand::Shutdown));
    }
}
