use anyhow::{anyhow, Result};
use serde_json::Value;
use session_log::{
    GetSessionRequest, ListSessionRecordsRequest, ListSessionsRequest, SessionLogCommand,
    SessionLogResponse, UpsertSessionRequest,
};

pub use session_log::{Page, SessionRecord, SessionSnapshot, WorkspaceSummary};

#[derive(Debug, Clone, Default)]
pub struct SessionLogClient;

impl SessionLogClient {
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
        match self.call(SessionLogCommand::UpsertSession(UpsertSessionRequest {
            session,
            parent_id,
            messages,
            todos,
        }))? {
            SessionLogResponse::Ok => Ok(()),
            SessionLogResponse::Error { error } => Err(anyhow!(error)),
            other => Err(anyhow!("unexpected session_log response: {other:?}")),
        }
    }

    pub fn list_workspaces(&self) -> Result<Vec<WorkspaceSummary>> {
        match self.call(SessionLogCommand::ListWorkspaces)? {
            SessionLogResponse::Workspaces { workspaces } => Ok(workspaces),
            SessionLogResponse::Error { error } => Err(anyhow!(error)),
            other => Err(anyhow!("unexpected session_log response: {other:?}")),
        }
    }

    pub fn list_sessions(
        &self,
        workspace: String,
        page: u64,
        page_size: u64,
    ) -> Result<(Page, Vec<SessionSnapshot>)> {
        match self.call(SessionLogCommand::ListSessions(ListSessionsRequest {
            workspace,
            page,
            page_size,
        }))? {
            SessionLogResponse::Sessions { page, sessions } => Ok((page, sessions)),
            SessionLogResponse::Error { error } => Err(anyhow!(error)),
            other => Err(anyhow!("unexpected session_log response: {other:?}")),
        }
    }

    pub fn get_session(&self, session_id: String) -> Result<Option<SessionSnapshot>> {
        match self.call(SessionLogCommand::GetSession(GetSessionRequest {
            session_id,
        }))? {
            SessionLogResponse::Session { session } => Ok(session.map(|session| *session)),
            SessionLogResponse::Error { error } => Err(anyhow!(error)),
            other => Err(anyhow!("unexpected session_log response: {other:?}")),
        }
    }

    pub fn list_session_records(
        &self,
        session_id: String,
        page: u64,
        page_size: u64,
    ) -> Result<(Page, Vec<SessionRecord>)> {
        match self.call(SessionLogCommand::ListSessionRecords(
            ListSessionRecordsRequest {
                session_id,
                page,
                page_size,
            },
        ))? {
            SessionLogResponse::Records { page, records } => Ok((page, records)),
            SessionLogResponse::Error { error } => Err(anyhow!(error)),
            other => Err(anyhow!("unexpected session_log response: {other:?}")),
        }
    }

    fn call(&self, command: SessionLogCommand) -> Result<SessionLogResponse> {
        let raw = serde_json::to_string(&command)?;
        std::thread::spawn(move || tura_router::session_log_forward::handle_session_log_json(&raw))
            .join()
            .map_err(|_| anyhow!("session_log worker thread panicked"))?
    }
}
