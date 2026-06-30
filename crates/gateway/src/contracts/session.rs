use runtime::state_machine::runtime_management::RuntimeSessionSyncStatus;
use serde::{Deserialize, Serialize};

use super::{GlobalEvent, Message};

fn default_context_token_limit() -> u64 {
    runtime::state_machine::session_management::DEFAULT_CONTEXT_TOKEN_LIMIT
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct SessionContextTokens {
    #[serde(default)]
    pub input: u64,
    #[serde(default = "default_context_token_limit")]
    pub limit: u64,
}

impl Default for SessionContextTokens {
    fn default() -> Self {
        Self {
            input: 0,
            limit: default_context_token_limit(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionUsage {
    #[serde(default)]
    pub context_tokens: SessionContextTokens,
    #[serde(default)]
    pub tokens: serde_json::Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cost: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub currency: Option<String>,
}

impl Default for SessionUsage {
    fn default() -> Self {
        Self {
            context_tokens: SessionContextTokens::default(),
            tokens: serde_json::Value::Null,
            cost: None,
            currency: None,
        }
    }
}

impl SessionUsage {
    pub fn new(context_tokens: SessionContextTokens, tokens: serde_json::Value) -> Self {
        let cost = tokens
            .get("total_cost")
            .and_then(serde_json::Value::as_f64)
            .filter(|value| value.is_finite());
        let currency = tokens
            .get("currency")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string);
        Self {
            context_tokens,
            tokens,
            cost,
            currency,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub name: Option<String>,
    #[serde(default)]
    pub parent_id: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_user_message_at: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_start_at: Option<i64>,
    pub directory: Option<String>,
    pub model: Option<String>,
    pub agent: Option<String>,
    pub session_type: Option<String>,
    #[serde(default)]
    pub auto_session_name: bool,
    #[serde(default)]
    pub kill_processes_on_start: bool,
    #[serde(default)]
    pub validator_enabled: bool,
    #[serde(default)]
    pub force_planning: bool,
    pub model_variant: Option<String>,
    #[serde(default)]
    pub model_acceleration_enabled: bool,
    #[serde(default)]
    pub disable_permission_restrictions: bool,
    pub status: SessionStatus,
    pub message_count: usize,
    #[serde(default)]
    pub task_management: serde_json::Value,
    #[serde(default)]
    pub context_tokens: SessionContextTokens,
    #[serde(default)]
    pub usage: SessionUsage,
    pub plan_summary: Option<String>,
    pub session_display_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SessionStatus {
    Idle,
    Busy,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendMessageRequest {
    pub content: String,
    pub attachments: Option<Vec<String>>,
    pub parent_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendMessageResponse {
    pub message: Message,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct SessionListParams {
    pub directory: Option<String>,
    pub workspace: Option<String>,
    pub roots: Option<bool>,
    #[serde(default, alias = "includeChildren")]
    pub include_children: bool,
    pub start: Option<i64>,
    pub search: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct CreateSessionRequest {
    pub directory: Option<String>,
    pub model: Option<String>,
    pub agent: Option<String>,
    pub session_type: Option<String>,
    pub kill_processes_on_start: Option<bool>,
    pub validator_enabled: Option<bool>,
    pub force_planning: Option<bool>,
    pub model_variant: Option<String>,
    pub model_acceleration_enabled: Option<bool>,
    pub disable_permission_restrictions: Option<bool>,
    pub auto_session_name: Option<bool>,
    pub task_management: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SessionDirectoryParams {
    pub directory: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateSessionTaskManagementRequest {
    pub task_management: serde_json::Value,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct UpdateSessionRequest {
    pub title: Option<String>,
    pub name: Option<String>,
    pub model: Option<String>,
    pub agent: Option<String>,
    pub session_type: Option<String>,
    pub kill_processes_on_start: Option<bool>,
    pub validator_enabled: Option<bool>,
    pub force_planning: Option<bool>,
    pub disable_permission_restrictions: Option<bool>,
    pub auto_session_name: Option<bool>,
    pub task_management: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AbortResponse {
    pub aborted: bool,
    pub sessions: Vec<String>,
    pub cleanup: Option<AbortCleanup>,
    pub cleanups: Vec<AbortCleanup>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AbortCleanup {
    pub session_id: String,
    pub status: String,
    pub stopped_worker: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ForkSessionRequest {
    pub directory: Option<String>,
    pub model: Option<String>,
    pub agent: Option<String>,
    #[serde(default)]
    pub copy_context: Option<bool>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct MessageListParams {
    pub limit: Option<usize>,
    pub before: Option<String>,
    pub after: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SendAgentMessageRequest {
    pub reply_message: String,
    pub new_learning: String,
    pub step_summary: Option<String>,
    #[serde(default)]
    pub media: Vec<SendAgentMedia>,
    pub runtime_id: Option<String>,
    pub tool_call: Option<SendAgentToolCall>,
    pub runtime_status: Option<RuntimeSessionSyncStatus>,
    #[serde(default)]
    pub context_tokens: Option<SessionContextTokens>,
    #[serde(default)]
    pub usage: Option<serde_json::Value>,
    #[serde(default)]
    pub command_updates: Vec<CommandUpdatePayload>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct StreamAgentTextRequest {
    pub delta: String,
    pub runtime_id: String,
    pub created_at: i64,
    pub updated_at: i64,
    #[serde(default)]
    pub context_tokens: Option<SessionContextTokens>,
    #[serde(default)]
    pub usage: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SendAgentToolCall {
    pub tool_name: String,
    pub call_id: String,
    pub state: serde_json::Value,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CommandUpdatePayload {
    #[serde(rename = "messageID", alias = "message_id")]
    pub message_id: String,
    #[serde(rename = "partID", alias = "part_id")]
    pub part_id: String,
    #[serde(rename = "runtimeID", alias = "runtime_id")]
    pub runtime_id: String,
    #[serde(rename = "commandRunID", alias = "command_run_id")]
    pub command_run_id: String,
    #[serde(rename = "commandID", alias = "command_id")]
    pub command_id: String,
    #[serde(
        rename = "providerToolCallID",
        alias = "provider_tool_call_id",
        default
    )]
    pub provider_tool_call_id: Option<String>,
    #[serde(rename = "commandIndex", alias = "command_index", default)]
    pub command_index: Option<u64>,
    #[serde(rename = "eventSeq", alias = "event_seq", default)]
    pub event_seq: Option<i64>,
    pub status: String,
    #[serde(default)]
    pub command: serde_json::Value,
    #[serde(default)]
    pub result: serde_json::Value,
    #[serde(rename = "createdAt", alias = "created_at")]
    pub created_at: i64,
    #[serde(rename = "updatedAt", alias = "updated_at")]
    pub updated_at: i64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SendAgentMedia {
    pub path: String,
    #[serde(rename = "type")]
    pub media_type: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SendAgentMessageResponse {
    pub ok: bool,
    pub session_id: String,
    pub message_id: Option<String>,
    pub event: Option<GlobalEvent>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SessionCommandRequest {
    pub command: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SessionCommandResponse {
    pub output: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SessionStatusResponse {
    pub session_id: String,
    pub status: SessionStatus,
    pub task_management: serde_json::Value,
    pub context_tokens: SessionContextTokens,
    #[serde(default)]
    pub usage: SessionUsage,
    pub plan_summary: Option<String>,
    pub session_display_name: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ShareResponse {
    pub url: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AppendUserCommandRequest {
    pub command: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RuntimeSessionStatusRequest {
    pub status: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RegisterChildSessionRequest {
    pub child_session_id: String,
    pub directory: String,
    pub name: String,
    pub task_instruction: String,
}
