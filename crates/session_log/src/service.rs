//! Session DB service entry points.
//!
//! This process is the only owner of local PostgreSQL connections. Router may
//! start and monitor it, but gateway/runtime data calls target this service.

use anyhow::Result;
use serde_json::json;

use crate::{cli, SessionLogResponse, SessionLogStore};

pub fn handle_command_json(raw: &str) -> Result<SessionLogResponse> {
    cli::handle_raw_command(raw)
}

pub fn run_one_shot() -> Result<()> {
    use std::io::Read;
    let mut raw = String::new();
    std::io::stdin().read_to_string(&mut raw)?;
    let response = handle_command_json(&raw)?;
    println!("{}", serde_json::to_string(&response)?);
    Ok(())
}

pub fn run_lifecycle_service() -> Result<()> {
    use std::io::{BufRead, Write};
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    let store = SessionLogStore::open_default()?;
    let replayed_queue_items = store.replay_pending_write_queue()?;
    let interrupted_running_sessions = store.mark_running_sessions_interrupted()?;
    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let response = json!({
            "ok": true,
            "service": "session_db",
            "status": "running",
            "replayed_queue_items": replayed_queue_items,
            "interrupted_running_sessions": interrupted_running_sessions
        });
        writeln!(stdout, "{}", serde_json::to_string(&response)?)?;
        stdout.flush()?;
    }
    Ok(())
}
