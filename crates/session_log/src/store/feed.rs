use super::SessionLogStore;
use anyhow::{Context, Result};
use rusqlite::{params, OptionalExtension, TransactionBehavior};
use session_log_contract::{
    AppendSessionFeedEventRequest, ReadSessionFeedRequest, SessionFeedAppendOutcome,
    SessionFeedEntry, SessionFeedEvent, UpdateSessionTodosRequest,
};

const MAX_FEED_PAGE_SIZE: u64 = 1_000;

pub(crate) struct UpdateSessionTodosOutcome {
    pub(crate) todos: Vec<serde_json::Value>,
    pub(crate) feed_entry: Option<SessionFeedEntry>,
    pub(crate) cursor: u64,
}

impl SessionLogStore {
    pub fn update_session_todos(
        &self,
        request: UpdateSessionTodosRequest,
    ) -> Result<Vec<serde_json::Value>> {
        Ok(self.update_session_todos_with_feed(request)?.todos)
    }

    pub(crate) fn update_session_todos_with_feed(
        &self,
        request: UpdateSessionTodosRequest,
    ) -> Result<UpdateSessionTodosOutcome> {
        if request.command_id.trim().is_empty() || request.session_id.trim().is_empty() {
            anyhow::bail!("command_id and session_id must be non-empty");
        }
        let workspace = self
            .workspace_db_path_for_session(&request.session_id)?
            .with_context(|| format!("session {} not found", request.session_id))?;
        self.with_workspace_connection(&workspace, |conn| {
            let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            let event = SessionFeedEvent::TodosUpdated {
                todos: request.todos.clone(),
                updated_at: request.updated_at,
            };
            let encoded = serde_json::to_string(&event)?;
            if let Some((cursor, runtime_id, session_id, event_json)) = tx
                .query_row(
                    "SELECT cursor, runtime_id, session_id, event_json
                     FROM session_feed_events WHERE event_id = ?1",
                    params![request.command_id],
                    |row| {
                        Ok((
                            row.get::<_, u64>(0)?,
                            row.get::<_, Option<String>>(1)?,
                            row.get::<_, String>(2)?,
                            row.get::<_, String>(3)?,
                        ))
                    },
                )
                .optional()?
            {
                if runtime_id.is_some() || session_id != request.session_id || event_json != encoded
                {
                    anyhow::bail!(
                        "session todo command id {} was reused with different content",
                        request.command_id
                    );
                }
                let (todos_json, latest_cursor) = tx.query_row(
                    "SELECT todos_json,
                            (SELECT COALESCE(MAX(cursor), 0)
                             FROM session_feed_events WHERE session_id = ?1)
                     FROM sessions WHERE session_id = ?1",
                    params![request.session_id],
                    |row| Ok((row.get::<_, String>(0)?, row.get::<_, u64>(1)?)),
                )?;
                tx.commit()?;
                return Ok(UpdateSessionTodosOutcome {
                    todos: serde_json::from_str(&todos_json).with_context(|| {
                        format!("invalid todos_json for session {}", request.session_id)
                    })?,
                    feed_entry: None,
                    cursor: latest_cursor.max(cursor),
                });
            }
            let changed = tx.execute(
                "UPDATE sessions SET todos_json = ?2 WHERE session_id = ?1",
                params![request.session_id, serde_json::to_string(&request.todos)?],
            )?;
            if changed != 1 {
                anyhow::bail!("session {} not found", request.session_id);
            }
            let cursor = append_session_feed_event_tx(
                &tx,
                &request.session_id,
                None,
                &request.command_id,
                &event,
            )?;
            let feed_entry = SessionFeedEntry {
                session_id: request.session_id,
                cursor,
                runtime_id: None,
                event_id: request.command_id,
                event,
            };
            tx.commit()?;
            Ok(UpdateSessionTodosOutcome {
                todos: request.todos,
                feed_entry: Some(feed_entry),
                cursor,
            })
        })
    }

    pub fn append_session_feed_event(
        &self,
        request: AppendSessionFeedEventRequest,
    ) -> Result<SessionFeedAppendOutcome> {
        validate_append_request(&request)?;
        if !runtime_may_append(&request.event) {
            return Ok(SessionFeedAppendOutcome::SessionOwnedEvent);
        }
        let Some(runtime_workspace) = self.runtime_workspace_db_path(&request.runtime_id)? else {
            return Ok(SessionFeedAppendOutcome::RuntimeNotFound);
        };
        let Some(target_workspace) =
            self.workspace_db_path_for_session(&request.target_session_id)?
        else {
            return Ok(SessionFeedAppendOutcome::TargetSessionNotFound);
        };
        if runtime_workspace != target_workspace {
            return Ok(SessionFeedAppendOutcome::TargetWorkspaceMismatch);
        }

        self.with_workspace_connection(&runtime_workspace, |conn| {
            let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            let runtime = tx
                .query_row(
                    "SELECT lease_id, lease_active, terminal FROM runtimes WHERE runtime_id = ?1",
                    params![request.runtime_id],
                    |row| {
                        Ok((
                            row.get::<_, Option<String>>(0)?,
                            row.get::<_, bool>(1)?,
                            row.get::<_, bool>(2)?,
                        ))
                    },
                )
                .optional()?;
            let Some((lease_id, lease_active, terminal)) = runtime else {
                return Ok(SessionFeedAppendOutcome::RuntimeNotFound);
            };

            let encoded = serde_json::to_string(&request.event)?;
            if let Some((cursor, runtime_id, session_id, event_json)) = tx
                .query_row(
                    "SELECT cursor, runtime_id, session_id, event_json
                     FROM session_feed_events WHERE event_id = ?1",
                    params![request.event_id],
                    |row| {
                        Ok((
                            row.get::<_, u64>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, String>(2)?,
                            row.get::<_, String>(3)?,
                        ))
                    },
                )
                .optional()?
            {
                return Ok(
                    if runtime_id == request.runtime_id
                        && session_id == request.target_session_id
                        && event_json == encoded
                    {
                        SessionFeedAppendOutcome::Duplicate { cursor }
                    } else {
                        SessionFeedAppendOutcome::EventIdConflict
                    },
                );
            }
            if terminal {
                return Ok(SessionFeedAppendOutcome::RuntimeTerminal);
            }
            if !lease_active || lease_id.as_deref() != Some(request.lease_id.as_str()) {
                return Ok(SessionFeedAppendOutcome::StaleLease);
            }

            let target_exists = tx
                .query_row(
                    "SELECT 1 FROM sessions WHERE session_id = ?1",
                    params![request.target_session_id],
                    |_| Ok(()),
                )
                .optional()?
                .is_some();
            if !target_exists {
                return Ok(SessionFeedAppendOutcome::TargetSessionNotFound);
            }
            if let SessionFeedEvent::TodosUpdated { todos, .. } = &request.event {
                tx.execute(
                    "UPDATE sessions SET todos_json = ?2 WHERE session_id = ?1",
                    params![request.target_session_id, serde_json::to_string(todos)?],
                )?;
            }
            let cursor = append_session_feed_event_tx(
                &tx,
                &request.target_session_id,
                Some(&request.runtime_id),
                &request.event_id,
                &request.event,
            )?;
            tx.commit()?;
            Ok(SessionFeedAppendOutcome::Applied { cursor })
        })
    }

    pub fn read_session_feed(
        &self,
        request: ReadSessionFeedRequest,
    ) -> Result<(Vec<SessionFeedEntry>, u64)> {
        if request.session_id.trim().is_empty() {
            anyhow::bail!("session feed session_id must be non-empty");
        }
        if request.limit == 0 || request.limit > MAX_FEED_PAGE_SIZE {
            anyhow::bail!("session feed limit must be between 1 and {MAX_FEED_PAGE_SIZE}");
        }
        let workspace = self
            .workspace_db_path_for_session(&request.session_id)?
            .with_context(|| format!("session {} not found", request.session_id))?;
        self.with_workspace_connection(&workspace, |conn| {
            let mut statement = conn.prepare(
                "SELECT cursor, runtime_id, event_id, event_json
                 FROM session_feed_events
                 WHERE session_id = ?1 AND cursor > ?2
                 ORDER BY cursor ASC LIMIT ?3",
            )?;
            let entries = statement
                .query_map(
                    params![request.session_id, request.after_cursor, request.limit],
                    |row| {
                        let event_json = row.get::<_, String>(3)?;
                        let event = serde_json::from_str::<SessionFeedEvent>(&event_json).map_err(
                            |error| {
                                rusqlite::Error::FromSqlConversionFailure(
                                    3,
                                    rusqlite::types::Type::Text,
                                    Box::new(error),
                                )
                            },
                        )?;
                        Ok(SessionFeedEntry {
                            session_id: request.session_id.clone(),
                            cursor: row.get(0)?,
                            runtime_id: row.get(1)?,
                            event_id: row.get(2)?,
                            event,
                        })
                    },
                )?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            let next_cursor = entries
                .last()
                .map(|entry| entry.cursor)
                .unwrap_or(request.after_cursor);
            Ok((entries, next_cursor))
        })
    }

    pub(crate) fn session_feed_entry_by_event_id(
        &self,
        runtime_id: &str,
        event_id: &str,
    ) -> Result<Option<SessionFeedEntry>> {
        let Some(workspace) = self.runtime_workspace_db_path(runtime_id)? else {
            return Ok(None);
        };
        self.with_workspace_connection(&workspace, |conn| {
            conn.query_row(
                "SELECT session_id, cursor, runtime_id, event_json
                 FROM session_feed_events WHERE event_id = ?1 AND runtime_id = ?2",
                params![event_id, runtime_id],
                |row| {
                    let event_json = row.get::<_, String>(3)?;
                    let event =
                        serde_json::from_str::<SessionFeedEvent>(&event_json).map_err(|error| {
                            rusqlite::Error::FromSqlConversionFailure(
                                3,
                                rusqlite::types::Type::Text,
                                Box::new(error),
                            )
                        })?;
                    Ok(SessionFeedEntry {
                        session_id: row.get(0)?,
                        cursor: row.get(1)?,
                        runtime_id: Some(row.get(2)?),
                        event_id: event_id.to_string(),
                        event,
                    })
                },
            )
            .optional()
            .map_err(Into::into)
        })
    }
}

pub(super) fn append_session_feed_event_tx(
    tx: &rusqlite::Transaction<'_>,
    session_id: &str,
    runtime_id: Option<&str>,
    event_id: &str,
    event: &SessionFeedEvent,
) -> Result<u64> {
    let cursor = tx.query_row(
        "SELECT COALESCE(MAX(cursor) + 1, 1) FROM session_feed_events
         WHERE session_id = ?1",
        params![session_id],
        |row| row.get::<_, u64>(0),
    )?;
    tx.execute(
        "INSERT INTO session_feed_events(session_id, cursor, runtime_id, event_id, event_json)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            session_id,
            cursor,
            runtime_id,
            event_id,
            serde_json::to_string(event)?,
        ],
    )?;
    Ok(cursor)
}

fn validate_append_request(request: &AppendSessionFeedEventRequest) -> Result<()> {
    if request.runtime_id.trim().is_empty()
        || request.target_session_id.trim().is_empty()
        || request.lease_id.trim().is_empty()
        || request.event_id.trim().is_empty()
    {
        anyhow::bail!("runtime_id, target_session_id, lease_id, and event_id must be non-empty");
    }
    Ok(())
}

fn runtime_may_append(event: &SessionFeedEvent) -> bool {
    matches!(
        event,
        SessionFeedEvent::AssistantTextDelta { .. }
            | SessionFeedEvent::AgentMessage { .. }
            | SessionFeedEvent::ToolCallUpdated { .. }
            | SessionFeedEvent::TodosUpdated { .. }
    )
}
