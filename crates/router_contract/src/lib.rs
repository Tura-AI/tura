#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

pub const IPC_KIND_CALL: &str = "call";
pub const IPC_KIND_HEALTH_CHECK: &str = "health_check";
pub const METHOD_HEALTH_CHECK: &str = "health_check";
pub const METHOD_ENQUEUE_TURN: &str = "execution.enqueue_turn";
pub const METHOD_LIST_COMMANDS: &str = "registry.commands.list";
pub const METHOD_EXECUTE_COMMAND: &str = "registry.commands.execute";
pub const METHOD_LIST_TOOLS: &str = "registry.tools.list";
pub const METHOD_GET_TOOL: &str = "registry.tools.get";
pub const METHOD_PATCH_TOOL: &str = "registry.tools.patch";
pub const METHOD_GET_TOOL_CONFIG: &str = "registry.tools.config.get";
pub const METHOD_PATCH_TOOL_CONFIG: &str = "registry.tools.config.patch";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RouterEndpoint {
    pub addr: String,
    pub version: String,
    pub pid: Option<u32>,
    pub process_start_time: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IpcRequest {
    pub request_id: String,
    pub kind: String,
    #[serde(default)]
    pub method: String,
    #[serde(default)]
    pub payload: Value,
    #[serde(default)]
    pub deadline_ms: Option<u64>,
}

impl IpcRequest {
    pub fn call(request_id: impl Into<String>, method: impl Into<String>, payload: Value) -> Self {
        Self {
            request_id: request_id.into(),
            kind: IPC_KIND_CALL.to_string(),
            method: method.into(),
            payload,
            deadline_ms: None,
        }
    }

    pub fn health_check(request_id: impl Into<String>, deadline_ms: u64) -> Self {
        Self {
            request_id: request_id.into(),
            kind: IPC_KIND_HEALTH_CHECK.to_string(),
            method: METHOD_HEALTH_CHECK.to_string(),
            payload: Value::Object(Default::default()),
            deadline_ms: Some(deadline_ms),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IpcResponse {
    pub request_id: String,
    pub ok: bool,
    #[serde(default)]
    pub payload: Value,
    #[serde(default)]
    pub error: Option<String>,
}

impl IpcResponse {
    pub fn ok(request_id: impl Into<String>, payload: Value) -> Self {
        Self {
            request_id: request_id.into(),
            ok: true,
            payload,
            error: None,
        }
    }

    pub fn error(request_id: impl Into<String>, error: impl Into<String>) -> Self {
        Self {
            request_id: request_id.into(),
            ok: false,
            payload: Value::Null,
            error: Some(error.into()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct EnqueueTurnRequest {
    pub runtime_id: String,
    pub session_id: String,
    pub payload: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CancelRuntimeRequest {
    pub session_id: String,
    pub runtime_id: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ProbeSessionsRequest {
    #[serde(default)]
    pub session_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ListCommandsRequest {
    pub directory: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CommandSpec {
    pub name: String,
    pub description: String,
    pub agent: Option<String>,
    pub model: Option<String>,
    pub source: String,
    pub template: Option<String>,
    pub subtask: bool,
    pub hints: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ListCommandsResponse {
    pub commands: Vec<CommandSpec>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ExecuteCommandRequest {
    pub directory: Option<String>,
    pub command: String,
    pub args: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ExecuteCommandResponse {
    pub output: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ToolRegistryRequest {
    pub repo_root: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ToolRequest {
    pub repo_root: String,
    pub tool_id: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ConfigurableEntry {
    pub key: String,
    #[serde(default)]
    pub label: String,
    pub description: String,
    #[serde(rename = "type")]
    pub value_type: String,
    pub default: Value,
    #[serde(default, rename = "enum")]
    pub enum_values: Vec<String>,
    #[serde(default)]
    pub required: bool,
    #[serde(default = "default_config_scope")]
    pub scope: String,
}

fn default_config_scope() -> String {
    "workspace".to_string()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ToolState {
    Discovered,
    Configured,
    Enabled,
    Disabled,
    Unavailable,
    Running,
    Succeeded,
    Failed,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ToolView {
    pub id: String,
    pub name: String,
    pub description: String,
    pub core: bool,
    pub category: String,
    pub execution: String,
    pub enabled: bool,
    pub aliases: Vec<String>,
    pub supports_macro_command: bool,
    pub mutating: bool,
    pub network: bool,
    pub configurable: Vec<ConfigurableEntry>,
    pub state: ToolState,
    pub binary: Option<String>,
    pub binary_path: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ToolPatch {
    pub enabled: Option<bool>,
    pub aliases: Option<Vec<String>>,
    pub core: Option<bool>,
    pub execution: Option<String>,
    pub binary: Option<String>,
    pub mutating: Option<bool>,
    pub network: Option<bool>,
    pub policy: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PatchToolRequest {
    pub repo_root: String,
    pub tool_id: String,
    pub patch: ToolPatch,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ToolConfigResponse {
    pub id: String,
    pub configurable: Vec<ConfigurableEntry>,
    pub values: BTreeMap<String, Value>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PatchToolConfigRequest {
    pub repo_root: String,
    pub tool_id: String,
    pub values: BTreeMap<String, Value>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ListToolsResponse {
    pub tools: Vec<ToolView>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct GetToolResponse {
    pub tool: Option<ToolView>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct GetToolConfigResponse {
    pub config: Option<ToolConfigResponse>,
}
