use serde_json::{json, Value};
#[cfg(test)]
use std::sync::atomic::Ordering;

use crate::app::AppState;
use crate::process_info::current_process_start_time;
use crate::services;
use crate::shutdown::mark_router_shutting_down;
use crate::IpcNotificationSender;
use router_contract::{IpcRequest, IpcResponse, METHOD_ENQUEUE_TURN, METHOD_HEALTH_CHECK};

pub(crate) async fn handle_ipc_request(state: &AppState, request: IpcRequest) -> IpcResponse {
    handle_ipc_request_with_notifications(state, request, None).await
}

pub(crate) async fn handle_ipc_request_with_notifications(
    state: &AppState,
    request: IpcRequest,
    notifications: Option<IpcNotificationSender>,
) -> IpcResponse {
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
                .enqueue_turn_with_notifications(
                    state,
                    request.payload,
                    &request.request_id,
                    notifications,
                )
                .await
        }
        "execution.command_run" => state.command_run.execute(request.payload).await,
        "execution.cancel_turn" => Ok(state.execution.cancel_turn(state, request.payload).await),
        "execution.probe_sessions" => state.execution.probe_sessions(state, request.payload).await,
        "execution.get_status" => Ok(json!({ "status": "ok" })),
        "session.append_user_command" => Ok(state.user_commands.append(&request.payload)),
        "session.take_user_commands" => Ok(state.user_commands.take(&request.payload)),
        "session.clear_user_commands" => Ok(state.user_commands.clear(&request.payload)),
        "execution.kill_session_workers" => Ok(state
            .execution
            .kill_session_workers(state, request.payload)
            .await),
        "execution.shutdown" => {
            let stopped = state
                .manager
                .stop_workers_with_prefix("runtime_worker:")
                .await;
            state.session_db.stop();
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

pub(crate) fn enqueue_turn_session_id(request: &IpcRequest) -> Option<String> {
    if request.method != METHOD_ENQUEUE_TURN {
        return None;
    }
    request
        .payload
        .get("session_id")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string)
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
                "turn_id": "turn-1",
                "payload": {}
            }),
            deadline_ms: None,
        };
        assert_eq!(enqueue_turn_session_id(&turn).as_deref(), Some("session-1"));

        let command_run = IpcRequest {
            method: "execution.command_run".to_string(),
            payload: json!({ "session_id": "session-1" }),
            ..turn
        };
        assert_eq!(enqueue_turn_session_id(&command_run), None);

        let blank_session = IpcRequest {
            method: "execution.enqueue_turn".to_string(),
            payload: json!({ "session_id": "   " }),
            ..command_run
        };
        assert_eq!(enqueue_turn_session_id(&blank_session), None);
    }

    #[test]
    fn kill_session_workers_clears_router_active_turn_state() -> anyhow::Result<()> {
        let state = build_state();
        state
            .execution
            .set_session_state_for_test("kill-session", "running");

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
}
