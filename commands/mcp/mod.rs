#![deny(clippy::unwrap_used)]
#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

pub const PROMPT: &str = include_str!("prompt.md");
pub const POLICY: &str = include_str!("policy.toml");
pub const SCHEMA: &str = include_str!("schema.json");

#[derive(Debug, Deserialize)]
struct Envelope {
    kind: String,
    #[serde(default)]
    payload: Value,
}

#[derive(Debug, Serialize)]
struct ProtocolResponse {
    ok: bool,
    output: Value,
    success: bool,
    stderr: String,
    exit_code: i32,
}

#[derive(Debug, Deserialize)]
struct McpConfig {
    #[serde(default)]
    servers: BTreeMap<String, ServerConfig>,
}

#[derive(Clone, Debug, Deserialize)]
struct ServerConfig {
    command: String,
    #[serde(default)]
    args: Vec<String>,
    cwd: Option<String>,
    #[serde(default)]
    env: BTreeMap<String, String>,
    #[serde(default = "default_true")]
    enabled: bool,
    #[serde(default)]
    allowed_tools: Vec<String>,
    #[serde(default = "default_timeout_ms")]
    timeout_ms: u64,
}

#[derive(Debug, Deserialize)]
struct McpInput {
    action: String,
    server: String,
    tool: Option<String>,
    #[serde(default)]
    arguments: Value,
}

fn default_true() -> bool {
    true
}

fn default_timeout_ms() -> u64 {
    30_000
}

pub fn main() {
    let mut input = String::new();
    let response = match std::io::stdin().read_to_string(&mut input) {
        Ok(_) => serde_json::from_str::<Envelope>(&input)
            .map_err(|error| format!("invalid request JSON: {error}"))
            .and_then(handle_envelope),
        Err(error) => Err(format!("failed to read request: {error}")),
    };
    let response = response.unwrap_or_else(protocol_error);
    if let Ok(text) = serde_json::to_string(&response) {
        println!("{text}");
    }
}

fn handle_envelope(envelope: Envelope) -> Result<ProtocolResponse, String> {
    match envelope.kind.as_str() {
        "health_check" => Ok(success(json!({ "status": "ok" }))),
        "capabilities" => Ok(success(json!({
            "id": "mcp",
            "supports_macro_command": true,
            "mutating": true
        }))),
        "access" => {
            let input = parse_input(&envelope.payload)?;
            Ok(success(json!({
                "workspace_write": input.action == "call_tool",
                "read_paths": [],
                "write_paths": []
            })))
        }
        "execute" => {
            let session_dir = payload_session_dir(&envelope.payload);
            let input = parse_input(&envelope.payload)?;
            execute(input, &session_dir).map(success)
        }
        other => Err(format!("unsupported protocol request: {other}")),
    }
}

fn parse_input(payload: &Value) -> Result<McpInput, String> {
    let arguments = payload.get("arguments").cloned().unwrap_or(Value::Null);
    let value = match arguments {
        Value::String(text) => serde_json::from_str(&text)
            .map_err(|error| format!("invalid mcp command_line JSON: {error}"))?,
        value => value,
    };
    serde_json::from_value(value).map_err(|error| format!("invalid mcp arguments: {error}"))
}

fn payload_session_dir(payload: &Value) -> PathBuf {
    payload
        .get("session_dir")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

fn config_path(session_dir: &Path) -> PathBuf {
    std::env::var_os("TURA_HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| session_dir.join(".tura"))
        .join("mcp.json")
}

fn execute(input: McpInput, session_dir: &Path) -> Result<Value, String> {
    let path = config_path(session_dir);
    let text = std::fs::read_to_string(&path)
        .map_err(|error| format!("failed to read MCP config {}: {error}", path.display()))?;
    let config: McpConfig = serde_json::from_str(&text)
        .map_err(|error| format!("invalid MCP config {}: {error}", path.display()))?;
    let server = config
        .servers
        .get(&input.server)
        .filter(|server| server.enabled)
        .ok_or_else(|| format!("MCP server `{}` is not configured or enabled", input.server))?;
    if server.command.trim().is_empty() {
        return Err(format!(
            "MCP server `{}` has an empty command",
            input.server
        ));
    }
    if input.action == "call_tool" {
        let tool = input
            .tool
            .as_deref()
            .filter(|tool| !tool.trim().is_empty())
            .ok_or_else(|| "call_tool requires `tool`".to_string())?;
        if !server
            .allowed_tools
            .iter()
            .any(|allowed| allowed == "*" || allowed == tool)
        {
            return Err(format!(
                "MCP tool `{tool}` is not allowed for server `{}`",
                input.server
            ));
        }
    }
    run_stdio(server, input, session_dir)
}

fn run_stdio(server: &ServerConfig, input: McpInput, session_dir: &Path) -> Result<Value, String> {
    let cwd = server
        .cwd
        .as_deref()
        .map(PathBuf::from)
        .map(|path| {
            if path.is_absolute() {
                path
            } else {
                session_dir.join(path)
            }
        })
        .unwrap_or_else(|| session_dir.to_path_buf());
    let mut command = Command::new(&server.command);
    command
        .args(&server.args)
        .current_dir(cwd)
        .envs(&server.env)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    let mut child = command
        .spawn()
        .map_err(|error| format!("failed to start MCP server `{}`: {error}", server.command))?;
    let result = communicate(&mut child, server.timeout_ms.clamp(1_000, 300_000), input);
    let _ = child.kill();
    let _ = child.wait();
    result
}

fn communicate(child: &mut Child, timeout_ms: u64, input: McpInput) -> Result<Value, String> {
    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| "MCP stdin unavailable".to_string())?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "MCP stdout unavailable".to_string())?;
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            match line {
                Ok(line) if !line.trim().is_empty() => {
                    if let Ok(value) = serde_json::from_str::<Value>(&line) {
                        let _ = sender.send(value);
                    }
                }
                Ok(_) => {}
                Err(_) => break,
            }
        }
    });
    let timeout = Duration::from_millis(timeout_ms);
    let deadline = Instant::now() + timeout;
    send(
        &mut stdin,
        json!({
            "jsonrpc": "2.0", "id": 1, "method": "initialize",
            "params": {
                "protocolVersion": "2025-03-26",
                "capabilities": {},
                "clientInfo": { "name": "tura", "version": env!("CARGO_PKG_VERSION") }
            }
        }),
    )?;
    wait_for(&receiver, 1, deadline)?;
    send(
        &mut stdin,
        json!({
            "jsonrpc": "2.0", "method": "notifications/initialized", "params": {}
        }),
    )?;
    let request = match input.action.as_str() {
        "list_tools" => json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list", "params": {} }),
        "call_tool" => json!({
            "jsonrpc": "2.0", "id": 2, "method": "tools/call",
            "params": { "name": input.tool.unwrap_or_default(), "arguments": input.arguments }
        }),
        other => return Err(format!("unsupported MCP action `{other}`")),
    };
    send(&mut stdin, request)?;
    wait_for(&receiver, 2, deadline)
}

fn send(stdin: &mut impl Write, value: Value) -> Result<(), String> {
    serde_json::to_writer(&mut *stdin, &value).map_err(|error| error.to_string())?;
    stdin.write_all(b"\n").map_err(|error| error.to_string())?;
    stdin.flush().map_err(|error| error.to_string())
}

fn wait_for(receiver: &Receiver<Value>, id: u64, deadline: Instant) -> Result<Value, String> {
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(format!("MCP request {id} timed out"));
        }
        let value = receiver
            .recv_timeout(remaining)
            .map_err(|_| format!("MCP request {id} timed out or server closed stdout"))?;
        if value.get("id").and_then(Value::as_u64) != Some(id) {
            continue;
        }
        if let Some(error) = value.get("error") {
            return Err(format!("MCP request failed: {error}"));
        }
        return Ok(value.get("result").cloned().unwrap_or(Value::Null));
    }
}

fn success(output: Value) -> ProtocolResponse {
    ProtocolResponse {
        ok: true,
        output,
        success: true,
        stderr: String::new(),
        exit_code: 0,
    }
}

fn protocol_error(error: String) -> ProtocolResponse {
    ProtocolResponse {
        ok: true,
        output: json!({ "error": error }),
        success: false,
        stderr: error,
        exit_code: 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_structured_mcp_arguments() {
        let input = parse_input(&json!({
            "arguments": "{\"action\":\"list_tools\",\"server\":\"graph\"}"
        }))
        .expect("parse input");
        assert_eq!(input.action, "list_tools");
        assert_eq!(input.server, "graph");
    }

    #[test]
    fn config_uses_explicit_tura_home() {
        let old = std::env::var_os("TURA_HOME");
        std::env::set_var("TURA_HOME", "/tmp/tura-mcp-home");
        assert_eq!(
            config_path(Path::new("/workspace")),
            PathBuf::from("/tmp/tura-mcp-home/mcp.json")
        );
        match old {
            Some(value) => std::env::set_var("TURA_HOME", value),
            None => std::env::remove_var("TURA_HOME"),
        }
    }

    #[cfg(unix)]
    #[test]
    fn stdio_server_handshake_and_tool_discovery_work() {
        let root = std::env::temp_dir().join(format!("tura-mcp-test-{}", std::process::id()));
        std::fs::create_dir_all(&root).expect("create temp home");
        let script = "while IFS= read -r line; do case \"$line\" in *\\\"method\\\":\\\"initialize\\\"*) printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{\"protocolVersion\":\"2025-03-26\",\"capabilities\":{\"tools\":{}},\"serverInfo\":{\"name\":\"fixture\",\"version\":\"1\"}}}' ;; *\\\"method\\\":\\\"tools/list\\\"*) printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"id\":2,\"result\":{\"tools\":[{\"name\":\"inspect\",\"inputSchema\":{\"type\":\"object\"}}]}}' ;; esac; done";
        let config = json!({
            "servers": {
                "fixture": {
                    "command": "/bin/sh",
                    "args": ["-c", script],
                    "allowed_tools": ["inspect"],
                    "timeout_ms": 2000
                }
            }
        });
        std::fs::write(
            root.join("mcp.json"),
            serde_json::to_vec(&config).expect("encode config"),
        )
        .expect("write config");
        let old = std::env::var_os("TURA_HOME");
        std::env::set_var("TURA_HOME", &root);
        let result = execute(
            McpInput {
                action: "list_tools".to_string(),
                server: "fixture".to_string(),
                tool: None,
                arguments: Value::Null,
            },
            &root,
        )
        .expect("list tools");
        assert_eq!(result["tools"][0]["name"], "inspect");
        match old {
            Some(value) => std::env::set_var("TURA_HOME", value),
            None => std::env::remove_var("TURA_HOME"),
        }
        let _ = std::fs::remove_dir_all(root);
    }
}
