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
    SessionUpdated { properties: Box<Session> },
}
