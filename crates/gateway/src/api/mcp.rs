//! MCP (Model Context Protocol) API handlers

use crate::api::types::*;
use crate::mock::global_store;
use axum::{
    extract::{Path, Query},
    Json,
};
use std::path::PathBuf;

// ============================================================================
// MCP List
// ============================================================================

pub async fn list_mcp_servers() -> Json<Vec<MCPServer>> {
    Json(discover_mcp_servers())
}

// ============================================================================
// MCP Server Operations
// ============================================================================

pub async fn mcp_connect(Path(name): Path<String>) -> Json<MCPServer> {
    let discovered = discover_mcp_servers()
        .into_iter()
        .find(|server| server.name == name);
    Json(
        discovered.unwrap_or_else(|| MCPServer {
            name,
            status: MCPStatus::Failed {
                error: "MCP server is not configured; dynamic MCP lifecycle is not running yet"
                    .to_string(),
            },
            tools: vec![],
        }),
    )
}

pub async fn mcp_disconnect(Path(_name): Path<String>) -> Json<bool> {
    Json(false)
}

pub async fn mcp_call_tool(
    Path((name, tool)): Path<(String, String)>,
    Json(payload): Json<serde_json::Value>,
) -> Json<serde_json::Value> {
    let Some(config) = discover_mcp_server_config(&name) else {
        return Json(serde_json::json!({
            "ok": false,
            "error": format!("MCP server `{name}` is not configured")
        }));
    };
    let Some(endpoint) = mcp_tool_endpoint(&config) else {
        return Json(serde_json::json!({
            "ok": false,
            "error": format!("MCP server `{name}` has no HTTP tool endpoint configured")
        }));
    };
    let request = serde_json::json!({
        "server": name,
        "tool": tool,
        "arguments": payload,
    });
    match reqwest::Client::new()
        .post(endpoint)
        .json(&request)
        .send()
        .await
    {
        Ok(response) if response.status().is_success() => {
            let value = response.json().await.unwrap_or_else(|error| {
                serde_json::json!({
                    "ok": false,
                    "error": format!("MCP tool response was not JSON: {error}")
                })
            });
            Json(value)
        }
        Ok(response) => {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            Json(serde_json::json!({
                "ok": false,
                "error": format!("MCP tool endpoint returned {status}: {body}")
            }))
        }
        Err(error) => Json(serde_json::json!({
            "ok": false,
            "error": format!("failed to call MCP tool endpoint: {error}")
        })),
    }
}

pub async fn list_mcp_resources() -> Json<serde_json::Value> {
    let mut resources = serde_json::Map::new();
    for (server_name, resource) in discover_mcp_resources() {
        let key = format!("{server_name}:{}", resource.uri);
        resources.insert(
            key,
            serde_json::json!({
                "name": resource.name,
                "uri": resource.uri,
                "description": resource.description,
                "mimeType": resource.mime_type,
                "client": server_name,
            }),
        );
    }
    Json(serde_json::Value::Object(resources))
}

pub async fn mcp_read_resource(
    Path(name): Path<String>,
    Query(query): Query<McpResourceReadQuery>,
) -> Json<serde_json::Value> {
    let Some(uri) = query.uri.filter(|value| !value.trim().is_empty()) else {
        return Json(serde_json::json!({
            "ok": false,
            "error": "resource uri query parameter is required"
        }));
    };
    let Some(config) = discover_mcp_server_config(&name) else {
        return Json(serde_json::json!({
            "ok": false,
            "error": format!("MCP server `{name}` is not configured")
        }));
    };
    if let Some(endpoint) = mcp_resource_endpoint(&config) {
        return match reqwest::Client::new()
            .get(endpoint)
            .query(&[("server", name.as_str()), ("uri", uri.as_str())])
            .send()
            .await
        {
            Ok(response) if response.status().is_success() => {
                Json(response.json().await.unwrap_or_else(|error| {
                    serde_json::json!({
                        "ok": false,
                        "error": format!("MCP resource response was not JSON: {error}")
                    })
                }))
            }
            Ok(response) => {
                let status = response.status();
                let body = response.text().await.unwrap_or_default();
                Json(serde_json::json!({
                    "ok": false,
                    "error": format!("MCP resource endpoint returned {status}: {body}")
                }))
            }
            Err(error) => Json(serde_json::json!({
                "ok": false,
                "error": format!("failed to read MCP resource endpoint: {error}")
            })),
        };
    }

    let Some(resource) = resources_from_config(&config)
        .into_iter()
        .find(|resource| resource.uri == uri)
    else {
        return Json(serde_json::json!({
            "ok": false,
            "error": format!("MCP resource `{uri}` is not configured for `{name}`")
        }));
    };
    match read_local_resource_content(&resource.uri) {
        Ok(content) => Json(serde_json::json!({
            "ok": true,
            "resource": {
                "name": resource.name,
                "uri": resource.uri,
                "description": resource.description,
                "mimeType": resource.mime_type,
                "client": name,
            },
            "content": content,
        })),
        Err(error) => Json(serde_json::json!({
            "ok": false,
            "error": error,
        })),
    }
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct McpResourceReadQuery {
    pub uri: Option<String>,
}

// ============================================================================
// MCP Auth
// ============================================================================

pub async fn mcp_auth(Path(name): Path<String>) -> Json<MCPAuthResponse> {
    Json(MCPAuthResponse {
        url: format!("/mcp/{}/auth/callback", name),
    })
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct MCPAuthResponse {
    pub url: String,
}

pub async fn mcp_authenticate(
    Path(name): Path<String>,
    Json(_payload): Json<MCPAuthRequest>,
) -> Json<MCPAuthResponse> {
    Json(MCPAuthResponse {
        url: format!("/mcp/{}/auth/callback", name),
    })
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct MCPAuthRequest {
    pub api_key: String,
}

pub async fn mcp_auth_callback(
    Path(_name): Path<String>,
    Query(_params): Query<MCPAuthCallbackParams>,
) -> Json<bool> {
    Json(false)
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct MCPAuthCallbackParams {
    pub code: Option<String>,
    pub state: Option<String>,
}

fn discover_mcp_servers() -> Vec<MCPServer> {
    let mut servers = Vec::new();
    for path in mcp_config_candidates() {
        if !path.exists() {
            continue;
        }
        match std::fs::read_to_string(&path) {
            Ok(content) => {
                servers.extend(parse_mcp_json_config(&content));
                servers.extend(parse_mcp_toml_like_config(&content));
            }
            Err(error) => {
                servers.push(MCPServer {
                    name: path.display().to_string(),
                    status: MCPStatus::Failed {
                        error: format!("failed to read MCP config: {error}"),
                    },
                    tools: vec![],
                });
            }
        }
    }
    dedupe_mcp_servers(servers)
}

fn mcp_config_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Ok(path) = std::env::var("TURA_MCP_CONFIG") {
        candidates.push(PathBuf::from(path));
    }
    if let Some(directory) = global_store().get_current_directory() {
        let root = PathBuf::from(directory);
        candidates.push(root.join(".tura").join("mcp.json"));
        candidates.push(root.join(".tura").join("config.conf"));
        candidates.push(root.join(".codex").join("config.toml"));
    }
    if let Ok(current) = std::env::current_dir() {
        candidates.push(current.join(".tura").join("mcp.json"));
        candidates.push(current.join(".tura").join("config.conf"));
        candidates.push(current.join(".codex").join("config.toml"));
    }
    candidates
}

fn parse_mcp_json_config(content: &str) -> Vec<MCPServer> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(content) else {
        return Vec::new();
    };
    let servers = value
        .get("mcp")
        .or_else(|| value.get("mcpServers"))
        .or_else(|| value.get("mcp_servers"))
        .or_else(|| value.get("servers"))
        .unwrap_or(&value);

    if let Some(array) = servers.as_array() {
        return array.iter().filter_map(server_from_json_value).collect();
    }
    if let Some(object) = servers.as_object() {
        return object
            .iter()
            .map(|(name, value)| {
                let mut server = server_from_json_value_with_name(value, Some(name))
                    .unwrap_or_else(|| configured_mcp_server(name));
                if server.name.is_empty() {
                    server.name.clone_from(name);
                }
                server
            })
            .collect();
    }
    Vec::new()
}

fn server_from_json_value(value: &serde_json::Value) -> Option<MCPServer> {
    server_from_json_value_with_name(value, None)
}

fn server_from_json_value_with_name(
    value: &serde_json::Value,
    fallback_name: Option<&str>,
) -> Option<MCPServer> {
    let object = value.as_object()?;
    let name = object
        .get("name")
        .and_then(serde_json::Value::as_str)
        .or(fallback_name)
        .unwrap_or_default();
    if name.is_empty() {
        return None;
    }
    Some(MCPServer {
        name: name.to_string(),
        status: if object
            .get("enabled")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(true)
        {
            MCPStatus::Connected
        } else {
            MCPStatus::Disabled
        },
        tools: object
            .get("tools")
            .and_then(serde_json::Value::as_array)
            .map(|items| items.iter().filter_map(mcp_tool_from_json).collect())
            .unwrap_or_default(),
    })
}

fn mcp_tool_from_json(value: &serde_json::Value) -> Option<McpTool> {
    let object = value.as_object()?;
    let name = object.get("name").and_then(serde_json::Value::as_str)?;
    Some(McpTool {
        name: name.to_string(),
        description: object
            .get("description")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string),
        input_schema: object
            .get("input_schema")
            .or_else(|| object.get("inputSchema"))
            .cloned()
            .unwrap_or_else(|| serde_json::json!({ "type": "object" })),
    })
}

fn discover_mcp_server_config(name: &str) -> Option<serde_json::Value> {
    for path in mcp_config_candidates() {
        let Ok(content) = std::fs::read_to_string(path) else {
            continue;
        };
        let Ok(value) = serde_json::from_str::<serde_json::Value>(&content) else {
            continue;
        };
        if let Some(found) = find_mcp_server_value(&value, name) {
            return Some(found);
        }
    }
    None
}

fn find_mcp_server_value(value: &serde_json::Value, name: &str) -> Option<serde_json::Value> {
    let servers = value
        .get("mcp")
        .or_else(|| value.get("mcpServers"))
        .or_else(|| value.get("mcp_servers"))
        .or_else(|| value.get("servers"))
        .unwrap_or(value);
    if let Some(object) = servers.as_object() {
        if let Some(value) = object.get(name) {
            return Some(value.clone());
        }
    }
    if let Some(array) = servers.as_array() {
        return array
            .iter()
            .find(|value| value.get("name").and_then(serde_json::Value::as_str) == Some(name))
            .cloned();
    }
    None
}

fn mcp_tool_endpoint(config: &serde_json::Value) -> Option<String> {
    config
        .get("tool_endpoint")
        .or_else(|| config.get("toolEndpoint"))
        .or_else(|| config.get("endpoint"))
        .or_else(|| config.get("url"))
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn mcp_resource_endpoint(config: &serde_json::Value) -> Option<String> {
    config
        .get("resource_endpoint")
        .or_else(|| config.get("resourceEndpoint"))
        .or_else(|| config.get("resources_endpoint"))
        .or_else(|| config.get("resourcesEndpoint"))
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

#[derive(Debug, Clone)]
struct DiscoveredMcpResource {
    name: String,
    uri: String,
    description: Option<String>,
    mime_type: Option<String>,
}

fn discover_mcp_resources() -> Vec<(String, DiscoveredMcpResource)> {
    let mut resources = Vec::new();
    for path in mcp_config_candidates() {
        let Ok(content) = std::fs::read_to_string(path) else {
            continue;
        };
        let Ok(value) = serde_json::from_str::<serde_json::Value>(&content) else {
            continue;
        };
        let servers = value
            .get("mcp")
            .or_else(|| value.get("mcpServers"))
            .or_else(|| value.get("mcp_servers"))
            .or_else(|| value.get("servers"))
            .unwrap_or(&value);
        if let Some(object) = servers.as_object() {
            for (server_name, config) in object {
                for resource in resources_from_config(config) {
                    resources.push((server_name.clone(), resource));
                }
            }
        }
        if let Some(array) = servers.as_array() {
            for config in array {
                let server_name = config
                    .get("name")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("mcp")
                    .to_string();
                for resource in resources_from_config(config) {
                    resources.push((server_name.clone(), resource));
                }
            }
        }
    }
    resources
}

fn resources_from_config(config: &serde_json::Value) -> Vec<DiscoveredMcpResource> {
    config
        .get("resources")
        .and_then(serde_json::Value::as_array)
        .map(|items| items.iter().filter_map(resource_from_json).collect())
        .unwrap_or_default()
}

fn resource_from_json(value: &serde_json::Value) -> Option<DiscoveredMcpResource> {
    let object = value.as_object()?;
    let uri = object.get("uri").and_then(serde_json::Value::as_str)?;
    let name = object
        .get("name")
        .and_then(serde_json::Value::as_str)
        .unwrap_or(uri)
        .to_string();
    Some(DiscoveredMcpResource {
        name,
        uri: uri.to_string(),
        description: object
            .get("description")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string),
        mime_type: object
            .get("mimeType")
            .or_else(|| object.get("mime_type"))
            .and_then(serde_json::Value::as_str)
            .map(str::to_string),
    })
}

fn read_local_resource_content(uri: &str) -> Result<String, String> {
    let path = if let Some(path) = uri.strip_prefix("file://") {
        PathBuf::from(path)
    } else {
        PathBuf::from(uri)
    };
    if !path.exists() {
        return Err(format!(
            "local MCP resource does not exist: {}",
            path.display()
        ));
    }
    std::fs::read_to_string(&path).map_err(|error| {
        format!(
            "failed to read local MCP resource {}: {error}",
            path.display()
        )
    })
}

fn parse_mcp_toml_like_config(content: &str) -> Vec<MCPServer> {
    let mut servers = Vec::new();
    for line in content.lines() {
        let trimmed = line.trim();
        if !(trimmed.starts_with("[mcp_servers.") || trimmed.starts_with("[mcp.")) {
            continue;
        }
        let Some(name) = trimmed
            .trim_matches(['[', ']'])
            .rsplit('.')
            .next()
            .map(|value| value.trim_matches('"').trim())
            .filter(|value| !value.is_empty())
        else {
            continue;
        };
        servers.push(configured_mcp_server(name));
    }
    servers
}

fn configured_mcp_server(name: &str) -> MCPServer {
    MCPServer {
        name: name.to_string(),
        status: MCPStatus::Disabled,
        tools: vec![],
    }
}

fn dedupe_mcp_servers(servers: Vec<MCPServer>) -> Vec<MCPServer> {
    let mut seen = std::collections::BTreeSet::new();
    servers
        .into_iter()
        .filter(|server| seen.insert(server.name.clone()))
        .collect()
}
