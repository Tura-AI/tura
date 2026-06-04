use std::io::Read;

use crate::{
    GetSessionRequest, ListSessionRecordsRequest, ListSessionsRequest, SessionLogCommand,
    SessionLogResponse, SessionLogStore, UpsertSessionRequest,
};

pub fn run() -> anyhow::Result<()> {
    let command = std::env::args()
        .nth(1)
        .ok_or_else(|| anyhow::anyhow!("session_log requires a command"))?;
    let store = SessionLogStore::open_default()?;
    let response = match command.as_str() {
        "upsert-session" => {
            let payload: UpsertSessionRequest = read_json()?;
            store.upsert_session(payload)?;
            SessionLogResponse::Ok
        }
        "list-workspaces" => SessionLogResponse::Workspaces {
            workspaces: store.list_workspaces()?,
        },
        "get-session" => {
            let payload: GetSessionRequest = read_json()?;
            SessionLogResponse::Session {
                session: store.get_session(payload)?,
            }
        }
        "list-sessions" => {
            let payload: ListSessionsRequest = read_json()?;
            let (page, sessions) = store.list_sessions(payload)?;
            SessionLogResponse::Sessions { page, sessions }
        }
        "list-session-records" => {
            let payload: ListSessionRecordsRequest = read_json()?;
            let (page, records) = store.list_session_records(payload)?;
            SessionLogResponse::Records { page, records }
        }
        "serve-once" => handle_command(read_json::<SessionLogCommand>()?, &store)?,
        other => anyhow::bail!("unknown session_log command: {other}"),
    };
    println!("{}", serde_json::to_string(&response)?);
    Ok(())
}

pub fn handle_raw_command(raw: &str) -> anyhow::Result<SessionLogResponse> {
    let store = SessionLogStore::open_default()?;
    handle_command(serde_json::from_str(raw.trim())?, &store)
}

fn read_json<T: serde::de::DeserializeOwned>() -> anyhow::Result<T> {
    let mut raw = String::new();
    std::io::stdin().read_to_string(&mut raw)?;
    serde_json::from_str(raw.trim()).map_err(Into::into)
}

fn handle_command(
    command: SessionLogCommand,
    store: &SessionLogStore,
) -> anyhow::Result<SessionLogResponse> {
    Ok(match command {
        SessionLogCommand::UpsertSession(payload) => {
            store.upsert_session(payload)?;
            SessionLogResponse::Ok
        }
        SessionLogCommand::ListWorkspaces => SessionLogResponse::Workspaces {
            workspaces: store.list_workspaces()?,
        },
        SessionLogCommand::GetSession(payload) => SessionLogResponse::Session {
            session: store.get_session(payload)?,
        },
        SessionLogCommand::ListSessions(payload) => {
            let (page, sessions) = store.list_sessions(payload)?;
            SessionLogResponse::Sessions { page, sessions }
        }
        SessionLogCommand::ListSessionRecords(payload) => {
            let (page, records) = store.list_session_records(payload)?;
            SessionLogResponse::Records { page, records }
        }
    })
}
