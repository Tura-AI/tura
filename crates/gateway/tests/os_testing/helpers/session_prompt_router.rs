pub(crate) use anyhow::{anyhow, Context, Result};
pub(crate) use axum::extract::{Json, Path};
pub(crate) use axum::response::IntoResponse;
pub(crate) use gateway::api::session::{
    prompt_async, send_agent_message, SendAgentMessageRequest, SendAgentToolCall,
};
pub(crate) use gateway::contracts::SessionStatus;
pub(crate) use gateway::session::config::{save_config, TuraSessionConfig};
pub(crate) use gateway::session::MessageRole;
pub(crate) use gateway::session_store;
pub(crate) use serde_json::{json, Value};
pub(crate) use session_log::{SessionLogCommand, SessionLogStore};
pub(crate) use std::collections::VecDeque;
pub(crate) use std::io::{BufRead, BufReader, Write};
pub(crate) use std::net::{TcpListener, TcpStream};
pub(crate) use std::path::{Path as FsPath, PathBuf};
pub(crate) use std::sync::{mpsc, Arc, Mutex as StdMutex};
pub(crate) use std::time::{Duration, Instant};
pub(crate) use tokio::sync::Mutex;

pub(crate) static ENV_LOCK: Mutex<()> = Mutex::const_new(());

pub(crate) struct EnvGuard {
    previous: Vec<(&'static str, Option<std::ffi::OsString>)>,
}

impl EnvGuard {
    pub(crate) fn new(home: &FsPath, workspace: &FsPath) -> Self {
        let keys = [
            "TURA_HOME",
            "SESSION_LOG_DB_ROOT",
            "TURA_DB_ROOT",
            "TURA_PROJECT_ROOT",
            "TURA_CWD",
            "TURA_SESSION_DB_PROBE_TIMEOUT_MS",
            "TURA_GATEWAY_ALLOW_IN_PROCESS_FAKE_ROUTER",
        ];
        let previous = keys
            .iter()
            .map(|key| (*key, std::env::var_os(key)))
            .collect::<Vec<_>>();
        std::env::set_var("TURA_HOME", home);
        std::env::remove_var("SESSION_LOG_DB_ROOT");
        std::env::remove_var("TURA_DB_ROOT");
        std::env::set_var("TURA_PROJECT_ROOT", workspace);
        std::env::set_var("TURA_CWD", workspace);
        std::env::set_var("TURA_SESSION_DB_PROBE_TIMEOUT_MS", "20");
        std::env::set_var("TURA_GATEWAY_ALLOW_IN_PROCESS_FAKE_ROUTER", "1");
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

pub(crate) struct ServiceThread {
    handle: Option<std::thread::JoinHandle<Result<()>>>,
}

impl ServiceThread {
    pub(crate) fn start() -> Result<Self> {
        let store = SessionLogStore::open_default().context("open session log store")?;
        let handle = std::thread::spawn(move || session_log::ipc::serve_blocking(store));
        wait_until(
            Duration::from_secs(10),
            session_log::ipc::service_is_running,
        )?;
        Ok(Self {
            handle: Some(handle),
        })
    }
}

impl Drop for ServiceThread {
    fn drop(&mut self) {
        let _ = session_log::ipc::call_service(&SessionLogCommand::Shutdown);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

#[derive(Clone)]
pub(crate) enum RouterReply {
    Payload(Value),
    DelayedPayload(Value, Duration),
    CallbackThenPayload(Arc<dyn Fn(Value) + Send + Sync>, Value),
    RawLine(String),
}

pub(crate) struct FakeRouter {
    received: mpsc::Receiver<Value>,
    stop: Arc<std::sync::atomic::AtomicBool>,
    handle: Option<std::thread::JoinHandle<()>>,
    connection_handles: Arc<StdMutex<Vec<std::thread::JoinHandle<()>>>>,
    addr_path: PathBuf,
    addr: std::net::SocketAddr,
}

impl FakeRouter {
    pub(crate) fn start(home: &FsPath, replies: Vec<RouterReply>) -> Result<Self> {
        let listener = TcpListener::bind(("127.0.0.1", 0)).context("bind fake router")?;
        listener.set_nonblocking(true)?;
        let addr = listener.local_addr()?;
        let addr_path = home.join("db").join("session_log").join("router.addr");
        if let Some(parent) = addr_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(
            &addr_path,
            serde_json::to_string(&json!({
                "addr": addr.to_string(),
                "version": tura_path::instance_version(),
                "pid": std::process::id(),
                "process_start_time": current_process_start_time(std::process::id()),
            }))?,
        )?;
        let (tx, rx) = mpsc::channel();
        let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let thread_stop = Arc::clone(&stop);
        let replies = Arc::new(StdMutex::new(VecDeque::from(replies)));
        let connection_handles = Arc::new(StdMutex::new(Vec::new()));
        let thread_replies = Arc::clone(&replies);
        let thread_connection_handles = Arc::clone(&connection_handles);
        let handle = std::thread::spawn(move || {
            while !thread_stop.load(std::sync::atomic::Ordering::SeqCst) {
                match listener.accept() {
                    Ok((stream, _)) => {
                        let tx = tx.clone();
                        let replies = Arc::clone(&thread_replies);
                        let handle = std::thread::spawn(move || {
                            let _ = handle_router_connection(stream, &tx, &replies);
                        });
                        thread_connection_handles
                            .lock()
                            .expect("fake router connection handles lock")
                            .push(handle);
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(10));
                    }
                    Err(_) => break,
                }
            }
        });
        Ok(Self {
            received: rx,
            stop,
            handle: Some(handle),
            connection_handles,
            addr_path,
            addr,
        })
    }

    pub(crate) fn next_request(&self, timeout: Duration) -> Result<Value> {
        self.received
            .recv_timeout(timeout)
            .context("fake router did not receive request")
    }
}

impl Drop for FakeRouter {
    fn drop(&mut self) {
        self.stop.store(true, std::sync::atomic::Ordering::SeqCst);
        let _ = TcpStream::connect(self.addr);
        let _ = std::fs::remove_file(&self.addr_path);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
        let mut handles = self
            .connection_handles
            .lock()
            .expect("fake router connection handles lock");
        while let Some(handle) = handles.pop() {
            let _ = handle.join();
        }
    }
}

pub(crate) fn current_process_start_time(pid: u32) -> Option<u64> {
    let mut system = sysinfo::System::new_all();
    system.refresh_processes();
    system
        .process(sysinfo::Pid::from_u32(pid))
        .map(sysinfo::Process::start_time)
}

pub(crate) fn handle_router_connection(
    stream: TcpStream,
    received: &mpsc::Sender<Value>,
    replies: &StdMutex<VecDeque<RouterReply>>,
) -> Result<()> {
    let mut writer = stream.try_clone()?;
    let mut line = String::new();
    BufReader::new(stream).read_line(&mut line)?;
    if line.trim().is_empty() {
        return Ok(());
    }
    let request: Value = serde_json::from_str(line.trim()).context("decode router request")?;
    if request["kind"] == "health_check" || request["method"] == "health_check" {
        let response = json!({
            "ok": true,
            "request_id": request.get("request_id").cloned().unwrap_or(Value::Null),
            "payload": {
                "status": "ok",
                "pid": std::process::id(),
                "process_start_time": current_process_start_time(std::process::id())
            }
        });
        writer.write_all(serde_json::to_string(&response)?.as_bytes())?;
        writer.write_all(b"\n")?;
        writer.flush()?;
        return Ok(());
    }

    let _ = received.send(request.clone());
    let reply = replies
        .lock()
        .expect("fake router replies lock")
        .pop_front()
        .ok_or_else(|| anyhow!("fake router has no reply for request: {request}"))?;
    let response = match reply {
        RouterReply::Payload(payload) => json!({
            "ok": true,
            "request_id": request.get("request_id").cloned().unwrap_or(Value::Null),
            "payload": payload
        }),
        RouterReply::DelayedPayload(payload, delay) => {
            std::thread::sleep(delay);
            json!({
                "ok": true,
                "request_id": request.get("request_id").cloned().unwrap_or(Value::Null),
                "payload": payload
            })
        }
        RouterReply::CallbackThenPayload(callback, payload) => {
            callback(request.clone());
            json!({
                "ok": true,
                "request_id": request.get("request_id").cloned().unwrap_or(Value::Null),
                "payload": payload
            })
        }
        RouterReply::RawLine(line) => {
            writer.write_all(line.as_bytes())?;
            writer.write_all(b"\n")?;
            writer.flush()?;
            return Ok(());
        }
    };
    writer.write_all(serde_json::to_string(&response)?.as_bytes())?;
    writer.write_all(b"\n")?;
    writer.flush()?;
    Ok(())
}

pub(crate) fn wait_until(timeout: Duration, mut condition: impl FnMut() -> bool) -> Result<()> {
    let started = Instant::now();
    while started.elapsed() < timeout {
        if condition() {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    Err(anyhow!(
        "condition was not met within {}ms",
        timeout.as_millis()
    ))
}

pub(crate) fn assert_gateway_did_not_prewrite_session_db(session_id: &str) -> Result<()> {
    let persisted = gateway::session_db_client::SessionDbClient::discover()?
        .get_session(session_id.to_string())?;
    assert!(
        persisted.is_none(),
        "gateway must not prewrite session_db for prompt handoff; found {persisted:#?}"
    );
    Ok(())
}
