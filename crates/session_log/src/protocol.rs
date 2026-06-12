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
pub struct DeleteSessionRequest {
    pub session_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteWorkspaceRequest {
    pub workspace: String,
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
    DeleteSession(DeleteSessionRequest),
    DeleteWorkspace(DeleteWorkspaceRequest),
    Shutdown,
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

#[cfg(test)]
mod tests {
    use super::{
        DeleteSessionRequest, DeleteWorkspaceRequest, GetSessionRequest, ListSessionRecordsRequest,
        ListSessionsRequest, Page, SessionLogCommand, SessionLogResponse, SessionRecord,
        SessionSnapshot, UpsertSessionRequest, WorkspaceSummary,
    };
    use serde_json::json;

    #[test]
    fn page_defaults_match_public_pagination_contract() {
        assert_eq!(
            Page::default(),
            Page {
                page: 0,
                page_size: 50,
                total: 0
            }
        );
        let page: Page = serde_json::from_value(json!({ "total": 7 })).expect("page");
        assert_eq!(page.page, 0);
        assert_eq!(page.page_size, 50);
        assert_eq!(page.total, 7);
    }

    #[test]
    fn request_defaults_keep_optional_payloads_empty() {
        let upsert: UpsertSessionRequest =
            serde_json::from_value(json!({ "session": { "id": "session" } })).expect("upsert");
        assert_eq!(upsert.parent_id, None);
        assert!(upsert.messages.is_empty());
        assert!(upsert.todos.is_empty());

        let list: ListSessionsRequest =
            serde_json::from_value(json!({ "workspace": "workspace" })).expect("list sessions");
        assert_eq!(list.page, 0);
        assert_eq!(list.page_size, 50);

        let records: ListSessionRecordsRequest =
            serde_json::from_value(json!({ "session_id": "session" })).expect("records");
        assert_eq!(records.page, 0);
        assert_eq!(records.page_size, 50);
    }

    #[test]
    fn command_serde_uses_snake_case_tagged_contract() {
        let commands = [
            SessionLogCommand::GetSession(GetSessionRequest {
                session_id: "session".to_string(),
            }),
            SessionLogCommand::DeleteSession(DeleteSessionRequest {
                session_id: "session".to_string(),
            }),
            SessionLogCommand::DeleteWorkspace(DeleteWorkspaceRequest {
                workspace: "workspace".to_string(),
            }),
            SessionLogCommand::Shutdown,
        ];

        for command in commands {
            let value = serde_json::to_value(&command).expect("command json");
            let command_name = value["command"].as_str().expect("command tag");
            assert!(
                command_name
                    .chars()
                    .all(|ch| ch.is_ascii_lowercase() || ch == '_'),
                "command tag must stay snake_case: {command_name}"
            );
            let round_trip: SessionLogCommand =
                serde_json::from_value(value).expect("command round trip");
            assert_eq!(
                std::mem::discriminant(&round_trip),
                std::mem::discriminant(&command)
            );
        }
    }

    #[test]
    fn response_serde_round_trips_all_read_shapes() {
        let workspaces = SessionLogResponse::Workspaces {
            workspaces: vec![WorkspaceSummary {
                directory: "workspace".to_string(),
                session_count: 1,
                last_updated_at: 2,
            }],
        };
        let sessions = SessionLogResponse::Sessions {
            page: Page {
                page: 1,
                page_size: 10,
                total: 11,
            },
            sessions: vec![SessionSnapshot {
                session_id: "session".to_string(),
                workspace: "workspace".to_string(),
                name: Some("Session".to_string()),
                parent_id: None,
                created_at: 1,
                updated_at: 2,
                state: Some("created".to_string()),
                status: Some("idle".to_string()),
                message_count: 1,
                task_management: json!({}),
                management: json!({ "state": "created" }),
                session: json!({ "id": "session" }),
                todos: Vec::new(),
            }],
        };
        let records = SessionLogResponse::Records {
            page: Page::default(),
            records: vec![SessionRecord {
                session_id: "session".to_string(),
                message_id: "message".to_string(),
                role: "assistant".to_string(),
                created_at: 1,
                updated_at: 2,
                record: json!({ "id": "message" }),
            }],
        };

        for response in [
            SessionLogResponse::Ok,
            workspaces,
            sessions,
            SessionLogResponse::Session { session: None },
            records,
            SessionLogResponse::Error {
                error: "boom".to_string(),
            },
        ] {
            let encoded = serde_json::to_string(&response).expect("response json");
            let decoded: SessionLogResponse =
                serde_json::from_str(&encoded).expect("response round trip");
            assert_eq!(
                std::mem::discriminant(&decoded),
                std::mem::discriminant(&response)
            );
        }
    }
}
