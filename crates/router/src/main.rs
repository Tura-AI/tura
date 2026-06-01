#![warn(clippy::unwrap_used)]

mod registry;
mod services;
use registry::agent::UpsertAgentRequest;
use registry::command::ExecuteCommandRequest;
use registry::persona::UpsertPersonaRequest;
use registry::{resolve_binary_target, Registry};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use services::managed_process::repo_root;
use services::manager::ServiceManager;
use services::models::{CallContext, WorkerSpec};

/// Maximum recursion depth for child sub-sessions (fork-bomb guard, T5.4).
const MAX_MULTIPLE_TASKS_DEPTH: usize = 3;
/// Concurrent runtime-worker cap (fork-bomb guard, T5.4).
const MAX_RUNTIME_WORKERS: usize = 16;

#[derive(Clone)]
struct AppState {
    manager: ServiceManager,
    registry: Registry,
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
    /// Worker env contract computed by the gateway (model / multiple_tasks /
    /// stall-guard / ...). The router injects it into the subprocess as-is.
    #[serde(default)]
    worker_env: std::collections::HashMap<String, String>,
}

fn build_state() -> AppState {
    AppState {
        manager: ServiceManager::new(),
        registry: Registry::from_static(),
    }
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
        .map_err(|error| anyhow::anyhow!(error))?;
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
        .map_err(|error| anyhow::anyhow!(error))?;
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
        .map_err(|error| anyhow::anyhow!(error))?;
    print_json(&persona)
}

fn registry_persona_delete_cli() -> anyhow::Result<()> {
    let persona_id = std::env::args()
        .nth(2)
        .ok_or_else(|| anyhow::anyhow!("persona id is required"))?;
    let registry = Registry::from_static();
    let deleted = registry
        .personas
        .delete(&persona_id)
        .map_err(|error| anyhow::anyhow!(error))?;
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

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let command = std::env::args()
        .nth(1)
        .ok_or_else(|| anyhow::anyhow!("router requires a CLI command"))?;
    match command.as_str() {
        "run-agent" => run_agent_cli().await,
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

/// Pure-logic core of run_agent: resolve agent spec, spawn the runtime-
/// worker subprocess (CLI/NDJSON), forward the call, and stream the result
/// back. Gateway and child runtime dispatch use the `run-agent` CLI
/// subcommand, never router HTTP.
async fn dispatch_run_agent(state: &AppState, req: RunAgentRequest) -> (u16, Value) {
    let session_id = req
        .session_id
        .clone()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| format!("router-{}", uuid::Uuid::new_v4()));

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
    if child_depth > MAX_MULTIPLE_TASKS_DEPTH {
        return (
            429,
            json!({
                "ok": false,
                "session_id": session_id,
                "error": format!(
                    "multiple_tasks depth {child_depth} exceeds limit {MAX_MULTIPLE_TASKS_DEPTH}"
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

    let worker_binary = match resolve_binary_target(&repo_root(), "gateway") {
        Some(path) => path,
        None => {
            return (
                500,
                json!({
                    "ok": false,
                    "session_id": session_id,
                    "error": "runtime worker binary (gateway) not found in target/{release,debug}"
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
            "TURA_MULTIPLE_TASKS_DEPTH".to_string(),
            req.depth.unwrap_or(1).to_string(),
        ));
    }
    // Pass through the gateway-supplied env contract (multiple_tasks,
    // reasoning, stall-guard, ...) verbatim.
    for (key, value) in &req.worker_env {
        env.push((key.clone(), value.clone()));
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

    let call_input = json!({
        "session_id": session_id,
        "directory": req.directory,
        "model": req.model,
        "agent": agent_spec.agent_name,
        "agent_spec": agent_spec,
        "prompt": prompt,
        "runtime_context": req.runtime_context,
    });
    let ctx = CallContext::new(
        "POST".to_string(),
        format!("/runtime_worker/{session_id}"),
        call_input,
    );

    match state.manager.call_worker(&worker.worker_id, ctx).await {
        Ok(result) => {
            let ok = result.get("ok").and_then(Value::as_bool).unwrap_or(true);
            let status = if ok { 200 } else { 502 };
            (
                status,
                json!({
                    "ok": ok,
                    "session_id": session_id,
                    "worker_id": worker.worker_id,
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
                "worker_id": worker.worker_id,
                "error": format!("runtime worker invocation failed: {error}")
            }),
        ),
    }
}
