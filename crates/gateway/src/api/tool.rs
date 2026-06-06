use axum::extract::{Json, Path};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use std::collections::BTreeMap;
use tura_router::registry::tools::{ToolPatch, ToolRegistry};

use super::registry::project_root;

pub async fn list_tools() -> Response {
    Json(registry().list()).into_response()
}

pub async fn get_tool(Path(tool_id): Path<String>) -> Response {
    match registry().get(&tool_id) {
        Some(tool) => Json(tool).into_response(),
        None => (StatusCode::NOT_FOUND, format!("unknown tool: {tool_id}")).into_response(),
    }
}

pub async fn patch_tool(Path(tool_id): Path<String>, Json(patch): Json<ToolPatch>) -> Response {
    match registry().patch_tool(&tool_id, patch) {
        Ok(tool) => Json(tool).into_response(),
        Err(error) => (StatusCode::BAD_REQUEST, error).into_response(),
    }
}

pub async fn get_tool_config(Path(tool_id): Path<String>) -> Response {
    match registry().config(&tool_id) {
        Some(config) => Json(config).into_response(),
        None => (StatusCode::NOT_FOUND, format!("unknown tool: {tool_id}")).into_response(),
    }
}

pub async fn patch_tool_config(
    Path(tool_id): Path<String>,
    Json(values): Json<BTreeMap<String, serde_json::Value>>,
) -> Response {
    match registry().patch_config(&tool_id, values) {
        Ok(config) => Json(config).into_response(),
        Err(error) => (StatusCode::BAD_REQUEST, error).into_response(),
    }
}

fn registry() -> ToolRegistry {
    ToolRegistry::discover(project_root())
}
