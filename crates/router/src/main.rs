#![deny(clippy::unwrap_used)]
#![deny(unsafe_code)]

mod ipc;
mod services;
use parking_lot::Mutex as ParkingMutex;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use services::managed_process::repo_root;
use services::manager::ServiceManager;
use services::models::{CallContext, WorkerSpec};
use services::{
    command_run::CommandRunService, execution::ExecutionService, recovery::recover_after_start,
    session_db::SessionDbService,
};
use std::collections::{HashMap, HashSet};
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    Arc,
};
use std::time::{Duration as StdDuration, Instant};
use tura_router::registry::agent::UpsertAgentRequest;
use tura_router::registry::command::ExecuteCommandRequest;
use tura_router::registry::persona::UpsertPersonaRequest;
use tura_router::registry::{resolve_binary_target, Registry};

/// Maximum recursion depth for child sub-sessions (fork-bomb guard, T5.4).
const MAX_PLANNING_DEPTH: usize = 3;
/// Concurrent runtime-worker cap (fork-bomb guard, T5.4).
const MAX_RUNTIME_WORKERS: usize = 16;

#[derive(Clone)]
struct AppState {
    manager: ServiceManager,
    registry: Registry,
    session_db: SessionDbService,
    execution: ExecutionService,
    command_run: CommandRunService,
    lifecycle: FrontLifecycle,
    shutdown: Arc<AtomicBool>,
}

impl Serialize for AppState {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str("AppState")
    }
}

#[derive(Debug, Deserialize)]
struct RunAgentRequest {
    #[serde(default)]
    session_id: Option<String>,
    #[serde(default)]
    directory: Option<String>,
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    agent: Option<String>,
    #[serde(default)]
    session_type: Option<String>,
    #[serde(default)]
    prompt: Option<String>,
    #[serde(default)]
    message: Option<String>,
    #[serde(default)]
    input: Option<Value>,
    #[serde(default)]
    parent_session_id: Option<String>,
    #[serde(default)]
    depth: Option<usize>,
    #[serde(default)]
    runtime_context: Option<String>,
    #[serde(default)]
    planning_mode_override: Option<bool>,
    /// Worker env contract computed by the gateway (model / planning /
    /// stall-guard / ...). The router injects it into the subprocess as-is.
    #[serde(default)]
    worker_env: std::collections::HashMap<String, String>,
}

fn build_state() -> AppState {
    AppState {
        manager: ServiceManager::new(),
        registry: Registry::from_static(),
        session_db: SessionDbService::new(),
        execution: ExecutionService::new(),
        command_run: CommandRunService::new(),
        lifecycle: FrontLifecycle::new(),
        shutdown: Arc::new(AtomicBool::new(false)),
    }
}

#[derive(Clone)]
struct FrontLifecycle {
    active_connections: Arc<AtomicUsize>,
    leases: Arc<ParkingMutex<HashMap<String, FrontLease>>>,
    last_activity: Arc<ParkingMutex<Instant>>,
    idle_shutdown_after: StdDuration,
}

#[derive(Clone)]
struct FrontLease {
    kind: String,
    expires_at: Instant,
}

impl FrontLifecycle {
    fn new() -> Self {
        Self {
            active_connections: Arc::new(AtomicUsize::new(0)),
            leases: Arc::new(ParkingMutex::new(HashMap::new())),
            last_activity: Arc::new(ParkingMutex::new(Instant::now())),
            idle_shutdown_after: router_idle_shutdown_after(),
        }
    }

    fn connection_opened(&self) {
        self.active_connections.fetch_add(1, Ordering::SeqCst);
    }

    fn connection_closed(&self) {
        self.active_connections.fetch_sub(1, Ordering::SeqCst);
    }

    fn heartbeat(&self, input: &Value) -> anyhow::Result<Value> {
        let front_id = input
            .get("front_id")
            .or_else(|| input.get("frontId"))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| anyhow::anyhow!("front_id is required"))?
            .to_string();
        let kind = input
            .get("kind")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("gateway")
            .to_string();
        let ttl_ms = input
            .get("ttl_ms")
            .or_else(|| input.get("ttlMs"))
            .and_then(Value::as_u64)
            .filter(|value| *value > 0)
            .unwrap_or_else(default_front_lease_ttl_ms);
        let expires_at = Instant::now() + StdDuration::from_millis(ttl_ms);
        self.leases.lock().insert(
            front_id.clone(),
            FrontLease {
                kind: kind.clone(),
                expires_at,
            },
        );
        self.mark_activity();
        Ok(json!({
            "status": "ok",
            "front_id": front_id,
            "kind": kind,
            "ttl_ms": ttl_ms,
        }))
    }

    fn snapshot(&self) -> Value {
        let active_fronts = self.prune_and_count_valid_leases();
        let front_kinds = self
            .leases
            .lock()
            .values()
            .map(|lease| lease.kind.clone())
            .collect::<Vec<_>>();
        json!({
            "active_connections": self.active_connections.load(Ordering::SeqCst),
            "active_fronts": active_fronts,
            "front_kinds": front_kinds,
            "idle_shutdown_after_ms": self.idle_shutdown_after.as_millis() as u64,
            "idle_for_ms": self.last_activity.lock().elapsed().as_millis() as u64,
        })
    }

    fn should_shutdown_idle(&self, active_runtime_workers: usize, active_sessions: usize) -> bool {
        let active_fronts = self.prune_and_count_valid_leases();
        let active_connections = self.active_connections.load(Ordering::SeqCst);
        if active_fronts > 0
            || active_connections > 0
            || active_runtime_workers > 0
            || active_sessions > 0
        {
            self.mark_activity();
            return false;
        }
        self.last_activity.lock().elapsed() >= self.idle_shutdown_after
    }

    fn mark_activity(&self) {
        *self.last_activity.lock() = Instant::now();
    }

    fn prune_and_count_valid_leases(&self) -> usize {
        let now = Instant::now();
        let mut leases = self.leases.lock();
        leases.retain(|_, lease| lease.expires_at > now);
        leases.len()
    }
}

fn router_idle_shutdown_after() -> StdDuration {
    std::env::var("TURA_ROUTER_IDLE_SHUTDOWN_SECS")
        .ok()
        .and_then(|value| value.trim().parse::<u64>().ok())
        .filter(|seconds| *seconds > 0)
        .map(StdDuration::from_secs)
        .unwrap_or_else(|| StdDuration::from_secs(60))
}

fn default_front_lease_ttl_ms() -> u64 {
    std::env::var("TURA_ROUTER_FRONT_LEASE_TTL_SECS")
        .ok()
        .and_then(|value| value.trim().parse::<u64>().ok())
        .filter(|seconds| *seconds > 0)
        .unwrap_or(15)
        .saturating_mul(1000)
}

/// CLI subcommand `run-agent`: reads a `RunAgentRequest` JSON from stdin,
/// dispatches a runtime worker, and writes the result JSON to stdout.
async fn run_agent_cli() -> anyhow::Result<()> {
    let raw = read_stdin()?;
    let req: RunAgentRequest = serde_json::from_str(raw.trim())
        .map_err(|error| anyhow::anyhow!("invalid run-agent request json: {error}"))?;
    let state = build_state();
    let (_status, body) = dispatch_run_agent(&state, req).await;
    println!("{}", serde_json::to_string(&body)?);
    Ok(())
}

fn read_stdin() -> anyhow::Result<String> {
    use std::io::Read;
    let mut raw = String::new();
    std::io::stdin().read_to_string(&mut raw)?;
    Ok(raw)
}

fn print_json<T: Serialize>(value: &T) -> anyhow::Result<()> {
    println!("{}", serde_json::to_string(value)?);
    Ok(())
}

fn registry_agents_list_cli() -> anyhow::Result<()> {
    print_json(&Registry::from_static().agents.list_catalog())
}

fn registry_agent_get_cli() -> anyhow::Result<()> {
    let agent_id = std::env::args()
        .nth(2)
        .ok_or_else(|| anyhow::anyhow!("agent id is required"))?;
    let registry = Registry::from_static();
    match registry.agents.get_stored(&agent_id) {
        Some(agent) => print_json(&agent),
        None => Err(anyhow::anyhow!("agent not found: {agent_id}")),
    }
}

fn registry_agent_upsert_cli(agent_id: Option<String>) -> anyhow::Result<()> {
    let raw = read_stdin()?;
    let payload: UpsertAgentRequest = serde_json::from_str(raw.trim())
        .map_err(|error| anyhow::anyhow!("invalid agent payload json: {error}"))?;
    let registry = Registry::from_static();
    let agent = registry
        .agents
        .upsert(agent_id, payload)
        .map_err(|error| anyhow::anyhow!("failed to upsert registry agent: {error}"))?;
    print_json(&agent)
}

fn registry_agent_delete_cli() -> anyhow::Result<()> {
    let agent_id = std::env::args()
        .nth(2)
        .ok_or_else(|| anyhow::anyhow!("agent id is required"))?;
    let registry = Registry::from_static();
    let deleted = registry
        .agents
        .delete(&agent_id)
        .map_err(|error| anyhow::anyhow!("failed to delete registry agent {agent_id}: {error}"))?;
    print_json(&deleted)
}

fn registry_personas_list_cli() -> anyhow::Result<()> {
    print_json(&Registry::from_static().personas.list())
}

fn registry_persona_get_cli() -> anyhow::Result<()> {
    let persona_id = std::env::args()
        .nth(2)
        .ok_or_else(|| anyhow::anyhow!("persona id is required"))?;
    let registry = Registry::from_static();
    match registry.personas.get(&persona_id) {
        Some(persona) => print_json(&persona),
        None => Err(anyhow::anyhow!("persona not found: {persona_id}")),
    }
}

fn registry_persona_upsert_cli(persona_id: Option<String>) -> anyhow::Result<()> {
    let raw = read_stdin()?;
    let payload: UpsertPersonaRequest = serde_json::from_str(raw.trim())
        .map_err(|error| anyhow::anyhow!("invalid persona payload json: {error}"))?;
    let registry = Registry::from_static();
    let persona = registry
        .personas
        .upsert(persona_id, payload)
        .map_err(|error| anyhow::anyhow!("failed to upsert registry persona: {error}"))?;
    print_json(&persona)
}

fn registry_persona_delete_cli() -> anyhow::Result<()> {
    let persona_id = std::env::args()
        .nth(2)
        .ok_or_else(|| anyhow::anyhow!("persona id is required"))?;
    let registry = Registry::from_static();
    let deleted = registry.personas.delete(&persona_id).map_err(|error| {
        anyhow::anyhow!("failed to delete registry persona {persona_id}: {error}")
    })?;
    print_json(&deleted)
}

#[derive(Debug, Deserialize)]
struct RegistryDirectoryPayload {
    #[serde(default)]
    directory: Option<String>,
}

fn registry_commands_list_cli() -> anyhow::Result<()> {
    let raw = read_stdin()?;
    let payload = if raw.trim().is_empty() {
        RegistryDirectoryPayload { directory: None }
    } else {
        serde_json::from_str::<RegistryDirectoryPayload>(raw.trim())
            .map_err(|error| anyhow::anyhow!("invalid command list payload json: {error}"))?
    };
    let registry = Registry::from_static();
    print_json(&registry.commands.list(payload.directory.as_deref()))
}

#[derive(Debug, Deserialize)]
struct RegistryCommandExecutePayload {
    #[serde(default)]
    directory: Option<String>,
    command: String,
    #[serde(default)]
    args: Option<Vec<String>>,
}

fn registry_command_execute_cli() -> anyhow::Result<()> {
    let raw = read_stdin()?;
    let payload: RegistryCommandExecutePayload = serde_json::from_str(raw.trim())
        .map_err(|error| anyhow::anyhow!("invalid command execute payload json: {error}"))?;
    let registry = Registry::from_static();
    let response = registry.commands.execute(
        payload.directory.as_deref(),
        ExecuteCommandRequest {
            command: payload.command,
            args: payload.args,
        },
    );
    print_json(&response)
}

async fn serve_stdio() -> anyhow::Result<()> {
    use std::sync::Arc;
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

    let state = build_state();
    let _ = recover_after_start(&state.session_db)?;
    let stdin = tokio::io::stdin();
    // Shared, locked writer: each request is handled on its own task and writes
    // its response (tagged with `request_id`) when ready, so a slow call (e.g. a
    // long-running `execution.enqueue_turn`) never head-of-line blocks a
    // concurrent `health_check`. The gateway client multiplexes responses back
    // to per-call mailboxes by `request_id`.
    let stdout = Arc::new(tokio::sync::Mutex::new(tokio::io::stdout()));
    let mut lines = BufReader::new(stdin).lines();
    while let Some(line) = lines.next_line().await? {
        let trimmed = line.trim().to_string();
        if trimmed.is_empty() {
            continue;
        }
        let state = state.clone();
        let stdout = Arc::clone(&stdout);
        tokio::spawn(async move {
            let response = match serde_json::from_str::<ipc::IpcRequest>(&trimmed) {
                Ok(request) => handle_ipc_request(&state, request).await,
                Err(error) => {
                    ipc::IpcResponse::error("invalid", format!("invalid ipc request: {error}"))
                }
            };
            if let Ok(encoded) = serde_json::to_string(&response) {
                let mut out = stdout.lock().await;
                let _ = out.write_all(format!("{encoded}\n").as_bytes()).await;
                let _ = out.flush().await;
            }
        });
    }
    Ok(())
}

/// File (under the instance's db dir) recording the running router daemon's
/// socket endpoint, so any front can probe-and-connect rather than spawn its own.
fn router_addr_path() -> std::path::PathBuf {
    session_log::path::default_db_dir().join("router.addr")
}

fn publish_router_addr(addr: &std::net::SocketAddr) -> anyhow::Result<()> {
    let path = router_addr_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let pid = std::process::id();
    let record = json!({
        "addr": addr.to_string(),
        "version": tura_path::instance_version(),
        "pid": pid,
        "process_start_time": current_process_start_time(pid),
    });
    let tmp = path.with_extension("addr.tmp");
    std::fs::write(&tmp, serde_json::to_string(&record)?)?;
    std::fs::rename(&tmp, &path)?;
    Ok(())
}

async fn serve_socket() -> anyhow::Result<()> {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    use tokio::net::TcpListener;
    use tokio::sync::Mutex as AsyncMutex;
    use tokio::time::{timeout, Duration};

    let _router_lock = RouterDaemonLock::acquire()?;
    let state = build_state();
    let _ = recover_after_start(&state.session_db)?;
    // The daemon owns the backend: bring up the single session_db owner now.
    let _ = state.session_db.start();

    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;
    publish_router_addr(&addr)?;
    std::env::set_var("TURA_ROUTER_ADDR", addr.to_string());
    eprintln!("router socket daemon listening on {addr}");
    start_idle_shutdown_monitor(state.clone());

    while !state.shutdown.load(Ordering::SeqCst) {
        let accepted = match timeout(Duration::from_millis(250), listener.accept()).await {
            Ok(accepted) => accepted?,
            Err(_) => continue,
        };
        let (stream, _) = accepted;
        let state = state.clone();
        state.lifecycle.connection_opened();
        tokio::spawn(async move {
            let (read, write) = stream.into_split();
            let write = Arc::new(AsyncMutex::new(write));
            let active_sessions = Arc::new(AsyncMutex::new(HashSet::<String>::new()));
            let pending_tasks =
                Arc::new(AsyncMutex::new(Vec::<tokio::task::JoinHandle<()>>::new()));
            let mut lines = BufReader::new(read).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let trimmed = line.trim().to_string();
                if trimmed.is_empty() {
                    continue;
                }
                let parsed = match serde_json::from_str::<ipc::IpcRequest>(&trimmed) {
                    Ok(request) => request,
                    Err(e) => {
                        let response =
                            ipc::IpcResponse::error("invalid", format!("invalid ipc request: {e}"));
                        if let Ok(encoded) = serde_json::to_string(&response) {
                            let mut w = write.lock().await;
                            let _ = w.write_all(format!("{encoded}\n").as_bytes()).await;
                            let _ = w.flush().await;
                        }
                        continue;
                    }
                };
                state.lifecycle.mark_activity();
                let active_session_id = enqueue_turn_session_id(&parsed);
                if let Some(session_id) = active_session_id.as_ref() {
                    active_sessions.lock().await.insert(session_id.clone());
                }
                let state = state.clone();
                let write = Arc::clone(&write);
                let active_sessions = Arc::clone(&active_sessions);
                let handle = tokio::spawn(async move {
                    let response = handle_ipc_request(&state, parsed).await;
                    if let Some(session_id) = active_session_id.as_ref() {
                        active_sessions.lock().await.remove(session_id);
                    }
                    if let Ok(encoded) = serde_json::to_string(&response) {
                        let mut w = write.lock().await;
                        let _ = w.write_all(format!("{encoded}\n").as_bytes()).await;
                        let _ = w.flush().await;
                    }
                });
                pending_tasks.lock().await.push(handle);
            }
            let sessions = active_sessions
                .lock()
                .await
                .iter()
                .cloned()
                .collect::<Vec<_>>();
            for session_id in sessions {
                let _ = state
                    .execution
                    .cancel_turn(&state, json!({ "session_id": session_id }))
                    .await;
            }
            let tasks = pending_tasks.lock().await.drain(..).collect::<Vec<_>>();
            for task in tasks {
                task.abort();
            }
            state.lifecycle.connection_closed();
        });
    }
    let _ = std::fs::remove_file(router_addr_path());
    Ok(())
}

fn start_idle_shutdown_monitor(state: AppState) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_millis(250));
        loop {
            interval.tick().await;
            if state.shutdown.load(Ordering::SeqCst) {
                return;
            }
            let active_runtime_workers = state.manager.count_workers_with_prefix("runtime_worker:");
            let active_sessions = state.execution.active_session_count();
            if state
                .lifecycle
                .should_shutdown_idle(active_runtime_workers, active_sessions)
            {
                unpublish_router_addr();
                let stopped = state
                    .manager
                    .stop_workers_with_prefix("runtime_worker:")
                    .await;
                state.session_db.stop();
                mark_router_shutting_down(&state);
                eprintln!("router idle shutdown: stopped {stopped} runtime workers and session_db");
                return;
            }
        }
    });
}

fn mark_router_shutting_down(state: &AppState) {
    state.shutdown.store(true, Ordering::SeqCst);
    unpublish_router_addr();
}

fn unpublish_router_addr() {
    let _ = std::fs::remove_file(router_addr_path());
}

struct RouterDaemonLock {
    file: std::fs::File,
    path: std::path::PathBuf,
}

impl RouterDaemonLock {
    fn acquire() -> anyhow::Result<Self> {
        use fs2::FileExt;
        use std::io::{Seek, SeekFrom, Write};

        let dir = tura_path::locks_dir();
        std::fs::create_dir_all(&dir)?;
        let path = dir.join(format!("router-{}.lock", tura_path::build_kind()));
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(&path)?;
        file.try_lock_exclusive().map_err(|error| {
            anyhow::anyhow!(
                "another router daemon already owns {}: {error}",
                path.display()
            )
        })?;
        file.set_len(0)?;
        file.seek(SeekFrom::Start(0))?;
        writeln!(file, "pid={}", std::process::id())?;
        writeln!(file, "kind=router")?;
        writeln!(file, "build_kind={}", tura_path::build_kind())?;
        writeln!(file, "home={}", tura_path::instance_home().display())?;
        Ok(Self { file, path })
    }
}

impl Drop for RouterDaemonLock {
    fn drop(&mut self) {
        let _ = self.file.unlock();
        let _ = std::fs::remove_file(&self.path);
    }
}

async fn handle_ipc_request(state: &AppState, request: ipc::IpcRequest) -> ipc::IpcResponse {
    let result = match request.method.as_str() {
        "" | "health_check"
            if request.kind == "health_check" || request.method == "health_check" =>
        {
            let session_db = state.session_db.start().unwrap_or_else(|error| {
                json!({
                    "status": "error",
                    "error": error.to_string()
                })
            });
            Ok(json!({
                "status": "ok",
                "pid": std::process::id(),
                "process_start_time": current_process_start_time(std::process::id()),
                "session_db": session_db,
                "runtime_policy": {
                    "max_active_runtime_workers": services::runtime_workers::MAX_ACTIVE_RUNTIME_WORKERS,
                    "runtime_worker_idle_ttl_secs": services::runtime_workers::RUNTIME_WORKER_IDLE_TTL_SECS,
                    "max_idle_runtime_workers": services::runtime_workers::MAX_IDLE_RUNTIME_WORKERS
                }
            }))
        }
        "session_db.lifecycle.start" => state.session_db.start(),
        "session_db.lifecycle.status" => Ok(state.session_db.status()),
        "session_db.lifecycle.restart" => state.session_db.restart(),
        "lifecycle.front_heartbeat" => state.lifecycle.heartbeat(&request.payload),
        "lifecycle.status" => Ok(state.lifecycle.snapshot()),
        "execution.enqueue_turn" => state.execution.enqueue_turn(state, request.payload).await,
        "execution.command_run" => state.command_run.execute(request.payload).await,
        "execution.cancel_turn" => Ok(state.execution.cancel_turn(state, request.payload).await),
        "execution.get_status" => Ok(json!({ "status": "ok" })),
        "execution.kill_session_workers" => {
            let session_id = request
                .payload
                .get("session_id")
                .and_then(Value::as_str)
                .filter(|value| !value.trim().is_empty())
                .map(str::to_string);
            let stopped = if let Some(session_id) = session_id {
                usize::from(
                    state
                        .manager
                        .stop_worker_by_key(&format!("runtime_worker:{session_id}"))
                        .await,
                )
            } else {
                state
                    .manager
                    .stop_workers_with_prefix("runtime_worker:")
                    .await
            };
            Ok(json!({ "status": "stopped", "stopped": stopped }))
        }
        "execution.shutdown" => {
            let stopped = state
                .manager
                .stop_workers_with_prefix("runtime_worker:")
                .await;
            state.session_db.stop();
            mark_router_shutting_down(state);
            Ok(json!({ "status": "shutting_down", "runtime_workers_stopped": stopped }))
        }
        other => Err(anyhow::anyhow!("unknown router method: {other}")),
    };
    match result {
        Ok(payload) => ipc::IpcResponse::ok(request.request_id, payload),
        Err(error) => ipc::IpcResponse::error(request.request_id, error.to_string()),
    }
}

fn enqueue_turn_session_id(request: &ipc::IpcRequest) -> Option<String> {
    if request.method != "execution.enqueue_turn" {
        return None;
    }
    request
        .payload
        .get("session_id")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string)
}

fn current_process_start_time(pid: u32) -> Option<u64> {
    let mut system = sysinfo::System::new_all();
    system.refresh_processes();
    system
        .process(sysinfo::Pid::from_u32(pid))
        .map(sysinfo::Process::start_time)
}

fn main() -> anyhow::Result<()> {
    tura_path::process_hardening::harden_current_process("router");
    let command = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "serve".to_string());
    match command.as_str() {
        "serve" => tokio_runtime()?.block_on(serve_stdio()),
        "serve-socket" => tokio_runtime()?.block_on(serve_socket()),
        "run-agent" => tokio_runtime()?.block_on(run_agent_cli()),
        "registry-agents-list" => registry_agents_list_cli(),
        "registry-agent-get" => registry_agent_get_cli(),
        "registry-agent-create" => registry_agent_upsert_cli(None),
        "registry-agent-update" => {
            let agent_id = std::env::args()
                .nth(2)
                .ok_or_else(|| anyhow::anyhow!("agent id is required"))?;
            registry_agent_upsert_cli(Some(agent_id))
        }
        "registry-agent-delete" => registry_agent_delete_cli(),
        "registry-personas-list" => registry_personas_list_cli(),
        "registry-persona-get" => registry_persona_get_cli(),
        "registry-persona-create" => registry_persona_upsert_cli(None),
        "registry-persona-update" => {
            let persona_id = std::env::args()
                .nth(2)
                .ok_or_else(|| anyhow::anyhow!("persona id is required"))?;
            registry_persona_upsert_cli(Some(persona_id))
        }
        "registry-persona-delete" => registry_persona_delete_cli(),
        "registry-commands-list" => registry_commands_list_cli(),
        "registry-command-execute" => registry_command_execute_cli(),
        _ => Err(anyhow::anyhow!("unknown router command: {command}")),
    }
}

fn tokio_runtime() -> anyhow::Result<tokio::runtime::Runtime> {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(Into::into)
}

/// Pure-logic core of run_agent: resolve agent spec, spawn the runtime-
/// worker subprocess (CLI/NDJSON), forward the call, and stream the result
/// back. Gateway and child runtime dispatch use the `run-agent` CLI
/// subcommand, never router HTTP.
fn resolve_runtime_worker_binary(root: &std::path::Path) -> Option<std::path::PathBuf> {
    resolve_binary_target(root, "tura_runtime")
}

fn push_router_owned_runtime_env(env: &mut Vec<(String, String)>) {
    env.push((
        "TURA_HOME".to_string(),
        tura_path::instance_home().display().to_string(),
    ));
    let project_root = std::env::var_os("TURA_PROJECT_ROOT")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(tura_path::canonical_root);
    env.push((
        "TURA_PROJECT_ROOT".to_string(),
        project_root.display().to_string(),
    ));
    for key in ["SESSION_LOG_DB_ROOT", "TURA_DB_ROOT"] {
        if let Some(value) = std::env::var_os(key).filter(|value| !value.is_empty()) {
            env.push((key.to_string(), value.to_string_lossy().to_string()));
        }
    }
}

async fn dispatch_run_agent(state: &AppState, req: RunAgentRequest) -> (u16, Value) {
    let session_id = req
        .session_id
        .clone()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| format!("router-{}", uuid::Uuid::new_v4()));
    if router_debug_enabled() {
        eprintln!(
            "router debug: dispatch_run_agent start session_id={} agent={:?} model={:?}",
            session_id, req.agent, req.model
        );
    }

    let prompt = req
        .prompt
        .clone()
        .or_else(|| req.message.clone())
        .or_else(|| {
            req.input
                .as_ref()
                .and_then(|value| value.as_str().map(str::to_string))
        });
    let Some(prompt) = prompt.filter(|value| !value.trim().is_empty()) else {
        return (
            200,
            json!({"ok": true, "session_id": session_id, "message": "session ready; no prompt provided"}),
        );
    };

    // T5.4 recursion-depth and concurrency cap: prevent child-session fork
    // bombs; on breach, reject and report back to the parent session.
    let child_depth = req.depth.unwrap_or(0);
    if child_depth > MAX_PLANNING_DEPTH {
        return (
            429,
            json!({
                "ok": false,
                "session_id": session_id,
                "error": format!(
                    "planning depth {child_depth} exceeds limit {MAX_PLANNING_DEPTH}"
                )
            }),
        );
    }
    let active_workers = state.manager.count_workers_with_prefix("runtime_worker:");
    if active_workers >= MAX_RUNTIME_WORKERS {
        return (
            429,
            json!({
                "ok": false,
                "session_id": session_id,
                "error": format!(
                    "runtime worker concurrency limit reached ({active_workers}/{MAX_RUNTIME_WORKERS})"
                )
            }),
        );
    }

    // Registry resolves the agent spec (registry belongs to the router, the
    // loop belongs to the runtime worker).
    let agent_spec = state
        .registry
        .agents
        .resolve(req.agent.as_deref(), req.session_type.as_deref());

    if let Err(error) = state.session_db.start() {
        return (
            503,
            json!({
                "ok": false,
                "session_id": session_id,
                "error": format!("session_db service is not ready for runtime dispatch: {error}")
            }),
        );
    }

    let worker_binary = match resolve_runtime_worker_binary(&repo_root()) {
        Some(path) => path,
        None => {
            return (
                500,
                json!({
                    "ok": false,
                    "session_id": session_id,
                    "error": "runtime worker binary (tura_runtime) not found in bin/ or target/{release,debug}"
                }),
            );
        }
    };

    // Env contract: role, callback channel, model, plus parent/depth for
    // child sub-sessions (T5.2).
    let mut env = vec![("TURA_ROLE".to_string(), "runtime_worker".to_string())];
    if let Some(model) = req
        .model
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        env.push(("TURA_SESSION_MODEL_OVERRIDE".to_string(), model.to_string()));
    }
    if let Some(parent) = req
        .parent_session_id
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        env.push(("TURA_PARENT_SESSION_ID".to_string(), parent.to_string()));
        env.push((
            "TURA_PLANNING_DEPTH".to_string(),
            req.depth.unwrap_or(1).to_string(),
        ));
    }
    // Pass through the gateway-supplied env contract (planning,
    // reasoning, stall-guard, ...) verbatim.
    for (key, value) in &req.worker_env {
        env.push((key.clone(), value.clone()));
    }
    push_router_owned_runtime_env(&mut env);
    if let Ok(addr) = std::env::var("TURA_ROUTER_ADDR") {
        if !addr.trim().is_empty() {
            env.push(("TURA_ROUTER_ADDR".to_string(), addr));
        }
    }

    let spec = WorkerSpec {
        key: format!("runtime_worker:{session_id}"),
        service_name: "runtime_worker".to_string(),
        executable: worker_binary,
        args: Vec::new(),
        env,
    };

    let worker = match state.manager.ensure_worker(spec).await {
        Ok(worker) => worker,
        Err(error) => {
            return (
                502,
                json!({
                    "ok": false,
                    "session_id": session_id,
                    "error": format!("failed to start runtime worker: {error}")
                }),
            );
        }
    };
    if router_debug_enabled() {
        eprintln!(
            "router debug: dispatch_run_agent worker ready session_id={} worker_id={}",
            session_id, worker.worker_id
        );
    }

    let call_input = json!({
        "session_id": session_id,
        "directory": req.directory,
        "model": req.model,
        "agent": agent_spec.agent_name,
        "agent_spec": agent_spec,
        "prompt": prompt,
        "runtime_context": req.runtime_context,
        "planning_mode_override": req.planning_mode_override,
    });
    let ctx = CallContext::new(
        "POST".to_string(),
        format!("/runtime_worker/{session_id}"),
        call_input,
    );

    let worker_id = worker.worker_id.clone();
    if router_debug_enabled() {
        eprintln!("router debug: dispatch_run_agent calling worker session_id={session_id} worker_id={worker_id}");
    }
    let call_result = state.manager.call_worker(&worker_id, ctx).await;
    state.manager.stop_worker(&worker_id).await;
    if router_debug_enabled() {
        eprintln!(
            "router debug: dispatch_run_agent worker returned session_id={} worker_id={} ok={}",
            session_id,
            worker_id,
            call_result.is_ok()
        );
    }
    match call_result {
        Ok(result) => {
            let ok = result.get("ok").and_then(Value::as_bool).unwrap_or(false);
            let status = if ok { 200 } else { 502 };
            (
                status,
                json!({
                    "ok": ok,
                    "session_id": session_id,
                    "worker_id": worker_id,
                    "agent": agent_spec.agent_name,
                    "result": result,
                }),
            )
        }
        Err(error) => (
            502,
            json!({
                "ok": false,
                "session_id": session_id,
                "worker_id": worker_id,
                "error": format!("runtime worker invocation failed: {error}")
            }),
        ),
    }
}

fn router_debug_enabled() -> bool {
    std::env::var("TURA_DEBUG_RUNTIME")
        .ok()
        .is_some_and(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn execution_shutdown_sets_daemon_exit_flag() -> anyhow::Result<()> {
        let state = build_state();
        let response = tokio_runtime()?.block_on(handle_ipc_request(
            &state,
            ipc::IpcRequest {
                request_id: "shutdown-test".to_string(),
                kind: "call".to_string(),
                method: "execution.shutdown".to_string(),
                payload: json!({}),
                deadline_ms: None,
            },
        ));

        assert!(response.ok, "shutdown failed: {:?}", response.error);
        assert!(state.shutdown.load(Ordering::SeqCst));
        assert_eq!(response.payload["status"], "shutting_down");
        assert_eq!(response.payload["runtime_workers_stopped"], 0);
        Ok(())
    }

    #[test]
    fn enqueue_turn_session_id_tracks_only_turn_requests() {
        let turn = ipc::IpcRequest {
            request_id: "turn".to_string(),
            kind: "call".to_string(),
            method: "execution.enqueue_turn".to_string(),
            payload: json!({
                "session_id": "session-1",
                "turn_id": "turn-1",
                "payload": {}
            }),
            deadline_ms: None,
        };
        assert_eq!(enqueue_turn_session_id(&turn).as_deref(), Some("session-1"));

        let command_run = ipc::IpcRequest {
            method: "execution.command_run".to_string(),
            payload: json!({ "session_id": "session-1" }),
            ..turn
        };
        assert_eq!(enqueue_turn_session_id(&command_run), None);

        let blank_session = ipc::IpcRequest {
            method: "execution.enqueue_turn".to_string(),
            payload: json!({ "session_id": "   " }),
            ..command_run
        };
        assert_eq!(enqueue_turn_session_id(&blank_session), None);
    }

    #[test]
    fn lifecycle_heartbeat_keeps_router_alive_until_lease_expires() -> anyhow::Result<()> {
        let lifecycle = FrontLifecycle {
            active_connections: Arc::new(AtomicUsize::new(0)),
            leases: Arc::new(ParkingMutex::new(HashMap::new())),
            last_activity: Arc::new(ParkingMutex::new(
                Instant::now() - StdDuration::from_millis(50),
            )),
            idle_shutdown_after: StdDuration::from_millis(10),
        };

        lifecycle.heartbeat(&json!({
            "front_id": "gateway-test",
            "kind": "gateway",
            "ttl_ms": 100,
        }))?;
        std::thread::sleep(StdDuration::from_millis(50));
        assert!(!lifecycle.should_shutdown_idle(0, 0));
        std::thread::sleep(StdDuration::from_millis(80));
        assert!(lifecycle.should_shutdown_idle(0, 0));
        Ok(())
    }

    #[test]
    fn lifecycle_active_connection_blocks_idle_shutdown() {
        let lifecycle = FrontLifecycle {
            active_connections: Arc::new(AtomicUsize::new(0)),
            leases: Arc::new(ParkingMutex::new(HashMap::new())),
            last_activity: Arc::new(ParkingMutex::new(
                Instant::now() - StdDuration::from_millis(50),
            )),
            idle_shutdown_after: StdDuration::from_millis(10),
        };

        lifecycle.connection_opened();
        std::thread::sleep(StdDuration::from_millis(20));
        assert!(!lifecycle.should_shutdown_idle(0, 0));
        lifecycle.connection_closed();
        assert!(!lifecycle.should_shutdown_idle(0, 0));
        std::thread::sleep(StdDuration::from_millis(20));
        assert!(lifecycle.should_shutdown_idle(0, 0));
    }
}
