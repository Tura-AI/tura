use super::SessionLogStore;
use anyhow::{anyhow, Result};
use rusqlite::params;
use session_log_contract::{
    CommandCheckpoint, DeleteSessionRequest, DeleteWorkspaceRequest, UpsertSessionRequest,
};

impl SessionLogStore {
    pub fn replay_pending_write_queue(&self) -> Result<u64> {
        let rows = self.with_index_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, event_type, payload_json
                 FROM session_write_queue
                 WHERE status = 'pending'
                 ORDER BY id
                 LIMIT 1000",
            )?;
            let rows = stmt
                .query_map([], |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                    ))
                })?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            Ok(rows)
        })?;

        let mut applied = 0;
        for (id, event_type, payload_json) in rows {
            match self.apply_queue_item(&event_type, &payload_json) {
                Ok(()) => {
                    let now_ms = chrono::Utc::now().timestamp_millis();
                    self.with_index_connection(|conn| {
                        conn.execute(
                            "UPDATE session_write_queue
                             SET status = 'applied', applied_at = ?2, last_error = NULL
                             WHERE id = ?1",
                            params![id, now_ms],
                        )?;
                        Ok(())
                    })?;
                    applied += 1;
                }
                Err(error) => {
                    if is_discardable_queue_error(&error) {
                        tracing::warn!(
                            queue_id = id,
                            event_type,
                            error = %error,
                            "discarding dirty session_write_queue item"
                        );
                        self.with_index_connection(|conn| {
                            conn.execute(
                                "DELETE FROM session_write_queue WHERE id = ?1",
                                params![id],
                            )?;
                            Ok(())
                        })?;
                    } else {
                        self.with_index_connection(|conn| {
                            conn.execute(
                                "UPDATE session_write_queue
                                 SET retry_count = retry_count + 1, last_error = ?2
                                 WHERE id = ?1",
                                params![id, error.to_string()],
                            )?;
                            Ok(())
                        })?;
                        return Err(error);
                    }
                }
            }
        }
        Ok(applied)
    }

    fn apply_queue_item(&self, event_type: &str, payload_json: &str) -> Result<()> {
        match event_type {
            "upsert_session" | "session.upsert" => {
                let request: UpsertSessionRequest = serde_json::from_str(payload_json)?;
                self.upsert_session(request)
            }
            "apply_command_checkpoint" | "command_checkpoint" | "checkpoint.apply" => {
                let checkpoint: CommandCheckpoint = serde_json::from_str(payload_json)?;
                self.apply_command_checkpoint(checkpoint)
            }
            "delete_session" | "session.delete" => {
                let request: DeleteSessionRequest = serde_json::from_str(payload_json)?;
                self.delete_session(request)
            }
            "delete_workspace" | "workspace.delete" => {
                let request: DeleteWorkspaceRequest = serde_json::from_str(payload_json)?;
                self.delete_workspace(request)
            }
            other => Err(anyhow!(
                "unsupported session_write_queue event_type: {other}"
            )),
        }
    }
}

fn is_discardable_queue_error(error: &anyhow::Error) -> bool {
    if error.is::<serde_json::Error>() {
        return true;
    }
    error.chain().any(|cause| {
        let text = cause.to_string();
        text.contains("invalid canonical session state")
            || text.contains("unsupported session_write_queue event_type")
    })
}
