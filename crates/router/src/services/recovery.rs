//! Router restart recovery hooks.
//!
//! First-phase recovery starts session_db, asks it to replay the durable queue,
//! and treats non-reattachable runtime work as interrupted.

use anyhow::Result;
use serde_json::json;

use super::session_db::SessionDbService;

pub fn recover_after_start(session_db: &SessionDbService) -> Result<serde_json::Value> {
    let session_db_status = session_db.start()?;
    Ok(json!({
        "session_db": session_db_status,
        "queue_replay": "requested",
        "runtime_reattach": false,
        "orphan_policy": "mark_interrupted"
    }))
}
