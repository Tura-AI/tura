use crate::checkpoint::CommandCheckpoint;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct Page {
    #[serde(default)]
    pub page: u64,
    #[serde(default = "default_page_size")]
    pub page_size: u64,
    pub total: u64,
}

fn default_page_size() -> u64 {
    50
}

impl Default for Page {
    fn default() -> Self {
        Self {
            page: 0,
            page_size: default_page_size(),
            total: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceSummary {
    pub directory: String,
    pub session_count: u64,
    pub last_updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSnapshot {
    pub session_id: String,
    pub workspace: String,
    pub name: Option<String>,
    pub parent_id: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub state: Option<String>,
    pub status: Option<String>,
    pub message_count: u64,
    pub task_management: serde_json::Value,
    pub management: serde_json::Value,
    #[serde(default)]
    pub session: serde_json::Value,
    #[serde(default)]
    pub todos: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionRecord {
    pub session_id: String,
    pub message_id: String,
    pub role: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub record: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpsertSessionRequest {
    pub session: serde_json::Value,
    #[serde(default)]
    pub parent_id: Option<String>,
    #[serde(default)]
    pub messages: Vec<serde_json::Value>,
    #[serde(default)]
    pub todos: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListSessionsRequest {
    pub workspace: String,
    #[serde(default)]
    pub page: u64,
    #[serde(default = "default_page_size")]
    pub page_size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListSessionRecordsRequest {
    pub session_id: String,
    #[serde(default)]
    pub page: u64,
    #[serde(default = "default_page_size")]
    pub page_size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetSessionRequest {
    pub session_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "command", rename_all = "snake_case")]
pub enum SessionLogCommand {
    UpsertSession(UpsertSessionRequest),
    ApplyCommandCheckpoint(Box<CommandCheckpoint>),
    GetSession(GetSessionRequest),
    ListWorkspaces,
    ListSessions(ListSessionsRequest),
    ListSessionRecords(ListSessionRecordsRequest),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SessionLogResponse {
    Ok,
    Workspaces {
        workspaces: Vec<WorkspaceSummary>,
    },
    Sessions {
        page: Page,
        sessions: Vec<SessionSnapshot>,
    },
    Session {
        session: Option<Box<SessionSnapshot>>,
    },
    Records {
        page: Page,
        records: Vec<SessionRecord>,
    },
    Error {
        error: String,
    },
}
