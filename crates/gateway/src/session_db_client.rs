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
        match self.call(SessionLogCommand::UpsertSession(UpsertSessionRequest {
            session,
            parent_id,
            messages,
            todos,
        }))? {
            SessionLogResponse::Ok => Ok(()),
            SessionLogResponse::Error { error } => Err(anyhow!(error)),
            other => Err(anyhow!("unexpected session_db response: {other:?}")),
        }
    }

    pub fn list_workspaces(&self) -> Result<Vec<WorkspaceSummary>> {
        match self.call(SessionLogCommand::ListWorkspaces)? {
            SessionLogResponse::Workspaces { workspaces } => Ok(workspaces),
            SessionLogResponse::Error { error } => Err(anyhow!(error)),
            other => Err(anyhow!("unexpected session_db response: {other:?}")),
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
            other => Err(anyhow!("unexpected session_db response: {other:?}")),
        }
    }

    pub fn get_session(&self, session_id: String) -> Result<Option<SessionSnapshot>> {
        match self.call(SessionLogCommand::GetSession(GetSessionRequest {
            session_id,
        }))? {
            SessionLogResponse::Session { session } => Ok(session.map(|session| *session)),
            SessionLogResponse::Error { error } => Err(anyhow!(error)),
            other => Err(anyhow!("unexpected session_db response: {other:?}")),
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
            other => Err(anyhow!("unexpected session_db response: {other:?}")),
        }
    }

    pub fn call(&self, command: SessionLogCommand) -> Result<SessionLogResponse> {
        if session_log::file_queue::is_async_write(&command) {
            session_log::file_queue::enqueue_command(&command)?;
            return Ok(SessionLogResponse::Ok);
        }
        let store = session_log::SessionLogStore::open_default()?;
        let _ = session_log::file_queue::drain_queue(&store, 10_000)?;
        handle_read_command(command, &store)
    }
}

fn handle_read_command(
    command: SessionLogCommand,
    store: &session_log::SessionLogStore,
) -> Result<SessionLogResponse> {
    Ok(match command {
        SessionLogCommand::ListWorkspaces => SessionLogResponse::Workspaces {
            workspaces: store.list_workspaces()?,
        },
        SessionLogCommand::GetSession(payload) => SessionLogResponse::Session {
            session: store.get_session(payload)?.map(Box::new),
        },
        SessionLogCommand::ListSessions(payload) => {
            let (page, sessions) = store.list_sessions(payload)?;
            SessionLogResponse::Sessions { page, sessions }
        }
        SessionLogCommand::ListSessionRecords(payload) => {
            let (page, records) = store.list_session_records(payload)?;
            SessionLogResponse::Records { page, records }
        }
        other => anyhow::bail!("unexpected session_db read command: {other:?}"),
    })
}
