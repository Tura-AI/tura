use crate::api::{registry, types::Command};
use crate::mock::global_store;
use axum::Json;

pub async fn list_commands() -> Json<Vec<Command>> {
    let payload = serde_json::json!({
        "directory": global_store().get_current_directory()
    });
    Json(
        registry::run_router_cli::<Vec<Command>>("registry-commands-list", &[], Some(payload))
            .await
            .unwrap_or_default(),
    )
}

pub async fn execute_command(
    Json(payload): Json<ExecuteCommandRequest>,
) -> Json<ExecuteCommandResponse> {
    let router_payload = serde_json::json!({
        "directory": global_store().get_current_directory(),
        "command": payload.command,
        "args": payload.args
    });
    Json(
        registry::run_router_cli::<ExecuteCommandResponse>(
            "registry-command-execute",
            &[],
            Some(router_payload),
        )
        .await
        .unwrap_or_else(|error| ExecuteCommandResponse { output: error }),
    )
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct ExecuteCommandRequest {
    pub command: String,
    pub args: Option<Vec<String>>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct ExecuteCommandResponse {
    pub output: String,
}
