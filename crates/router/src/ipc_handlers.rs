use serde_json::{json, Value};
#[cfg(test)]
use std::sync::atomic::Ordering;

use crate::app::AppState;
use crate::process_info::current_process_start_time;
use crate::services;
use crate::shutdown::mark_router_shutting_down;
use router_contract::{
    ExecuteCommandRequest, GetToolConfigResponse, GetToolResponse, IpcRequest, IpcResponse,
    ListCommandsRequest, ListCommandsResponse, ListToolsResponse, PatchToolConfigRequest,
    PatchToolRequest, ToolRegistryRequest, ToolRequest, METHOD_ENQUEUE_TURN,
    METHOD_EXECUTE_COMMAND, METHOD_GET_TOOL, METHOD_GET_TOOL_CONFIG, METHOD_HEALTH_CHECK,
    METHOD_LIST_COMMANDS, METHOD_LIST_TOOLS, METHOD_PATCH_TOOL, METHOD_PATCH_TOOL_CONFIG,
};
use tura_router::registry::ToolRegistry;

pub(crate) async fn handle_ipc_request(state: &AppState, request: IpcRequest) -> IpcResponse {
    let result = match request.method.as_str() {
        "" | METHOD_HEALTH_CHECK
            if request.kind == "health_check" || request.method == METHOD_HEALTH_CHECK =>
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
        METHOD_ENQUEUE_TURN => {
            state
                .execution
                .enqueue_turn_request(state, request.payload, &request.request_id)
                .await
        }
        "execution.command_run" => state.command_run.execute(request.payload).await,
        "execution.cancel_turn" => Ok(state.execution.cancel_turn(state, request.payload).await),
        "execution.probe_sessions" => state.execution.probe_sessions(state, request.payload).await,
        "execution.get_status" => Ok(json!({ "status": "ok" })),
        "session.take_user_commands" => services::user_commands::take(&request.payload),
        "execution.kill_session_workers" => Ok(state
            .execution
            .kill_session_workers(state, request.payload)
            .await),
        METHOD_LIST_COMMANDS
        | METHOD_EXECUTE_COMMAND
        | METHOD_LIST_TOOLS
        | METHOD_GET_TOOL
        | METHOD_PATCH_TOOL
        | METHOD_GET_TOOL_CONFIG
        | METHOD_PATCH_TOOL_CONFIG => {
            handle_registry_request(state, request.method.as_str(), request.payload)
        }
        "execution.shutdown" => {
            let stopped = state
                .manager
                .stop_workers_with_prefix("runtime_worker:")
                .await;
            state.session_db.shutdown();
            let background_process_scopes_terminated = mark_router_shutting_down(state);
            Ok(json!({
                "status": "shutting_down",
                "runtime_workers_stopped": stopped,
                "background_process_scopes_terminated": background_process_scopes_terminated
            }))
        }
        other => Err(anyhow::anyhow!("unknown router method: {other}")),
    };
    match result {
        Ok(payload) => IpcResponse::ok(request.request_id, payload),
        Err(error) => IpcResponse::error(request.request_id, error.to_string()),
    }
}

fn handle_registry_request(
    state: &AppState,
    method: &str,
    payload: Value,
) -> anyhow::Result<Value> {
    match method {
        METHOD_LIST_COMMANDS => {
            let request: ListCommandsRequest = decode_payload(payload)?;
            encode_payload(ListCommandsResponse {
                commands: state.registry.commands.list(request.directory.as_deref()),
            })
        }
        METHOD_EXECUTE_COMMAND => {
            let request: ExecuteCommandRequest = decode_payload(payload)?;
            encode_payload(state.registry.commands.execute(request))
        }
        METHOD_LIST_TOOLS => {
            let request: ToolRegistryRequest = decode_payload(payload)?;
            encode_payload(ListToolsResponse {
                tools: ToolRegistry::discover(request.repo_root).list(),
            })
        }
        METHOD_GET_TOOL => {
            let request: ToolRequest = decode_payload(payload)?;
            encode_payload(GetToolResponse {
                tool: ToolRegistry::discover(request.repo_root).get(&request.tool_id),
            })
        }
        METHOD_PATCH_TOOL => {
            let request: PatchToolRequest = decode_payload(payload)?;
            let tool = ToolRegistry::discover(request.repo_root)
                .patch_tool(&request.tool_id, request.patch)
                .map_err(anyhow::Error::msg)?;
            encode_payload(GetToolResponse { tool: Some(tool) })
        }
        METHOD_GET_TOOL_CONFIG => {
            let request: ToolRequest = decode_payload(payload)?;
            encode_payload(GetToolConfigResponse {
                config: ToolRegistry::discover(request.repo_root).config(&request.tool_id),
            })
        }
        METHOD_PATCH_TOOL_CONFIG => {
            let request: PatchToolConfigRequest = decode_payload(payload)?;
            let config = ToolRegistry::discover(request.repo_root)
                .patch_config(&request.tool_id, request.values)
                .map_err(anyhow::Error::msg)?;
            encode_payload(GetToolConfigResponse {
                config: Some(config),
            })
        }
        _ => unreachable!("registry method was filtered by the IPC dispatcher"),
    }
}

fn decode_payload<T: serde::de::DeserializeOwned>(payload: Value) -> anyhow::Result<T> {
    serde_json::from_value(payload)
        .map_err(|error| anyhow::anyhow!("invalid router payload: {error}"))
}

fn encode_payload(payload: impl serde::Serialize) -> anyhow::Result<Value> {
    serde_json::to_value(payload).map_err(Into::into)
}

pub(crate) fn enqueue_turn_identity(request: &IpcRequest) -> Option<(String, String)> {
    if request.method != METHOD_ENQUEUE_TURN {
        return None;
    }
    let session_id = request
        .payload
        .get("session_id")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string)?;
    let runtime_id = request
        .payload
        .get("runtime_id")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string)?;
    Some((session_id, runtime_id))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{build_state, runtime_utils::tokio_runtime};

    #[test]
    fn execution_shutdown_sets_daemon_exit_flag() -> anyhow::Result<()> {
        let state = build_state();
        let response = tokio_runtime()?.block_on(handle_ipc_request(
            &state,
            IpcRequest {
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
        assert!(response.payload["background_process_scopes_terminated"]
            .as_u64()
            .is_some());
        Ok(())
    }

    #[test]
    fn enqueue_turn_session_id_tracks_only_turn_requests() {
        let turn = IpcRequest {
            request_id: "turn".to_string(),
            kind: "call".to_string(),
            method: "execution.enqueue_turn".to_string(),
            payload: json!({
                "session_id": "session-1",
                "runtime_id": "runtime-1",
                "payload": {}
            }),
            deadline_ms: None,
        };
        assert_eq!(
            enqueue_turn_identity(&turn),
            Some(("session-1".to_string(), "runtime-1".to_string()))
        );

        let command_run = IpcRequest {
            method: "execution.command_run".to_string(),
            payload: json!({ "session_id": "session-1" }),
            ..turn
        };
        assert_eq!(enqueue_turn_identity(&command_run), None);

        let blank_session = IpcRequest {
            method: "execution.enqueue_turn".to_string(),
            payload: json!({ "session_id": "   " }),
            ..command_run
        };
        assert_eq!(enqueue_turn_identity(&blank_session), None);
    }

    #[test]
    fn kill_session_workers_clears_router_active_turn_state() -> anyhow::Result<()> {
        let state = build_state();
        state
            .execution
            .set_session_lease_for_test("kill-session", true);

        let response = tokio_runtime()?.block_on(handle_ipc_request(
            &state,
            IpcRequest {
                request_id: "kill-session-workers-test".to_string(),
                kind: "call".to_string(),
                method: "execution.kill_session_workers".to_string(),
                payload: json!({ "session_id": "kill-session" }),
                deadline_ms: None,
            },
        ));

        assert!(response.ok, "kill failed: {:?}", response.error);
        assert_eq!(response.payload["status"], "stopped");
        assert_eq!(response.payload["session_id"], "kill-session");
        assert_eq!(response.payload["active_turn_removed"], true);

        let probe = tokio_runtime()?.block_on(handle_ipc_request(
            &state,
            IpcRequest {
                request_id: "probe-after-kill".to_string(),
                kind: "call".to_string(),
                method: "execution.probe_sessions".to_string(),
                payload: json!({ "session_ids": ["kill-session"] }),
                deadline_ms: None,
            },
        ));
        assert_eq!(probe.payload["sessions"][0]["status"], "inactive");
        assert_eq!(probe.payload["sessions"][0]["active_turn"], false);
        Ok(())
    }

    #[test]
    fn registry_ipc_decodes_typed_requests_and_preserves_behavior() -> anyhow::Result<()> {
        let temp = tempfile::tempdir()?;
        let commands = temp.path().join(".tura").join("commands");
        std::fs::create_dir_all(&commands)?;
        std::fs::write(commands.join("audit.md"), "Audit {{args}}")?;
        let state = build_state();

        let list = tokio_runtime()?.block_on(handle_ipc_request(
            &state,
            IpcRequest::call(
                "commands-list",
                METHOD_LIST_COMMANDS,
                serde_json::to_value(ListCommandsRequest {
                    directory: Some(temp.path().display().to_string()),
                })?,
            ),
        ));
        assert!(list.ok, "command list failed: {:?}", list.error);
        let list: ListCommandsResponse = serde_json::from_value(list.payload)?;
        assert!(list.commands.iter().any(|command| command.name == "audit"));

        let execute = tokio_runtime()?.block_on(handle_ipc_request(
            &state,
            IpcRequest::call(
                "commands-execute",
                METHOD_EXECUTE_COMMAND,
                serde_json::to_value(ExecuteCommandRequest {
                    directory: Some(temp.path().display().to_string()),
                    command: "audit".to_string(),
                    args: Some(vec!["runtime".to_string()]),
                })?,
            ),
        ));
        assert!(execute.ok, "command execute failed: {:?}", execute.error);
        let execute: router_contract::ExecuteCommandResponse =
            serde_json::from_value(execute.payload)?;
        assert_eq!(execute.output, "Audit runtime");

        let malformed = tokio_runtime()?.block_on(handle_ipc_request(
            &state,
            IpcRequest::call(
                "commands-malformed",
                METHOD_LIST_COMMANDS,
                json!({ "directory": null, "legacy": true }),
            ),
        ));
        assert!(!malformed.ok);
        assert!(malformed
            .error
            .as_deref()
            .is_some_and(|error| error.contains("unknown field `legacy`")));
        Ok(())
    }
}
