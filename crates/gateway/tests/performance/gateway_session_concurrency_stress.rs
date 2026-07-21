//! Gateway/session_db concurrency stress coverage.
//!
//! This performance E2E keeps providers out of the loop while still exercising
//! the gateway session API, the real session_db IPC service, and a mock runtime
//! router accepting concurrent turns.

#[path = "../support/typed_session.rs"]
mod typed_session;

use anyhow::{anyhow, Context, Result};
use axum::body::to_bytes;
use axum::extract::{Json, Path, Query};
use axum::response::IntoResponse;
use gateway::api::session::{create_session, list_messages, list_sessions};
use gateway::contracts::{
    CreateSessionRequest, MessageListParams, SendMessageRequest, SessionDirectoryParams,
    SessionListParams, SessionStatus,
};
use gateway::session::MessageRole;
use gateway::session_db_client::SessionDbClient;
use gateway::session_feed::SessionFeedReducer;
use gateway::session_store;
use serde::de::DeserializeOwned;
use serde_json::{json, Value};
use session_log::SessionLogStore;
use session_log_contract::SessionLogCommand;
use std::collections::{BTreeMap, BTreeSet};
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path as FsPath, PathBuf};
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    Arc, Mutex as StdMutex,
};
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

static ENV_LOCK: Mutex<()> = Mutex::const_new(());

const WORKSPACE_COUNT: usize = 10;
const TASKS_PER_WORKSPACE: usize = 20;
const SESSION_COUNT: usize = WORKSPACE_COUNT * TASKS_PER_WORKSPACE;
const ROUTER_TURNS_PER_SESSION: usize = 1;
const MOCK_RUNTIME_WRITES_PER_SESSION: usize = 8;
const EXPECTED_MESSAGES_PER_SESSION: usize =
    ROUTER_TURNS_PER_SESSION * 2 + MOCK_RUNTIME_WRITES_PER_SESSION;
const EXPECTED_TOTAL_MESSAGES: usize = SESSION_COUNT * EXPECTED_MESSAGES_PER_SESSION;
const OPERATION_BUDGET: Duration = Duration::from_secs(120);
const TEST_TIMEOUT: Duration = Duration::from_secs(180);

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn gateway_session_db_mock_runtime_handles_10_workspaces_20_tasks_2000_rich_records(
) -> Result<()> {
    tokio::time::timeout(
        TEST_TIMEOUT,
        gateway_session_db_mock_runtime_pressure_impl(),
    )
    .await
    .context("gateway/session_db performance stress exceeded total timeout")?
}

async fn gateway_session_db_mock_runtime_pressure_impl() -> Result<()> {
    let _guard = ENV_LOCK.lock().await;
    let root = tempfile::tempdir().context("temp root")?;
    let home = root.path().join("home");
    let workspace = root.path().join("workspace-0");
    std::fs::create_dir_all(&home)?;
    let workspaces = (0..WORKSPACE_COUNT)
        .map(|index| root.path().join(format!("workspace-{index}")))
        .collect::<Vec<_>>();
    for workspace in &workspaces {
        std::fs::create_dir_all(workspace)?;
    }
    let _env = EnvGuard::new(&home, &workspace);
    let service = ServiceThread::start()?;
    let router = MockRuntimeRouter::start(&home)?;
    let workspace_strings = workspaces
        .iter()
        .map(|workspace| workspace.to_string_lossy().to_string())
        .collect::<Vec<_>>();

    let mut sessions = Vec::with_capacity(SESSION_COUNT);
    for index in 0..SESSION_COUNT {
        let workspace_index = index / TASKS_PER_WORKSPACE;
        let workspace_string = workspace_strings[workspace_index].clone();
        let session: gateway::contracts::Session = decode_json_response(
            create_session(
                axum::http::HeaderMap::new(),
                Query(SessionDirectoryParams { directory: None }),
                Some(Json(CreateSessionRequest {
                    directory: Some(workspace_string.clone()),
                    model: Some("openai/gpt-stress".to_string()),
                    agent: Some("performance-stress-agent".to_string()),
                    session_type: Some("coding".to_string()),
                    model_variant: Some("low".to_string()),
                    auto_session_name: Some(false),
                    ..CreateSessionRequest::default()
                })),
            )
            .await,
        )
        .await?;
        sessions.push((index, workspace_index, workspace_string, session.id));
    }

    let started = Instant::now();
    tokio::time::timeout(
        OPERATION_BUDGET,
        run_concurrent_session_turns(sessions.clone()),
    )
    .await
    .context("gateway stress operation exceeded 30s")??;
    let elapsed = started.elapsed();
    assert!(
        elapsed <= OPERATION_BUDGET,
        "gateway stress operation took {elapsed:?}, over {OPERATION_BUDGET:?}"
    );

    eprintln!(
        "gateway_session_concurrency_stress summary: workspaces={WORKSPACE_COUNT} tasks_per_workspace={TASKS_PER_WORKSPACE} sessions={SESSION_COUNT} total_rich_records={EXPECTED_TOTAL_MESSAGES} operation_ms={}",
        elapsed.as_millis()
    );

    assert_eq!(
        router.enqueue_count(),
        SESSION_COUNT * ROUTER_TURNS_PER_SESSION,
        "mock runtime router should receive one enqueue per router-backed turn"
    );
    assert!(
        router.max_active_connections() > 1,
        "mock runtime router should observe concurrent gateway enqueues"
    );

    for workspace_string in &workspace_strings {
        let Json(listed_sessions) = list_sessions(
            axum::http::HeaderMap::new(),
            Query(SessionListParams {
                directory: Some(workspace_string.clone()),
                limit: Some(TASKS_PER_WORKSPACE + 5),
                include_children: true,
                ..SessionListParams::default()
            }),
        )
        .await;
        let listed_ids = listed_sessions
            .iter()
            .map(|session| session.id.clone())
            .collect::<BTreeSet<_>>();
        for (_, _, session_workspace, session_id) in &sessions {
            if session_workspace == workspace_string {
                assert!(
                    listed_ids.contains(session_id),
                    "gateway list_sessions should include stress session {session_id} for workspace {workspace_string}"
                );
            }
        }
    }

    let client = gateway::session_db_client::SessionDbClient::discover()?;
    wait_until_async(Duration::from_secs(10), || {
        let client = client.clone();
        let workspace_strings = workspace_strings.clone();
        let sessions = sessions.clone();
        async move {
            for workspace_string in workspace_strings {
                let Ok((page, snapshots)) = client.list_sessions(workspace_string.clone(), 0, 500)
                else {
                    return false;
                };
                if page.total != TASKS_PER_WORKSPACE as u64 {
                    return false;
                }
                let counts = snapshots
                    .into_iter()
                    .map(|snapshot| (snapshot.session_id, snapshot.message_count))
                    .collect::<BTreeMap<_, _>>();
                if !sessions
                    .iter()
                    .filter(|(_, _, session_workspace, _)| session_workspace == &workspace_string)
                    .all(|(_, _, _, id)| {
                        counts
                            .get(id)
                            .is_some_and(|count| *count >= EXPECTED_MESSAGES_PER_SESSION as u64)
                    })
                {
                    return false;
                }
            }
            true
        }
    })
    .await
    .context("session_db did not converge to expected stress message counts")?;

    let mut total_messages = 0usize;
    let workspace_summaries = client.list_workspaces()?;
    let summary_counts = workspace_summaries
        .into_iter()
        .map(|summary| (summary.directory, summary.session_count))
        .collect::<BTreeMap<_, _>>();
    for workspace_string in &workspace_strings {
        let workspace_key = session_log::path::normalize_workspace(workspace_string);
        assert_eq!(
            summary_counts.get(&workspace_key).copied(),
            Some(TASKS_PER_WORKSPACE as u64),
            "session_db workspace summary should include all tasks for {workspace_string}"
        );
    }

    for (_, _, _, session_id) in &sessions {
        let Json(messages) = list_messages(
            Path::<String>(session_id.clone()),
            Query(MessageListParams {
                limit: Some(EXPECTED_MESSAGES_PER_SESSION + 5),
                ..MessageListParams::default()
            }),
        )
        .await;
        assert_eq!(
            messages.len(),
            EXPECTED_MESSAGES_PER_SESSION,
            "gateway read should return the full session transcript for {session_id}"
        );
        total_messages += messages.len();

        let persisted = client
            .get_session(session_id.clone())?
            .ok_or_else(|| anyhow!("session_db missing stress session {session_id}"))?;
        assert!(
            persisted.message_count >= EXPECTED_MESSAGES_PER_SESSION as u64,
            "session_db message_count for {session_id} should be at least {EXPECTED_MESSAGES_PER_SESSION}, got {}",
            persisted.message_count
        );
        let (_, records) = client.list_session_records(session_id.clone(), 0, 500)?;
        assert!(
            records.len() >= EXPECTED_MESSAGES_PER_SESSION,
            "session_db records for {session_id} should include all messages, got {}",
            records.len()
        );
    }
    assert_eq!(total_messages, EXPECTED_TOTAL_MESSAGES);

    drop(router);
    drop(service);
    Ok(())
}

async fn run_concurrent_session_turns(sessions: Vec<(usize, usize, String, String)>) -> Result<()> {
    let mut tasks = Vec::with_capacity(sessions.len());
    for (session_index, workspace_index, _workspace, session_id) in sessions {
        tasks.push(tokio::spawn(async move {
            for turn in 0..ROUTER_TURNS_PER_SESSION {
                let reply: gateway::contracts::Message = decode_json_response(
                    gateway::api::session::send_message(
                        Path::<String>(session_id.clone()),
                        Json(SendMessageRequest {
                            content: rich_text_payload(
                                workspace_index,
                                session_index,
                                turn,
                                "gateway prompt",
                            ),
                            attachments: None,
                            parent_id: None,
                        }),
                    )
                    .await,
                )
                .await?;
                assert_eq!(reply.session_id, session_id);
                assert_eq!(reply.role, gateway::contracts::MessageRole::Assistant);

                let expected_messages = (turn + 1) * 2;
                wait_until_async(Duration::from_secs(5), || {
                    let session_id = session_id.clone();
                    async move {
                        session_store()
                            .get_session(&session_id)
                            .is_some_and(|session| {
                                session.status == SessionStatus::Idle
                                    && session.message_count >= expected_messages
                            })
                    }
                })
                .await?;

            }

            let client = SessionDbClient::discover()?;
            let mut reducer = SessionFeedReducer::new(session_store().clone());
            let mut feed_cursor = 0;
            let mut pending_messages = Vec::with_capacity(MOCK_RUNTIME_WRITES_PER_SESSION);
            for write in 0..MOCK_RUNTIME_WRITES_PER_SESSION {
                let message_number = ROUTER_TURNS_PER_SESSION * 2 + write + 1;
                let runtime_id = format!("mock-runtime-session-{session_index}-{write}");
                let message_id = format!("{runtime_id}.message");
                let part_id = format!("{runtime_id}.message");
                pending_messages.push(db_text_message(
                    &session_id,
                    &message_id,
                    &part_id,
                    "assistant",
                    &rich_text_payload(workspace_index, session_index, write, "mock runtime write"),
                    10_000 + message_number as i64,
                ));
                let flush_snapshot =
                    write % 4 == 0 || write + 1 == MOCK_RUNTIME_WRITES_PER_SESSION;
                if flush_snapshot {
                    typed_session::persist_messages_via_service(
                        &session_id,
                        std::mem::take(&mut pending_messages),
                    )
                    .context("persist incremental runtime-owned stress messages")?;
                    replay_new_session_feed(
                        &client,
                        &mut reducer,
                        &session_id,
                        &mut feed_cursor,
                    )?;
                }

                if write % 4 == 0 {
                    let Json(messages) = list_messages(
                        Path::<String>(session_id.clone()),
                        Query(MessageListParams {
                            limit: Some(message_number),
                            ..MessageListParams::default()
                        }),
                    )
                    .await;
                    assert_eq!(
                        messages.len(),
                        message_number,
                        "gateway read during stress should stay stable for {session_id} at runtime write {write}"
                    );
                }
            }
            Result::<()>::Ok(())
        }));
    }

    for task in tasks {
        task.await.context("stress session task panicked")??;
    }
    Ok(())
}

async fn decode_json_response<T: DeserializeOwned>(response: impl IntoResponse) -> Result<T> {
    let response = response.into_response();
    let status = response.status();
    let body = to_bytes(response.into_body(), 1024 * 1024)
        .await
        .context("read gateway response body")?;
    if !status.is_success() {
        return Err(anyhow!(
            "gateway returned {status}: {}",
            String::from_utf8_lossy(&body)
        ));
    }
    serde_json::from_slice(&body).context("decode gateway JSON response")
}

fn replay_new_session_feed(
    client: &SessionDbClient,
    reducer: &mut SessionFeedReducer,
    session_id: &str,
    cursor: &mut u64,
) -> Result<()> {
    loop {
        let (entries, next_cursor) =
            client.read_session_feed(session_id.to_string(), *cursor, 1_000)?;
        let count = entries.len();
        for entry in entries {
            reducer.apply(entry)?;
        }
        if next_cursor < *cursor {
            return Err(anyhow!(
                "session feed cursor moved backwards for {session_id}"
            ));
        }
        *cursor = next_cursor;
        if count < 1_000 {
            return Ok(());
        }
    }
}

fn db_text_message(
    session_id: &str,
    message_id: &str,
    part_id: &str,
    role: &str,
    text: &str,
    timestamp: i64,
) -> Value {
    json!({
        "id": message_id,
        "session_id": session_id,
        "role": role,
        "parent_id": null,
        "parts": [{
            "id": part_id,
            "type": "text",
            "content": text,
            "text": text,
            "metadata": null,
            "call_id": null,
            "tool": null,
            "state": null
        }],
        "created_at": timestamp,
        "updated_at": timestamp
    })
}

fn rich_text_payload(
    workspace_index: usize,
    session_index: usize,
    record_index: usize,
    label: &str,
) -> String {
    format!(
        "### {label} workspace-{workspace_index} task-{session_index} record-{record_index}\n\n\
This rich transcript record exercises gateway, router, runtime, and session_db history pressure with markdown tables, code blocks, links, and inline HTML.\n\n\
| surface | workspace | task | record |\n\
| --- | ---: | ---: | ---: |\n\
| gateway | {workspace_index} | {session_index} | {record_index} |\n\
| runtime | {workspace_index} | {session_index} | {record_index} |\n\n\
```ts\n\
const workspace = {workspace_index};\n\
const task = {session_index};\n\
const record = {record_index};\n\
console.log(workspace, task, record);\n\
```\n\n\
<b>rich text marker</b> [local](file:///tmp/tura/workspace-{workspace_index}/task-{session_index})"
    )
}

async fn wait_until_async<F, Fut>(timeout: Duration, mut condition: F) -> Result<()>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = bool>,
{
    let started = Instant::now();
    while started.elapsed() < timeout {
        if condition().await {
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    Err(anyhow!(
        "condition was not met within {}ms",
        timeout.as_millis()
    ))
}

struct EnvGuard {
    previous: Vec<(&'static str, Option<std::ffi::OsString>)>,
}

impl EnvGuard {
    fn new(home: &FsPath, workspace: &FsPath) -> Self {
        let keys = [
            "TURA_HOME",
            "SESSION_LOG_DB_ROOT",
            "TURA_DB_ROOT",
            "TURA_PROJECT_ROOT",
            "TURA_CWD",
            "TURA_SESSION_DB_PROBE_TIMEOUT_MS",
            "TURA_SESSION_DB_PROBE_RESPONSE_TIMEOUT_MS",
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
        std::env::set_var("TURA_SESSION_DB_PROBE_TIMEOUT_MS", "1000");
        std::env::set_var("TURA_SESSION_DB_PROBE_RESPONSE_TIMEOUT_MS", "5000");
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

struct ServiceThread {
    handle: Option<std::thread::JoinHandle<Result<()>>>,
}

impl ServiceThread {
    fn start() -> Result<Self> {
        let store = SessionLogStore::open_default().context("open session log store")?;
        let handle = std::thread::spawn(move || session_log::ipc::serve_blocking(store));
        wait_until_blocking(
            Duration::from_secs(10),
            session_log_contract::client::service_is_running,
        )?;
        Ok(Self {
            handle: Some(handle),
        })
    }
}

impl Drop for ServiceThread {
    fn drop(&mut self) {
        let _ = session_log_contract::client::call_service(&SessionLogCommand::Shutdown);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

struct MockRuntimeRouter {
    stop: Arc<AtomicBool>,
    handle: Option<std::thread::JoinHandle<()>>,
    connection_handles: Arc<StdMutex<Vec<std::thread::JoinHandle<()>>>>,
    addr_path: PathBuf,
    addr: std::net::SocketAddr,
    enqueue_count: Arc<AtomicUsize>,
    _active_connections: Arc<AtomicUsize>,
    max_active_connections: Arc<AtomicUsize>,
}

impl MockRuntimeRouter {
    fn start(home: &FsPath) -> Result<Self> {
        let listener = TcpListener::bind(("127.0.0.1", 0)).context("bind mock runtime router")?;
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
            }))?,
        )?;

        let stop = Arc::new(AtomicBool::new(false));
        let enqueue_count = Arc::new(AtomicUsize::new(0));
        let active_connections = Arc::new(AtomicUsize::new(0));
        let max_active_connections = Arc::new(AtomicUsize::new(0));
        let connection_handles = Arc::new(StdMutex::new(Vec::new()));
        let thread_stop = Arc::clone(&stop);
        let thread_connection_handles = Arc::clone(&connection_handles);
        let thread_enqueue_count = Arc::clone(&enqueue_count);
        let thread_active_connections = Arc::clone(&active_connections);
        let thread_max_active_connections = Arc::clone(&max_active_connections);
        let handle = std::thread::spawn(move || {
            while !thread_stop.load(Ordering::SeqCst) {
                match listener.accept() {
                    Ok((stream, _)) => {
                        let enqueue_count = Arc::clone(&thread_enqueue_count);
                        let active_connections = Arc::clone(&thread_active_connections);
                        let max_active_connections = Arc::clone(&thread_max_active_connections);
                        let handle = std::thread::spawn(move || {
                            let _ = handle_mock_runtime_connection(
                                stream,
                                enqueue_count,
                                active_connections,
                                max_active_connections,
                            );
                        });
                        thread_connection_handles
                            .lock()
                            .expect("mock runtime connection handles lock")
                            .push(handle);
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(1));
                    }
                    Err(_) => break,
                }
            }
        });

        Ok(Self {
            stop,
            handle: Some(handle),
            connection_handles,
            addr_path,
            addr,
            enqueue_count,
            _active_connections: active_connections,
            max_active_connections,
        })
    }

    fn enqueue_count(&self) -> usize {
        self.enqueue_count.load(Ordering::SeqCst)
    }

    fn max_active_connections(&self) -> usize {
        self.max_active_connections.load(Ordering::SeqCst)
    }
}

impl Drop for MockRuntimeRouter {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        let _ = TcpStream::connect(self.addr);
        let _ = std::fs::remove_file(&self.addr_path);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
        let mut handles = self
            .connection_handles
            .lock()
            .expect("mock runtime connection handles lock");
        while let Some(handle) = handles.pop() {
            let _ = handle.join();
        }
    }
}

fn handle_mock_runtime_connection(
    stream: TcpStream,
    enqueue_count: Arc<AtomicUsize>,
    active_connections: Arc<AtomicUsize>,
    max_active_connections: Arc<AtomicUsize>,
) -> Result<()> {
    let active = active_connections.fetch_add(1, Ordering::SeqCst) + 1;
    max_active_connections.fetch_max(active, Ordering::SeqCst);
    let result = handle_mock_runtime_connection_inner(stream, enqueue_count);
    active_connections.fetch_sub(1, Ordering::SeqCst);
    result
}

fn handle_mock_runtime_connection_inner(
    stream: TcpStream,
    enqueue_count: Arc<AtomicUsize>,
) -> Result<()> {
    let mut writer = stream.try_clone()?;
    let mut line = String::new();
    BufReader::new(stream).read_line(&mut line)?;
    if line.trim().is_empty() {
        return Ok(());
    }
    let request: Value = serde_json::from_str(line.trim()).context("decode router request")?;
    if request["kind"] == "health_check" || request["method"] == "health_check" {
        write_router_response(
            &mut writer,
            json!({
                "ok": true,
                "request_id": request.get("request_id").cloned().unwrap_or(Value::Null),
                "payload": {
                    "status": "ok",
                    "pid": std::process::id()
                }
            }),
        )?;
        return Ok(());
    }

    if request["method"] == "execution.enqueue_turn" {
        enqueue_count.fetch_add(1, Ordering::SeqCst);
        let session_id = request["payload"]["session_id"]
            .as_str()
            .ok_or_else(|| anyhow!("enqueue request missing session_id: {request}"))?;
        let runtime_id = request["payload"]["runtime_id"]
            .as_str()
            .ok_or_else(|| anyhow!("enqueue request missing runtime_id: {request}"))?;
        let prompt = request["payload"]["payload"]["prompt"]
            .as_str()
            .unwrap_or_default();
        std::thread::sleep(Duration::from_millis(2));
        let message = session_store()
            .add_message_with_ids(
                session_id,
                MessageRole::Assistant,
                format!("mock runtime reply for {prompt}"),
                Some(format!("assistant-{runtime_id}")),
                Some(format!("assistant-part-{runtime_id}")),
                None,
            )
            .context("mock runtime assistant message")?;
        typed_session::persist_messages_via_service(
            session_id,
            vec![serde_json::to_value(message).context("mock runtime message projection")?],
        )?;
        write_router_response(
            &mut writer,
            json!({
                "ok": true,
                "request_id": request.get("request_id").cloned().unwrap_or(Value::Null),
                "payload": {
                    "ok": true,
                    "accepted": true,
                    "worker_id": format!("mock-runtime-{session_id}")
                }
            }),
        )?;
        return Ok(());
    }

    write_router_response(
        &mut writer,
        json!({
            "ok": false,
            "request_id": request.get("request_id").cloned().unwrap_or(Value::Null),
            "error": format!("unexpected mock router method {}", request["method"])
        }),
    )?;
    Ok(())
}

fn write_router_response(writer: &mut TcpStream, response: Value) -> Result<()> {
    writer.write_all(serde_json::to_string(&response)?.as_bytes())?;
    writer.write_all(b"\n")?;
    writer.flush()?;
    Ok(())
}

fn wait_until_blocking(timeout: Duration, mut condition: impl FnMut() -> bool) -> Result<()> {
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
