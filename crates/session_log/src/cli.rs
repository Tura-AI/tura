use std::io::Read;

use crate::{
    file_queue, GetSessionRequest, ListSessionRecordsRequest, ListSessionsRequest,
    SessionLogResponse, SessionLogStore, UpsertSessionRequest,
};

pub fn run() -> anyhow::Result<()> {
    let command = std::env::args()
        .nth(1)
        .ok_or_else(|| anyhow::anyhow!("session_log requires a command"))?;
    let store = SessionLogStore::open_default()?;
    let _ = file_queue::drain_queue(&store, 10_000)?;
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
                session: store.get_session(payload)?.map(Box::new),
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
        other => anyhow::bail!("unknown session_log command: {other}"),
    };
    println!("{}", serde_json::to_string(&response)?);
    Ok(())
}

fn read_json<T: serde::de::DeserializeOwned>() -> anyhow::Result<T> {
    let mut raw = String::new();
    std::io::stdin().read_to_string(&mut raw)?;
    serde_json::from_str(raw.trim()).map_err(Into::into)
}
