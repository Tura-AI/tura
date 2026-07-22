use super::helpers::{
    append_session_event, replay_session_events, session_state_text, task_management_value,
};
use super::SessionLogStore;
use anyhow::{Context, Result};
use lifecycle::{
    RuntimeAggregate, RuntimeEvent, RuntimeQuery, RuntimeState, SessionAggregate, SessionCommand,
    SessionManagement, SessionQuery,
};
use rusqlite::{params, OptionalExtension, Transaction, TransactionBehavior};
use session_log_contract::{
    ActivateRuntimeLeaseRequest, CommitRuntimeEventRequest, RegisterRuntimeRequest,
    ReplayRuntimeRequest, RuntimeEventCommitOutcome, RuntimeLeaseOutcome,
    RuntimeRegistrationOutcome, RuntimeReplay, SessionFeedEvent, SessionMetadata,
};
use std::path::{Path, PathBuf};

impl SessionLogStore {
    pub fn register_runtime(
        &self,
        request: RegisterRuntimeRequest,
    ) -> Result<RuntimeRegistrationOutcome> {
        if request.runtime_id.trim().is_empty() || request.session_id.trim().is_empty() {
            anyhow::bail!("runtime_id and session_id must be non-empty");
        }
        if let Some(fallback_from_id) = request.fallback_from_id.as_deref() {
            if fallback_from_id.trim().is_empty() {
                anyhow::bail!("fallback_from_id must be non-empty when provided");
            }
            if fallback_from_id == request.runtime_id {
                anyhow::bail!("runtime cannot fall back from itself");
            }
        }
        let workspace_db_path = self
            .workspace_db_path_for_session(&request.session_id)?
            .with_context(|| format!("session {} not found", request.session_id))?;
        let now_ms = chrono::Utc::now().timestamp_millis();
        let reservation = self.reserve_runtime_location(&request, &workspace_db_path)?;
        if reservation == RuntimeLocationReservation::Conflict {
            return Ok(RuntimeRegistrationOutcome::RuntimeIdConflict);
        }
        let outcome = self.with_workspace_connection(&workspace_db_path, |conn| {
            let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            if let Some((existing_session_id, existing_fallback_from_id)) = tx
                .query_row(
                    "SELECT session_id, fallback_from_id FROM runtimes WHERE runtime_id = ?1",
                    params![request.runtime_id],
                    |row| Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?)),
                )
                .optional()?
            {
                return if existing_session_id == request.session_id
                    && existing_fallback_from_id == request.fallback_from_id
                {
                    let (revision, last_event_seq) = runtime_cursor(&tx, &request.runtime_id)?;
                    let aggregate = replay_session_events(&tx, &request.session_id)?;
                    Ok(RuntimeRegistrationOutcome::AlreadyRegistered {
                        revision,
                        next_event_seq: last_event_seq + 1,
                        projection: aggregate.query(SessionQuery::Lifecycle),
                    })
                } else {
                    Ok(RuntimeRegistrationOutcome::RuntimeIdConflict)
                };
            }

            let row = load_session_projection_row(&tx, &request.session_id)?;
            let Some(mut row) = row else {
                return Ok(RuntimeRegistrationOutcome::SessionNotFound);
            };
            let mut aggregate = replay_session_events(&tx, &request.session_id)?;
            if let Some(active_runtime_id) = aggregate.active_runtime_id.as_ref() {
                return Ok(RuntimeRegistrationOutcome::SessionBusy {
                    active_runtime_id: active_runtime_id.clone(),
                });
            }
            let start_command = match request.fallback_from_id.clone() {
                Some(fallback_from_id) => SessionCommand::RuntimeRetried {
                    runtime_id: request.runtime_id.clone(),
                    fallback_from_id,
                },
                None => SessionCommand::RuntimeStarted {
                    runtime_id: request.runtime_id.clone(),
                },
            };
            let event = aggregate.execute(start_command)?;
            persist_session_projection(&tx, &request.session_id, &aggregate, &mut row, now_ms)?;
            append_session_event(&tx, &request.session_id, &event)?;
            tx.execute(
                "INSERT INTO runtimes(runtime_id, session_id, fallback_from_id)
                 VALUES (?1, ?2, ?3)",
                params![
                    request.runtime_id,
                    request.session_id,
                    request.fallback_from_id
                ],
            )?;
            let projection = aggregate.query(SessionQuery::Lifecycle);
            super::feed::append_session_feed_event_tx(
                &tx,
                &request.session_id,
                Some(&request.runtime_id),
                &format!("{}:session-projection:registered", request.runtime_id),
                &SessionFeedEvent::SessionProjectionUpdated {
                    projection: projection.clone(),
                    session_name: None,
                    updated_at: now_ms,
                },
            )?;
            tx.commit()?;
            Ok(RuntimeRegistrationOutcome::Registered {
                revision: 0,
                next_event_seq: 1,
                projection,
            })
        });

        let outcome = match outcome {
            Ok(outcome) => outcome,
            Err(error) => {
                if reservation == RuntimeLocationReservation::Inserted {
                    self.release_runtime_location(&request)?;
                }
                return Err(error);
            }
        };
        if reservation == RuntimeLocationReservation::Inserted
            && matches!(
                outcome,
                RuntimeRegistrationOutcome::SessionBusy { .. }
                    | RuntimeRegistrationOutcome::SessionNotFound
            )
        {
            self.release_runtime_location(&request)?;
        }

        match &outcome {
            RuntimeRegistrationOutcome::Registered { projection, .. } => self
                .update_session_index_lifecycle(
                    &request.session_id,
                    projection.state,
                    Some(now_ms),
                )?,
            RuntimeRegistrationOutcome::AlreadyRegistered { projection, .. } => {
                self.update_session_index_lifecycle(&request.session_id, projection.state, None)?
            }
            _ => {}
        }

        Ok(outcome)
    }

    pub fn activate_runtime_lease(
        &self,
        request: ActivateRuntimeLeaseRequest,
    ) -> Result<RuntimeLeaseOutcome> {
        if request.lease_id.trim().is_empty() {
            anyhow::bail!("lease_id must be non-empty");
        }
        let Some(workspace_db_path) = self.runtime_workspace_db_path(&request.runtime_id)? else {
            return Ok(RuntimeLeaseOutcome::RuntimeNotFound);
        };
        self.with_workspace_connection(&workspace_db_path, |conn| {
            let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            let row = tx
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
            let Some((lease_id, lease_active, terminal)) = row else {
                return Ok(RuntimeLeaseOutcome::RuntimeNotFound);
            };
            if terminal {
                return Ok(RuntimeLeaseOutcome::RuntimeTerminal);
            }
            if lease_active {
                return Ok(if lease_id.as_deref() == Some(request.lease_id.as_str()) {
                    RuntimeLeaseOutcome::AlreadyActive
                } else {
                    RuntimeLeaseOutcome::LeaseConflict
                });
            }
            tx.execute(
                "UPDATE runtimes SET lease_id = ?2, lease_active = 1 WHERE runtime_id = ?1",
                params![request.runtime_id, request.lease_id],
            )?;
            tx.commit()?;
            Ok(RuntimeLeaseOutcome::Activated)
        })
    }

    pub fn commit_runtime_event(
        &self,
        request: CommitRuntimeEventRequest,
    ) -> Result<RuntimeEventCommitOutcome> {
        if request.lease_id.trim().is_empty() || request.idempotency_key.trim().is_empty() {
            anyhow::bail!("lease_id and idempotency_key must be non-empty");
        }
        let Some(workspace_db_path) = self.runtime_workspace_db_path(&request.runtime_id)? else {
            return Ok(RuntimeEventCommitOutcome::RuntimeNotFound);
        };
        let now_ms = chrono::Utc::now().timestamp_millis();
        let runtime_id = request.runtime_id.clone();
        let terminal_projection_event_id =
            format!("{}:session-projection", request.idempotency_key);
        let outcome = self.with_workspace_connection(&workspace_db_path, |conn| {
            let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            let Some(row) = load_runtime_row(&tx, &request.runtime_id)? else {
                return Ok(RuntimeEventCommitOutcome::RuntimeNotFound);
            };
            if let Some((runtime_id, event_seq, event_json)) = tx
                .query_row(
                    "SELECT runtime_id, event_seq, event_json FROM runtime_events
                     WHERE idempotency_key = ?1",
                    params![request.idempotency_key],
                    |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, u64>(1)?,
                            row.get::<_, String>(2)?,
                        ))
                    },
                )
                .optional()?
            {
                let requested_json = serde_json::to_string(&request.event)?;
                return Ok(
                    if runtime_id == request.runtime_id
                        && event_seq == request.event_seq
                        && event_json == requested_json
                    {
                        RuntimeEventCommitOutcome::Duplicate {
                            revision: row.revision,
                            next_event_seq: row.last_event_seq + 1,
                        }
                    } else {
                        RuntimeEventCommitOutcome::InvalidEvent {
                            error: "idempotency key was already used for a different runtime event"
                                .to_string(),
                        }
                    },
                );
            }
            if row.terminal {
                return Ok(RuntimeEventCommitOutcome::RuntimeTerminal);
            }
            if !row.lease_active || row.lease_id.as_deref() != Some(request.lease_id.as_str()) {
                return Ok(RuntimeEventCommitOutcome::StaleLease);
            }
            let next_event_seq = row.last_event_seq + 1;
            if request.event_seq != next_event_seq {
                return Ok(RuntimeEventCommitOutcome::OutOfOrder {
                    expected_event_seq: next_event_seq,
                    received_event_seq: request.event_seq,
                });
            }
            if request.expected_revision != row.revision {
                return Ok(RuntimeEventCommitOutcome::RevisionConflict {
                    current_revision: row.revision,
                    expected_revision: request.expected_revision,
                });
            }

            let mut events = load_runtime_events(&tx, &request.runtime_id)?;
            events.push(request.event.clone());
            let aggregate = match RuntimeAggregate::replay(request.runtime_id.clone(), events) {
                Ok(aggregate)
                    if aggregate.session_id == row.session_id
                        && aggregate.fallback_from_id == row.fallback_from_id =>
                {
                    aggregate
                }
                Ok(aggregate) if aggregate.session_id != row.session_id => {
                    return Ok(RuntimeEventCommitOutcome::InvalidEvent {
                        error: "runtime_created session_id does not match registered session"
                            .to_string(),
                    })
                }
                Ok(_) => {
                    return Ok(RuntimeEventCommitOutcome::InvalidEvent {
                        error: "runtime_created fallback_from_id does not match registered runtime"
                            .to_string(),
                    })
                }
                Err(error) => return Ok(RuntimeEventCommitOutcome::InvalidEvent { error }),
            };
            let revision = row.revision + 1;
            let event_json = serde_json::to_string(&request.event)?;
            tx.execute(
                "INSERT INTO runtime_events(
                    runtime_id, event_seq, revision, idempotency_key, event_json
                 ) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    request.runtime_id,
                    request.event_seq,
                    revision,
                    request.idempotency_key,
                    event_json
                ],
            )?;
            let terminal = aggregate.state.is_terminal();
            tx.execute(
                "UPDATE runtimes SET revision = ?2, last_event_seq = ?3,
                    terminal = ?4, lease_active = CASE WHEN ?4 THEN 0 ELSE lease_active END
                 WHERE runtime_id = ?1",
                params![request.runtime_id, revision, request.event_seq, terminal],
            )?;
            if terminal {
                let projection =
                    apply_terminal_runtime_to_session(&tx, &row.session_id, &aggregate, now_ms)?;
                super::feed::append_session_feed_event_tx(
                    &tx,
                    &row.session_id,
                    Some(&request.runtime_id),
                    &terminal_projection_event_id,
                    &SessionFeedEvent::SessionProjectionUpdated {
                        projection,
                        session_name: None,
                        updated_at: now_ms,
                    },
                )?;
            }
            tx.commit()?;
            Ok(RuntimeEventCommitOutcome::Applied {
                revision,
                next_event_seq: request.event_seq + 1,
                projection: aggregate.query(RuntimeQuery::Lifecycle),
            })
        })?;
        let index_updated_at = match &outcome {
            RuntimeEventCommitOutcome::Applied { projection, .. }
                if projection.state.is_terminal() =>
            {
                Some(Some(now_ms))
            }
            RuntimeEventCommitOutcome::Duplicate { .. } => Some(None),
            _ => None,
        };
        if let Some(updated_at) = index_updated_at {
            let session_id = self.with_workspace_connection(&workspace_db_path, |conn| {
                load_runtime_row(conn, &runtime_id)?
                    .map(|row| row.session_id)
                    .context("runtime disappeared after committed event")
            })?;
            let session_state = self.with_workspace_connection(&workspace_db_path, |conn| {
                Ok(replay_session_events(conn, &session_id)?.state)
            })?;
            self.update_session_index_lifecycle(&session_id, session_state, updated_at)?;
        }
        Ok(outcome)
    }

    pub fn replay_runtime(&self, request: ReplayRuntimeRequest) -> Result<Option<RuntimeReplay>> {
        let Some(workspace_db_path) = self.runtime_workspace_db_path(&request.runtime_id)? else {
            return Ok(None);
        };
        self.with_workspace_connection(&workspace_db_path, |conn| {
            let Some(row) = load_runtime_row(conn, &request.runtime_id)? else {
                return Ok(None);
            };
            let events = load_runtime_events(conn, &request.runtime_id)?;
            if events.is_empty() {
                return Ok(None);
            }
            let aggregate =
                RuntimeAggregate::replay(request.runtime_id, events).map_err(anyhow::Error::msg)?;
            Ok(Some(RuntimeReplay {
                aggregate,
                revision: row.revision,
                next_event_seq: row.last_event_seq + 1,
            }))
        })
    }

    pub(super) fn runtime_workspace_db_path(&self, runtime_id: &str) -> Result<Option<PathBuf>> {
        self.with_index_connection(|conn| {
            conn.query_row(
                "SELECT workspace_db_path FROM runtime_locations WHERE runtime_id = ?1",
                params![runtime_id],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map(|path| path.map(PathBuf::from))
            .map_err(Into::into)
        })
    }

    fn reserve_runtime_location(
        &self,
        request: &RegisterRuntimeRequest,
        workspace_db_path: &Path,
    ) -> Result<RuntimeLocationReservation> {
        self.with_index_connection(|conn| {
            let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            let existing = tx
                .query_row(
                    "SELECT session_id, workspace_db_path FROM runtime_locations
                     WHERE runtime_id = ?1",
                    params![request.runtime_id],
                    |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
                )
                .optional()?;
            let workspace_db_text = workspace_db_path.to_string_lossy();
            let reservation = match existing {
                Some((session_id, path)) => {
                    if session_id == request.session_id && path == workspace_db_text {
                        RuntimeLocationReservation::Existing
                    } else {
                        RuntimeLocationReservation::Conflict
                    }
                }
                None => {
                    tx.execute(
                        "INSERT INTO runtime_locations(runtime_id, session_id, workspace_db_path)
                         VALUES (?1, ?2, ?3)",
                        params![request.runtime_id, request.session_id, workspace_db_text],
                    )?;
                    RuntimeLocationReservation::Inserted
                }
            };
            tx.commit()?;
            Ok(reservation)
        })
    }

    fn release_runtime_location(&self, request: &RegisterRuntimeRequest) -> Result<()> {
        self.with_index_connection(|conn| {
            conn.execute(
                "DELETE FROM runtime_locations WHERE runtime_id = ?1 AND session_id = ?2",
                params![request.runtime_id, request.session_id],
            )?;
            Ok(())
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RuntimeLocationReservation {
    Inserted,
    Existing,
    Conflict,
}

struct RuntimeRow {
    session_id: String,
    fallback_from_id: Option<String>,
    lease_id: Option<String>,
    lease_active: bool,
    revision: u64,
    last_event_seq: u64,
    terminal: bool,
}

fn load_runtime_row(conn: &rusqlite::Connection, runtime_id: &str) -> Result<Option<RuntimeRow>> {
    conn.query_row(
        "SELECT session_id, fallback_from_id, lease_id, lease_active, revision, last_event_seq,
                terminal
         FROM runtimes WHERE runtime_id = ?1",
        params![runtime_id],
        |row| {
            Ok(RuntimeRow {
                session_id: row.get(0)?,
                fallback_from_id: row.get(1)?,
                lease_id: row.get(2)?,
                lease_active: row.get(3)?,
                revision: row.get(4)?,
                last_event_seq: row.get(5)?,
                terminal: row.get(6)?,
            })
        },
    )
    .optional()
    .map_err(Into::into)
}

fn runtime_cursor(conn: &rusqlite::Connection, runtime_id: &str) -> Result<(u64, u64)> {
    conn.query_row(
        "SELECT revision, last_event_seq FROM runtimes WHERE runtime_id = ?1",
        params![runtime_id],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )
    .map_err(Into::into)
}

fn load_runtime_events(conn: &rusqlite::Connection, runtime_id: &str) -> Result<Vec<RuntimeEvent>> {
    let mut statement = conn.prepare(
        "SELECT event_json FROM runtime_events WHERE runtime_id = ?1 ORDER BY event_seq",
    )?;
    let rows = statement.query_map(params![runtime_id], |row| row.get::<_, String>(0))?;
    let events = rows
        .map(|row| {
            let json = row?;
            serde_json::from_str(&json).map_err(|error| {
                rusqlite::Error::FromSqlConversionFailure(
                    0,
                    rusqlite::types::Type::Text,
                    Box::new(error),
                )
            })
        })
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(events)
}

struct SessionProjectionRow {
    management: SessionManagement,
    metadata: SessionMetadata,
    updated_at: i64,
}

fn load_session_projection_row(
    tx: &Transaction<'_>,
    session_id: &str,
) -> Result<Option<SessionProjectionRow>> {
    tx.query_row(
        "SELECT management_json, session_json, updated_at
         FROM sessions WHERE session_id = ?1",
        params![session_id],
        |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
            ))
        },
    )
    .optional()?
    .map(|(management_json, session_json, updated_at)| {
        Ok(SessionProjectionRow {
            management: serde_json::from_str(&management_json)?,
            metadata: serde_json::from_str(&session_json)?,
            updated_at,
        })
    })
    .transpose()
}

fn persist_session_projection(
    tx: &Transaction<'_>,
    session_id: &str,
    aggregate: &SessionAggregate,
    row: &mut SessionProjectionRow,
    updated_at: i64,
) -> Result<()> {
    let projection = aggregate.query(SessionQuery::Lifecycle);
    let updated_at = row.updated_at.max(updated_at);
    row.management.session_last_update_at =
        chrono::DateTime::<chrono::Utc>::from_timestamp_millis(updated_at)
            .context("runtime terminal timestamp is outside the supported range")?;
    row.management
        .replace_lifecycle_projection(projection.clone());
    row.metadata.disable_permission_restrictions = row.management.disable_permission_restrictions;
    row.metadata.use_last_tool_call_response = row.management.use_last_tool_call_response;
    row.metadata.auto_session_name = row.management.auto_session_name;
    row.metadata.context_tokens = row.management.context_tokens;
    row.metadata
        .runtime_usage
        .clone_from(&row.management.runtime_usage);
    let task_management = task_management_value(&projection.task_plan);
    tx.execute(
        "UPDATE sessions SET parent_id = ?2, state = ?3, status = ?4,
            task_management_json = ?5, management_json = ?6,
            session_json = ?7, updated_at = ?8
         WHERE session_id = ?1",
        params![
            session_id,
            projection.parent_id,
            session_state_text(projection.state)?,
            projection.state.ui_status(),
            serde_json::to_string(&task_management)?,
            serde_json::to_string(&row.management)?,
            serde_json::to_string(&row.metadata)?,
            updated_at,
        ],
    )?;
    Ok(())
}

fn apply_terminal_runtime_to_session(
    tx: &Transaction<'_>,
    session_id: &str,
    runtime: &RuntimeAggregate,
    updated_at: i64,
) -> Result<lifecycle::SessionProjection> {
    let mut row = load_session_projection_row(tx, session_id)?
        .with_context(|| format!("session {session_id} not found"))?;
    let mut aggregate = replay_session_events(tx, session_id)?;
    let command = match runtime.state {
        RuntimeState::Finished => SessionCommand::RuntimeCompleted {
            runtime_id: runtime.runtime_id.clone(),
        },
        RuntimeState::Failed | RuntimeState::TimedOut => SessionCommand::RuntimeFailed {
            runtime_id: runtime.runtime_id.clone(),
        },
        RuntimeState::Cancelled => SessionCommand::RuntimeCancelled {
            runtime_id: runtime.runtime_id.clone(),
        },
        _ => anyhow::bail!("runtime {} is not terminal", runtime.runtime_id),
    };
    let event = aggregate.execute(command)?;
    persist_session_projection(tx, session_id, &aggregate, &mut row, updated_at)?;
    append_session_event(tx, session_id, &event)?;
    Ok(aggregate.query(SessionQuery::Lifecycle))
}
