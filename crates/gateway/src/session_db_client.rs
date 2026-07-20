//! Direct client for the session DB service data path.
//!
//! Gateway/session reads and typed lifecycle commands use this client directly.
//! Runtime owns whole-session checkpoints; Gateway has one narrow projection
//! payload path for forked conversation records and todos.

use anyhow::{anyhow, Result};
use lifecycle::SessionCommand;
use session_log_contract::{
    CreateSessionRequest, ExecuteSessionCommandRequest, GetSessionRequest,
    ListSessionRecordsRequest, ListSessionsRequest, Page, PersistSessionPayloadRequest,
    SessionCommandResult, SessionLogCommand, SessionLogResponse, SessionRecord, SessionSnapshot,
    SessionSummary, WorkspaceSummary,
};

#[derive(Debug, Clone, Default)]
pub struct SessionDbClient;

impl SessionDbClient {
    pub fn discover() -> Result<Self> {
        Ok(Self)
    }

    pub fn list_workspaces(&self) -> Result<Vec<WorkspaceSummary>> {
        workspaces_response(self.call(SessionLogCommand::ListWorkspaces)?)
    }

    pub fn create_session(&self, request: CreateSessionRequest) -> Result<SessionCommandResult> {
        session_command_response(
            "create_session",
            self.call(SessionLogCommand::CreateSession(request))?,
        )
    }

    pub fn execute_session_command(
        &self,
        session_id: String,
        command: SessionCommand,
    ) -> Result<SessionCommandResult> {
        session_command_response(
            "execute_session_command",
            self.call(SessionLogCommand::ExecuteSessionCommand(
                ExecuteSessionCommandRequest {
                    session_id,
                    session_command: command,
                },
            ))?,
        )
    }

    pub fn persist_session_payload(&self, request: PersistSessionPayloadRequest) -> Result<()> {
        match self.call(SessionLogCommand::PersistSessionPayload(request))? {
            SessionLogResponse::Ok => Ok(()),
            SessionLogResponse::Error { error } => {
                Err(service_error("persist_session_payload", error))
            }
            other => Err(unexpected_response("persist_session_payload", other)),
        }
    }

    pub fn list_sessions(
        &self,
        workspace: String,
        page: u64,
        page_size: u64,
    ) -> Result<(Page, Vec<SessionSnapshot>)> {
        sessions_response(
            self.call(SessionLogCommand::ListSessions(ListSessionsRequest {
                workspace,
                page,
                page_size,
            }))?,
        )
    }

    pub fn list_session_summaries(
        &self,
        workspace: String,
        page: u64,
        page_size: u64,
    ) -> Result<(Page, Vec<SessionSummary>)> {
        session_summaries_response(self.call(SessionLogCommand::ListSessionSummaries(
            ListSessionsRequest {
                workspace,
                page,
                page_size,
            },
        ))?)
    }

    pub fn get_session(&self, session_id: String) -> Result<Option<SessionSnapshot>> {
        session_response(self.call(SessionLogCommand::GetSession(GetSessionRequest {
            session_id,
        }))?)
    }

    pub fn list_session_records(
        &self,
        session_id: String,
        page: u64,
        page_size: u64,
    ) -> Result<(Page, Vec<SessionRecord>)> {
        records_response(self.call(SessionLogCommand::ListSessionRecords(
            ListSessionRecordsRequest {
                session_id,
                page,
                page_size,
            },
        ))?)
    }

    pub fn call(&self, command: SessionLogCommand) -> Result<SessionLogResponse> {
        if !is_gateway_command(&command) {
            return Err(anyhow!(
                "gateway session_db client only accepts queries and typed session commands"
            ));
        }
        self.call_service_command(command)
    }

    fn call_service_command(&self, command: SessionLogCommand) -> Result<SessionLogResponse> {
        if tokio::runtime::Handle::try_current().is_ok() {
            return std::thread::spawn(move || Self::call_blocking(command))
                .join()
                .map_err(|_| anyhow!("session_db client worker thread panicked"))?;
        }
        Self::call_blocking(command)
    }

    fn call_blocking(command: SessionLogCommand) -> Result<SessionLogResponse> {
        if session_log::ipc::service_is_running() {
            return session_log::ipc::call_service(&command);
        }
        Err(anyhow!(
            "session_db service is not running; start the per-home tura_router/tura_session_db owner before reading session data"
        ))
    }
}

fn is_gateway_command(command: &SessionLogCommand) -> bool {
    matches!(
        command,
        SessionLogCommand::Health
            | SessionLogCommand::CreateSession(_)
            | SessionLogCommand::ExecuteSessionCommand(_)
            | SessionLogCommand::PersistSessionPayload(_)
            | SessionLogCommand::GetSession(_)
            | SessionLogCommand::ListWorkspaces
            | SessionLogCommand::ListSessions(_)
            | SessionLogCommand::ListSessionSummaries(_)
            | SessionLogCommand::ListSessionRecords(_)
            | SessionLogCommand::Shutdown
    )
}

fn session_command_response(
    operation: &str,
    response: SessionLogResponse,
) -> Result<SessionCommandResult> {
    match response {
        SessionLogResponse::SessionCommandApplied { result } => Ok(*result),
        SessionLogResponse::Error { error } => Err(service_error(operation, error)),
        other => Err(unexpected_response(operation, other)),
    }
}

fn workspaces_response(response: SessionLogResponse) -> Result<Vec<WorkspaceSummary>> {
    match response {
        SessionLogResponse::Workspaces { workspaces } => Ok(workspaces),
        SessionLogResponse::Error { error } => Err(service_error("list_workspaces", error)),
        other => Err(unexpected_response("list_workspaces", other)),
    }
}

fn sessions_response(response: SessionLogResponse) -> Result<(Page, Vec<SessionSnapshot>)> {
    match response {
        SessionLogResponse::Sessions { page, sessions } => Ok((page, sessions)),
        SessionLogResponse::Error { error } => Err(service_error("list_sessions", error)),
        other => Err(unexpected_response("list_sessions", other)),
    }
}

fn session_summaries_response(response: SessionLogResponse) -> Result<(Page, Vec<SessionSummary>)> {
    match response {
        SessionLogResponse::SessionSummaries { page, sessions } => Ok((page, sessions)),
        SessionLogResponse::Error { error } => Err(service_error("list_session_summaries", error)),
        other => Err(unexpected_response("list_session_summaries", other)),
    }
}

fn session_response(response: SessionLogResponse) -> Result<Option<SessionSnapshot>> {
    match response {
        SessionLogResponse::Session { session } => Ok(session.map(|session| *session)),
        SessionLogResponse::Error { error } => Err(service_error("get_session", error)),
        other => Err(unexpected_response("get_session", other)),
    }
}

fn records_response(response: SessionLogResponse) -> Result<(Page, Vec<SessionRecord>)> {
    match response {
        SessionLogResponse::Records { page, records } => Ok((page, records)),
        SessionLogResponse::Error { error } => Err(service_error("list_session_records", error)),
        other => Err(unexpected_response("list_session_records", other)),
    }
}

fn service_error(operation: &str, error: String) -> anyhow::Error {
    anyhow!("session_db {operation} failed: {error}")
}

fn unexpected_response(operation: &str, response: SessionLogResponse) -> anyhow::Error {
    anyhow!("unexpected session_db response for {operation}: {response:?}")
}

#[cfg(test)]
mod tests {
    use super::{
        is_gateway_command, records_response, session_response, sessions_response,
        workspaces_response,
    };
    use lifecycle::SessionCommand;
    use serde_json::json;
    use session_log_contract::{
        CommandCheckpoint, DeleteSessionRequest, MarkSessionInterruptedRequest, Page,
        SessionLogCommand, SessionLogResponse, SessionSnapshot, UpsertSessionRequest,
        WorkspaceSummary,
    };

    fn snapshot(session_id: &str) -> SessionSnapshot {
        SessionSnapshot {
            session_id: session_id.to_string(),
            workspace: "workspace".to_string(),
            name: Some("Session".to_string()),
            parent_id: None,
            created_at: 1,
            updated_at: 2,
            last_user_message_at: Some(1),
            state: Some("running".to_string()),
            status: Some("running".to_string()),
            message_count: 3,
            task_management: json!({}),
            lifecycle_projection: None,
            management: json!({}),
            session: json!({ "session_id": session_id }),
            todos: Vec::new(),
        }
    }

    #[test]
    fn response_mappers_accept_expected_success_variants() {
        let workspaces = workspaces_response(SessionLogResponse::Workspaces {
            workspaces: vec![WorkspaceSummary {
                directory: "workspace".to_string(),
                session_count: 1,
                last_updated_at: 2,
            }],
        })
        .expect("workspaces response should map");
        assert_eq!(workspaces[0].directory, "workspace");

        let (page, sessions) = sessions_response(SessionLogResponse::Sessions {
            page: Page {
                page: 1,
                page_size: 2,
                total: 3,
            },
            sessions: vec![snapshot("session-1")],
        })
        .expect("sessions response should map");
        assert_eq!(page.total, 3);
        assert_eq!(sessions[0].session_id, "session-1");

        let session = session_response(SessionLogResponse::Session {
            session: Some(Box::new(snapshot("session-2"))),
        })
        .expect("session response should map")
        .expect("session should be present");
        assert_eq!(session.session_id, "session-2");

        let (records_page, records) = records_response(SessionLogResponse::Records {
            page: Page::default(),
            records: Vec::new(),
        })
        .expect("records response should map");
        assert_eq!(records_page.total, 0);
        assert!(records.is_empty());
    }

    #[test]
    fn response_mappers_preserve_service_error_text() {
        let error = workspaces_response(SessionLogResponse::Error {
            error: "sqlite busy".to_string(),
        })
        .expect_err("service errors should be returned");

        assert_eq!(
            error.to_string(),
            "session_db list_workspaces failed: sqlite busy"
        );
    }

    #[test]
    fn response_mappers_report_unexpected_variant_with_operation_context() {
        let error = workspaces_response(SessionLogResponse::Ok)
            .expect_err("wrong response variant should fail");

        assert!(
            error
                .to_string()
                .contains("unexpected session_db response for list_workspaces"),
            "unexpected response error should include operation context: {error}"
        );
    }

    #[test]
    fn gateway_session_db_client_only_accepts_queries_and_typed_mutations() {
        assert!(is_gateway_command(&SessionLogCommand::ListWorkspaces));
        assert!(is_gateway_command(
            &SessionLogCommand::ExecuteSessionCommand(
                session_log_contract::ExecuteSessionCommandRequest {
                    session_id: "session-1".to_string(),
                    session_command: SessionCommand::SubmitUserInput,
                }
            )
        ));
        assert!(!is_gateway_command(&SessionLogCommand::UpsertSession(
            UpsertSessionRequest {
                session: json!({}),
                parent_id: None,
                messages: Vec::new(),
                todos: Vec::new(),
            }
        )));
        assert!(!is_gateway_command(&SessionLogCommand::DeleteSession(
            DeleteSessionRequest {
                session_id: "session-1".to_string()
            }
        )));
        assert!(!is_gateway_command(
            &SessionLogCommand::MarkSessionInterrupted(MarkSessionInterruptedRequest {
                session_id: "session-1".to_string()
            })
        ));
        assert!(!is_gateway_command(
            &SessionLogCommand::ApplyCommandCheckpoint(Box::new(CommandCheckpoint {
                session_id: "session-1".to_string(),
                turn_id: "turn-1".to_string(),
                runtime_worker_id: None,
                provider_call_id: None,
                command_run_id: None,
                command_id: None,
                event_seq: None,
                command_type: None,
                command_line: None,
                status: "turn_started".to_string(),
                output_summary: None,
                changes: json!({}),
                started_at: None,
                finished_at: None,
            }))
        ));
    }
}
