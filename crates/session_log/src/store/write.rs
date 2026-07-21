use super::connection::{init_workspace_db, with_connection};
use super::helpers::{remove_sqlite_files, replay_session_events};
use super::SessionLogStore;
use crate::path::normalize_workspace;
use anyhow::Result;
use lifecycle::{SessionCommand, SessionState};
use rusqlite::{params, OptionalExtension, TransactionBehavior};
use session_log_contract::{
    CommandCheckpoint, DeleteSessionRequest, DeleteWorkspaceRequest, ExecuteSessionCommandRequest,
    MarkSessionInterruptedRequest, SessionFeedEntry, SessionFeedEvent,
};
use std::path::Path;

pub(crate) struct DeleteSessionsOutcome {
    pub(crate) feed_entries: Vec<SessionFeedEntry>,
}

pub(crate) struct MarkSessionInterruptedOutcome {
    pub(crate) interrupted: bool,
    pub(crate) feed_entries: Vec<SessionFeedEntry>,
}

impl SessionLogStore {
    pub fn mark_session_interrupted(&self, request: MarkSessionInterruptedRequest) -> Result<bool> {
        Ok(self
            .mark_session_interrupted_with_feed(request)?
            .interrupted)
    }

    pub fn mark_session_interrupted_by_id(&self, session_id: &str) -> Result<bool> {
        Ok(self
            .interrupt_session_if_recoverable_with_feed(session_id)?
            .interrupted)
    }

    pub(crate) fn mark_session_interrupted_with_feed(
        &self,
        request: MarkSessionInterruptedRequest,
    ) -> Result<MarkSessionInterruptedOutcome> {
        self.interrupt_session_if_recoverable_with_feed(&request.session_id)
    }

    pub fn apply_command_checkpoint(&self, checkpoint: CommandCheckpoint) -> Result<()> {
        let idempotency_key = checkpoint.idempotency_key();
        let checkpoint_type = checkpoint.checkpoint_type.as_str();
        let changes_json = serde_json::to_string(&checkpoint.changes)?;
        let now_ms = chrono::Utc::now().timestamp_millis();
        self.with_index_connection(|conn| {
            let inserted = conn.execute(
                "INSERT INTO command_checkpoints(
                    idempotency_key, session_id, runtime_id, runtime_worker_id,
                    provider_call_id, command_run_id, command_id, event_seq,
                    checkpoint_type, command_type, command_line, output_summary,
                    changes_json, started_at, finished_at, applied_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)
                ON CONFLICT(idempotency_key) DO NOTHING",
                params![
                    idempotency_key,
                    checkpoint.session_id,
                    checkpoint.runtime_id,
                    checkpoint.runtime_worker_id,
                    checkpoint.provider_call_id,
                    checkpoint.command_run_id,
                    checkpoint.command_id,
                    checkpoint.event_seq,
                    checkpoint_type,
                    checkpoint.command_type,
                    checkpoint.command_line,
                    checkpoint.output_summary,
                    changes_json,
                    checkpoint.started_at,
                    checkpoint.finished_at,
                    now_ms,
                ],
            )?;
            if inserted == 0 {
                let identical = conn.query_row(
                    "SELECT EXISTS(
                        SELECT 1 FROM command_checkpoints
                        WHERE idempotency_key = ?1 AND session_id = ?2 AND runtime_id = ?3
                          AND runtime_worker_id IS ?4 AND provider_call_id IS ?5
                          AND command_run_id IS ?6 AND command_id IS ?7 AND event_seq IS ?8
                          AND checkpoint_type = ?9 AND command_type IS ?10
                          AND command_line IS ?11 AND output_summary IS ?12
                          AND changes_json = ?13 AND started_at IS ?14 AND finished_at IS ?15
                    )",
                    params![
                        idempotency_key,
                        checkpoint.session_id,
                        checkpoint.runtime_id,
                        checkpoint.runtime_worker_id,
                        checkpoint.provider_call_id,
                        checkpoint.command_run_id,
                        checkpoint.command_id,
                        checkpoint.event_seq,
                        checkpoint_type,
                        checkpoint.command_type,
                        checkpoint.command_line,
                        checkpoint.output_summary,
                        changes_json,
                        checkpoint.started_at,
                        checkpoint.finished_at,
                    ],
                    |row| row.get::<_, bool>(0),
                )?;
                if !identical {
                    anyhow::bail!(
                        "command checkpoint idempotency key {idempotency_key} was reused with different content"
                    );
                }
            }
            Ok(())
        })
    }

    pub fn mark_running_sessions_interrupted(&self) -> Result<u64> {
        let candidates = self.with_index_connection(|conn| {
            let mut stmt = conn.prepare("SELECT session_id FROM sessions")?;
            let rows = stmt
                .query_map([], |row| row.get::<_, String>(0))?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            Ok(rows)
        })?;

        let mut affected: u64 = 0;
        for session_id in candidates {
            affected += u64::from(
                self.interrupt_session_if_recoverable_with_feed(&session_id)?
                    .interrupted,
            );
        }
        Ok(affected)
    }

    fn interrupt_session_if_recoverable_with_feed(
        &self,
        session_id: &str,
    ) -> Result<MarkSessionInterruptedOutcome> {
        let Some(snapshot) = self.get_session_canonical(session_id)? else {
            return Ok(MarkSessionInterruptedOutcome {
                interrupted: false,
                feed_entries: Vec::new(),
            });
        };
        let projection = snapshot.lifecycle_projection;
        if !projection.state.is_recoverable_running() {
            return Ok(MarkSessionInterruptedOutcome {
                interrupted: false,
                feed_entries: Vec::new(),
            });
        }
        let outcome = self.execute_session_command_with_feed(ExecuteSessionCommandRequest {
            command_id: format!("interrupt:{}:{}", session_id, uuid::Uuid::new_v4()),
            session_id: session_id.to_string(),
            session_command: SessionCommand::InterruptSession,
            message_projection: None,
        })?;
        Ok(MarkSessionInterruptedOutcome {
            interrupted: outcome.result.projection.state == SessionState::Interrupted,
            feed_entries: outcome.feed_entries,
        })
    }

    pub fn delete_session(&self, request: DeleteSessionRequest) -> Result<()> {
        self.delete_session_with_feed(request).map(|_| ())
    }

    pub(crate) fn delete_session_with_feed(
        &self,
        request: DeleteSessionRequest,
    ) -> Result<DeleteSessionsOutcome> {
        let workspace_db_path = self.with_index_connection(|conn| {
            conn.query_row(
                "SELECT workspace_db_path FROM sessions WHERE session_id = ?1",
                params![request.session_id],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(Into::into)
        })?;
        let feed_entries = if let Some(path) = workspace_db_path.as_deref().map(Path::new) {
            if path.exists() {
                with_connection(path, init_workspace_db, |conn| {
                    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
                    let entry = deletion_feed_entry(&tx, &request.session_id)?;
                    tx.execute(
                        "DELETE FROM sessions WHERE session_id = ?1",
                        params![request.session_id],
                    )?;
                    tx.commit()?;
                    Ok(vec![entry])
                })?
            } else {
                vec![deletion_feed_entry_without_database(&request.session_id)]
            }
        } else {
            Vec::new()
        };
        if let Err(error) = self.delete_index_session(&request.session_id) {
            tracing::warn!(
                session_id = %request.session_id,
                error = %error,
                "session deletion committed but derived index cleanup failed"
            );
        }
        Ok(DeleteSessionsOutcome { feed_entries })
    }

    pub fn delete_workspace(&self, request: DeleteWorkspaceRequest) -> Result<()> {
        self.delete_workspace_with_feed(request).map(|_| ())
    }

    pub(crate) fn delete_workspace_with_feed(
        &self,
        request: DeleteWorkspaceRequest,
    ) -> Result<DeleteSessionsOutcome> {
        let workspace = normalize_workspace(&request.workspace);
        let indexed_sessions = self.with_index_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT session_id, workspace_db_path FROM sessions
                 WHERE workspace = ?1 ORDER BY session_id",
            )?;
            let sessions = stmt
                .query_map(params![workspace], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                })?
                .collect::<std::result::Result<Vec<_>, _>>()
                .map_err(anyhow::Error::from)?;
            Ok(sessions)
        })?;
        let mut feed_entries = Vec::with_capacity(indexed_sessions.len());
        let mut db_paths = std::collections::BTreeSet::new();
        for (session_id, path) in &indexed_sessions {
            db_paths.insert(path.clone());
            let path = Path::new(path);
            let entry = if path.is_file() {
                with_connection(path, init_workspace_db, |conn| {
                    let tx = conn.transaction_with_behavior(TransactionBehavior::Deferred)?;
                    let entry = deletion_feed_entry(&tx, session_id)?;
                    tx.commit()?;
                    Ok(entry)
                })?
            } else {
                deletion_feed_entry_without_database(session_id)
            };
            feed_entries.push(entry);
        }
        for path in db_paths {
            remove_sqlite_files(Path::new(&path))?;
        }
        self.with_index_connection(|conn| {
            conn.execute(
                "DELETE FROM sessions WHERE workspace = ?1",
                params![workspace],
            )?;
            Ok(())
        })?;
        Ok(DeleteSessionsOutcome { feed_entries })
    }
}

fn deletion_feed_entry(
    tx: &rusqlite::Transaction<'_>,
    session_id: &str,
) -> Result<SessionFeedEntry> {
    replay_session_events(tx, session_id)?;
    let cursor = tx.query_row(
        "SELECT COALESCE(MAX(cursor) + 1, 1) FROM session_feed_events WHERE session_id = ?1",
        params![session_id],
        |row| row.get::<_, u64>(0),
    )?;
    Ok(SessionFeedEntry {
        session_id: session_id.to_string(),
        cursor,
        runtime_id: None,
        event_id: format!("delete:{session_id}:{cursor}"),
        event: SessionFeedEvent::SessionDeleted {},
    })
}

fn deletion_feed_entry_without_database(session_id: &str) -> SessionFeedEntry {
    SessionFeedEntry {
        session_id: session_id.to_string(),
        cursor: u64::MAX,
        runtime_id: None,
        event_id: format!("delete:{session_id}:missing-database"),
        event: SessionFeedEvent::SessionDeleted {},
    }
}
