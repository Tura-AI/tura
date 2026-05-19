//! OpenCode API Compatible Types
//! This module re-exports and defines types compatible with the OpenCode SDK API

use serde::{ser::SerializeStruct, Deserialize, Serialize, Serializer};
use std::collections::HashMap;

// ============================================================================
// Import original gateway types (保留原有 attachment/emoji/OutboundAction 功能)
// ============================================================================

pub use crate::types::{
    AttachmentKind, GatewayAttachment, InboundMessage, OutboundAction, OutboundMediaType,
    ProcessedInboundMessage,
};

// ============================================================================
// Global / Health Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthResponse {
    pub healthy: bool,
    pub version: String,
}

impl Default for HealthResponse {
    fn default() -> Self {
        Self {
            healthy: true,
            version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }
}

// ============================================================================
// Config Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    pub language: Option<String>,
    pub theme: Option<String>,
    pub model: Option<String>,
    pub agent: Option<String>,
    pub skill_folders: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigPatch {
    pub language: Option<String>,
    pub theme: Option<String>,
    pub model: Option<String>,
    pub agent: Option<String>,
    pub skill_folders: Option<Vec<String>>,
}

// ============================================================================
// Session Types
// ============================================================================

#[derive(Debug, Clone, Deserialize)]
pub struct Session {
    pub id: String,
    pub name: Option<String>,
    #[serde(default)]
    pub parent_id: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub directory: Option<String>,
    pub model: Option<String>,
    pub agent: Option<String>,
    pub session_type: Option<String>,
    pub lsp: Option<serde_json::Value>,
    #[serde(default)]
    pub kill_processes_on_start: bool,
    #[serde(default)]
    pub validator_enabled: bool,
    #[serde(default)]
    pub force_multiple_tasks: bool,
    pub model_variant: Option<String>,
    #[serde(default)]
    pub model_acceleration_enabled: bool,
    #[serde(default)]
    pub disable_permission_restrictions: bool,
    pub status: SessionStatus,
    pub message_count: usize,
}

#[derive(Serialize)]
struct SessionTime {
    created: i64,
    updated: i64,
}

impl Serialize for Session {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("Session", 32)?;
        let directory = self.directory.clone().unwrap_or_default();
        let title = self
            .name
            .clone()
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "New Session".to_string());
        let session_type = self
            .session_type
            .clone()
            .or_else(|| self.agent.as_deref().map(session_type_from_agent))
            .unwrap_or_else(|| "coding".to_string());

        state.serialize_field("id", &self.id)?;
        state.serialize_field("slug", &self.id)?;
        state.serialize_field("parentID", &self.parent_id)?;
        state.serialize_field("parent_id", &self.parent_id)?;
        state.serialize_field("projectID", &directory)?;
        state.serialize_field("directory", &directory)?;
        state.serialize_field("title", &title)?;
        state.serialize_field("version", &env!("CARGO_PKG_VERSION"))?;
        state.serialize_field(
            "time",
            &SessionTime {
                created: self.created_at,
                updated: self.updated_at,
            },
        )?;

        // Legacy mock fields kept for older local callers.
        state.serialize_field("name", &self.name)?;
        state.serialize_field("created_at", &self.created_at)?;
        state.serialize_field("updated_at", &self.updated_at)?;
        state.serialize_field("model", &self.model)?;
        state.serialize_field("agent", &self.agent)?;
        state.serialize_field("session_type", &session_type)?;
        state.serialize_field("sessionType", &session_type)?;
        state.serialize_field("lsp", &self.lsp)?;
        state.serialize_field("kill_processes_on_start", &self.kill_processes_on_start)?;
        state.serialize_field("killProcessesOnStart", &self.kill_processes_on_start)?;
        state.serialize_field("validator_enabled", &self.validator_enabled)?;
        state.serialize_field("validatorEnabled", &self.validator_enabled)?;
        state.serialize_field("force_multiple_tasks", &self.force_multiple_tasks)?;
        state.serialize_field("forceMultipleTasks", &self.force_multiple_tasks)?;
        state.serialize_field("model_variant", &self.model_variant)?;
        state.serialize_field("modelVariant", &self.model_variant)?;
        state.serialize_field(
            "model_acceleration_enabled",
            &self.model_acceleration_enabled,
        )?;
        state.serialize_field("modelAccelerationEnabled", &self.model_acceleration_enabled)?;
        state.serialize_field(
            "disable_permission_restrictions",
            &self.disable_permission_restrictions,
        )?;
        state.serialize_field(
            "disablePermissionRestrictions",
            &self.disable_permission_restrictions,
        )?;
        state.serialize_field("status", &self.status)?;
        state.serialize_field("message_count", &self.message_count)?;
        state.end()
    }
}

fn session_type_from_agent(agent: &str) -> String {
    match agent {
        "coding_agent" | "coding_agent_fast" | "coding" => "coding".to_string(),
        other => other.to_string(),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SessionStatus {
    Idle,
    Busy,
    Error,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Message {
    pub id: String,
    pub session_id: String,
    pub role: MessageRole,
    pub parts: Vec<MessagePart>,
    pub created_at: i64,
    pub updated_at: i64,
    pub parent_id: Option<String>,
}

impl Serialize for Message {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("Message", 14)?;
        state.serialize_field("id", &self.id)?;
        state.serialize_field("sessionID", &self.session_id)?;
        state.serialize_field("session_id", &self.session_id)?;
        state.serialize_field("parentID", &self.parent_id)?;
        state.serialize_field("parent_id", &self.parent_id)?;
        state.serialize_field("role", &self.role)?;
        state.serialize_field("parts", &self.parts)?;
        state.serialize_field(
            "time",
            &serde_json::json!({
                "created": self.created_at,
                "updated": self.updated_at,
            }),
        )?;
        state.serialize_field("created_at", &self.created_at)?;
        state.serialize_field("updated_at", &self.updated_at)?;
        let runtime = runtime_metrics_from_parts(&self.parts);
        state.serialize_field("cost", &runtime.cost)?;
        state.serialize_field(
            "providerID",
            &runtime
                .provider_id
                .unwrap_or_else(crate::session::manager::coding_agent_provider),
        )?;
        state.serialize_field("modelID", &runtime.model_id)?;
        state.serialize_field("tokens", &runtime.tokens)?;
        state.end()
    }
}

#[derive(Debug, Clone)]
struct RuntimeMessageMetrics {
    cost: f64,
    provider_id: Option<String>,
    model_id: Option<String>,
    tokens: serde_json::Value,
}

fn runtime_metrics_from_parts(parts: &[MessagePart]) -> RuntimeMessageMetrics {
    let mut input = 0_u64;
    let mut output = 0_u64;
    let mut reasoning = 0_u64;
    let mut cache_read = 0_u64;
    let mut cache_write = 0_u64;
    let mut cost = 0.0_f64;
    let mut provider_id = None;
    let mut model_id = None;

    for part in parts {
        let candidates = [
            part.metadata.as_ref(),
            part.state.as_ref().and_then(|state| state.get("metadata")),
        ];
        for metadata in candidates.into_iter().flatten() {
            let Some(usage) = metadata.get("usage") else {
                continue;
            };
            input = input.saturating_add(json_u64(usage, "input_tokens"));
            output = output.saturating_add(json_u64(usage, "output_tokens"));
            reasoning = reasoning.saturating_add(json_u64(usage, "reasoning_tokens"));
            cache_read = cache_read.saturating_add(json_u64(usage, "cached_input_tokens"));
            cache_write = cache_write.saturating_add(json_u64(usage, "cache_write_tokens"));
            cost += json_f64(usage, "total_cost");

            if provider_id.is_none() {
                provider_id = metadata
                    .get("provider")
                    .and_then(|provider| provider.get("provider_name"))
                    .and_then(|value| value.as_str())
                    .map(ToString::to_string);
            }
            if model_id.is_none() {
                model_id = metadata
                    .get("provider")
                    .and_then(|provider| provider.get("model_name"))
                    .and_then(|value| value.as_str())
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(ToString::to_string);
            }
        }
    }

    RuntimeMessageMetrics {
        cost,
        provider_id,
        model_id,
        tokens: serde_json::json!({
            "input": input,
            "output": output,
            "reasoning": reasoning,
            "cache": {
                "read": cache_read,
                "write": cache_write,
            },
        }),
    }
}

fn json_u64(value: &serde_json::Value, key: &str) -> u64 {
    value.get(key).and_then(|value| value.as_u64()).unwrap_or(0)
}

fn json_f64(value: &serde_json::Value, key: &str) -> f64 {
    value
        .get(key)
        .and_then(|value| value.as_f64())
        .unwrap_or(0.0)
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    User,
    Assistant,
    System,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MessagePart {
    pub id: String,
    #[serde(rename = "type")]
    pub part_type: String,
    pub content: Option<String>,
    pub text: Option<String>,
    pub metadata: Option<serde_json::Value>,
    pub call_id: Option<String>,
    pub tool: Option<String>,
    pub state: Option<serde_json::Value>,
}

impl Serialize for MessagePart {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("MessagePart", 8)?;
        state.serialize_field("id", &self.id)?;
        state.serialize_field("type", &self.part_type)?;
        state.serialize_field("content", &self.content)?;
        state.serialize_field("text", &self.text)?;
        state.serialize_field("metadata", &self.metadata)?;
        state.serialize_field("callID", &self.call_id)?;
        state.serialize_field("tool", &self.tool)?;
        state.serialize_field("state", &self.state)?;
        state.end()
    }
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

#[cfg(test)]
mod tests {
    use super::{Message, MessagePart, MessageRole};

    #[test]
    fn message_serialization_uses_runtime_usage_metadata() {
        let message = Message {
            id: "msg-1".to_string(),
            session_id: "session-1".to_string(),
            role: MessageRole::Assistant,
            parts: vec![MessagePart {
                id: "part-1".to_string(),
                part_type: "tool".to_string(),
                content: None,
                text: None,
                metadata: Some(serde_json::json!({
                    "usage": {
                        "input_tokens": 10,
                        "output_tokens": 4,
                        "reasoning_tokens": 2,
                        "cached_input_tokens": 3,
                        "cache_write_tokens": 1,
                        "total_cost": 0.25
                    },
                    "provider": {
                        "provider_name": "openai",
                        "model_name": "gpt-test"
                    }
                })),
                call_id: Some("runtime-1".to_string()),
                tool: Some("runtime".to_string()),
                state: None,
            }],
            created_at: 1,
            updated_at: 2,
            parent_id: None,
        };

        let value = serde_json::to_value(message).expect("message should serialize");

        assert_eq!(value["tokens"]["input"], 10);
        assert_eq!(value["tokens"]["output"], 4);
        assert_eq!(value["tokens"]["reasoning"], 2);
        assert_eq!(value["tokens"]["cache"]["read"], 3);
        assert_eq!(value["tokens"]["cache"]["write"], 1);
        assert_eq!(value["cost"], 0.25);
        assert_eq!(value["providerID"], "openai");
        assert_eq!(value["modelID"], "gpt-test");
    }
}

// ============================================================================
// Project / Worktree Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub id: String,
    pub worktree: String,
    pub vcs: Option<String>,
    pub name: Option<String>,
    pub icon: Option<ProjectIcon>,
    pub time: ProjectTime,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectIcon {
    pub url: Option<String>,
    pub override_: Option<String>,
    pub color: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectTime {
    pub created: i64,
    pub updated: i64,
    pub initialized: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CurrentProjectResponse {
    pub project: Option<Project>,
}

// ============================================================================
// File Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileContentResponse {
    #[serde(rename = "type")]
    pub content_type: String,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub encoding: Option<String>,
    #[serde(rename = "mimeType", skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileStatusResponse {
    pub files: Vec<FileStatus>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileStatus {
    pub path: String,
    pub added: i32,
    pub removed: i32,
    pub status: String,
}

// ============================================================================
// Provider / Auth Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Provider {
    pub id: String,
    pub name: String,
    pub auth_type: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderListResponse {
    pub all: Vec<SdkProvider>,
    pub default: HashMap<String, String>,
    pub connected: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SdkProvider {
    pub id: String,
    pub name: String,
    pub source: String,
    pub env: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    pub options: HashMap<String, serde_json::Value>,
    pub models: HashMap<String, SdkProviderModel>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub npm: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SdkProviderModel {
    pub id: String,
    pub name: String,
    pub family: String,
    pub release_date: String,
    pub attachment: bool,
    pub reasoning: bool,
    pub temperature: bool,
    pub tool_call: bool,
    pub limit: SdkProviderModelLimit,
    pub modalities: SdkProviderModelModalities,
    pub options: HashMap<String, serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SdkProviderModelLimit {
    pub context: u32,
    pub input: u32,
    pub output: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SdkProviderModelModalities {
    pub input: Vec<String>,
    pub output: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderAuth {
    #[serde(rename = "type")]
    pub auth_type: String,
    pub key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub access: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refresh: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires: Option<i64>,
    #[serde(default, rename = "accountId", skip_serializing_if = "Option::is_none")]
    pub account_id: Option<String>,
    #[serde(default)]
    pub metadata: Option<HashMap<String, serde_json::Value>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderAuthResponse {
    pub success: bool,
}

// ============================================================================
// Permission Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionRequest {
    pub id: String,
    pub session_id: String,
    pub permission: String,
    pub args: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionCreateRequest {
    pub permission: String,
    #[serde(default)]
    pub args: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionReplyRequest {
    pub approve: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionReplyResponse {
    pub success: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionStatusResponse {
    pub responded: bool,
    pub approve: Option<bool>,
}

// ============================================================================
// Question Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuestionRequest {
    pub id: String,
    pub session_id: String,
    pub question: String,
    pub metadata: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuestionReplyRequest {
    pub response: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuestionReplyResponse {
    pub success: bool,
}

// ============================================================================
// Event Types (SSE)
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum GlobalEvent {
    #[serde(rename = "server.connected")]
    ServerConnected {
        properties: HashMap<String, serde_json::Value>,
    },
    #[serde(rename = "server.instance.disposed")]
    ServerInstanceDisposed {
        properties: InstanceDisposedProperties,
    },
    #[serde(rename = "project.updated")]
    ProjectUpdated { properties: Project },
    #[serde(rename = "session.created")]
    SessionCreated {
        properties: SessionCreatedProperties,
    },
    #[serde(rename = "session.updated")]
    SessionUpdated {
        properties: SessionUpdatedProperties,
    },
    #[serde(rename = "session.deleted")]
    SessionDeleted {
        properties: SessionDeletedProperties,
    },
    #[serde(rename = "session.status")]
    SessionStatus { properties: SessionStatusProperties },
    #[serde(rename = "message.updated")]
    MessageUpdated {
        properties: MessageUpdatedProperties,
    },
    #[serde(rename = "message.removed")]
    MessageRemoved {
        properties: MessageRemovedProperties,
    },
    #[serde(rename = "message.part.delta")]
    MessagePartDelta {
        properties: MessagePartDeltaProperties,
    },
    #[serde(rename = "message.part.updated")]
    MessagePartUpdated {
        properties: MessagePartUpdatedProperties,
    },
    #[serde(rename = "todo.updated")]
    TodoUpdated { properties: serde_json::Value },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstanceDisposedProperties {
    pub directory: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionCreatedProperties {
    #[serde(rename = "sessionID")]
    pub session_id: String,
    pub info: Session,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionUpdatedProperties {
    #[serde(rename = "sessionID")]
    pub session_id: String,
    pub info: Session,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionDeletedProperties {
    #[serde(rename = "sessionID")]
    pub session_id: String,
    pub info: Session,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionStatusProperties {
    #[serde(rename = "sessionID")]
    pub session_id: String,
    pub status: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageUpdatedProperties {
    #[serde(rename = "sessionID")]
    pub session_id: String,
    pub info: Message,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageRemovedProperties {
    pub session_id: String,
    pub message_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessagePartDeltaProperties {
    pub session_id: String,
    pub message_id: String,
    pub part_id: String,
    pub field: String,
    pub delta: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessagePartUpdatedProperties {
    #[serde(rename = "sessionID")]
    pub session_id: String,
    pub part: serde_json::Value,
}

// Sync events (lighter weight)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum SyncEvent {
    #[serde(rename = "sync.project.updated")]
    ProjectUpdated { properties: Project },
    #[serde(rename = "sync.session.updated")]
    SessionUpdated { properties: Session },
}

// ============================================================================
// PTY Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PtyCreateRequest {
    pub command: Option<String>,
    pub args: Option<Vec<String>>,
    pub cwd: Option<String>,
    pub title: Option<String>,
    pub env: Option<std::collections::HashMap<String, String>>,
    pub rows: Option<u16>,
    pub cols: Option<u16>,
    pub shell: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PtyResponse {
    pub id: String,
    pub pty_id: String,
    pub title: String,
    pub command: String,
    pub args: Vec<String>,
    pub cwd: String,
    pub status: String,
    pub pid: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PtyUpdateRequest {
    pub title: Option<String>,
    pub size: Option<PtySize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PtySize {
    pub rows: u16,
    pub cols: u16,
}

// ============================================================================
// MCP Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MCPServer {
    pub name: String,
    pub status: MCPStatus,
    pub tools: Vec<McpTool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status")]
pub enum MCPStatus {
    #[serde(rename = "connected")]
    Connected,
    #[serde(rename = "disabled")]
    Disabled,
    #[serde(rename = "failed")]
    Failed { error: String },
    #[serde(rename = "needs_auth")]
    NeedsAuth,
    #[serde(rename = "needs_client_registration")]
    NeedsClientRegistration { error: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpTool {
    pub name: String,
    pub description: Option<String>,
    pub input_schema: serde_json::Value,
}

// ============================================================================
// LSP Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LSPStatus {
    pub id: String,
    pub name: String,
    pub root: String,
    pub pid: Option<u32>,
    pub executable_path: Option<String>,
    pub status: String,
}

// ============================================================================
// VCS Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VcsInfo {
    pub branch: String,
    pub default_branch: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VcsDiffResponse {
    pub files: Vec<FileDiff>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileDiff {
    pub old_file_name: String,
    pub new_file_name: String,
    pub hunks: Vec<DiffHunk>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffHunk {
    pub old_start: u32,
    pub old_lines: u32,
    pub new_start: u32,
    pub new_lines: u32,
    pub lines: Vec<String>,
}

// ============================================================================
// Skill Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Skill {
    pub name: String,
    pub description: String,
    pub path: String,
}

// ============================================================================
// Path Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathResponse {
    pub home: String,
    pub state: String,
    pub config: String,
    pub worktree: String,
    pub directory: String,
}

// ============================================================================
// Agent Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Agent {
    pub name: String,
    pub description: String,
    pub mode: String,
    pub native: bool,
    pub hidden: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<AgentModel>,
    #[serde(default)]
    pub options: HashMap<String, serde_json::Value>,
    pub permission: PermissionRuleset,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentModel {
    #[serde(rename = "providerID")]
    pub provider_id: String,
    #[serde(rename = "modelID")]
    pub model_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionRuleset {
    pub allow: Vec<String>,
    pub deny: Vec<String>,
}

// ============================================================================
// Command Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Command {
    pub name: String,
    pub description: String,
    pub agent: Option<String>,
    pub model: Option<String>,
    pub source: String,
    pub template: Option<String>,
    pub subtask: bool,
    pub hints: Vec<String>,
}

// ============================================================================
// Log Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogRequest {
    pub service: String,
    pub level: String,
    pub message: String,
    pub extra: Option<HashMap<String, serde_json::Value>>,
}

// ============================================================================
// Helper Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BadRequestError {
    pub error: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpgradeRequest {
    pub target: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpgradeResponse {
    pub success: bool,
    pub version: Option<String>,
    pub error: Option<String>,
}
