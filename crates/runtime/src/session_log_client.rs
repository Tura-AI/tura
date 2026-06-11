use anyhow::{anyhow, Result};
use serde_json::Value;
use session_log::{
    CommandCheckpoint, GetSessionRequest, ListSessionRecordsRequest, ListSessionsRequest,
    SessionLogCommand, SessionLogResponse, UpsertSessionRequest,
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

    pub fn apply_command_checkpoint(&self, checkpoint: CommandCheckpoint) -> Result<()> {
        match self.call(SessionLogCommand::ApplyCommandCheckpoint(Box::new(
            checkpoint,
        )))? {
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
        if session_log::ipc::service_is_running() {
            match session_log::ipc::call_service(&command) {
                Ok(response) => return Ok(response),
                Err(error) if session_log::file_queue::is_async_write(&command) => {
                    tracing::warn!(
                        error = %error,
                        "session_db service call failed for async write; enqueueing for later drain"
                    );
                    session_log::file_queue::enqueue_command(&command)?;
                    return Ok(SessionLogResponse::Ok);
                }
                Err(error) => return Err(error),
            }
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
