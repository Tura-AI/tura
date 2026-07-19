use std::io::Read;

use crate::{file_queue, ipc, SessionLogStore};
use session_log_contract::{
    DeleteSessionRequest, DeleteWorkspaceRequest, GetSessionRequest, ListSessionRecordsRequest,
    ListSessionsRequest, MarkSessionInterruptedRequest, SessionLogCommand, SessionLogResponse,
};

/// Developer query CLI for the session DB.
///
/// By default this routes through the running `tura_session_db` service so it
/// uses the same data path as gateway/runtime. Pass `--admin` to bypass the
/// service and inspect the SQLite store directly.
pub fn run() -> anyhow::Result<()> {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let admin = take_flag(&mut args, "--admin");
    let command = args
        .first()
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("session_log requires a command"))?;

    let parsed = match command.as_str() {
        "upsert-session" => SessionLogCommand::UpsertSession(read_json()?),
        "list-workspaces" => SessionLogCommand::ListWorkspaces,
        "get-session" => SessionLogCommand::GetSession(read_json::<GetSessionRequest>()?),
        "list-sessions" => SessionLogCommand::ListSessions(read_json::<ListSessionsRequest>()?),
        "list-session-records" => {
            SessionLogCommand::ListSessionRecords(read_json::<ListSessionRecordsRequest>()?)
        }
        "mark-session-interrupted" => {
            SessionLogCommand::MarkSessionInterrupted(read_json::<MarkSessionInterruptedRequest>()?)
        }
        "delete-session" => SessionLogCommand::DeleteSession(read_json::<DeleteSessionRequest>()?),
        "delete-workspace" => {
            SessionLogCommand::DeleteWorkspace(read_json::<DeleteWorkspaceRequest>()?)
        }
        other => anyhow::bail!("unknown session_log command: {other}"),
    };

    let response = if !admin && ipc::service_is_running() {
        ipc::call_service(&parsed)?
    } else {
        // Direct path: tests/admin inspection only.
        let store = SessionLogStore::open_default()?;
        let _ = file_queue::drain_queue(&store, 10_000)?;
        ipc::dispatch_command(&store, parsed)
    };

    println!("{}", serde_json::to_string(&response)?);
    if let SessionLogResponse::Error { error } = &response {
        anyhow::bail!("{error}");
    }
    Ok(())
}

fn take_flag(args: &mut Vec<String>, flag: &str) -> bool {
    if let Some(index) = args.iter().position(|arg| arg == flag) {
        args.remove(index);
        true
    } else {
        false
    }
}

fn read_json<T: serde::de::DeserializeOwned>() -> anyhow::Result<T> {
    let mut raw = String::new();
    std::io::stdin().read_to_string(&mut raw)?;
    serde_json::from_str(raw.trim()).map_err(Into::into)
}
