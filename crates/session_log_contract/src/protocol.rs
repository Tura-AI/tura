use crate::CommandCheckpoint;
use lifecycle::{
    ContextTokenStats, RuntimeAggregate, RuntimeEvent, RuntimeProjection, SessionCommand,
    SessionEvent, SessionManagement, SessionManagementDelta, SessionProjection, UsageReport,
};
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct SessionMetadata {
    pub session_directory: String,
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
    pub context_tokens: ContextTokenStats,
    pub runtime_usage: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct SessionSnapshot {
    pub session_id: String,
    pub workspace: String,
    pub name: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_user_message_at: Option<i64>,
    pub message_count: u64,
    pub lifecycle_projection: SessionProjection,
    pub management: SessionManagement,
    pub metadata: SessionMetadata,
    #[serde(default)]
    pub todos: Vec<serde_json::Value>,
}

impl SessionSnapshot {
    pub fn validate(&self) -> Result<(), String> {
        if self.session_id != self.lifecycle_projection.session_id {
            return Err(format!(
                "snapshot session id {} does not match lifecycle projection {}",
                self.session_id, self.lifecycle_projection.session_id
            ));
        }
        if self.session_id != self.management.session_id {
            return Err(format!(
                "snapshot session id {} does not match management {}",
                self.session_id, self.management.session_id
            ));
        }
        let management_projection = self.management.lifecycle_projection();
        if management_projection.state != self.lifecycle_projection.state
            || management_projection.task_plan != self.lifecycle_projection.task_plan
        {
            return Err(format!(
                "snapshot lifecycle projection does not match persisted management for {}",
                self.session_id
            ));
        }
        if self.name.as_deref() != Some(self.management.session_name.as_str()) {
            return Err(format!(
                "snapshot name does not match persisted management for {}",
                self.session_id
            ));
        }
        let management_directory = self.management.session_directory.to_string_lossy();
        if self.metadata.session_directory != management_directory {
            return Err(format!(
                "snapshot metadata directory does not match persisted management for {}",
                self.session_id
            ));
        }
        if self.metadata.disable_permission_restrictions
            != self.management.disable_permission_restrictions
            || self.metadata.use_last_tool_call_response
                != self.management.use_last_tool_call_response
            || self.metadata.auto_session_name != self.management.auto_session_name
            || self.metadata.context_tokens != self.management.context_tokens
            || self.metadata.runtime_usage != self.management.runtime_usage
        {
            return Err(format!(
                "snapshot metadata does not match persisted management for {}",
                self.session_id
            ));
        }
        Ok(())
    }

    pub fn into_management(mut self) -> Result<SessionManagement, String> {
        self.validate()?;
        self.management
            .replace_lifecycle_projection(self.lifecycle_projection);
        Ok(self.management)
    }
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct SessionContextRecord {
    pub sequence: u64,
    pub raw_record: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SessionRecordProjection {
    pub session_id: String,
    pub message_id: String,
    pub role: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub record: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct SessionDeltaEntry {
    pub context: SessionContextRecord,
    #[serde(deserialize_with = "Option::deserialize")]
    pub projection: Option<SessionRecordProjection>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PersistSessionDeltaRequest {
    pub session_id: String,
    pub management_sequence: u64,
    pub management_delta: SessionManagementDelta,
    pub retained_from_sequence: u64,
    pub entries: Vec<SessionDeltaEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ReadContextSliceRequest {
    pub session_id: String,
    pub max_estimated_tokens: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ContextSlice {
    pub records: Vec<SessionContextRecord>,
    pub retained_from_sequence: u64,
    pub next_sequence: u64,
    pub next_management_sequence: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CreateSessionRequest {
    pub command_id: String,
    pub session_id: String,
    pub creation_command: SessionCommand,
    pub copy_context: bool,
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
    #[serde(deserialize_with = "Option::deserialize")]
    pub initial_task_plan_patch: Option<lifecycle::SessionTaskPlanPatch>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ExecuteSessionCommandRequest {
    pub command_id: String,
    pub session_id: String,
    pub session_command: SessionCommand,
    #[serde(deserialize_with = "Option::deserialize")]
    pub message_projection: Option<SessionRecordProjection>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SessionMetadataPatch {
    #[serde(deserialize_with = "Option::deserialize")]
    pub name: Option<String>,
    #[serde(deserialize_with = "Option::deserialize")]
    pub model: Option<String>,
    #[serde(deserialize_with = "Option::deserialize")]
    pub agent: Option<String>,
    pub clear_agent: bool,
    #[serde(deserialize_with = "Option::deserialize")]
    pub session_type: Option<String>,
    #[serde(deserialize_with = "Option::deserialize")]
    pub kill_processes_on_start: Option<bool>,
    #[serde(deserialize_with = "Option::deserialize")]
    pub validator_enabled: Option<bool>,
    #[serde(deserialize_with = "Option::deserialize")]
    pub force_planning: Option<bool>,
    #[serde(deserialize_with = "Option::deserialize")]
    pub disable_permission_restrictions: Option<bool>,
    #[serde(deserialize_with = "Option::deserialize")]
    pub use_last_tool_call_response: Option<bool>,
    #[serde(deserialize_with = "Option::deserialize")]
    pub auto_session_name: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct UpdateSessionRequest {
    pub command_id: String,
    pub session_id: String,
    pub metadata: SessionMetadataPatch,
    #[serde(deserialize_with = "Option::deserialize")]
    pub task_plan_patch: Option<lifecycle::SessionTaskPlanPatch>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct UpdateSessionTodosRequest {
    pub command_id: String,
    pub session_id: String,
    pub todos: Vec<serde_json::Value>,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SessionCommandResult {
    pub event: SessionEvent,
    pub projection: SessionProjection,
    pub session_name: Option<String>,
    pub message_count: u64,
    pub last_user_message_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RegisterRuntimeRequest {
    pub runtime_id: String,
    pub session_id: String,
    #[serde(deserialize_with = "Option::deserialize")]
    pub fallback_from_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ActivateRuntimeLeaseRequest {
    pub runtime_id: String,
    pub lease_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct CommitRuntimeEventRequest {
    pub runtime_id: String,
    pub event_seq: u64,
    pub expected_revision: u64,
    pub lease_id: String,
    pub idempotency_key: String,
    pub event: RuntimeEvent,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct SessionFeedCommandUpdate {
    pub message_id: String,
    pub part_id: String,
    pub command_run_id: String,
    pub command_id: String,
    #[serde(deserialize_with = "Option::deserialize")]
    pub provider_tool_call_id: Option<String>,
    #[serde(deserialize_with = "Option::deserialize")]
    pub command_index: Option<u64>,
    #[serde(deserialize_with = "Option::deserialize")]
    pub event_seq: Option<i64>,
    pub status: String,
    pub command: serde_json::Value,
    pub result: serde_json::Value,
    pub created_at: i64,
    pub updated_at: i64,
}

/// Durable frontend projection facts. Runtime and Session commands author them
/// through the Session service, which owns ordering and replay; Gateway only
/// reduces them into its cache and existing public event API.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "event", rename_all = "snake_case", deny_unknown_fields)]
pub enum SessionFeedEvent {
    MessageUpserted {
        message: SessionRecordProjection,
    },
    AssistantTextDelta {
        message_id: String,
        part_id: String,
        delta: String,
        created_at: i64,
        updated_at: i64,
    },
    AgentMessage {
        message_id: String,
        part_id: String,
        reply_message: String,
        new_learning: String,
        #[serde(deserialize_with = "Option::deserialize")]
        runtime_status: Option<RuntimeProjection>,
        #[serde(deserialize_with = "Option::deserialize")]
        context_tokens: Option<ContextTokenStats>,
        #[serde(deserialize_with = "Option::deserialize")]
        usage: Option<UsageReport>,
        created_at: i64,
        updated_at: i64,
    },
    ToolCallUpdated {
        message_id: String,
        part_id: String,
        tool_name: String,
        call_id: String,
        state: serde_json::Value,
        #[serde(deserialize_with = "Option::deserialize")]
        metadata: Option<serde_json::Value>,
        #[serde(deserialize_with = "Option::deserialize")]
        runtime_status: Option<RuntimeProjection>,
        #[serde(deserialize_with = "Option::deserialize")]
        context_tokens: Option<ContextTokenStats>,
        #[serde(deserialize_with = "Option::deserialize")]
        usage: Option<UsageReport>,
        command_updates: Vec<SessionFeedCommandUpdate>,
        created_at: i64,
        updated_at: i64,
    },
    TodosUpdated {
        todos: Vec<serde_json::Value>,
        updated_at: i64,
    },
    SessionProjectionUpdated {
        projection: SessionProjection,
        #[serde(deserialize_with = "Option::deserialize")]
        session_name: Option<String>,
        updated_at: i64,
    },
    SessionSnapshotCreated {
        snapshot: Box<SessionSnapshot>,
    },
    SessionSnapshotUpdated {
        snapshot: Box<SessionSnapshot>,
    },
    SessionDeleted {},
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct AppendSessionFeedEventRequest {
    pub runtime_id: String,
    pub target_session_id: String,
    pub lease_id: String,
    pub event_id: String,
    pub event: SessionFeedEvent,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ReadSessionFeedRequest {
    pub session_id: String,
    pub after_cursor: u64,
    pub limit: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct SessionFeedEntry {
    pub session_id: String,
    pub cursor: u64,
    #[serde(deserialize_with = "Option::deserialize")]
    pub runtime_id: Option<String>,
    pub event_id: String,
    pub event: SessionFeedEvent,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "outcome", rename_all = "snake_case", deny_unknown_fields)]
pub enum SessionFeedAppendOutcome {
    Applied { cursor: u64 },
    Duplicate { cursor: u64 },
    RuntimeNotFound,
    TargetSessionNotFound,
    TargetWorkspaceMismatch,
    StaleLease,
    RuntimeTerminal,
    EventIdConflict,
    SessionOwnedEvent,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ReplayRuntimeRequest {
    pub runtime_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "outcome", rename_all = "snake_case", deny_unknown_fields)]
pub enum RuntimeRegistrationOutcome {
    Registered {
        revision: u64,
        next_event_seq: u64,
        projection: SessionProjection,
    },
    AlreadyRegistered {
        revision: u64,
        next_event_seq: u64,
        projection: SessionProjection,
    },
    SessionBusy {
        active_runtime_id: String,
    },
    RuntimeIdConflict,
    SessionNotFound,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "outcome", rename_all = "snake_case", deny_unknown_fields)]
pub enum RuntimeLeaseOutcome {
    Activated,
    AlreadyActive,
    RuntimeNotFound,
    RuntimeTerminal,
    LeaseConflict,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "outcome", rename_all = "snake_case", deny_unknown_fields)]
pub enum RuntimeEventCommitOutcome {
    Applied {
        revision: u64,
        next_event_seq: u64,
        projection: RuntimeProjection,
    },
    Duplicate {
        revision: u64,
        next_event_seq: u64,
    },
    RuntimeNotFound,
    StaleLease,
    RuntimeTerminal,
    OutOfOrder {
        expected_event_seq: u64,
        received_event_seq: u64,
    },
    RevisionConflict {
        current_revision: u64,
        expected_revision: u64,
    },
    InvalidEvent {
        error: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct RuntimeReplay {
    pub aggregate: RuntimeAggregate,
    pub revision: u64,
    pub next_event_seq: u64,
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
#[serde(tag = "command", rename_all = "snake_case", deny_unknown_fields)]
pub enum SessionLogCommand {
    Health,
    CreateSession(CreateSessionRequest),
    ExecuteSessionCommand(ExecuteSessionCommandRequest),
    UpdateSession(UpdateSessionRequest),
    UpdateSessionTodos(UpdateSessionTodosRequest),
    RegisterRuntime(RegisterRuntimeRequest),
    ActivateRuntimeLease(ActivateRuntimeLeaseRequest),
    CommitRuntimeEvent(CommitRuntimeEventRequest),
    AppendSessionFeedEvent(AppendSessionFeedEventRequest),
    ReadSessionFeed(ReadSessionFeedRequest),
    SubscribeSessionFeed,
    ReplayRuntime(ReplayRuntimeRequest),
    PersistSessionDelta(Box<PersistSessionDeltaRequest>),
    ReadContextSlice(ReadContextSliceRequest),
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
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum SessionLogResponse {
    Ok,
    SessionCommandApplied {
        result: Box<SessionCommandResult>,
    },
    SessionUpdated {
        session: Box<SessionSnapshot>,
    },
    SessionTodosUpdated {
        todos: Vec<serde_json::Value>,
        cursor: u64,
    },
    RuntimeRegistered {
        result: RuntimeRegistrationOutcome,
    },
    RuntimeLeaseActivated {
        result: RuntimeLeaseOutcome,
    },
    RuntimeEventCommitted {
        result: RuntimeEventCommitOutcome,
    },
    SessionFeedEventAppended {
        result: SessionFeedAppendOutcome,
    },
    SessionFeed {
        entries: Vec<SessionFeedEntry>,
        next_cursor: u64,
    },
    SessionFeedSubscribed,
    SessionFeedEvent {
        entry: Box<SessionFeedEntry>,
    },
    RuntimeReplayed {
        runtime: Option<Box<RuntimeReplay>>,
    },
    SessionDeltaPersisted {
        next_sequence: u64,
        next_management_sequence: u64,
    },
    ContextSlice {
        context: ContextSlice,
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
        ListSessionsRequest, MarkSessionInterruptedRequest, Page, PersistSessionDeltaRequest,
        RegisterRuntimeRequest, SessionCommandResult, SessionFeedEvent, SessionLogCommand,
        SessionLogResponse, SessionMetadata, SessionMetadataPatch, SessionRecord,
        SessionSnapshot,
        UpdateSessionRequest, WorkspaceSummary,
    };
    use lifecycle::{
        SessionAggregate, SessionCommand, SessionEvent, SessionInput, SessionManagement,
        SessionQuery,
    };
    use serde_json::json;

    fn snapshot_fixture(session_id: &str) -> SessionSnapshot {
        let timestamp = chrono::DateTime::<chrono::Utc>::from_timestamp_millis(1)
            .expect("snapshot timestamp");
        let projection =
            SessionAggregate::new(session_id.to_string()).query(SessionQuery::Lifecycle);
        let mut management = SessionManagement::new(
            session_id.to_string(),
            "Session".to_string(),
            "workspace".into(),
            false,
            Vec::<String>::new(),
            SessionInput {
                user_input: String::new(),
                file_input: Vec::new(),
                agent: None,
                runtime_context: None,
                planning_mode_override: None,
            },
            String::new(),
            timestamp,
        );
        management.replace_lifecycle_projection(projection.clone());
        SessionSnapshot {
            session_id: session_id.to_string(),
            workspace: "workspace".to_string(),
            name: Some(management.session_name.clone()),
            created_at: 1,
            updated_at: 2,
            last_user_message_at: Some(1),
            message_count: 1,
            lifecycle_projection: projection,
            metadata: SessionMetadata {
                session_directory: "workspace".to_string(),
                model: None,
                agent: None,
                session_type: "coding".to_string(),
                kill_processes_on_start: false,
                validator_enabled: false,
                force_planning: false,
                model_variant: None,
                model_acceleration_enabled: false,
                disable_permission_restrictions: management.disable_permission_restrictions,
                use_last_tool_call_response: management.use_last_tool_call_response,
                auto_session_name: management.auto_session_name,
                context_tokens: management.context_tokens,
                runtime_usage: management.runtime_usage.clone(),
            },
            management,
            todos: Vec::new(),
        }
    }

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
        let list: ListSessionsRequest =
            serde_json::from_value(json!({ "workspace": "workspace" })).expect("list sessions");
        assert_eq!(list.page, 0);
        assert_eq!(list.page_size, 50);

        let records: ListSessionRecordsRequest =
            serde_json::from_value(json!({ "session_id": "session" })).expect("records");
        assert_eq!(records.page, 0);
        assert_eq!(records.page_size, 50);

        let register = RegisterRuntimeRequest {
            runtime_id: "runtime-retry".to_string(),
            session_id: "session".to_string(),
            fallback_from_id: Some("runtime-failed".to_string()),
        };
        let value = serde_json::to_value(&register).expect("register runtime request");
        assert_eq!(value["fallback_from_id"], "runtime-failed");
        assert_eq!(
            serde_json::from_value::<RegisterRuntimeRequest>(value)
                .expect("register runtime round trip"),
            register
        );
        assert!(serde_json::from_value::<RegisterRuntimeRequest>(json!({
            "runtime_id": "runtime",
            "session_id": "session"
        }))
        .is_err());
    }

    #[test]
    fn command_serde_uses_snake_case_tagged_contract() {
        let commands = [
            SessionLogCommand::Health,
            SessionLogCommand::ExecuteSessionCommand(ExecuteSessionCommandRequest {
                command_id: "command".to_string(),
                session_id: "session".to_string(),
                session_command: SessionCommand::SubmitUserInput,
                message_projection: None,
            }),
            SessionLogCommand::UpdateSession(UpdateSessionRequest {
                command_id: "update-command".to_string(),
                session_id: "session".to_string(),
                metadata: SessionMetadataPatch {
                    name: Some("Updated".to_string()),
                    ..SessionMetadataPatch::default()
                },
                task_plan_patch: None,
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
            let round_trip: SessionLogCommand =
                serde_json::from_value(value).expect("command round trip");
            assert_eq!(
                std::mem::discriminant(&round_trip),
                std::mem::discriminant(&command)
            );
        }
    }

    #[test]
    fn boxed_session_delta_keeps_the_flat_wire_contract() {
        let now =
            serde_json::from_value(json!("2026-07-20T00:00:00Z")).expect("valid session timestamp");
        let management = SessionManagement::new(
            "session".to_string(),
            "Session".to_string(),
            "C:/workspace".into(),
            false,
            Vec::<String>::new(),
            SessionInput {
                user_input: "hello".to_string(),
                file_input: Vec::new(),
                agent: None,
                runtime_context: None,
                planning_mode_override: None,
            },
            "goal".to_string(),
            now,
        );
        let command =
            SessionLogCommand::PersistSessionDelta(Box::new(PersistSessionDeltaRequest {
                session_id: "session".to_string(),
                management_sequence: 0,
                management_delta: SessionManagement::persistence_delta(None, &management),
                retained_from_sequence: 0,
                entries: Vec::new(),
            }));

        let value = serde_json::to_value(&command).expect("session delta command json");
        assert_eq!(value["command"], "persist_session_delta");
        assert_eq!(value["session_id"], "session");
        assert_eq!(value["management_sequence"], 0);
        assert_eq!(value["management_delta"]["session_name"], "Session");
        assert_eq!(value["retained_from_sequence"], 0);
        assert!(value.get("payload").is_none());

        let round_trip: SessionLogCommand =
            serde_json::from_value(value).expect("session delta command round trip");
        assert!(matches!(
            round_trip,
            SessionLogCommand::PersistSessionDelta(payload) if payload.session_id == "session"
        ));
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
            sessions: vec![snapshot_fixture("session")],
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
                    message_count: 0,
                    last_user_message_at: None,
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
    fn session_snapshot_requires_canonical_lifecycle_projection() {
        let mut value = serde_json::to_value(snapshot_fixture("session"))
            .expect("canonical snapshot json");
        value
            .as_object_mut()
            .expect("snapshot object")
            .remove("lifecycle_projection");

        let error = serde_json::from_value::<SessionSnapshot>(value)
            .expect_err("snapshot without a canonical lifecycle projection must fail");

        assert!(error.to_string().contains("lifecycle_projection"));
    }

    #[test]
    fn typed_session_mutations_reject_extra_fields() {
        let create = CreateSessionRequest {
            command_id: "create:session".to_string(),
            session_id: "session".to_string(),
            creation_command: SessionCommand::CreateSession {
                task_plan: lifecycle::TaskPlan::default(),
            },
            copy_context: false,
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
            initial_task_plan_patch: None,
        };
        let value = serde_json::to_value(create).expect("create json");
        assert!(serde_json::from_value::<CreateSessionRequest>(value).is_ok());
        assert!(serde_json::from_value::<CreateSessionRequest>(json!({
            "session_id": "session",
            "creation_command": {
                "command": "create_session",
                "task_plan": lifecycle::TaskPlan::default()
            },
            "copy_context": false,
            "workspace": "C:/workspace",
            "session_directory": "C:/workspace",
            "name": "Session",
            "created_at": 1,
            "model": null,
            "agent": null,
            "session_type": "coding",
            "kill_processes_on_start": false,
            "validator_enabled": false,
            "force_planning": false,
            "model_variant": null,
            "model_acceleration_enabled": false,
            "disable_permission_restrictions": false,
            "use_last_tool_call_response": false,
            "auto_session_name": true
        }))
        .is_err());
        assert!(
            serde_json::from_value::<ExecuteSessionCommandRequest>(json!({
                "session_id": "session",
                "session_command": { "command": "submit_user_input" }
            }))
            .is_err()
        );
        assert!(
            serde_json::from_value::<ExecuteSessionCommandRequest>(json!({
                "command_id": "command",
                "session_id": "session",
                "session_command": { "command": "submit_user_input" },
                "extra": true
            }))
            .is_err()
        );
        assert!(serde_json::from_value::<UpdateSessionRequest>(json!({
            "command_id": "update",
            "session_id": "session",
            "metadata": { "extra": true },
            "task_plan_patch": null
        }))
        .is_err());
        assert!(serde_json::from_value::<SessionFeedEvent>(json!({
            "event": "session_snapshot_created",
            "snapshot": {
                "session_id": "session",
                "workspace": "C:/workspace",
                "name": "Session",
                "parent_id": null,
                "created_at": 1,
                "updated_at": 2,
                "state": "created",
                "status": "idle",
                "message_count": 0,
                "task_management": {},
                "lifecycle_projection": {
                    "session_id": "session",
                    "state": "created",
                    "parent_id": null,
                    "task_plan": {},
                    "pending_user_inputs": [],
                    "cancelled": false,
                    "runtime_ids": [],
                    "active_runtime_id": null
                },
                "management": {},
                "session": {},
                "todos": [],
                "extra": true
            }
        }))
        .is_err());
    }
}
