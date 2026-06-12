//! Direct client for the session DB service data path.
//!
//! Gateway/session reads and writes use this client directly. Router is only
//! responsible for service lifecycle and is intentionally not on this path.

use anyhow::{anyhow, Result};
use serde_json::Value;
use session_log::{
    GetSessionRequest, ListSessionRecordsRequest, ListSessionsRequest, Page, SessionLogCommand,
    SessionLogResponse, SessionRecord, SessionSnapshot, UpsertSessionRequest, WorkspaceSummary,
};

#[derive(Debug, Clone, Default)]
pub struct SessionDbClient;

impl SessionDbClient {
    pub fn discover() -> Result<Self> {
        Ok(Self)
    }

    pub fn upsert_session(
        &self,
        session: Value,
        parent_id: Option<String>,
        messages: Vec<Value>,
        todos: Vec<Value>,
    ) -> Result<()> {
        let response = self.call(SessionLogCommand::UpsertSession(UpsertSessionRequest {
            session,
            parent_id,
            messages,
            todos,
        }))?;
        ok_response(response, "upsert_session")
    }

    pub fn list_workspaces(&self) -> Result<Vec<WorkspaceSummary>> {
        workspaces_response(self.call(SessionLogCommand::ListWorkspaces)?)
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
        if session_log::file_queue::is_async_write(&command) {
            session_log::file_queue::enqueue_command(&command)?;
            return Ok(SessionLogResponse::Ok);
        }
        Err(anyhow!(
            "session_db service is not running; start the per-home tura_router/tura_session_db owner before reading session data"
        ))
    }
}

fn ok_response(response: SessionLogResponse, operation: &str) -> Result<()> {
    match response {
        SessionLogResponse::Ok => Ok(()),
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
        ok_response, records_response, session_response, sessions_response, workspaces_response,
    };
    use serde_json::json;
    use session_log::{Page, SessionLogResponse, SessionSnapshot, WorkspaceSummary};

    fn snapshot(session_id: &str) -> SessionSnapshot {
        SessionSnapshot {
            session_id: session_id.to_string(),
            workspace: "workspace".to_string(),
            name: Some("Session".to_string()),
            parent_id: None,
            created_at: 1,
            updated_at: 2,
            state: Some("running".to_string()),
            status: Some("running".to_string()),
            message_count: 3,
            task_management: json!({}),
            management: json!({}),
            session: json!({ "session_id": session_id }),
            todos: Vec::new(),
        }
    }

    #[test]
    fn response_mappers_accept_expected_success_variants() {
        ok_response(SessionLogResponse::Ok, "upsert_session")
            .expect("ok response should map to success");

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
        let error = ok_response(
            SessionLogResponse::Workspaces {
                workspaces: Vec::new(),
            },
            "upsert_session",
        )
        .expect_err("wrong response variant should fail");

        assert!(
            error
                .to_string()
                .contains("unexpected session_db response for upsert_session"),
            "unexpected response error should include operation context: {error}"
        );
    }
}
