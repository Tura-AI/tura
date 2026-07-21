use axum::extract::{Json, Path};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use router_contract::{PatchToolConfigRequest, PatchToolRequest, ToolPatch, ToolRequest};
use std::collections::BTreeMap;

use crate::router_client::RouterClient;

use super::registry::project_root;

pub async fn list_tools() -> Response {
    let request = router_contract::ToolRegistryRequest {
        repo_root: project_root().display().to_string(),
    };
    match RouterClient::global().list_tools(request) {
        Ok(response) => Json(response.tools).into_response(),
        Err(error) => router_failure(error),
    }
}

pub async fn get_tool(Path(tool_id): Path<String>) -> Response {
    match RouterClient::global().get_tool(tool_request(tool_id.clone())) {
        Ok(response) => match response.tool {
            Some(tool) => Json(tool).into_response(),
            None => (StatusCode::NOT_FOUND, format!("unknown tool: {tool_id}")).into_response(),
        },
        Err(error) => router_failure(error),
    }
}

pub async fn patch_tool(Path(tool_id): Path<String>, Json(patch): Json<ToolPatch>) -> Response {
    let request = PatchToolRequest {
        repo_root: project_root().display().to_string(),
        tool_id,
        patch,
    };
    match RouterClient::global().patch_tool(request) {
        Ok(response) => match response.tool {
            Some(tool) => Json(tool).into_response(),
            None => router_failure(anyhow::anyhow!("router returned no patched tool")),
        },
        Err(error) => (StatusCode::BAD_REQUEST, error.to_string()).into_response(),
    }
}

pub async fn get_tool_config(Path(tool_id): Path<String>) -> Response {
    match RouterClient::global().get_tool_config(tool_request(tool_id.clone())) {
        Ok(response) => match response.config {
            Some(config) => Json(config).into_response(),
            None => (StatusCode::NOT_FOUND, format!("unknown tool: {tool_id}")).into_response(),
        },
        Err(error) => router_failure(error),
    }
}

pub async fn patch_tool_config(
    Path(tool_id): Path<String>,
    Json(values): Json<BTreeMap<String, serde_json::Value>>,
) -> Response {
    let request = PatchToolConfigRequest {
        repo_root: project_root().display().to_string(),
        tool_id,
        values,
    };
    match RouterClient::global().patch_tool_config(request) {
        Ok(response) => match response.config {
            Some(config) => Json(config).into_response(),
            None => router_failure(anyhow::anyhow!("router returned no patched tool config")),
        },
        Err(error) => (StatusCode::BAD_REQUEST, error.to_string()).into_response(),
    }
}

fn tool_request(tool_id: String) -> ToolRequest {
    ToolRequest {
        repo_root: project_root().display().to_string(),
        tool_id,
    }
}

fn router_failure(error: anyhow::Error) -> Response {
    (
        StatusCode::BAD_GATEWAY,
        format!("router registry request failed: {error}"),
    )
        .into_response()
}
