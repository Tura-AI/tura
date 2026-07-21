//! Typed client for the persistent gateway-owned router process.
//!
//! This client is for execution supervision only. Session DB data reads/writes
//! must use `SessionDbClient`, never router calls.

use anyhow::{anyhow, Result};
use router_contract::{
    CancelRuntimeRequest, EnqueueTurnRequest, ExecuteCommandRequest, ExecuteCommandResponse,
    GetToolConfigResponse, GetToolResponse, ListCommandsRequest, ListCommandsResponse,
    ListToolsResponse, PatchToolConfigRequest, PatchToolRequest, ProbeSessionsRequest,
    ToolRegistryRequest, ToolRequest, METHOD_ENQUEUE_TURN, METHOD_EXECUTE_COMMAND, METHOD_GET_TOOL,
    METHOD_GET_TOOL_CONFIG, METHOD_LIST_COMMANDS, METHOD_LIST_TOOLS, METHOD_PATCH_TOOL,
    METHOD_PATCH_TOOL_CONFIG,
};
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::{json, Value};
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct RouterClient;

impl RouterClient {
    pub fn global() -> Self {
        Self
    }

    pub fn health_check(&self) -> Result<Value> {
        crate::router_process::global_router_process()?.call("health_check", json!({}))
    }

    pub fn enqueue_turn(&self, request: EnqueueTurnRequest) -> Result<Value> {
        let payload = enqueue_turn_payload(request)?;
        crate::router_process::global_router_process()?
            .call(METHOD_ENQUEUE_TURN, payload)
            .map_err(|error| anyhow!("router execution enqueue failed: {error}"))
    }

    pub fn cancel_runtime(&self, session_id: &str, runtime_id: &str) -> Result<Value> {
        crate::router_process::global_router_process()?.call(
            "execution.cancel_turn",
            serde_json::to_value(CancelRuntimeRequest {
                session_id: session_id.to_string(),
                runtime_id: runtime_id.to_string(),
            })?,
        )
    }

    pub fn kill_session_workers(&self, session_id: &str) -> Result<Value> {
        crate::router_process::global_router_process()?.call(
            "execution.kill_session_workers",
            kill_session_workers_payload(session_id),
        )
    }

    pub fn probe_sessions(&self, session_ids: &[String]) -> Result<Value> {
        crate::router_process::global_router_process()?.call_existing_with_timeout(
            "execution.probe_sessions",
            serde_json::to_value(ProbeSessionsRequest {
                session_ids: session_ids.to_vec(),
            })?,
            Duration::from_secs(5),
        )
    }

    pub fn list_commands(&self, request: ListCommandsRequest) -> Result<ListCommandsResponse> {
        self.call_typed(METHOD_LIST_COMMANDS, request)
    }

    pub fn execute_command(
        &self,
        request: ExecuteCommandRequest,
    ) -> Result<ExecuteCommandResponse> {
        self.call_typed(METHOD_EXECUTE_COMMAND, request)
    }

    pub fn list_tools(&self, request: ToolRegistryRequest) -> Result<ListToolsResponse> {
        self.call_typed(METHOD_LIST_TOOLS, request)
    }

    pub fn get_tool(&self, request: ToolRequest) -> Result<GetToolResponse> {
        self.call_typed(METHOD_GET_TOOL, request)
    }

    pub fn patch_tool(&self, request: PatchToolRequest) -> Result<GetToolResponse> {
        self.call_typed(METHOD_PATCH_TOOL, request)
    }

    pub fn get_tool_config(&self, request: ToolRequest) -> Result<GetToolConfigResponse> {
        self.call_typed(METHOD_GET_TOOL_CONFIG, request)
    }

    pub fn patch_tool_config(
        &self,
        request: PatchToolConfigRequest,
    ) -> Result<GetToolConfigResponse> {
        self.call_typed(METHOD_PATCH_TOOL_CONFIG, request)
    }

    pub fn shutdown(&self) -> Result<Value> {
        crate::router_process::global_router_process()?.shutdown()
    }

    fn call_typed<Request, Response>(&self, method: &str, request: Request) -> Result<Response>
    where
        Request: Serialize,
        Response: DeserializeOwned,
    {
        let payload = serde_json::to_value(request)?;
        let response = crate::router_process::global_router_process()?.call(method, payload)?;
        serde_json::from_value(response)
            .map_err(|error| anyhow!("invalid typed response for router method {method}: {error}"))
    }
}

fn enqueue_turn_payload(request: EnqueueTurnRequest) -> Result<Value> {
    serde_json::to_value(request).map_err(Into::into)
}

fn kill_session_workers_payload(session_id: &str) -> Value {
    json!({ "session_id": session_id })
}

#[cfg(test)]
mod tests {
    use super::{enqueue_turn_payload, kill_session_workers_payload};
    use router_contract::EnqueueTurnRequest;
    use serde_json::json;

    #[test]
    fn enqueue_turn_payload_preserves_runtime_session_and_nested_payload() {
        let payload = enqueue_turn_payload(EnqueueTurnRequest {
            runtime_id: "runtime-1".to_string(),
            session_id: "session-1".to_string(),
            payload: json!({
                "prompt": "hello",
                "worker_env": { "TURA_REASONING_EFFORT": "low" }
            }),
        })
        .expect("enqueue request should serialize");

        assert_eq!(payload["runtime_id"], "runtime-1");
        assert_eq!(payload["session_id"], "session-1");
        assert_eq!(payload["payload"]["prompt"], "hello");
        assert_eq!(
            payload["payload"]["worker_env"]["TURA_REASONING_EFFORT"],
            "low"
        );
    }

    #[test]
    fn kill_session_workers_payload_targets_session_runtime() {
        assert_eq!(
            kill_session_workers_payload("session-1"),
            json!({ "session_id": "session-1" })
        );
    }
}
