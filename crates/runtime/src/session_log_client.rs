use anyhow::{anyhow, Result};
use std::time::Instant;

use crate::profile_timings;
use session_log_contract::{
    CommandCheckpoint, ContextSlice, GetSessionRequest, ListSessionRecordsRequest,
    ListSessionsRequest, Page, PersistSessionDeltaRequest, ReadContextSliceRequest,
    SessionLogCommand, SessionLogResponse, SessionRecord, SessionSnapshot, WorkspaceSummary,
};

#[derive(Debug, Clone, Default)]
pub struct SessionLogClient;

impl SessionLogClient {
    pub fn discover() -> Result<Self> {
        Ok(Self)
    }

    pub(crate) fn call_typed(
        &self,
        command: SessionLogCommand,
    ) -> Result<SessionLogResponse, String> {
        self.call(command).map_err(|error| error.to_string())
    }

    pub(crate) fn call_typed_sync(
        &self,
        command: SessionLogCommand,
    ) -> Result<SessionLogResponse, String> {
        if !session_log_contract::client::service_is_running() {
            return Err(
                "session_db service is not running; start the per-home tura_router/tura_session_db owner before accessing session data"
                    .to_string(),
            );
        }
        session_log_contract::client::call_service(&command).map_err(|error| error.to_string())
    }

    pub(crate) fn persist_session_delta(
        &self,
        request: PersistSessionDeltaRequest,
    ) -> Result<(u64, u64), String> {
        match self.call_typed_sync(SessionLogCommand::PersistSessionDelta(Box::new(request)))? {
            SessionLogResponse::SessionDeltaPersisted {
                next_sequence,
                next_management_sequence,
            } => Ok((next_sequence, next_management_sequence)),
            SessionLogResponse::Error { error } => {
                Err(format!("session_log persist_session_delta failed: {error}"))
            }
            other => Err(format!(
                "unexpected session_log response for persist_session_delta: {other:?}"
            )),
        }
    }

    pub(crate) fn read_context_slice(
        &self,
        session_id: String,
        max_estimated_tokens: u64,
    ) -> Result<ContextSlice, String> {
        match self.call_typed_sync(SessionLogCommand::ReadContextSlice(
            ReadContextSliceRequest {
                session_id,
                max_estimated_tokens,
            },
        ))? {
            SessionLogResponse::ContextSlice { context } => Ok(context),
            SessionLogResponse::Error { error } => {
                Err(format!("session_log read_context_slice failed: {error}"))
            }
            other => Err(format!(
                "unexpected session_log response for read_context_slice: {other:?}"
            )),
        }
    }

    pub fn apply_command_checkpoint(&self, checkpoint: CommandCheckpoint) -> Result<()> {
        match self.call(SessionLogCommand::ApplyCommandCheckpoint(Box::new(
            checkpoint,
        )))? {
            SessionLogResponse::Ok => Ok(()),
            SessionLogResponse::Error { error } => {
                Err(session_log_error("apply_command_checkpoint", error))
            }
            other => Err(unexpected_session_log_response(
                "apply_command_checkpoint",
                other,
            )),
        }
    }

    pub fn list_workspaces(&self) -> Result<Vec<WorkspaceSummary>> {
        match self.call(SessionLogCommand::ListWorkspaces)? {
            SessionLogResponse::Workspaces { workspaces } => Ok(workspaces),
            SessionLogResponse::Error { error } => Err(session_log_error("list_workspaces", error)),
            other => Err(unexpected_session_log_response("list_workspaces", other)),
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
            SessionLogResponse::Error { error } => Err(session_log_error("list_sessions", error)),
            other => Err(unexpected_session_log_response("list_sessions", other)),
        }
    }

    pub fn get_session(&self, session_id: String) -> Result<Option<SessionSnapshot>> {
        match self.call(SessionLogCommand::GetSession(GetSessionRequest {
            session_id,
        }))? {
            SessionLogResponse::Session { session } => Ok(session.map(|session| *session)),
            SessionLogResponse::Error { error } => Err(session_log_error("get_session", error)),
            other => Err(unexpected_session_log_response("get_session", other)),
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
            SessionLogResponse::Error { error } => {
                Err(session_log_error("list_session_records", error))
            }
            other => Err(unexpected_session_log_response(
                "list_session_records",
                other,
            )),
        }
    }

    fn call(&self, command: SessionLogCommand) -> Result<SessionLogResponse> {
        let command_name = session_log_command_name(&command);
        let async_write = session_log_contract::client::is_async_write(&command);
        let command_payload = if async_write || profile_timings::enabled() {
            Some(serde_json::to_vec(&command)?)
        } else {
            None
        };
        let command_bytes = command_payload
            .as_ref()
            .map(|bytes| bytes.len())
            .unwrap_or(0);
        if async_write {
            let enqueue_start = Instant::now();
            if let Some(payload) = command_payload.as_deref() {
                session_log_contract::client::enqueue_serialized_command(payload)?;
            } else {
                session_log_contract::client::enqueue_command(&command)?;
            }
            profile_timings::log_elapsed(
                "session_log_client.enqueue_async_write",
                enqueue_start,
                serde_json::json!({
                    "command": command_name,
                    "command_bytes": command_bytes,
                }),
            );
            return Ok(SessionLogResponse::Ok);
        }
        let service_check_start = Instant::now();
        let service_running = session_log_contract::client::service_is_running();
        profile_timings::log_elapsed(
            "session_log_client.service_is_running",
            service_check_start,
            serde_json::json!({
                "command": command_name,
                "async_write": async_write,
                "command_bytes": command_bytes,
                "service_running": service_running,
            }),
        );
        if service_running {
            let ipc_start = Instant::now();
            let ipc_result = session_log_contract::client::call_service(&command);
            profile_timings::log_elapsed(
                "session_log_client.call_service",
                ipc_start,
                serde_json::json!({
                    "command": command_name,
                    "async_write": async_write,
                    "command_bytes": command_bytes,
                    "success": ipc_result.is_ok(),
                }),
            );
            match ipc_result {
                Ok(response) => return Ok(response),
                Err(error) => return Err(error),
            }
        }
        Err(anyhow!(
            "session_db service is not running; start the per-home tura_router/tura_session_db owner before reading session data"
        ))
    }
}

fn session_log_command_name(command: &SessionLogCommand) -> &'static str {
    match command {
        SessionLogCommand::Health => "health",
        SessionLogCommand::CreateSession(_) => "create_session",
        SessionLogCommand::ExecuteSessionCommand(_) => "execute_session_command",
        SessionLogCommand::UpdateSession(_) => "update_session",
        SessionLogCommand::UpdateSessionTodos(_) => "update_session_todos",
        SessionLogCommand::RegisterRuntime(_) => "register_runtime",
        SessionLogCommand::ActivateRuntimeLease(_) => "activate_runtime_lease",
        SessionLogCommand::CommitRuntimeEvent(_) => "commit_runtime_event",
        SessionLogCommand::AppendSessionFeedEvent(_) => "append_session_feed_event",
        SessionLogCommand::ReadSessionFeed(_) => "read_session_feed",
        SessionLogCommand::SubscribeSessionFeed => "subscribe_session_feed",
        SessionLogCommand::ReplayRuntime(_) => "replay_runtime",
        SessionLogCommand::PersistSessionDelta(_) => "persist_session_delta",
        SessionLogCommand::ReadContextSlice(_) => "read_context_slice",
        SessionLogCommand::ApplyCommandCheckpoint(_) => "apply_command_checkpoint",
        SessionLogCommand::GetSession(_) => "get_session",
        SessionLogCommand::ListWorkspaces => "list_workspaces",
        SessionLogCommand::ListSessions(_) => "list_sessions",
        SessionLogCommand::ListSessionSummaries(_) => "list_session_summaries",
        SessionLogCommand::ListSessionRecords(_) => "list_session_records",
        SessionLogCommand::MarkSessionInterrupted(_) => "mark_session_interrupted",
        SessionLogCommand::DeleteSession(_) => "delete_session",
        SessionLogCommand::DeleteWorkspace(_) => "delete_workspace",
        SessionLogCommand::Shutdown => "shutdown",
    }
}

fn session_log_error(operation: &str, error: String) -> anyhow::Error {
    anyhow!("session_log {operation} failed: {error}")
}

fn unexpected_session_log_response(operation: &str, response: SessionLogResponse) -> anyhow::Error {
    anyhow!("unexpected session_log response for {operation}: {response:?}")
}
