#![warn(clippy::unwrap_used)]

mod services;
mod utils;
use axum::{
    extract::{Path, State},
    http::{Method, StatusCode},
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use services::managed_process::{repo_root, ManagedProcessManager};
use services::manager::ServiceManager;
use services::models::CallContext;
use std::{
    fs,
    net::SocketAddr,
    path::{Path as StdPath, PathBuf},
    process::Stdio,
    time::SystemTime,
};
use tokio::time::{self, Duration};
use tracing::{error, info, warn};
use utils::{
    cli,
    port::{ensure_port_is_free, port_is_occupied},
};

#[derive(Clone)]
struct AppState {
    manager: ServiceManager,
    processes: ManagedProcessManager,
    router_port: u16,
    gateway_port: u16,
    frontend_port: u16,
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
struct TuraPathsConfig {
    network: TuraNetworkConfig,
}

#[derive(Debug, Deserialize)]
struct TuraNetworkConfig {
    #[serde(default = "default_router_port", rename = "ROUTER_PORT")]
    router_port: u16,
    #[serde(default = "default_gateway_port", rename = "GATEWAY_PORT")]
    gateway_port: u16,
}

fn default_router_port() -> u16 {
    8080
}

fn default_gateway_port() -> u16 {
    4096
}

fn load_tura_config() -> TuraPathsConfig {
    let root = repo_root();
    let candidates = [
        root.join("config").join("tura_paths.json"),
        root.join("tura_paths.json"),
    ];

    for path in candidates {
        if !path.exists() {
            continue;
        }
        match std::fs::read_to_string(&path)
            .ok()
            .and_then(|content| serde_json::from_str::<TuraPathsConfig>(&content).ok())
        {
            Some(config) => return config,
            None => {
                warn!(path = %path.display(), "failed to parse tura path config, trying fallback")
            }
        }
    }

    TuraPathsConfig {
        network: TuraNetworkConfig {
            router_port: default_router_port(),
            gateway_port: default_gateway_port(),
        },
    }
}

#[derive(Debug, Deserialize)]
struct RunServiceRequest {
    services_dir: String,
    input: Value,
}

#[derive(Debug, Deserialize)]
struct RunToolRequest {
    tool: String,
    input: Value,
    #[serde(default)]
    lsp_worker_id: Option<String>,
    #[serde(default)]
    lsp_language: Option<String>,
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
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing();

    let config = load_tura_config();
    let router_port = std::env::var("TURA_ROUTER_PORT")
        .ok()
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(config.network.router_port);
    let gateway_port = std::env::var("TURA_GATEWAY_PORT")
        .ok()
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(config.network.gateway_port);
    let frontend_port = std::env::var("TURA_FRONTEND_PORT")
        .ok()
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(3000);
    info!(port = router_port, "router boot requested");
    ensure_port_is_free(router_port).await?;

    let state = AppState {
        manager: ServiceManager::new(),
        processes: ManagedProcessManager::new(),
        router_port,
        gateway_port,
        frontend_port,
    };

    let bootstrap_state = state.clone();
    tokio::spawn(async move {
        bootstrap_services(&bootstrap_state).await;
    });

    let app = Router::new()
        .route("/health", get(health))
        .route("/services/status", get(services_status))
        .route("/services/bootstrap", post(bootstrap))
        .route("/run_tool", post(run_tool))
        .route("/run_agent", post(run_agent))
        .route("/run_service", post(run_service))
        .route("/services/:service_name/:worker_id", post(call_service))
        .route("/lsp/:worker_id/check/symbols", post(lsp_check_symbols))
        .route(
            "/lsp/:worker_id/check/definition",
            post(lsp_check_definition),
        )
        .route(
            "/lsp/:worker_id/check/references",
            post(lsp_check_references),
        )
        .route(
            "/lsp/:worker_id/check/diagnostics",
            post(lsp_check_diagnostics),
        )
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], router_port));
    info!(%addr, "router listening");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

fn init_tracing() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,tura_router=debug".into()),
        )
        .init();
}

async fn health(State(state): State<AppState>) -> impl IntoResponse {
    Json(json!({
        "ok": true,
        "service": "tura_router",
        "port": state.router_port,
        "gateway_url": format!("http://127.0.0.1:{}", state.gateway_port),
        "frontend_url": format!("http://127.0.0.1:{}", state.frontend_port),
    }))
}

async fn services_status(State(state): State<AppState>) -> Json<Value> {
    Json(json!({
        "ok": true,
        "processes": state.processes.statuses().await,
        "workers": state.manager.statuses().await,
    }))
}

#[axum::debug_handler]
async fn bootstrap(State(state): State<AppState>) -> Json<Value> {
    bootstrap_services(&state).await;
    Json(json!({
        "ok": true,
        "processes": state.processes.statuses().await,
        "workers": state.manager.statuses().await,
    }))
}

async fn bootstrap_services(state: &AppState) {
    if let Err(err) = ensure_gateway(state).await {
        warn!(error = %err, "gateway bootstrap failed");
    }
    if let Err(err) = ensure_frontend(state).await {
        warn!(error = %err, "frontend bootstrap failed");
    }
    if let Err(err) = ensure_lsp(state).await {
        warn!(error = %err, "lsp bootstrap failed");
    }
}

async fn ensure_gateway(state: &AppState) -> anyhow::Result<()> {
    let gateway_url = format!("http://127.0.0.1:{}", state.gateway_port);
    if port_is_occupied(state.gateway_port).await? {
        info!(
            port = state.gateway_port,
            "gateway port already occupied, assuming gateway is managed externally"
        );
        wait_for_http_ok(&format!("{gateway_url}/global/health"), "gateway").await?;
        return Ok(());
    }

    let root = repo_root();
    let release_gateway_executable = root.join("target").join("release").join(if cfg!(windows) {
        "gateway.exe"
    } else {
        "gateway"
    });
    let debug_gateway_executable = root.join("target").join("debug").join(if cfg!(windows) {
        "gateway.exe"
    } else {
        "gateway"
    });
    let gateway_executable = if release_gateway_executable.exists()
        && !gateway_binary_is_stale(&root, &release_gateway_executable)
    {
        release_gateway_executable
    } else {
        info!("gateway executable missing or stale; building debug gateway before launch");
        cli::run_command(&root, "cargo build -p gateway").await?;
        debug_gateway_executable
    };
    let (program, args) = (
        gateway_executable.to_string_lossy().into_owned(),
        Vec::new(),
    );
    state
        .processes
        .ensure(
            "gateway",
            &program,
            &args,
            &root,
            &[
                ("PORT", state.gateway_port.to_string()),
                ("TURA_GATEWAY_URL", gateway_url.clone()),
                (
                    "TURA_ROUTER_URL",
                    format!("http://127.0.0.1:{}", state.router_port),
                ),
            ],
            Some(gateway_url),
        )
        .await?;
    wait_for_http_ok(
        &format!("http://127.0.0.1:{}/global/health", state.gateway_port),
        "gateway",
    )
    .await?;
    Ok(())
}

fn gateway_binary_is_stale(root: &StdPath, executable: &StdPath) -> bool {
    let Ok(binary_mtime) = fs::metadata(executable).and_then(|meta| meta.modified()) else {
        return true;
    };
    let source_root = root.join("crates").join("gateway").join("src");
    newest_source_mtime(&source_root).is_some_and(|mtime| mtime > binary_mtime)
}

fn newest_source_mtime(path: &StdPath) -> Option<SystemTime> {
    let entries = fs::read_dir(path).ok()?;
    let mut newest = None;
    for entry in entries.flatten() {
        let entry_path = entry.path();
        let metadata = entry.metadata().ok()?;
        let modified = metadata.modified().ok();
        if metadata.is_dir() {
            newest = newest.max(newest_source_mtime(&entry_path));
        } else if matches!(
            entry_path.extension().and_then(|ext| ext.to_str()),
            Some("rs")
        ) {
            newest = newest.max(modified);
        }
    }
    newest
}

async fn wait_for_http_ok(url: &str, service_name: &str) -> anyhow::Result<()> {
    let client = reqwest::Client::new();
    let deadline = time::Instant::now() + time::Duration::from_secs(90);
    let mut last_error = String::from("not attempted");

    while time::Instant::now() < deadline {
        match client.get(url).send().await {
            Ok(response) if response.status().is_success() => {
                info!(service_name, url, "service health check passed");
                return Ok(());
            }
            Ok(response) => {
                last_error = format!("HTTP {}", response.status());
            }
            Err(err) => {
                last_error = err.to_string();
            }
        }

        time::sleep(time::Duration::from_millis(500)).await;
    }

    anyhow::bail!("{service_name} did not become healthy at {url}: {last_error}");
}

async fn ensure_frontend(state: &AppState) -> anyhow::Result<()> {
    if port_is_occupied(state.frontend_port).await? {
        info!(
            port = state.frontend_port,
            "frontend port already occupied, assuming frontend is managed externally"
        );
        return Ok(());
    }

    let ui_root = repo_root().join("apps").join("ui");
    let args = vec![
        "run".to_string(),
        "--cwd".to_string(),
        "packages/app".to_string(),
        "dev".to_string(),
        "--host".to_string(),
        "0.0.0.0".to_string(),
        "--port".to_string(),
        state.frontend_port.to_string(),
    ];
    state
        .processes
        .ensure(
            "frontend",
            "bun",
            &args,
            &ui_root,
            &[
                ("VITE_OPENCODE_SERVER_HOST", "127.0.0.1".to_string()),
                ("VITE_OPENCODE_SERVER_PORT", state.gateway_port.to_string()),
            ],
            Some(format!("http://127.0.0.1:{}", state.frontend_port)),
        )
        .await?;
    Ok(())
}

async fn ensure_lsp(state: &AppState) -> anyhow::Result<()> {
    let lsp_dir = repo_root().join("services").join("lsp");
    let worker = state.manager.ensure_service_ready(&lsp_dir).await?;
    info!(
        worker_id = worker.worker_id,
        service_name = worker.service_name,
        "lsp service ready"
    );
    Ok(())
}

async fn run_tool(
    State(state): State<AppState>,
    Json(req): Json<RunToolRequest>,
) -> (StatusCode, Json<Value>) {
    let tools_dir = PathBuf::from("C:/Users/liuliu/RustroverProjects/turaOSv2/target/release");
    let tool_bin = tools_dir.join(format!("{}.exe", req.tool));

    if !tool_bin.exists() {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({
                "ok": false,
                "error": format!("tool {} not found at {}", req.tool, tool_bin.display())
            })),
        );
    }

    let input_json = serde_json::to_string(&req.input).unwrap_or_default();

    let mut cmd = tokio::process::Command::new(&tool_bin);
    cmd.stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    if let Some(ref worker_id) = req.lsp_worker_id {
        cmd.env(
            "TURA_ROUTER_URL",
            format!("http://localhost:{}", state.router_port),
        );
        cmd.env("TURA_LSP_WORKER_ID", worker_id);
        if let Some(ref lang) = req.lsp_language {
            cmd.env("TURA_LSP_LANGUAGE", lang);
        }
    }

    match cmd.spawn() {
        Ok(mut child) => {
            if let Some(mut stdin) = child.stdin.take() {
                use tokio::io::AsyncWriteExt;
                if let Err(e) = stdin.write_all(input_json.as_bytes()).await {
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({"ok": false, "error": e.to_string()})),
                    );
                }
                if let Err(e) = stdin.flush().await {
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({"ok": false, "error": e.to_string()})),
                    );
                }
            }

            match time::timeout(time::Duration::from_secs(30), child.wait_with_output()).await {
                Ok(Ok(output)) => {
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    match serde_json::from_str(&stdout) {
                        Ok(v) => (StatusCode::OK, Json(v)),
                        Err(_) => (
                            StatusCode::OK,
                            Json(json!({"ok": true, "output": stdout.to_string()})),
                        ),
                    }
                }
                Ok(Err(e)) => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"ok": false, "error": e.to_string()})),
                ),
                Err(_) => (
                    StatusCode::REQUEST_TIMEOUT,
                    Json(json!({"ok": false, "error": "tool execution timeout"})),
                ),
            }
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"ok": false, "error": e.to_string()})),
        ),
    }
}

#[axum::debug_handler]
async fn run_agent(
    State(state): State<AppState>,
    Json(req): Json<RunAgentRequest>,
) -> (StatusCode, Json<Value>) {
    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
    {
        Ok(client) => client,
        Err(error) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(
                    json!({"ok": false, "error": format!("failed to build HTTP client: {error}")}),
                ),
            );
        }
    };
    let gateway_url = format!("http://127.0.0.1:{}", state.gateway_port);

    let session_id = match req.session_id {
        Some(session_id) => session_id,
        None => {
            let payload = json!({
                "directory": req.directory,
                "model": req.model,
                "agent": req.agent,
                "session_type": req.session_type,
            });
            let response = match client
                .post(format!("{gateway_url}/session"))
                .json(&payload)
                .send()
                .await
            {
                Ok(response) => response,
                Err(error) => {
                    return (
                        StatusCode::BAD_GATEWAY,
                        Json(
                            json!({"ok": false, "error": format!("gateway session create failed: {error}")}),
                        ),
                    );
                }
            };
            if !response.status().is_success() {
                return (
                    StatusCode::BAD_GATEWAY,
                    Json(
                        json!({"ok": false, "error": format!("gateway session create returned HTTP {}", response.status())}),
                    ),
                );
            }
            let value = match response.json::<Value>().await {
                Ok(value) => value,
                Err(error) => {
                    return (
                        StatusCode::BAD_GATEWAY,
                        Json(
                            json!({"ok": false, "error": format!("gateway session create response parse failed: {error}")}),
                        ),
                    );
                }
            };
            match value.get("id").and_then(Value::as_str) {
                Some(id) => id.to_string(),
                None => {
                    return (
                        StatusCode::BAD_GATEWAY,
                        Json(
                            json!({"ok": false, "error": "gateway session create response did not include id", "response": value}),
                        ),
                    );
                }
            }
        }
    };

    let prompt = req.prompt.or(req.message).or_else(|| {
        req.input
            .and_then(|value| value.as_str().map(str::to_string))
    });
    let Some(prompt) = prompt.filter(|value| !value.trim().is_empty()) else {
        return (
            StatusCode::OK,
            Json(
                json!({"ok": true, "session_id": session_id, "message": "session ready; no prompt provided"}),
            ),
        );
    };

    match client
        .post(format!("{gateway_url}/session/{session_id}/prompt_async"))
        .json(&json!({ "prompt": prompt }))
        .send()
        .await
    {
        Ok(response) => {
            let status = response.status();
            let value = response.json::<Value>().await.unwrap_or_else(
                |error| json!({"ok": false, "error": format!("failed to parse gateway run response: {error}")}),
            );
            let status = StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
            (
                status,
                Json(
                    json!({"ok": status.is_success(), "session_id": session_id, "gateway": value}),
                ),
            )
        }
        Err(error) => (
            StatusCode::BAD_GATEWAY,
            Json(
                json!({"ok": false, "session_id": session_id, "error": format!("gateway prompt submit failed: {error}")}),
            ),
        ),
    }
}

#[axum::debug_handler]
#[allow(clippy::type_complexity)]
async fn run_service(
    State(state): State<AppState>,
    Json(req): Json<RunServiceRequest>,
) -> (StatusCode, Json<Value>) {
    let service_dir = PathBuf::from(req.services_dir);

    match state.manager.ensure_service_ready(&service_dir).await {
        Ok(worker) => {
            let call_ctx = CallContext::new(
                Method::POST.as_str().to_string(),
                format!("/services/{}/{}", worker.service_name, worker.worker_id),
                req.input,
            );

            match state.manager.call_worker(&worker.worker_id, call_ctx).await {
                Ok(result) => (
                    StatusCode::OK,
                    Json(json!({
                        "ok": true,
                        "worker_id": worker.worker_id,
                        "service_name": worker.service_name,
                        "url": worker.url,
                        "invocation": result,
                    })),
                ),
                Err(err) => {
                    warn!(error = %err, "service invocation failed");
                    (
                        StatusCode::BAD_GATEWAY,
                        Json(json!({
                            "ok": false,
                            "worker_id": worker.worker_id,
                            "service_name": worker.service_name,
                            "url": worker.url,
                            "error": err.to_string(),
                        })),
                    )
                }
            }
        }
        Err(err) => {
            error!(error = %err, "failed to start or connect service");
            (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "ok": false,
                    "error": err.to_string(),
                })),
            )
        }
    }
}

#[axum::debug_handler]
#[allow(clippy::type_complexity)]
async fn call_service(
    State(state): State<AppState>,
    Path((service_name, worker_id)): Path<(String, String)>,
    method: Method,
    Json(payload): Json<Value>,
) -> (StatusCode, Json<Value>) {
    let ctx = CallContext::new(
        method.as_str().to_string(),
        format!("/services/{service_name}/{worker_id}"),
        payload,
    );

    match state.manager.call_worker(&worker_id, ctx).await {
        Ok(result) => (StatusCode::OK, Json(result)),
        Err(err) => {
            warn!(worker_id, error = %err, "service route call failed");
            (
                StatusCode::BAD_GATEWAY,
                Json(json!({
                    "ok": false,
                    "service_name": service_name,
                    "worker_id": worker_id,
                    "error": err.to_string(),
                })),
            )
        }
    }
}

async fn lsp_check_symbols(
    State(state): State<AppState>,
    Path(worker_id): Path<String>,
    Json(payload): Json<Value>,
) -> (StatusCode, Json<Value>) {
    let ctx = CallContext::new(
        "POST".to_string(),
        format!("/services/lsp/{}", worker_id),
        json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "check/symbols",
            "params": payload
        }),
    );

    match state.manager.call_worker(&worker_id, ctx).await {
        Ok(result) => (StatusCode::OK, Json(result)),
        Err(err) => {
            warn!(worker_id, error = %err, "lsp check symbols failed");
            (
                StatusCode::BAD_GATEWAY,
                Json(json!({
                    "ok": false,
                    "error": err.to_string(),
                })),
            )
        }
    }
}

async fn lsp_check_definition(
    State(state): State<AppState>,
    Path(worker_id): Path<String>,
    Json(payload): Json<Value>,
) -> (StatusCode, Json<Value>) {
    let ctx = CallContext::new(
        "POST".to_string(),
        format!("/services/lsp/{}", worker_id),
        json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "check/definition",
            "params": payload
        }),
    );

    match state.manager.call_worker(&worker_id, ctx).await {
        Ok(result) => (StatusCode::OK, Json(result)),
        Err(err) => {
            warn!(worker_id, error = %err, "lsp check definition failed");
            (
                StatusCode::BAD_GATEWAY,
                Json(json!({
                    "ok": false,
                    "error": err.to_string(),
                })),
            )
        }
    }
}

async fn lsp_check_references(
    State(state): State<AppState>,
    Path(worker_id): Path<String>,
    Json(payload): Json<Value>,
) -> (StatusCode, Json<Value>) {
    let ctx = CallContext::new(
        "POST".to_string(),
        format!("/services/lsp/{}", worker_id),
        json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "check/references",
            "params": payload
        }),
    );

    match state.manager.call_worker(&worker_id, ctx).await {
        Ok(result) => (StatusCode::OK, Json(result)),
        Err(err) => {
            warn!(worker_id, error = %err, "lsp check references failed");
            (
                StatusCode::BAD_GATEWAY,
                Json(json!({
                    "ok": false,
                    "error": err.to_string(),
                })),
            )
        }
    }
}

async fn lsp_check_diagnostics(
    State(state): State<AppState>,
    Path(worker_id): Path<String>,
    Json(payload): Json<Value>,
) -> (StatusCode, Json<Value>) {
    let ctx = CallContext::new(
        "POST".to_string(),
        format!("/services/lsp/{}", worker_id),
        json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "check/diagnostics",
            "params": payload
        }),
    );

    match state.manager.call_worker(&worker_id, ctx).await {
        Ok(result) => (StatusCode::OK, Json(result)),
        Err(err) => {
            warn!(worker_id, error = %err, "lsp check diagnostics failed");
            (
                StatusCode::BAD_GATEWAY,
                Json(json!({
                    "ok": false,
                    "error": err.to_string(),
                })),
            )
        }
    }
}
