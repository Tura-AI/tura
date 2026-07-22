use super::helpers::{
    append_session_event, i64_at, path_text, replay_session_events, session_state_text, string_at,
    task_management_value,
};
use super::SessionLogStore;
use crate::path::{normalize_workspace, workspace_session_log_db};
use anyhow::{Context, Result};
use lifecycle::{
    SessionAggregate, SessionCommand, SessionInput, SessionManagement, SessionQuery,
};
use rusqlite::{params, OptionalExtension, Transaction, TransactionBehavior};
use serde_json::{json, Value};
use session_log_contract::{
    CreateSessionRequest, ExecuteSessionCommandRequest, PersistSessionDeltaRequest,
    SessionCommandResult, SessionFeedEntry, SessionFeedEvent, SessionMetadata, SessionMetadataPatch,
    SessionRecordProjection, SessionSnapshot, UpdateSessionRequest,
};
use std::collections::HashMap;
use std::path::PathBuf;
use uuid::Uuid;

impl SessionLogStore {
    pub fn persist_session_delta(&self, request: PersistSessionDeltaRequest) -> Result<(u64, u64)> {
        let outcome = self.persist_session_delta_with_feed(request)?;
        Ok((outcome.next_sequence, outcome.next_management_sequence))
    }

    pub(crate) fn persist_session_delta_with_feed(
        &self,
        request: PersistSessionDeltaRequest,
    ) -> Result<PersistSessionDeltaOutcome> {
        if request.session_id.trim().is_empty() {
            anyhow::bail!("session_id must be non-empty");
        }
        if request
            .management_delta
            .session_log_retention
            .is_some_and(|retention| retention.omitted_entries != request.retained_from_sequence)
        {
            anyhow::bail!(
                "session delta retained sequence does not match its management delta retention"
            );
        }
        let workspace_db_path = self
            .workspace_db_path_for_session(&request.session_id)?
            .with_context(|| format!("session {} not found", request.session_id))?;
        let PersistSessionDeltaRequest {
            session_id,
            management_sequence,
            management_delta,
            retained_from_sequence,
            entries,
        } = request;
        let outcome = self.with_workspace_connection(&workspace_db_path, |conn| {
            let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            let row = load_delta_session_row(&tx, &session_id)?
                .with_context(|| format!("session {session_id} not found"))?;
            let mut metadata = row.metadata;
            let aggregate = replay_session_events(&tx, &session_id)?;
            let projection = aggregate.query(SessionQuery::Lifecycle);
            let mut management: SessionManagement = serde_json::from_str(&row.management_json)
                .with_context(|| format!("invalid management_json for session {session_id}"))?;
            let management_delta_json = serde_json::to_string(&management_delta)?;
            let management_replay = management_sequence < row.next_management_sequence;
            let next_management_sequence = if management_replay {
                let existing = tx
                    .query_row(
                        "SELECT delta_json FROM management_deltas
                         WHERE session_id = ?1 AND sequence = ?2",
                        params![session_id, management_sequence],
                        |row| row.get::<_, String>(0),
                    )
                    .optional()?;
                if existing.as_deref() != Some(management_delta_json.as_str()) {
                    anyhow::bail!(
                        "management sequence {management_sequence} was already used for a different delta in session {session_id}"
                    );
                }
                row.next_management_sequence
            } else if management_sequence > row.next_management_sequence {
                anyhow::bail!(
                    "out-of-order management delta for session {session_id}: expected {}, received {management_sequence}",
                    row.next_management_sequence
                );
            } else {
                management.apply_persistence_delta(management_delta);
                tx.execute(
                    "INSERT INTO management_deltas(session_id, sequence, delta_json)
                     VALUES (?1, ?2, ?3)",
                    params![session_id, management_sequence, management_delta_json],
                )?;
                row.next_management_sequence + 1
            };
            management.replace_lifecycle_projection(projection.clone());
            let effective_retained_from_sequence = if management_replay {
                row.retained_from_sequence
            } else {
                retained_from_sequence
            };
            if management.session_log_retention.omitted_entries != effective_retained_from_sequence {
                anyhow::bail!(
                    "session {session_id} retained sequence {effective_retained_from_sequence} does not match persisted management retention {}",
                    management.session_log_retention.omitted_entries
                );
            }
            let updated_at = management.session_last_update_at.timestamp_millis();
            let last_user_message_at = Some(
                management
                    .session_last_user_message_at
                    .timestamp_millis(),
            );
            sync_metadata_from_management(&mut metadata, &management);
            let task_management = task_management_value(&projection.task_plan);

            let mut next_sequence = row.next_context_sequence;
            let mut inserted_messages = 0_i64;
            let mut feed_entries = Vec::new();
            if management_replay
                && entries
                    .iter()
                    .any(|entry| entry.context.sequence >= row.next_context_sequence)
            {
                anyhow::bail!(
                    "historical management delta cannot append new context in session {session_id}"
                );
            }
            for entry in entries {
                let sequence = entry.context.sequence;
                let raw_record = entry.context.raw_record;
                let projection_json = serde_json::to_string(&entry.projection)?;
                let inserted_context;
                if entry.context.sequence < next_sequence {
                    let existing = tx
                        .query_row(
                            "SELECT record_json, projection_json FROM session_context_records
                             WHERE session_id = ?1 AND sequence = ?2",
                            params![session_id, entry.context.sequence],
                            |row| {
                                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                            },
                        )
                        .optional()?;
                    if existing.as_ref().map(|(raw, projection)| {
                        (raw.as_str(), projection.as_str())
                    }) != Some((raw_record.as_str(), projection_json.as_str()))
                    {
                        anyhow::bail!(
                            "context sequence {} was already used for a different entry in session {}",
                            entry.context.sequence,
                            session_id
                        );
                    }
                    inserted_context = false;
                } else if entry.context.sequence != next_sequence {
                    anyhow::bail!(
                        "out-of-order context record for session {}: expected {}, received {}",
                        session_id,
                        next_sequence,
                        entry.context.sequence
                    );
                } else {
                    tx.execute(
                        "INSERT INTO session_context_records(
                            session_id, sequence, record_json, projection_json
                         ) VALUES (?1, ?2, ?3, ?4)",
                        params![
                            session_id,
                            entry.context.sequence,
                            raw_record,
                            projection_json
                        ],
                    )?;
                    next_sequence += 1;
                    inserted_context = true;
                }
                if inserted_context {
                    if let Some(projection) = entry.projection {
                        let (inserted, projection) =
                            upsert_delta_projection(&tx, &session_id, projection)?;
                        inserted_messages += inserted;
                        let event_id = format!("{session_id}:context:{sequence}:message");
                        let event = session_log_contract::SessionFeedEvent::MessageUpserted {
                            message: projection,
                        };
                        let cursor = super::feed::append_session_feed_event_tx(
                            &tx,
                            &session_id,
                            None,
                            &event_id,
                            &event,
                        )?;
                        feed_entries.push(session_log_contract::SessionFeedEntry {
                            session_id: session_id.clone(),
                            cursor,
                            runtime_id: None,
                            event_id,
                            event,
                        });
                    }
                }
            }
            if !management_replay && retained_from_sequence < row.retained_from_sequence {
                anyhow::bail!(
                    "session {} retained sequence cannot move backward from {} to {}",
                    session_id,
                    row.retained_from_sequence,
                    retained_from_sequence
                );
            }
            if !management_replay && retained_from_sequence > next_sequence {
                anyhow::bail!(
                    "session {} retained sequence {} exceeds next context sequence {}",
                    session_id,
                    retained_from_sequence,
                    next_sequence
                );
            }

            let retained_from_sequence = effective_retained_from_sequence;
            let message_count = row.message_count.saturating_add(inserted_messages);
            let state_text = session_state_text(projection.state)?;
            let status = projection.state.ui_status().to_string();
            let task_management_json = serde_json::to_string(&task_management)?;
            let management_json = serde_json::to_string(&management)?;
            let session_json = serde_json::to_string(&metadata)?;
            tx.execute(
                "UPDATE sessions SET name = ?2, updated_at = ?3, last_user_message_at = ?4,
                    state = ?5, status = ?6, message_count = ?7,
                    task_management_json = ?8, management_json = ?9, session_json = ?10,
                    next_context_sequence = ?11, retained_from_sequence = ?12,
                    next_management_sequence = ?13
                 WHERE session_id = ?1",
                params![
                    session_id,
                    management.session_name,
                    updated_at,
                    last_user_message_at,
                    state_text,
                    status,
                    message_count,
                    task_management_json,
                    management_json,
                    session_json,
                    next_sequence,
                    retained_from_sequence,
                    next_management_sequence,
                ],
            )?;
            tx.commit()?;
            Ok(DeltaIndexUpdate {
                updated_at,
                last_user_message_at,
                state_text,
                next_sequence,
                next_management_sequence,
                feed_entries,
            })
        })?;
        self.with_index_connection(|conn| {
            let changed = conn.execute(
                "UPDATE sessions SET updated_at = ?2, last_user_message_at = ?3,
                    state = ?4
                 WHERE session_id = ?1",
                params![
                    session_id,
                    outcome.updated_at,
                    outcome.last_user_message_at,
                    outcome.state_text,
                ],
            )?;
            if changed == 0 {
                anyhow::bail!("session {session_id} not found in index");
            }
            Ok(())
        })?;
        Ok(PersistSessionDeltaOutcome {
            next_sequence: outcome.next_sequence,
            next_management_sequence: outcome.next_management_sequence,
            feed_entries: outcome.feed_entries,
        })
    }

    pub fn create_session(&self, request: CreateSessionRequest) -> Result<SessionCommandResult> {
        Ok(self.create_session_with_feed(request)?.result)
    }

    pub(crate) fn create_session_with_feed(
        &self,
        request: CreateSessionRequest,
    ) -> Result<CreateSessionOutcome> {
        validate_command_identity(&request.command_id, &request.session_id)?;
        let workspace = normalize_workspace(&request.workspace);
        let workspace_db = workspace_session_log_db(&workspace);
        let workspace_db_text = path_text(&workspace_db);
        let request_json = serde_json::to_string(&request)?;
        if !matches!(
            &request.creation_command,
            SessionCommand::CreateSession { .. }
                | SessionCommand::ForkSession { .. }
                | SessionCommand::RegisterChildSession { .. }
        ) {
            anyhow::bail!("create_session requires a creation command");
        }
        if request.copy_context
            && !matches!(
                &request.creation_command,
                SessionCommand::ForkSession { .. }
            )
        {
            anyhow::bail!("copy_context requires a fork_session creation command");
        }
        if let Some((result, index)) = self.with_workspace_connection(&workspace_db, |conn| {
            let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            let result = load_session_command_receipt(
                &tx,
                &request.command_id,
                &request.session_id,
                &request_json,
            )?;
            let replay = match result {
                Some(result) => Some((result, load_index_projection(&tx, &request.session_id)?)),
                None => None,
            };
            tx.commit()?;
            Ok(replay)
        })? {
            self.write_session_index(
                &request.session_id,
                &index.workspace,
                &workspace_db_text,
                index.updated_at,
                index.last_user_message_at,
                &index.state_text,
            )?;
            return Ok(CreateSessionOutcome {
                result,
                feed_entries: Vec::new(),
            });
        }
        let fork_projection = self.load_fork_projection(&request)?;
        let message_count = fork_projection.records.len() as i64;
        let last_user_message_at = if request.copy_context {
            fork_projection.last_user_message_at
        } else {
            Some(request.created_at)
        };
        let todos_json = serde_json::to_string(&fork_projection.todos)?;
        let mut aggregate = SessionAggregate::new(request.session_id.clone());
        let event = aggregate.execute(request.creation_command.clone())?;
        let mut lifecycle_events = vec![event.clone()];
        if let Some(patch) = request.initial_task_plan_patch.clone() {
            lifecycle_events.push(
                aggregate.execute(SessionCommand::ApplyTaskPlanPatch { patch })?,
            );
        }
        let projection = aggregate.query(SessionQuery::Lifecycle);
        let state_text = session_state_text(projection.state)?;
        let status = projection.state.ui_status();
        let timestamp = chrono::DateTime::<chrono::Utc>::from_timestamp_millis(request.created_at)
            .context("session creation timestamp is outside the supported range")?;
        let last_user_timestamp = chrono::DateTime::<chrono::Utc>::from_timestamp_millis(
            last_user_message_at.unwrap_or(request.created_at),
        )
        .context("session last-user timestamp is outside the supported range")?;
        let task_management = task_management_value(&projection.task_plan);
        let mut management = SessionManagement::new(
            request.session_id.clone(),
            request.name.clone(),
            PathBuf::from(&request.session_directory),
            false,
            Vec::<String>::new(),
            SessionInput {
                user_input: String::new(),
                file_input: Vec::new(),
                agent: request.agent.clone(),
                runtime_context: None,
                planning_mode_override: None,
            },
            String::new(),
            timestamp,
        );
        management.auto_session_name = request.auto_session_name;
        management.session_last_user_message_at = last_user_timestamp;
        management.use_last_tool_call_response = request.use_last_tool_call_response;
        management.disable_permission_restrictions = request.disable_permission_restrictions;
        management.replace_lifecycle_projection(projection.clone());
        if request.auto_session_name {
            if let Some(name) = request
                .initial_task_plan_patch
                .as_ref()
                .and_then(task_plan_patch_summary_for_auto_name)
            {
                management.session_name = name;
            }
        }
        let metadata = SessionMetadata {
            session_directory: request.session_directory.clone(),
            model: request.model.clone(),
            agent: request.agent.clone(),
            session_type: request.session_type.clone(),
            kill_processes_on_start: request.kill_processes_on_start,
            validator_enabled: request.validator_enabled,
            force_planning: request.force_planning,
            model_variant: request.model_variant.clone(),
            model_acceleration_enabled: request.model_acceleration_enabled,
            disable_permission_restrictions: management.disable_permission_restrictions,
            use_last_tool_call_response: management.use_last_tool_call_response,
            auto_session_name: management.auto_session_name,
            context_tokens: management.context_tokens,
            runtime_usage: management.runtime_usage.clone(),
        };
        let management_json = serde_json::to_string(&management)?;
        let session_json = serde_json::to_string(&metadata)?;
        let task_management_json = serde_json::to_string(&task_management)?;

        let result = SessionCommandResult {
            event: event.clone(),
            projection: projection.clone(),
            session_name: Some(management.session_name.clone()),
            message_count: message_count as u64,
            last_user_message_at,
        };
        let result_json = serde_json::to_string(&result)?;
        let (raced_replay, feed_entries) =
            self.with_workspace_connection(&workspace_db, |conn| {
                let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
                if let Some(result) = load_session_command_receipt(
                    &tx,
                    &request.command_id,
                    &request.session_id,
                    &request_json,
                )? {
                    let index = load_index_projection(&tx, &request.session_id)?;
                    tx.commit()?;
                    return Ok((Some((result, index)), Vec::new()));
                }
                tx.execute(
                    "INSERT INTO sessions(
                    session_id, workspace, name, parent_id, created_at, updated_at,
                    last_user_message_at, state, status, message_count, task_management_json,
                    management_json, session_json, todos_json
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
                    params![
                        request.session_id,
                        workspace,
                        request.name,
                        projection.parent_id,
                        request.created_at,
                        last_user_message_at,
                        state_text,
                        status,
                        message_count,
                        task_management_json,
                        management_json,
                        session_json,
                        todos_json,
                    ],
                )?;
                for lifecycle_event in &lifecycle_events {
                    append_session_event(&tx, &request.session_id, lifecycle_event)?;
                }
                let feed_entries = {
                    let snapshot =
                        load_session_snapshot_tx(&tx, &request.session_id, projection.clone())?;
                    let snapshot_event_id = format!("{}:session-snapshot", request.command_id);
                    let snapshot_event = SessionFeedEvent::SessionSnapshotCreated {
                        snapshot: Box::new(snapshot),
                    };
                    let snapshot_cursor = super::feed::append_session_feed_event_tx(
                        &tx,
                        &request.session_id,
                        None,
                        &snapshot_event_id,
                        &snapshot_event,
                    )?;
                    let mut statement = tx.prepare(
                        "INSERT INTO session_records(
                        session_id, message_id, role, created_at, updated_at, record_json
                     ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    )?;
                    let mut feed_entries = Vec::with_capacity(fork_projection.records.len() + 1);
                    feed_entries.push(SessionFeedEntry {
                        session_id: request.session_id.clone(),
                        cursor: snapshot_cursor,
                        runtime_id: None,
                        event_id: snapshot_event_id,
                        event: snapshot_event,
                    });
                    for record in fork_projection.records {
                        statement.execute(params![
                            &request.session_id,
                            &record.message_id,
                            &record.role,
                            record.created_at,
                            record.updated_at,
                            serde_json::to_string(&record.record)?,
                        ])?;
                        let event_id =
                            format!("{}:fork-message:{}", request.session_id, record.message_id);
                        let event = SessionFeedEvent::MessageUpserted { message: record };
                        let cursor = super::feed::append_session_feed_event_tx(
                            &tx,
                            &request.session_id,
                            None,
                            &event_id,
                            &event,
                        )?;
                        feed_entries.push(SessionFeedEntry {
                            session_id: request.session_id.clone(),
                            cursor,
                            runtime_id: None,
                            event_id,
                            event,
                        });
                    }
                    feed_entries
                };
                insert_session_command_receipt(
                    &tx,
                    &request.command_id,
                    &request.session_id,
                    &request_json,
                    &result_json,
                )?;
                tx.commit()?;
                Ok((None, feed_entries))
            })?;
        if let Some((result, index)) = raced_replay {
            self.write_session_index(
                &request.session_id,
                &index.workspace,
                &workspace_db_text,
                index.updated_at,
                index.last_user_message_at,
                &index.state_text,
            )?;
            return Ok(CreateSessionOutcome {
                result,
                feed_entries: Vec::new(),
            });
        }
        self.write_session_index(
            &request.session_id,
            &workspace,
            &workspace_db_text,
            request.created_at,
            last_user_message_at,
            &state_text,
        )?;
        Ok(CreateSessionOutcome {
            result,
            feed_entries,
        })
    }

    pub fn execute_session_command(
        &self,
        request: ExecuteSessionCommandRequest,
    ) -> Result<SessionCommandResult> {
        Ok(self.execute_session_command_with_feed(request)?.result)
    }

    pub(crate) fn execute_session_command_with_feed(
        &self,
        request: ExecuteSessionCommandRequest,
    ) -> Result<ExecuteSessionCommandOutcome> {
        validate_command_identity(&request.command_id, &request.session_id)?;
        let workspace_db_path = self
            .workspace_db_path_for_session(&request.session_id)?
            .with_context(|| format!("session {} not found", request.session_id))?;
        if let Some(message) = &request.message_projection {
            let expected_role = match &request.session_command {
                SessionCommand::StartUserTurn
                | SessionCommand::SubmitUserInput
                | SessionCommand::QueueUserInputWhileBusy { .. }
                | SessionCommand::StartScheduledTask { .. } => Some("user"),
                SessionCommand::RuntimeCompleted { .. } | SessionCommand::RuntimeFailed { .. } => {
                    Some("assistant")
                }
                _ => None,
            }
            .with_context(|| {
                "only user-input, scheduler, or runtime terminal commands may carry a message projection"
            })?;
            if message.role != expected_role {
                anyhow::bail!(
                    "command requires message projection role {expected_role}, got {}",
                    message.role
                );
            }
            let runtime_terminal = matches!(
                &request.session_command,
                SessionCommand::RuntimeCompleted { .. } | SessionCommand::RuntimeFailed { .. }
            );
            if runtime_terminal && message.session_id != request.session_id {
                anyhow::bail!(
                    "runtime terminal message projection must target command session {}",
                    request.session_id
                );
            }
            let message_workspace = self
                .workspace_db_path_for_session(&message.session_id)?
                .with_context(|| format!("message session {} not found", message.session_id))?;
            if message_workspace != workspace_db_path {
                anyhow::bail!(
                    "command session {} and message session {} belong to different workspaces",
                    request.session_id,
                    message.session_id
                );
            }
        }
        let now_ms = chrono::Utc::now().timestamp_millis();
        let request_json = serde_json::to_string(&request)?;
        let command = request.session_command.clone();
        if matches!(
            &command,
            SessionCommand::CreateSession { .. }
                | SessionCommand::ForkSession { .. }
                | SessionCommand::DeleteSession
        ) {
            anyhow::bail!("creation and deletion commands must use their dedicated store methods");
        }
        let auto_name = task_summary_for_auto_name(&command);
        let task_projection_requested = matches!(
            &command,
            SessionCommand::ApplyTaskStatus { .. }
                | SessionCommand::ApplyTaskPatch { .. }
                | SessionCommand::ApplyTaskPatches { .. }
                | SessionCommand::ApplyTaskPlanPatch { .. }
                | SessionCommand::StartScheduledTask { .. }
        );
        let result = self.with_workspace_connection(&workspace_db_path, |conn| {
            let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            if let Some(result) = load_session_command_receipt(
                &tx,
                &request.command_id,
                &request.session_id,
                &request_json,
            )? {
                let indexes = load_command_indexes(
                    &tx,
                    &request.session_id,
                    request
                        .message_projection
                        .as_ref()
                        .map(|message| message.session_id.as_str()),
                )?;
                tx.commit()?;
                return Ok(CommandTransactionResult {
                    result,
                    indexes,
                    feed_entries: Vec::new(),
                });
            }
            let row = load_command_row(&tx, &request.session_id)?
                .with_context(|| format!("session {} not found", request.session_id))?;
            let mut aggregate = replay_session_events(&tx, &request.session_id)?;
            let previous_task_plan = aggregate.task_plan.clone();
            let event = aggregate.execute(command)?;
            let projection = aggregate.query(SessionQuery::Lifecycle);
            let task_plan_changed = projection.task_plan != previous_task_plan;
            let publish_task_projection = task_projection_requested || task_plan_changed;
            let mut management: SessionManagement =
                serde_json::from_str(&row.management_json).with_context(|| {
                    format!("invalid management_json for session {}", request.session_id)
                })?;
            let mut metadata: SessionMetadata =
                serde_json::from_str(&row.session_json).with_context(|| {
                    format!("invalid session_json for session {}", request.session_id)
                })?;
            let state_text = session_state_text(projection.state)?;
            let status = projection.state.ui_status();
            management.session_last_update_at =
                chrono::DateTime::<chrono::Utc>::from_timestamp_millis(now_ms)
                    .context("session command timestamp is outside the supported range")?;
            if let Some(name) = auto_name
                .filter(|_| task_plan_changed)
                .filter(|_| management.auto_session_name)
            {
                management.session_name = name;
            }
            management.replace_lifecycle_projection(projection.clone());
            let task_management = if publish_task_projection {
                task_management_value(&projection.task_plan)
            } else {
                serde_json::from_str(&row.task_management_json).with_context(|| {
                    format!(
                        "invalid task_management_json for session {}",
                        request.session_id
                    )
                })?
            };
            sync_metadata_from_management(&mut metadata, &management);
            let management_json = serde_json::to_string(&management)?;
            let session_json = serde_json::to_string(&metadata)?;
            let task_management_json = serde_json::to_string(&task_management)?;
            let name = management.session_name.clone();
            tx.execute(
                "UPDATE sessions
                 SET name = ?2, parent_id = ?3, updated_at = MAX(updated_at, ?4),
                     state = ?5, status = ?6, task_management_json = ?7,
                     management_json = ?8, session_json = ?9
                 WHERE session_id = ?1",
                params![
                    request.session_id,
                    name,
                    projection.parent_id,
                    now_ms,
                    state_text,
                    status,
                    task_management_json,
                    management_json,
                    session_json,
                ],
            )?;
            append_session_event(&tx, &request.session_id, &event)?;
            let mut feed_entries = Vec::new();
            if let Some(message) = request.message_projection.clone() {
                feed_entries.push(persist_command_message_projection(
                    &tx,
                    &request.command_id,
                    message,
                )?);
            }
            let final_row = load_command_row(&tx, &request.session_id)?
                .with_context(|| format!("session {} disappeared", request.session_id))?;
            let projection_event_id = format!("{}:session-projection", request.command_id);
            let projection_event = SessionFeedEvent::SessionProjectionUpdated {
                projection: projection.clone(),
                session_name: name.clone(),
                updated_at: final_row.updated_at,
            };
            let projection_cursor = super::feed::append_session_feed_event_tx(
                &tx,
                &request.session_id,
                None,
                &projection_event_id,
                &projection_event,
            )?;
            feed_entries.push(SessionFeedEntry {
                session_id: request.session_id.clone(),
                cursor: projection_cursor,
                runtime_id: None,
                event_id: projection_event_id,
                event: projection_event,
            });
            let result = SessionCommandResult {
                event,
                projection,
                session_name: name,
                message_count: final_row.message_count as u64,
                last_user_message_at: final_row.last_user_message_at,
            };
            insert_session_command_receipt(
                &tx,
                &request.command_id,
                &request.session_id,
                &request_json,
                &serde_json::to_string(&result)?,
            )?;
            let indexes = load_command_indexes(
                &tx,
                &request.session_id,
                request
                    .message_projection
                    .as_ref()
                    .map(|message| message.session_id.as_str()),
            )?;
            tx.commit()?;
            Ok(CommandTransactionResult {
                result,
                indexes,
                feed_entries,
            })
        })?;

        for (session_id, index) in &result.indexes {
            if let Err(error) = self.write_session_index(
                session_id,
                &index.workspace,
                &path_text(&workspace_db_path),
                index.updated_at,
                index.last_user_message_at,
                &index.state_text,
            ) {
                tracing::warn!(
                    session_id,
                    error = %error,
                    "session command committed but derived index sync failed"
                );
            }
        }
        Ok(ExecuteSessionCommandOutcome {
            result: result.result,
            feed_entries: result.feed_entries,
        })
    }

    pub fn update_session(&self, request: UpdateSessionRequest) -> Result<SessionSnapshot> {
        Ok(self.update_session_with_feed(request)?.snapshot)
    }

    pub(crate) fn update_session_with_feed(
        &self,
        request: UpdateSessionRequest,
    ) -> Result<UpdateSessionOutcome> {
        validate_command_identity(&request.command_id, &request.session_id)?;
        validate_metadata_patch(&request.metadata)?;
        let workspace_db_path = self
            .workspace_db_path_for_session(&request.session_id)?
            .with_context(|| format!("session {} not found", request.session_id))?;
        let request_json = serde_json::to_string(&request)?;
        let now_ms = chrono::Utc::now().timestamp_millis();
        let transaction = self.with_workspace_connection(&workspace_db_path, |conn| {
            let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            if let Some(snapshot) = load_session_update_receipt(
                &tx,
                &request.command_id,
                &request.session_id,
                &request_json,
            )? {
                let index = load_index_projection(&tx, &request.session_id)?;
                tx.commit()?;
                return Ok((snapshot, index, Vec::new()));
            }

            let row = load_command_row(&tx, &request.session_id)?
                .with_context(|| format!("session {} not found", request.session_id))?;
            let mut aggregate = replay_session_events(&tx, &request.session_id)?;
            if let Some(patch) = request.task_plan_patch.clone() {
                let event = aggregate.execute(SessionCommand::ApplyTaskPlanPatch { patch })?;
                append_session_event(&tx, &request.session_id, &event)?;
            }
            let projection = aggregate.query(SessionQuery::Lifecycle);
            let mut management: SessionManagement = serde_json::from_str(&row.management_json)
                .with_context(|| {
                    format!("invalid management_json for session {}", request.session_id)
                })?;
            let mut metadata: SessionMetadata =
                serde_json::from_str(&row.session_json).with_context(|| {
                    format!("invalid session_json for session {}", request.session_id)
                })?;
            let updated_at = row.updated_at.max(now_ms);
            apply_metadata_patch(&mut management, &mut metadata, &request.metadata);
            if request.metadata.name.is_none() && management.auto_session_name {
                if let Some(name) = request
                    .task_plan_patch
                    .as_ref()
                    .and_then(task_plan_patch_summary_for_auto_name)
                {
                    management.session_name = name;
                }
            }
            management.session_last_update_at =
                chrono::DateTime::<chrono::Utc>::from_timestamp_millis(updated_at)
                    .context("session update timestamp is outside the supported range")?;
            management.replace_lifecycle_projection(projection.clone());
            sync_metadata_from_management(&mut metadata, &management);
            let task_management = task_management_value(&projection.task_plan);
            let state_text = session_state_text(projection.state)?;
            let status = projection.state.ui_status();
            let name = management.session_name.clone();
            tx.execute(
                "UPDATE sessions
                 SET name = ?2, parent_id = ?3, updated_at = ?4,
                     state = ?5, status = ?6, task_management_json = ?7,
                     management_json = ?8, session_json = ?9
                 WHERE session_id = ?1",
                params![
                    request.session_id,
                    name,
                    projection.parent_id,
                    updated_at,
                    state_text,
                    status,
                    serde_json::to_string(&task_management)?,
                    serde_json::to_string(&management)?,
                    serde_json::to_string(&metadata)?,
                ],
            )?;
            let snapshot = load_session_snapshot_tx(&tx, &request.session_id, projection)?;
            let event_id = format!("{}:session-snapshot", request.command_id);
            let event = SessionFeedEvent::SessionSnapshotUpdated {
                snapshot: Box::new(snapshot.clone()),
            };
            let cursor = super::feed::append_session_feed_event_tx(
                &tx,
                &request.session_id,
                None,
                &event_id,
                &event,
            )?;
            let feed_entry = SessionFeedEntry {
                session_id: request.session_id.clone(),
                cursor,
                runtime_id: None,
                event_id,
                event,
            };
            insert_session_command_receipt(
                &tx,
                &request.command_id,
                &request.session_id,
                &request_json,
                &serde_json::to_string(&snapshot)?,
            )?;
            let index = load_index_projection(&tx, &request.session_id)?;
            tx.commit()?;
            Ok((snapshot, index, vec![feed_entry]))
        })?;
        let (snapshot, index, feed_entries) = transaction;
        if let Err(error) = self.write_session_index(
            &request.session_id,
            &index.workspace,
            &path_text(&workspace_db_path),
            index.updated_at,
            index.last_user_message_at,
            &index.state_text,
        ) {
            tracing::warn!(
                session_id = %request.session_id,
                error = %error,
                "session update committed but derived index sync failed"
            );
        }
        Ok(UpdateSessionOutcome {
            snapshot,
            feed_entries,
        })
    }

    pub(super) fn workspace_db_path_for_session(
        &self,
        session_id: &str,
    ) -> Result<Option<std::path::PathBuf>> {
        self.with_index_connection(|conn| {
            conn.query_row(
                "SELECT workspace_db_path FROM sessions WHERE session_id = ?1",
                params![session_id],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map(|value| value.map(Into::into))
            .map_err(Into::into)
        })
    }

    fn load_fork_projection(&self, request: &CreateSessionRequest) -> Result<ForkProjection> {
        if !request.copy_context {
            return Ok(ForkProjection::default());
        }
        let SessionCommand::ForkSession { parent_id } = &request.creation_command else {
            anyhow::bail!("copy_context requires a fork_session creation command");
        };
        let source_db = self
            .workspace_db_path_for_session(parent_id)?
            .with_context(|| format!("fork source session {parent_id} not found"))?;
        let (records, todos) = self.with_workspace_connection(&source_db, |conn| {
            let tx = conn.transaction_with_behavior(TransactionBehavior::Deferred)?;
            let todos_json = tx
                .query_row(
                    "SELECT todos_json FROM sessions WHERE session_id = ?1",
                    params![parent_id],
                    |row| row.get::<_, String>(0),
                )
                .optional()?
                .with_context(|| format!("fork source session {parent_id} not found"))?;
            let records = {
                let mut statement = tx.prepare(
                    "SELECT message_id, role, created_at, updated_at, record_json
                     FROM session_records
                     WHERE session_id = ?1 AND role IN ('user', 'assistant')
                     ORDER BY created_at ASC, id ASC",
                )?;
                let rows = statement.query_map(params![parent_id], |row| {
                    Ok(SourceProjection {
                        message_id: row.get(0)?,
                        role: row.get(1)?,
                        created_at: row.get(2)?,
                        updated_at: row.get(3)?,
                        record_json: row.get(4)?,
                    })
                })?;
                rows.collect::<std::result::Result<Vec<_>, _>>()?
            };
            tx.commit()?;
            Ok((records, serde_json::from_str(&todos_json)?))
        })?;
        rewrite_fork_projection(records, todos, &request.session_id)
    }

    pub(super) fn write_session_index(
        &self,
        session_id: &str,
        workspace: &str,
        workspace_db_path: &str,
        updated_at: i64,
        last_user_message_at: Option<i64>,
        state: &str,
    ) -> Result<()> {
        self.with_index_connection(|conn| {
            conn.execute(
                "INSERT INTO sessions(
                    session_id, workspace, workspace_db_path, updated_at,
                    last_user_message_at, state
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                 ON CONFLICT(session_id) DO UPDATE SET
                    workspace=excluded.workspace, workspace_db_path=excluded.workspace_db_path,
                    updated_at=excluded.updated_at,
                    last_user_message_at=excluded.last_user_message_at,
                    state=excluded.state",
                params![
                    session_id,
                    workspace,
                    workspace_db_path,
                    updated_at,
                    last_user_message_at,
                    state,
                ],
            )?;
            Ok(())
        })
    }

    pub(super) fn update_session_index_lifecycle(
        &self,
        session_id: &str,
        state: lifecycle::SessionState,
        updated_at: Option<i64>,
    ) -> Result<()> {
        let state = session_state_text(state)?;
        self.with_index_connection(|conn| {
            let changed = conn.execute(
                "UPDATE sessions SET state = ?2,
                    updated_at = CASE
                        WHEN ?3 IS NULL THEN updated_at
                        ELSE MAX(updated_at, ?3)
                    END
                 WHERE session_id = ?1",
                params![session_id, state, updated_at],
            )?;
            if changed == 0 {
                anyhow::bail!("session {session_id} not found in index");
            }
            Ok(())
        })
    }
}

#[derive(Default)]
struct ForkProjection {
    records: Vec<SessionRecordProjection>,
    todos: Vec<Value>,
    last_user_message_at: Option<i64>,
}

struct SourceProjection {
    message_id: String,
    role: String,
    created_at: i64,
    updated_at: i64,
    record_json: String,
}

fn rewrite_fork_projection(
    source: Vec<SourceProjection>,
    todos: Vec<Value>,
    target_session_id: &str,
) -> Result<ForkProjection> {
    let now = chrono::Utc::now().timestamp_millis();
    let id_map = source
        .iter()
        .enumerate()
        .map(|(index, record)| {
            (
                record.message_id.clone(),
                new_fork_message_id(now.saturating_add(index as i64)),
            )
        })
        .collect::<HashMap<_, _>>();
    let last_user_message_at = source
        .iter()
        .filter(|record| record.role == "user")
        .map(|record| record.updated_at.max(record.created_at))
        .max();
    let records = source
        .into_iter()
        .map(|source| {
            let message_id = id_map
                .get(&source.message_id)
                .cloned()
                .with_context(|| format!("fork message id map missing {}", source.message_id))?;
            let mut record: Value = serde_json::from_str(&source.record_json)
                .with_context(|| format!("invalid fork source record {}", source.message_id))?;
            if string_at(&record, &["id"]).as_deref() != Some(source.message_id.as_str()) {
                anyhow::bail!(
                    "fork source record id does not match message {}",
                    source.message_id
                );
            }
            if string_at(&record, &["role"]).as_deref() != Some(source.role.as_str()) {
                anyhow::bail!(
                    "fork source record role does not match message {}",
                    source.message_id
                );
            }
            let source_parent_id = record
                .get("parent_id")
                .and_then(Value::as_str)
                .map(ToString::to_string);
            let parts = record
                .get_mut("parts")
                .and_then(Value::as_array_mut)
                .with_context(|| {
                    format!("fork source message {} has no parts", source.message_id)
                })?;
            for part in parts {
                let object = part.as_object_mut().with_context(|| {
                    format!(
                        "fork source message {} contains a non-object part",
                        source.message_id
                    )
                })?;
                let part_id = object.get("id").and_then(Value::as_str).with_context(|| {
                    format!(
                        "fork source message {} contains a part without an id",
                        source.message_id
                    )
                })?;
                if part_id.trim().is_empty() {
                    anyhow::bail!(
                        "fork source message {} contains an empty part id",
                        source.message_id
                    );
                }
                object.insert("id".to_string(), Value::String(Uuid::new_v4().to_string()));
            }
            let object = record.as_object_mut().with_context(|| {
                format!("fork source record {} is not an object", source.message_id)
            })?;
            object.insert("id".to_string(), Value::String(message_id.clone()));
            object.insert(
                "session_id".to_string(),
                Value::String(target_session_id.to_string()),
            );
            object.insert(
                "parent_id".to_string(),
                source_parent_id
                    .and_then(|parent_id| id_map.get(&parent_id).cloned())
                    .map(Value::String)
                    .unwrap_or(Value::Null),
            );
            Ok(SessionRecordProjection {
                session_id: target_session_id.to_string(),
                message_id,
                role: source.role,
                created_at: source.created_at,
                updated_at: source.updated_at,
                record,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(ForkProjection {
        records,
        todos,
        last_user_message_at,
    })
}

fn new_fork_message_id(now: i64) -> String {
    format!("msg-{now:013}-{}", Uuid::new_v4())
}

struct CommandRow {
    updated_at: i64,
    last_user_message_at: Option<i64>,
    message_count: i64,
    task_management_json: String,
    management_json: String,
    session_json: String,
}

fn load_command_row(tx: &Transaction<'_>, session_id: &str) -> Result<Option<CommandRow>> {
    tx.query_row(
        "SELECT updated_at, last_user_message_at, message_count,
                task_management_json, management_json, session_json
         FROM sessions WHERE session_id = ?1",
        params![session_id],
        |row| {
            Ok(CommandRow {
                updated_at: row.get(0)?,
                last_user_message_at: row.get(1)?,
                message_count: row.get(2)?,
                task_management_json: row.get(3)?,
                management_json: row.get(4)?,
                session_json: row.get(5)?,
            })
        },
    )
    .optional()
    .map_err(Into::into)
}

struct CommandTransactionResult {
    result: SessionCommandResult,
    indexes: Vec<(String, WorkspaceIndexProjection)>,
    feed_entries: Vec<SessionFeedEntry>,
}

struct WorkspaceIndexProjection {
    workspace: String,
    updated_at: i64,
    last_user_message_at: Option<i64>,
    state_text: String,
}

fn validate_command_identity(command_id: &str, session_id: &str) -> Result<()> {
    if command_id.trim().is_empty() {
        anyhow::bail!("command_id must be non-empty");
    }
    if session_id.trim().is_empty() {
        anyhow::bail!("session_id must be non-empty");
    }
    Ok(())
}

fn load_session_command_receipt(
    tx: &Transaction<'_>,
    command_id: &str,
    session_id: &str,
    request_json: &str,
) -> Result<Option<SessionCommandResult>> {
    let receipt = tx
        .query_row(
            "SELECT session_id, request_json, result_json
             FROM session_command_receipts WHERE command_id = ?1",
            params![command_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            },
        )
        .optional()?;
    let Some((stored_session_id, stored_request_json, result_json)) = receipt else {
        return Ok(None);
    };
    if stored_session_id != session_id || stored_request_json != request_json {
        anyhow::bail!("session command id {command_id} was reused with different content");
    }

    serde_json::from_str(&result_json)
        .with_context(|| format!("invalid result receipt for session command {command_id}"))
        .map(Some)
}

fn load_session_update_receipt(
    tx: &Transaction<'_>,
    command_id: &str,
    session_id: &str,
    request_json: &str,
) -> Result<Option<SessionSnapshot>> {
    let receipt = tx
        .query_row(
            "SELECT session_id, request_json, result_json
             FROM session_command_receipts WHERE command_id = ?1",
            params![command_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            },
        )
        .optional()?;
    let Some((stored_session_id, stored_request_json, result_json)) = receipt else {
        return Ok(None);
    };
    if stored_session_id != session_id || stored_request_json != request_json {
        anyhow::bail!("session command id {command_id} was reused with different content");
    }
    serde_json::from_str(&result_json)
        .with_context(|| format!("invalid result receipt for session update {command_id}"))
        .map(Some)
}

fn insert_session_command_receipt(
    tx: &Transaction<'_>,
    command_id: &str,
    session_id: &str,
    request_json: &str,
    result_json: &str,
) -> Result<()> {
    tx.execute(
        "INSERT INTO session_command_receipts(command_id, session_id, request_json, result_json)
         VALUES (?1, ?2, ?3, ?4)",
        params![command_id, session_id, request_json, result_json],
    )?;
    Ok(())
}

fn load_index_projection(
    tx: &Transaction<'_>,
    session_id: &str,
) -> Result<WorkspaceIndexProjection> {
    tx.query_row(
        "SELECT workspace, updated_at, last_user_message_at, state
         FROM sessions WHERE session_id = ?1",
        params![session_id],
        |row| {
            Ok(WorkspaceIndexProjection {
                workspace: row.get(0)?,
                updated_at: row.get(1)?,
                last_user_message_at: row.get(2)?,
                state_text: row.get(3)?,
            })
        },
    )
    .with_context(|| format!("session {session_id} receipt has no workspace session"))
}

fn load_command_indexes(
    tx: &Transaction<'_>,
    command_session_id: &str,
    message_session_id: Option<&str>,
) -> Result<Vec<(String, WorkspaceIndexProjection)>> {
    let mut indexes = vec![(
        command_session_id.to_string(),
        load_index_projection(tx, command_session_id)?,
    )];
    if let Some(message_session_id) =
        message_session_id.filter(|session_id| *session_id != command_session_id)
    {
        indexes.push((
            message_session_id.to_string(),
            load_index_projection(tx, message_session_id)?,
        ));
    }
    Ok(indexes)
}

struct DeltaSessionRow {
    message_count: i64,
    metadata: SessionMetadata,
    management_json: String,
    next_context_sequence: u64,
    retained_from_sequence: u64,
    next_management_sequence: u64,
}

struct DeltaIndexUpdate {
    updated_at: i64,
    last_user_message_at: Option<i64>,
    state_text: String,
    next_sequence: u64,
    next_management_sequence: u64,
    feed_entries: Vec<session_log_contract::SessionFeedEntry>,
}

pub(crate) struct PersistSessionDeltaOutcome {
    pub(crate) next_sequence: u64,
    pub(crate) next_management_sequence: u64,
    pub(crate) feed_entries: Vec<session_log_contract::SessionFeedEntry>,
}

pub(crate) struct CreateSessionOutcome {
    pub(crate) result: SessionCommandResult,
    pub(crate) feed_entries: Vec<SessionFeedEntry>,
}

pub(crate) struct ExecuteSessionCommandOutcome {
    pub(crate) result: SessionCommandResult,
    pub(crate) feed_entries: Vec<SessionFeedEntry>,
}

pub(crate) struct UpdateSessionOutcome {
    pub(crate) snapshot: SessionSnapshot,
    pub(crate) feed_entries: Vec<SessionFeedEntry>,
}

fn load_delta_session_row(
    tx: &Transaction<'_>,
    session_id: &str,
) -> Result<Option<DeltaSessionRow>> {
    tx.query_row(
        "SELECT message_count, session_json, management_json,
                next_context_sequence, retained_from_sequence, next_management_sequence
         FROM sessions WHERE session_id = ?1",
        params![session_id],
        |row| {
            Ok(DeltaSessionRow {
                message_count: row.get(0)?,
                metadata: serde_json::from_str(&row.get::<_, String>(1)?).map_err(|error| {
                    rusqlite::Error::FromSqlConversionFailure(
                        1,
                        rusqlite::types::Type::Text,
                        Box::new(error),
                    )
                })?,
                management_json: row.get(2)?,
                next_context_sequence: row.get(3)?,
                retained_from_sequence: row.get(4)?,
                next_management_sequence: row.get(5)?,
            })
        },
    )
    .optional()
    .map_err(Into::into)
}

fn upsert_delta_projection(
    tx: &Transaction<'_>,
    session_id: &str,
    mut incoming: session_log_contract::SessionRecordProjection,
) -> Result<(i64, SessionRecordProjection)> {
    if incoming.session_id != session_id {
        anyhow::bail!(
            "delta projection session {} does not match transaction session {session_id}",
            incoming.session_id
        );
    }
    if incoming.message_id.trim().is_empty() || incoming.role.trim().is_empty() {
        anyhow::bail!("delta projection identity missing for session {session_id}");
    }
    if incoming.record.get("id").and_then(Value::as_str) != Some(incoming.message_id.as_str()) {
        anyhow::bail!(
            "delta projection record id does not match message {} in session {}",
            incoming.message_id,
            session_id
        );
    }
    if incoming.record.get("role").and_then(Value::as_str) != Some(incoming.role.as_str()) {
        anyhow::bail!(
            "delta projection record role does not match {} in session {}",
            incoming.role,
            session_id
        );
    }
    let record = incoming.record.as_object_mut().with_context(|| {
        format!("delta projection record is not an object in session {session_id}")
    })?;
    if record.get("session_id").and_then(Value::as_str) != Some(session_id) {
        anyhow::bail!("delta projection record session does not match session {session_id}");
    }
    let existing = tx
        .query_row(
            "SELECT record_json FROM session_records WHERE session_id = ?1 AND message_id = ?2",
            params![session_id, incoming.message_id],
            |row| row.get::<_, String>(0),
        )
        .optional()?;
    let inserted = i64::from(existing.is_none());
    let record = match existing {
        Some(existing) if incoming.role == "assistant" => {
            merge_delta_projection(serde_json::from_str(&existing)?, incoming.record)
        }
        _ => incoming.record,
    };
    let created_at = i64_at(&record, &["created_at"]).unwrap_or(incoming.created_at);
    let updated_at = i64_at(&record, &["updated_at"]).unwrap_or(incoming.updated_at);
    let projection = SessionRecordProjection {
        session_id: incoming.session_id,
        message_id: incoming.message_id,
        role: incoming.role,
        created_at,
        updated_at,
        record,
    };
    tx.execute(
        "INSERT INTO session_records(
            session_id, message_id, role, created_at, updated_at, record_json
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT(session_id, message_id) DO UPDATE SET
            role = excluded.role, created_at = excluded.created_at,
            updated_at = excluded.updated_at, record_json = excluded.record_json",
        params![
            session_id,
            projection.message_id,
            projection.role,
            projection.created_at,
            projection.updated_at,
            serde_json::to_string(&projection.record)?,
        ],
    )?;
    Ok((inserted, projection))
}

fn persist_command_message_projection(
    tx: &Transaction<'_>,
    command_id: &str,
    projection: SessionRecordProjection,
) -> Result<SessionFeedEntry> {
    let session_id = projection.session_id.clone();
    let row = load_command_row(tx, &session_id)?
        .with_context(|| format!("message session {session_id} not found"))?;
    let (inserted, projection) = upsert_delta_projection(tx, &session_id, projection)?;
    let mut management: SessionManagement = serde_json::from_str(&row.management_json)
        .with_context(|| format!("invalid management_json for session {session_id}"))?;
    let metadata: SessionMetadata = serde_json::from_str(&row.session_json)
        .with_context(|| format!("invalid session_json for session {session_id}"))?;
    let message_count = row.message_count.saturating_add(inserted);
    let mut last_user_message_at = row.last_user_message_at;
    if projection.role == "user" {
        let message_at =
            chrono::DateTime::<chrono::Utc>::from_timestamp_millis(projection.updated_at)
                .with_context(|| {
                    format!("invalid user message timestamp in session {session_id}")
                })?;
        last_user_message_at = Some(
            last_user_message_at
                .unwrap_or(projection.updated_at)
                .max(projection.updated_at),
        );
        if message_at > management.session_last_user_message_at {
            management.record_user_message_at(message_at);
        }
        if inserted > 0 {
            if management.input.user_input.trim().is_empty() {
                if let Some(text) = projection
                    .record
                    .get("parts")
                    .and_then(Value::as_array)
                    .and_then(|parts| parts.iter().find_map(|part| part.get("text")?.as_str()))
                {
                    management.input.user_input = text.to_string();
                }
            }
            management
                .session_log
                .push(serde_json::to_string(&projection.record)?);
        }
    }
    let management_json = serde_json::to_string(&management)?;
    let updated_at = row.updated_at.max(projection.updated_at);
    tx.execute(
        "UPDATE sessions
         SET updated_at = MAX(updated_at, ?2), last_user_message_at = ?3,
             message_count = ?4, management_json = ?5, session_json = ?6
         WHERE session_id = ?1",
        params![
            session_id,
            projection.updated_at,
            last_user_message_at,
            message_count,
            management_json,
            serde_json::to_string(&metadata)?,
        ],
    )?;
    let event_id = format!("{command_id}:message:{}", projection.message_id);
    let event = SessionFeedEvent::MessageUpserted {
        message: projection,
    };
    let cursor =
        super::feed::append_session_feed_event_tx(tx, &session_id, None, &event_id, &event)?;
    Ok(SessionFeedEntry {
        session_id,
        cursor,
        runtime_id: None,
        event_id,
        event,
    })
}

fn merge_delta_projection(mut existing: Value, incoming: Value) -> Value {
    if projection_has_unstable_parts(&existing) || projection_has_unstable_parts(&incoming) {
        return incoming;
    }
    let Some(existing_parts) = existing.get_mut("parts").and_then(Value::as_array_mut) else {
        return incoming;
    };
    if let Some(incoming_parts) = incoming.get("parts").and_then(Value::as_array) {
        for part in incoming_parts {
            let part_id = part.get("id").and_then(Value::as_str);
            if let Some(current) = part_id.and_then(|part_id| {
                existing_parts
                    .iter_mut()
                    .find(|candidate| candidate.get("id").and_then(Value::as_str) == Some(part_id))
            }) {
                *current = part.clone();
            } else {
                existing_parts.push(part.clone());
            }
        }
        existing_parts.sort_by_key(|part| match part.get("type").and_then(Value::as_str) {
            Some("text") => 0,
            Some("tool") => 1,
            _ => 2,
        });
    }
    let existing_created = i64_at(&existing, &["created_at"]);
    let incoming_created = i64_at(&incoming, &["created_at"]);
    let existing_updated = i64_at(&existing, &["updated_at"]);
    let incoming_updated = i64_at(&incoming, &["updated_at"]);
    if let Some(object) = existing.as_object_mut() {
        if let Some(created_at) = existing_created.into_iter().chain(incoming_created).min() {
            object.insert("created_at".to_string(), Value::Number(created_at.into()));
        }
        if let Some(updated_at) = existing_updated.into_iter().chain(incoming_updated).max() {
            object.insert("updated_at".to_string(), Value::Number(updated_at.into()));
        }
    }
    existing
}

fn projection_has_unstable_parts(projection: &Value) -> bool {
    projection
        .get("parts")
        .and_then(Value::as_array)
        .is_some_and(|parts| {
            parts.iter().any(|part| {
                part.get("id")
                    .and_then(Value::as_str)
                    .is_none_or(|id| id.trim().is_empty())
            })
        })
}

fn task_summary_for_auto_name(command: &SessionCommand) -> Option<String> {
    match command {
        SessionCommand::ApplyTaskPatch { patch, .. } => patch.task_summary.clone(),
        SessionCommand::ApplyTaskPatches { tasks, .. } => tasks
            .iter()
            .rev()
            .find_map(|patch| patch.task_summary.clone()),
        SessionCommand::ApplyTaskPlanPatch { patch } => patch
            .task
            .as_ref()
            .and_then(|patch| patch.task_summary.clone())
            .or_else(|| {
                patch.tasks.as_ref().and_then(|tasks| {
                    tasks
                        .iter()
                        .rev()
                        .find_map(|patch| patch.task_summary.clone())
                })
            }),
        _ => None,
    }
    .map(|value| value.trim().to_string())
    .filter(|value| !value.is_empty())
}

fn task_plan_patch_summary_for_auto_name(
    patch: &lifecycle::SessionTaskPlanPatch,
) -> Option<String> {
    patch
        .task
        .as_ref()
        .and_then(|patch| patch.task_summary.clone())
        .or_else(|| {
            patch.tasks.as_ref().and_then(|tasks| {
                tasks
                    .iter()
                    .rev()
                    .find_map(|patch| patch.task_summary.clone())
            })
        })
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn set_json_field(value: &mut Value, key: &str, field: Value) {
    if let Some(object) = value.as_object_mut() {
        object.insert(key.to_string(), field);
    }
}

fn validate_metadata_patch(patch: &SessionMetadataPatch) -> Result<()> {
    if patch
        .name
        .as_deref()
        .is_some_and(|name| name.trim().is_empty())
    {
        anyhow::bail!("session name must be non-empty");
    }
    Ok(())
}

fn apply_metadata_patch(
    management: &mut SessionManagement,
    metadata: &mut SessionMetadata,
    patch: &SessionMetadataPatch,
) {
    if let Some(name) = &patch.name {
        management.session_name = name.clone();
    }
    if let Some(model) = &patch.model {
        metadata.model = Some(model.clone());
    }
    if patch.clear_agent {
        metadata.agent = None;
    } else if let Some(agent) = &patch.agent {
        metadata.agent = Some(agent.clone());
    }
    if let Some(session_type) = &patch.session_type {
        metadata.session_type = session_type.clone();
    }
    if let Some(value) = patch.kill_processes_on_start {
        metadata.kill_processes_on_start = value;
    }
    if let Some(value) = patch.validator_enabled {
        metadata.validator_enabled = value;
    }
    if let Some(value) = patch.force_planning {
        metadata.force_planning = value;
    }
    if let Some(value) = patch.disable_permission_restrictions {
        management.disable_permission_restrictions = value;
    }
    if let Some(value) = patch.use_last_tool_call_response {
        management.use_last_tool_call_response = value;
    }
    if let Some(value) = patch.auto_session_name {
        management.auto_session_name = value;
    }
}

fn sync_metadata_from_management(
    metadata: &mut SessionMetadata,
    management: &SessionManagement,
) {
    metadata.disable_permission_restrictions = management.disable_permission_restrictions;
    metadata.use_last_tool_call_response = management.use_last_tool_call_response;
    metadata.auto_session_name = management.auto_session_name;
    metadata.context_tokens = management.context_tokens;
    metadata.runtime_usage.clone_from(&management.runtime_usage);
}

pub(super) fn load_session_snapshot_tx(
    tx: &Transaction<'_>,
    session_id: &str,
    lifecycle_projection: lifecycle::SessionProjection,
) -> Result<SessionSnapshot> {
    tx.query_row(
        "SELECT workspace, name, created_at, updated_at,
                last_user_message_at, message_count, management_json, session_json, todos_json
         FROM sessions WHERE session_id = ?1",
        params![session_id],
        |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, Option<i64>>(4)?,
                row.get::<_, i64>(5)?,
                row.get::<_, String>(6)?,
                row.get::<_, String>(7)?,
                row.get::<_, String>(8)?,
            ))
        },
    )
    .map_err(anyhow::Error::from)
    .and_then(
        |(
            workspace,
            name,
            created_at,
            updated_at,
            last_user_message_at,
            message_count,
            management_json,
            session_json,
            todos_json,
        )| {
            let snapshot = SessionSnapshot {
                session_id: session_id.to_string(),
                workspace,
                name,
                created_at,
                updated_at,
                last_user_message_at,
                message_count: message_count as u64,
                lifecycle_projection,
                management: serde_json::from_str(&management_json)?,
                metadata: serde_json::from_str(&session_json)?,
                todos: serde_json::from_str(&todos_json)?,
            };
            snapshot.validate().map_err(anyhow::Error::msg)?;
            Ok(snapshot)
        },
    )
}
