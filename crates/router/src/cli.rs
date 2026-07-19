use serde::{Deserialize, Serialize};

use crate::app::build_state;
use crate::daemon::{serve_socket, serve_stdio};
use crate::runtime_dispatch::dispatch_run_agent;
use crate::runtime_utils::tokio_runtime;
use router_contract::RunAgentRequest;
use tura_router::registry::agent::UpsertAgentRequest;
use tura_router::registry::command::ExecuteCommandRequest;
use tura_router::registry::persona::UpsertPersonaRequest;
use tura_router::registry::Registry;

pub(crate) fn run_router_command(command: &str) -> anyhow::Result<()> {
    match command {
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

/// CLI subcommand `run-agent`: reads a `RunAgentRequest` JSON from stdin,
/// dispatches a runtime worker, and writes the result JSON to stdout.
async fn run_agent_cli() -> anyhow::Result<()> {
    let raw = read_stdin()?;
    let req: RunAgentRequest = serde_json::from_str(raw.trim())
        .map_err(|error| anyhow::anyhow!("invalid run-agent request json: {error}"))?;
    let state = build_state();
    let (_status, body) =
        dispatch_run_agent(&state, req, "router-cli-run-agent".to_string(), None).await;
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
