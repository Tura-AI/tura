use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::{Message, Project, Session, SessionContextTokens, SessionUsage};

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
    #[serde(rename = "command.updated")]
    CommandUpdated {
        properties: CommandUpdatedProperties,
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
    #[serde(rename = "updatedAt")]
    pub updated_at: i64,
    pub status: serde_json::Value,
    #[serde(default)]
    pub context_tokens: SessionContextTokens,
    #[serde(default)]
    pub usage: SessionUsage,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageUpdatedProperties {
    #[serde(rename = "sessionID")]
    pub session_id: String,
    pub info: Message,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageRemovedProperties {
    #[serde(rename = "sessionID")]
    pub session_id: String,
    #[serde(rename = "messageID")]
    pub message_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessagePartDeltaProperties {
    #[serde(rename = "sessionID")]
    pub session_id: String,
    #[serde(rename = "messageID")]
    pub message_id: String,
    #[serde(rename = "partID")]
    pub part_id: String,
    #[serde(rename = "createdAt")]
    pub created_at: i64,
    #[serde(rename = "updatedAt")]
    pub updated_at: i64,
    pub field: String,
    pub delta: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessagePartUpdatedProperties {
    #[serde(rename = "sessionID")]
    pub session_id: String,
    #[serde(rename = "createdAt")]
    pub created_at: i64,
    #[serde(rename = "updatedAt")]
    pub updated_at: i64,
    pub part: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandUpdatedProperties {
    #[serde(rename = "sessionID")]
    pub session_id: String,
    #[serde(rename = "messageID")]
    pub message_id: String,
    #[serde(rename = "partID")]
    pub part_id: String,
    #[serde(rename = "runtimeID")]
    pub runtime_id: String,
    #[serde(rename = "commandRunID")]
    pub command_run_id: String,
    #[serde(rename = "commandID")]
    pub command_id: String,
    #[serde(rename = "providerToolCallID", default)]
    pub provider_tool_call_id: Option<String>,
    #[serde(rename = "commandIndex", default)]
    pub command_index: Option<u64>,
    #[serde(rename = "eventSeq", default)]
    pub event_seq: Option<i64>,
    pub status: String,
    #[serde(default)]
    pub command: serde_json::Value,
    #[serde(default)]
    pub result: serde_json::Value,
    #[serde(rename = "createdAt")]
    pub created_at: i64,
    #[serde(rename = "updatedAt")]
    pub updated_at: i64,
}

// Sync events (lighter weight)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum SyncEvent {
    #[serde(rename = "sync.project.updated")]
    ProjectUpdated { properties: Project },
    #[serde(rename = "sync.session.updated")]
    SessionUpdated { properties: Box<Session> },
}
