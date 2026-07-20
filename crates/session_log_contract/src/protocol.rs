use crate::CommandCheckpoint;
use lifecycle::{SessionCommand, SessionEvent, SessionProjection};
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_user_message_at: Option<i64>,
    pub state: Option<String>,
    pub status: Option<String>,
    pub message_count: u64,
    pub task_management: serde_json::Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lifecycle_projection: Option<SessionProjection>,
    pub management: serde_json::Value,
    #[serde(default)]
    pub session: serde_json::Value,
    #[serde(default)]
    pub todos: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSummary {
    pub session_id: String,
    pub workspace: String,
    pub name: Option<String>,
    pub parent_id: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_user_message_at: Option<i64>,
    pub state: Option<String>,
    pub status: Option<String>,
    pub message_count: u64,
    pub task_management: serde_json::Value,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PersistSessionPayloadRequest {
    pub session_id: String,
    pub records: Vec<serde_json::Value>,
    pub todos: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CreateSessionRequest {
    pub session_id: String,
    pub creation_command: SessionCommand,
    pub workspace: String,
    pub session_directory: String,
    pub name: String,
    pub created_at: i64,
    pub model: Option<String>,
    pub agent: Option<String>,
    pub session_type: String,
    pub kill_processes_on_start: bool,
    pub validator_enabled: bool,
    pub force_planning: bool,
    pub model_variant: Option<String>,
    pub model_acceleration_enabled: bool,
    pub disable_permission_restrictions: bool,
    pub use_last_tool_call_response: bool,
    pub auto_session_name: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ExecuteSessionCommandRequest {
    pub session_id: String,
    pub session_command: SessionCommand,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SessionCommandResult {
    pub event: SessionEvent,
    pub projection: SessionProjection,
    pub session_name: Option<String>,
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
pub struct MarkSessionInterruptedRequest {
    pub session_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteWorkspaceRequest {
    pub workspace: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "command", rename_all = "snake_case")]
pub enum SessionLogCommand {
    Health,
    CreateSession(CreateSessionRequest),
    ExecuteSessionCommand(ExecuteSessionCommandRequest),
    PersistSessionPayload(PersistSessionPayloadRequest),
    UpsertSession(UpsertSessionRequest),
    ApplyCommandCheckpoint(Box<CommandCheckpoint>),
    GetSession(GetSessionRequest),
    ListWorkspaces,
    ListSessions(ListSessionsRequest),
    ListSessionSummaries(ListSessionsRequest),
    ListSessionRecords(ListSessionRecordsRequest),
    MarkSessionInterrupted(MarkSessionInterruptedRequest),
    DeleteSession(DeleteSessionRequest),
    DeleteWorkspace(DeleteWorkspaceRequest),
    Shutdown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SessionLogResponse {
    Ok,
    SessionCommandApplied {
        result: Box<SessionCommandResult>,
    },
    Workspaces {
        workspaces: Vec<WorkspaceSummary>,
    },
    Sessions {
        page: Page,
        sessions: Vec<SessionSnapshot>,
    },
    SessionSummaries {
        page: Page,
        sessions: Vec<SessionSummary>,
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
        CreateSessionRequest, DeleteSessionRequest, DeleteWorkspaceRequest,
        ExecuteSessionCommandRequest, GetSessionRequest, ListSessionRecordsRequest,
        ListSessionsRequest, MarkSessionInterruptedRequest, Page, PersistSessionPayloadRequest,
        SessionCommandResult, SessionLogCommand, SessionLogResponse, SessionRecord,
        SessionSnapshot, UpsertSessionRequest, WorkspaceSummary,
    };
    use lifecycle::{SessionAggregate, SessionCommand, SessionEvent, SessionQuery};
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
            SessionLogCommand::Health,
            SessionLogCommand::ExecuteSessionCommand(ExecuteSessionCommandRequest {
                session_id: "session".to_string(),
                session_command: SessionCommand::SubmitUserInput,
            }),
            SessionLogCommand::PersistSessionPayload(PersistSessionPayloadRequest {
                session_id: "session".to_string(),
                records: vec![json!({ "id": "message", "role": "assistant" })],
                todos: vec![json!({ "id": "todo" })],
            }),
            SessionLogCommand::GetSession(GetSessionRequest {
                session_id: "session".to_string(),
            }),
            SessionLogCommand::DeleteSession(DeleteSessionRequest {
                session_id: "session".to_string(),
            }),
            SessionLogCommand::MarkSessionInterrupted(MarkSessionInterruptedRequest {
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
            if command_name == "execute_session_command" {
                assert_eq!(
                    value["session_command"],
                    json!({ "command": "submit_user_input" })
                );
            }
            if command_name == "persist_session_payload" {
                assert_eq!(value["session_id"], "session");
                assert_eq!(value["records"][0]["id"], "message");
                assert_eq!(value["todos"][0]["id"], "todo");
            }
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
                last_user_message_at: Some(1),
                state: Some("created".to_string()),
                status: Some("idle".to_string()),
                message_count: 1,
                task_management: json!({}),
                lifecycle_projection: None,
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
            SessionLogResponse::SessionCommandApplied {
                result: Box::new(SessionCommandResult {
                    event: SessionEvent::SessionCreated {
                        task_plan: lifecycle::TaskPlan::default(),
                    },
                    projection: SessionAggregate::new("session".to_string())
                        .query(SessionQuery::Lifecycle),
                    session_name: Some("Session".to_string()),
                }),
            },
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

    #[test]
    fn typed_session_mutations_reject_extra_fields() {
        let create = CreateSessionRequest {
            session_id: "session".to_string(),
            creation_command: SessionCommand::CreateSession {
                task_plan: lifecycle::TaskPlan::default(),
            },
            workspace: "C:/workspace".to_string(),
            session_directory: "C:/workspace".to_string(),
            name: "Session".to_string(),
            created_at: 1,
            model: None,
            agent: None,
            session_type: "coding".to_string(),
            kill_processes_on_start: false,
            validator_enabled: false,
            force_planning: false,
            model_variant: None,
            model_acceleration_enabled: false,
            disable_permission_restrictions: false,
            use_last_tool_call_response: false,
            auto_session_name: true,
        };
        let value = serde_json::to_value(create).expect("create json");
        assert!(serde_json::from_value::<CreateSessionRequest>(value).is_ok());
        assert!(
            serde_json::from_value::<ExecuteSessionCommandRequest>(json!({
                "session_id": "session",
                "session_command": { "command": "submit_user_input" },
                "extra": true
            }))
            .is_err()
        );
        assert!(
            serde_json::from_value::<PersistSessionPayloadRequest>(json!({
                "session_id": "session",
                "records": [],
                "todos": [],
                "lifecycle": { "state": "running" }
            }))
            .is_err()
        );
    }
}
