use serde::Deserialize;
use serde_json::{json, Value};

use crate::app::AppState;
use crate::process_info::current_process_start_time;
use crate::services::managed_process::repo_root;
use crate::services::manager::ServiceManager;
use crate::services::models::{CallContext, WorkerSpec};
use tura_router::registry::resolve_binary_target;

/// Maximum recursion depth for child sub-sessions (fork-bomb guard, T5.4).
const MAX_PLANNING_DEPTH: usize = 3;
/// Concurrent runtime-worker cap (fork-bomb guard, T5.4).
const MAX_RUNTIME_WORKERS: usize = 24;

#[derive(Debug, Deserialize)]
pub(crate) struct RunAgentRequest {
    #[serde(default)]
    pub(crate) session_id: Option<String>,
    #[serde(default)]
    pub(crate) directory: Option<String>,
    #[serde(default)]
    pub(crate) model: Option<String>,
    #[serde(default)]
    pub(crate) agent: Option<String>,
    #[serde(default)]
    pub(crate) session_type: Option<String>,
    #[serde(default)]
    pub(crate) prompt: Option<String>,
    #[serde(default)]
    pub(crate) message: Option<String>,
    #[serde(default)]
    pub(crate) input: Option<Value>,
    #[serde(default)]
    pub(crate) parent_session_id: Option<String>,
    #[serde(default)]
    pub(crate) depth: Option<usize>,
    #[serde(default)]
    pub(crate) runtime_context: Option<String>,
    #[serde(default)]
    pub(crate) planning_mode_override: Option<bool>,
    /// Worker env contract computed by the gateway (model / planning /
    /// stall-guard / ...). The router injects it into the subprocess as-is.
    #[serde(default)]
    pub(crate) worker_env: std::collections::HashMap<String, String>,
}

/// Pure-logic core of run_agent: resolve agent spec, spawn the runtime-
/// worker subprocess (CLI/NDJSON), forward the call, and stream the result
/// back. Gateway and child runtime dispatch use the `run-agent` CLI
/// subcommand, never router HTTP.
pub(crate) async fn dispatch_run_agent(state: &AppState, req: RunAgentRequest) -> (u16, Value) {
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

    let router_pid = std::process::id();
    let mut env = vec![
        ("TURA_ROLE".to_string(), "runtime_worker".to_string()),
        ("TURA_RUNTIME_WORKER".to_string(), "1".to_string()),
        ("TURA_WORKER_MODE".to_string(), "one-shot".to_string()),
        (
            "TURA_WORKER_ONESHOT_PROTOCOL".to_string(),
            "envelope".to_string(),
        ),
        ("TURA_RUNTIME_ONESHOT".to_string(), "1".to_string()),
        ("TURA_ROUTER_PARENT_PID".to_string(), router_pid.to_string()),
    ];
    if let Some(start_time) = current_process_start_time(router_pid) {
        env.push((
            "TURA_ROUTER_PARENT_START_TIME".to_string(),
            start_time.to_string(),
        ));
    }
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
    let worker_cleanup = RuntimeWorkerCleanupGuard::new(state.manager.clone(), worker_id.clone());
    if router_debug_enabled() {
        eprintln!("router debug: dispatch_run_agent calling worker session_id={session_id} worker_id={worker_id}");
    }
    let call_result = state.manager.call_worker(&worker_id, ctx).await;
    worker_cleanup.stop_now().await;
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

struct RuntimeWorkerCleanupGuard {
    manager: ServiceManager,
    worker_id: String,
    active: std::sync::atomic::AtomicBool,
}

impl RuntimeWorkerCleanupGuard {
    fn new(manager: ServiceManager, worker_id: String) -> Self {
        Self {
            manager,
            worker_id,
            active: std::sync::atomic::AtomicBool::new(true),
        }
    }

    async fn stop_now(&self) {
        self.active
            .store(false, std::sync::atomic::Ordering::SeqCst);
        self.manager.stop_worker(&self.worker_id).await;
    }
}

impl Drop for RuntimeWorkerCleanupGuard {
    fn drop(&mut self) {
        if !self.active.load(std::sync::atomic::Ordering::SeqCst) {
            return;
        }
        let manager = self.manager.clone();
        let worker_id = self.worker_id.clone();
        tokio::spawn(async move {
            manager.stop_worker(&worker_id).await;
        });
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
    use crate::{build_state, runtime_utils::tokio_runtime};

    #[test]
    fn dispatch_run_agent_rejects_requests_over_runtime_worker_limit() -> anyhow::Result<()> {
        let state = build_state();
        let runtime = tokio_runtime()?;

        runtime.block_on(async {
            for index in 0..MAX_RUNTIME_WORKERS {
                state
                    .manager
                    .ensure_worker(WorkerSpec {
                        key: format!("runtime_worker:limit-fill-{index}"),
                        service_name: "runtime".to_string(),
                        executable: std::path::PathBuf::from(
                            "definitely-missing-runtime-worker-for-limit-test",
                        ),
                        args: vec!["--serve".to_string()],
                        env: vec![("TURA_DEBUG_RUNTIME".to_string(), "0".to_string())],
                    })
                    .await?;
            }

            assert_eq!(
                state.manager.count_workers_with_prefix("runtime_worker:"),
                MAX_RUNTIME_WORKERS
            );

            let request = serde_json::from_value(json!({
                "session_id": "over-limit-session",
                "prompt": "this request should be rejected before another runtime worker starts"
            }))?;
            let (status, body) = dispatch_run_agent(&state, request).await;

            assert_eq!(status, 429);
            assert_eq!(body["ok"], false);
            assert_eq!(body["session_id"], "over-limit-session");
            assert!(
                body["error"].as_str().is_some_and(
                    |error| error.contains("runtime worker concurrency limit reached (24/24)")
                ),
                "unexpected limit error body: {body}"
            );
            assert_eq!(
                state.manager.count_workers_with_prefix("runtime_worker:"),
                MAX_RUNTIME_WORKERS,
                "rejected dispatch must not create another worker"
            );

            let stopped = state
                .manager
                .stop_workers_with_prefix("runtime_worker:")
                .await;
            assert_eq!(stopped, MAX_RUNTIME_WORKERS);
            Ok::<_, anyhow::Error>(())
        })?;
        Ok(())
    }

    #[test]
    fn runtime_worker_cleanup_guard_drop_stops_registered_worker() -> anyhow::Result<()> {
        let state = build_state();
        let runtime = tokio_runtime()?;

        runtime.block_on(async {
            let handle = state
                .manager
                .ensure_worker(WorkerSpec {
                    key: "runtime_worker:drop-cleanup".to_string(),
                    service_name: "runtime_worker".to_string(),
                    executable: std::path::PathBuf::from(
                        "definitely-missing-runtime-worker-for-drop-cleanup",
                    ),
                    args: Vec::new(),
                    env: vec![("TURA_WORKER_MODE".to_string(), "one-shot".to_string())],
                })
                .await?;
            assert_eq!(
                state.manager.count_workers_with_prefix("runtime_worker:"),
                1
            );

            let guard =
                RuntimeWorkerCleanupGuard::new(state.manager.clone(), handle.worker_id.clone());
            drop(guard);
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;

            assert_eq!(
                state.manager.count_workers_with_prefix("runtime_worker:"),
                0,
                "dropping the dispatch cleanup guard should remove the worker"
            );
            assert!(!state.manager.stop_worker(&handle.worker_id).await);
            Ok::<_, anyhow::Error>(())
        })?;
        Ok(())
    }
}
