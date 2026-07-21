//! Single reducer for the durable Session feed consumed by Gateway.

use crate::api::session::frontend_safe_reply_message;
use crate::contracts::{CommandUpdatedProperties, GlobalEvent, SessionContextTokens};
use crate::session::{session_store, MessageRole, SessionStore};
use crate::session_db_client::SessionDbClient;
use anyhow::{bail, Context, Result};
use session_log_contract::client::{
    open_session_feed_subscription, SessionFeedSubscription, SessionFeedSubscriptionCancellation,
};
use session_log_contract::{SessionFeedCommandUpdate, SessionFeedEntry, SessionFeedEvent};
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::task::Poll;
use std::thread::JoinHandle;
use std::time::Duration;

const REPLAY_PAGE_SIZE: u64 = 1_000;
const RECONNECT_DELAY: Duration = Duration::from_millis(100);
const SUBSCRIPTION_POLL_INTERVAL: Duration = Duration::from_millis(250);

pub struct SessionFeedTailer {
    cancellation: Arc<Mutex<Option<SessionFeedSubscriptionCancellation>>>,
    stopping: Arc<AtomicBool>,
    failure: Arc<Mutex<Option<String>>>,
    thread: Option<JoinHandle<()>>,
}

impl SessionFeedTailer {
    pub fn shutdown(mut self) -> Result<()> {
        self.stopping.store(true, Ordering::SeqCst);
        let cancel_result = self
            .cancellation
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .take()
            .map_or(Ok(()), |cancellation| cancellation.cancel());
        let Some(thread) = self.thread.take() else {
            return Ok(());
        };
        match thread.join() {
            Ok(()) => {}
            Err(_) => bail!("session feed tailer thread panicked"),
        }
        if let Some(error) = self
            .failure
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .take()
        {
            bail!("{error}");
        }
        cancel_result
    }
}

pub fn start_session_feed_tailer() -> Result<(SessionFeedTailer, tokio::sync::oneshot::Receiver<()>)>
{
    let mut subscription = open_session_feed_subscription()?;
    let cancellation = subscription.cancellation_handle()?;
    let client = SessionDbClient::discover()?;
    let mut reducer = SessionFeedReducer::new(session_store().clone());
    replay_all_sessions(&client, &mut reducer)?;
    let stopping = Arc::new(AtomicBool::new(false));
    let thread_stopping = Arc::clone(&stopping);
    let cancellation = Arc::new(Mutex::new(Some(cancellation)));
    let thread_cancellation = Arc::clone(&cancellation);
    let failure = Arc::new(Mutex::new(None));
    let thread_failure = Arc::clone(&failure);
    let (done_sender, done_receiver) = tokio::sync::oneshot::channel();

    let thread = std::thread::Builder::new()
        .name("gateway-session-feed".to_string())
        .spawn(move || {
            let outcome = loop {
                if thread_stopping.load(Ordering::SeqCst) {
                    break Ok(());
                }
                match subscription.poll_next_entry(SUBSCRIPTION_POLL_INTERVAL) {
                    Ok(Poll::Ready(Some(entry))) => {
                        if let Err(error) = reducer.apply(entry) {
                            break Err(error.context("failed to reduce durable Session feed"));
                        }
                    }
                    Ok(Poll::Ready(None)) if thread_stopping.load(Ordering::SeqCst) => break Ok(()),
                    Ok(Poll::Ready(None)) => match reconnect_session_feed(
                        &mut reducer,
                        &thread_stopping,
                        &thread_cancellation,
                    ) {
                        Ok(Some(reconnected)) => subscription = reconnected,
                        Ok(None) => break Ok(()),
                        Err(error) => break Err(error),
                    },
                    Ok(Poll::Pending) => {
                        if thread_stopping.load(Ordering::SeqCst) {
                            break Ok(());
                        }
                    }
                    Err(_) if thread_stopping.load(Ordering::SeqCst) => break Ok(()),
                    Err(error) if is_subscription_shutdown(&error) => {
                        match reconnect_session_feed(
                            &mut reducer,
                            &thread_stopping,
                            &thread_cancellation,
                        ) {
                            Ok(Some(reconnected)) => subscription = reconnected,
                            Ok(None) => break Ok(()),
                            Err(error) => break Err(error),
                        }
                    }
                    Err(error) => break Err(error.context("durable Session feed stopped")),
                }
            };
            if let Err(error) = outcome {
                *thread_failure
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(format!("{error:#}"));
            }
            let _ = done_sender.send(());
        })
        .context("failed to spawn Gateway session feed tailer")?;
    Ok((
        SessionFeedTailer {
            cancellation,
            stopping,
            failure,
            thread: Some(thread),
        },
        done_receiver,
    ))
}

fn reconnect_session_feed(
    reducer: &mut SessionFeedReducer,
    stopping: &AtomicBool,
    cancellation: &Mutex<Option<SessionFeedSubscriptionCancellation>>,
) -> Result<Option<SessionFeedSubscription>> {
    cancellation
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .take();
    loop {
        if stopping.load(Ordering::SeqCst) {
            return Ok(None);
        }
        let subscription = match open_session_feed_subscription() {
            Ok(subscription) => subscription,
            Err(error) if is_reconnectable_subscription_error(&error) => {
                std::thread::sleep(RECONNECT_DELAY);
                continue;
            }
            Err(error) => {
                return Err(error.context("failed to reconnect durable Session feed"));
            }
        };
        let current_cancellation = subscription.cancellation_handle()?;
        {
            let mut active = cancellation
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            if stopping.load(Ordering::SeqCst) {
                let _ = current_cancellation.cancel();
                return Ok(None);
            }
            *active = Some(current_cancellation);
        }

        let client = SessionDbClient::discover()?;
        match replay_all_sessions(&client, reducer) {
            Ok(()) => return Ok(Some(subscription)),
            Err(error)
                if is_reconnectable_subscription_error(&error)
                    || !session_log_contract::client::service_is_running() =>
            {
                cancellation
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .take();
                std::thread::sleep(RECONNECT_DELAY);
            }
            Err(error) => {
                return Err(error.context("failed to replay Session feed after reconnect"));
            }
        }
    }
}

fn replay_all_sessions(client: &SessionDbClient, reducer: &mut SessionFeedReducer) -> Result<()> {
    let mut canonical_session_ids = HashSet::new();
    for workspace in client.list_workspaces()? {
        canonical_session_ids.extend(replay_directory_with_reducer(
            client,
            reducer,
            &workspace.directory,
        )?);
    }
    let stale_session_ids = reducer
        .cursors
        .keys()
        .filter(|session_id| !canonical_session_ids.contains(*session_id))
        .cloned()
        .collect::<Vec<_>>();
    for session_id in stale_session_ids {
        reducer.store.remove_session_projection(&session_id);
        reducer.cursors.remove(&session_id);
        reducer.generation_event_ids.remove(&session_id);
    }
    Ok(())
}

pub(crate) fn replay_directory(
    client: &SessionDbClient,
    store: SessionStore,
    directory: &str,
) -> Result<()> {
    replay_directory_with_reducer(client, &mut SessionFeedReducer::new(store), directory)
        .map(|_| ())
}

fn replay_directory_with_reducer(
    client: &SessionDbClient,
    reducer: &mut SessionFeedReducer,
    directory: &str,
) -> Result<HashSet<String>> {
    let mut session_ids = HashSet::new();
    let mut page = 0;
    loop {
        let (page_info, sessions) =
            client.list_sessions(directory.to_string(), page, REPLAY_PAGE_SIZE)?;
        for session in sessions {
            let session_id = session.session_id;
            session_ids.insert(session_id.clone());
            if let Err(error) = replay_session(client, reducer, session_id.clone()) {
                if client.get_session(session_id.clone())?.is_none() {
                    session_ids.remove(&session_id);
                    continue;
                }
                return Err(error);
            }
            if reducer.store.get_session(&session_id).is_none() {
                if client.get_session(session_id.clone())?.is_none() {
                    continue;
                }
                bail!("session {session_id} feed is missing its creation snapshot");
            }
        }
        if (page_info.page + 1).saturating_mul(page_info.page_size) >= page_info.total {
            return Ok(session_ids);
        }
        page = page.saturating_add(1);
    }
}

fn replay_session(
    client: &SessionDbClient,
    reducer: &mut SessionFeedReducer,
    session_id: String,
) -> Result<()> {
    let mut cursor = 0;
    loop {
        let (entries, next_cursor) =
            client.read_session_feed(session_id.clone(), cursor, REPLAY_PAGE_SIZE)?;
        let count = entries.len();
        for entry in entries {
            reducer.apply(entry)?;
        }
        if count < REPLAY_PAGE_SIZE as usize {
            return Ok(());
        }
        if next_cursor <= cursor {
            bail!("session feed replay made no cursor progress for {session_id}");
        }
        cursor = next_cursor;
    }
}

fn is_subscription_shutdown(error: &anyhow::Error) -> bool {
    error.chain().any(|cause| {
        cause.downcast_ref::<std::io::Error>().is_some_and(|error| {
            matches!(
                error.kind(),
                std::io::ErrorKind::UnexpectedEof
                    | std::io::ErrorKind::BrokenPipe
                    | std::io::ErrorKind::ConnectionReset
                    | std::io::ErrorKind::ConnectionAborted
                    | std::io::ErrorKind::NotConnected
            )
        })
    })
}

fn is_reconnectable_subscription_error(error: &anyhow::Error) -> bool {
    error.chain().any(|cause| {
        cause.downcast_ref::<std::io::Error>().is_some_and(|error| {
            matches!(
                error.kind(),
                std::io::ErrorKind::NotFound
                    | std::io::ErrorKind::ConnectionRefused
                    | std::io::ErrorKind::ConnectionReset
                    | std::io::ErrorKind::ConnectionAborted
                    | std::io::ErrorKind::NotConnected
                    | std::io::ErrorKind::UnexpectedEof
                    | std::io::ErrorKind::BrokenPipe
                    | std::io::ErrorKind::TimedOut
            )
        })
    })
}

pub struct SessionFeedReducer {
    store: SessionStore,
    cursors: HashMap<String, u64>,
    generation_event_ids: HashMap<String, String>,
}

impl SessionFeedReducer {
    pub fn new(store: SessionStore) -> Self {
        Self {
            store,
            cursors: HashMap::new(),
            generation_event_ids: HashMap::new(),
        }
    }

    pub fn apply(&mut self, entry: SessionFeedEntry) -> Result<bool> {
        let session_id = entry.session_id.clone();
        let mut previous = self.cursors.get(&session_id).copied().unwrap_or(0);
        let generation_event_id = (entry.cursor == 1).then(|| entry.event_id.clone());
        if entry.cursor == 1 {
            match self.generation_event_ids.get(&session_id) {
                Some(event_id) if event_id == &entry.event_id => return Ok(false),
                Some(_) | None if previous > 0 => {
                    self.store.remove_session_projection(&session_id);
                    self.cursors.remove(&session_id);
                    previous = 0;
                }
                _ => {}
            }
        }
        if matches!(&entry.event, SessionFeedEvent::SessionDeleted {}) {
            if entry.cursor <= previous {
                return Ok(false);
            }
            self.apply_event(
                &entry.session_id,
                entry.runtime_id.as_deref(),
                entry.cursor,
                entry.event,
            )?;
            if let Some(event_id) = generation_event_id {
                self.generation_event_ids
                    .insert(session_id.clone(), event_id);
            }
            self.cursors.insert(session_id, entry.cursor);
            return Ok(true);
        }
        if entry.cursor <= previous {
            return Ok(false);
        }
        if entry.cursor != previous.saturating_add(1) {
            bail!(
                "session feed cursor gap for {}: expected {}, received {}",
                entry.session_id,
                previous.saturating_add(1),
                entry.cursor
            );
        }

        self.apply_event(
            &entry.session_id,
            entry.runtime_id.as_deref(),
            entry.cursor,
            entry.event,
        )?;
        if let Some(event_id) = generation_event_id {
            self.generation_event_ids
                .insert(session_id.clone(), event_id);
        }
        self.cursors.insert(session_id, entry.cursor);
        Ok(true)
    }

    fn apply_event(
        &self,
        session_id: &str,
        runtime_id: Option<&str>,
        cursor: u64,
        event: SessionFeedEvent,
    ) -> Result<()> {
        match event {
            SessionFeedEvent::MessageUpserted { message } => {
                let projected: crate::session::Message = serde_json::from_value(message.record)
                    .context("invalid typed Session message projection")?;
                let projected_role = match projected.role {
                    MessageRole::User => "user",
                    MessageRole::Assistant => "assistant",
                    MessageRole::System => "system",
                };
                if projected.id != message.message_id
                    || message.session_id != session_id
                    || projected.session_id != session_id
                    || projected_role != message.role
                    || projected.created_at != message.created_at
                    || projected.updated_at != message.updated_at
                {
                    bail!("Session message projection envelope does not match its payload");
                }
                self.store.upsert_feed_message(session_id, projected);
            }
            SessionFeedEvent::AssistantTextDelta {
                message_id,
                part_id,
                delta,
                created_at,
                updated_at,
            } => self.store.append_feed_text_delta(
                session_id, message_id, part_id, delta, created_at, updated_at,
            ),
            SessionFeedEvent::AgentMessage {
                message_id,
                part_id,
                reply_message,
                new_learning: _,
                runtime_status: _,
                context_tokens,
                usage,
                created_at,
                updated_at,
            } => {
                self.apply_metrics(session_id, context_tokens, usage)?;
                let reply_message = frontend_safe_reply_message(&reply_message);
                if !reply_message.trim().is_empty() {
                    let message = self.store.build_text_message_with_ids_and_times(
                        session_id,
                        MessageRole::Assistant,
                        reply_message,
                        Some(message_id),
                        Some(part_id),
                        None,
                        created_at,
                        updated_at,
                    );
                    self.store.upsert_feed_message(session_id, message);
                }
            }
            SessionFeedEvent::ToolCallUpdated {
                message_id,
                part_id,
                tool_name,
                call_id,
                state,
                metadata,
                runtime_status: _,
                context_tokens,
                usage,
                command_updates,
                created_at,
                updated_at,
            } => {
                self.apply_metrics(session_id, context_tokens, usage)?;
                let message = self.store.build_transient_tool_message_with_ids_and_times(
                    session_id, tool_name, call_id, state, metadata, message_id, part_id,
                    created_at, updated_at,
                );
                self.store.upsert_feed_message(session_id, message);
                let runtime_id = runtime_id
                    .context("tool command update feed event is missing its runtime source")?;
                self.emit_command_updates(session_id, runtime_id, command_updates);
            }
            SessionFeedEvent::TodosUpdated {
                todos,
                updated_at: _,
            } => {
                self.store.apply_todos_projection(session_id, cursor, todos);
            }
            SessionFeedEvent::SessionProjectionUpdated {
                projection,
                session_name,
                updated_at,
            } => {
                if let Some(write) =
                    self.store
                        .write_reduced_projection_cache(projection, session_name, updated_at)
                {
                    self.store.publish_session_updated(&write);
                }
            }
            SessionFeedEvent::SessionSnapshotCreated { snapshot } => {
                if snapshot.session_id != session_id {
                    bail!("Session snapshot feed envelope does not match its payload");
                }
                let write = self
                    .store
                    .write_feed_snapshot_projection_cache(&snapshot, cursor)
                    .map_err(anyhow::Error::msg)?;
                self.store.publish_session_created(&write);
            }
            SessionFeedEvent::SessionSnapshotUpdated { snapshot } => {
                if snapshot.session_id != session_id {
                    bail!("Session snapshot feed envelope does not match its payload");
                }
                let write = self
                    .store
                    .write_feed_snapshot_projection_cache(&snapshot, cursor)
                    .map_err(anyhow::Error::msg)?;
                self.store.publish_session_updated(&write);
            }
            SessionFeedEvent::SessionDeleted {} => {
                if let Some(session) = self.store.remove_session_projection(session_id) {
                    self.store.push_event(GlobalEvent::SessionDeleted {
                        properties: crate::contracts::SessionDeletedProperties {
                            session_id: session_id.to_string(),
                            info: session,
                        },
                    });
                }
            }
        }
        Ok(())
    }

    fn apply_metrics(
        &self,
        session_id: &str,
        context_tokens: Option<lifecycle::ContextTokenStats>,
        usage: Option<lifecycle::UsageReport>,
    ) -> Result<()> {
        let mut changed = false;
        if let Some(context_tokens) = context_tokens {
            changed |= self.store.update_session_context_tokens(
                session_id,
                SessionContextTokens {
                    input: context_tokens.input,
                    limit: context_tokens.limit,
                },
            );
        }
        if let Some(usage) = usage {
            changed |= self
                .store
                .update_session_runtime_usage(session_id, serde_json::to_value(usage)?);
        }
        if changed {
            self.store.push_current_session_status_event(session_id);
        }
        Ok(())
    }

    fn emit_command_updates(
        &self,
        session_id: &str,
        runtime_id: &str,
        updates: Vec<SessionFeedCommandUpdate>,
    ) {
        for update in updates {
            self.store.push_event(GlobalEvent::CommandUpdated {
                properties: CommandUpdatedProperties {
                    session_id: session_id.to_string(),
                    message_id: update.message_id,
                    part_id: update.part_id,
                    runtime_id: runtime_id.to_string(),
                    command_run_id: update.command_run_id,
                    command_id: update.command_id,
                    provider_tool_call_id: update.provider_tool_call_id,
                    command_index: update.command_index,
                    event_seq: update.event_seq,
                    status: update.status,
                    command: update.command,
                    result: update.result,
                    created_at: update.created_at,
                    updated_at: update.updated_at,
                },
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contracts::GlobalEvent;
    use lifecycle::{RuntimeProjection, RuntimeState};
    use serde_json::json;
    use session_log_contract::{
        SessionFeedCommandUpdate, SessionFeedEvent, SessionRecordProjection, SessionSnapshot,
    };

    fn entry(cursor: u64, event: SessionFeedEvent) -> SessionFeedEntry {
        SessionFeedEntry {
            session_id: "feed-session".to_string(),
            cursor,
            runtime_id: Some("feed-runtime".to_string()),
            event_id: format!("feed-event-{cursor}"),
            event,
        }
    }

    fn test_store() -> (SessionStore, String) {
        let store = SessionStore::new();
        let default_id = store
            .list_sessions()
            .into_iter()
            .next()
            .expect("default session")
            .id;
        (store, default_id)
    }

    fn snapshot(
        store: &SessionStore,
        session_id: &str,
        name: &str,
        updated_at: i64,
    ) -> SessionSnapshot {
        let mut info = store
            .get_session_info(session_id)
            .expect("snapshot source session");
        info.updated_at = updated_at;
        info.management.session_name = name.to_string();
        info.model = Some(format!("model-{name}"));
        info.agent = Some(format!("agent-{name}"));
        info.session_type = Some("general".to_string());
        info.validator_enabled = true;
        info.force_planning = true;
        let projection = info.management.lifecycle_projection();
        SessionSnapshot {
            session_id: session_id.to_string(),
            workspace: "C:/workspace".to_string(),
            name: Some(name.to_string()),
            parent_id: projection.parent_id.clone(),
            created_at: info.created_at,
            updated_at,
            last_user_message_at: info.last_user_message_at,
            state: Some("created".to_string()),
            status: Some("idle".to_string()),
            message_count: info.message_count as u64,
            task_management: info.management.task_management_json(),
            lifecycle_projection: projection,
            management: serde_json::to_value(&info.management).expect("snapshot management"),
            session: serde_json::to_value(&info).expect("snapshot session"),
            todos: vec![json!({"id": format!("todo-{name}")})],
        }
    }

    #[test]
    fn created_snapshot_establishes_an_empty_gateway_cache() {
        let (store, session_id) = test_store();
        let created = snapshot(&store, &session_id, "Created by feed", 10);
        assert!(store.delete_session(&session_id));
        let mut event_cursor = store.event_cursor();
        let mut reducer = SessionFeedReducer::new(store.clone());
        let mut created_entry = entry(
            1,
            SessionFeedEvent::SessionSnapshotCreated {
                snapshot: Box::new(created),
            },
        );
        created_entry.session_id = session_id.clone();
        created_entry.runtime_id = None;

        assert!(reducer
            .apply(created_entry)
            .expect("reduce created snapshot"));
        let session = store.get_session(&session_id).expect("created cache entry");
        assert_eq!(session.name.as_deref(), Some("Created by feed"));
        assert_eq!(session.model.as_deref(), Some("model-Created by feed"));
        assert_eq!(
            store.get_todos(&session_id)[0]["id"],
            "todo-Created by feed"
        );
        assert!(matches!(
            store.next_event(&mut event_cursor),
            Some(GlobalEvent::SessionCreated { .. })
        ));
    }

    #[test]
    fn new_feed_generation_replaces_reused_session_identity() {
        let (store, session_id) = test_store();
        let old_snapshot = snapshot(&store, &session_id, "Old generation", 10);
        assert!(store.delete_session(&session_id));
        let mut reducer = SessionFeedReducer::new(store.clone());
        let mut old_created = entry(
            1,
            SessionFeedEvent::SessionSnapshotCreated {
                snapshot: Box::new(old_snapshot),
            },
        );
        old_created.session_id = session_id.clone();
        old_created.runtime_id = None;
        old_created.event_id = "old-generation-root".to_string();
        assert!(reducer.apply(old_created).expect("reduce old generation"));

        let mut old_delta = entry(
            2,
            SessionFeedEvent::AssistantTextDelta {
                message_id: "old-generation-message".to_string(),
                part_id: "old-generation-part".to_string(),
                delta: "stale".to_string(),
                created_at: 11,
                updated_at: 12,
            },
        );
        old_delta.session_id = session_id.clone();
        assert!(reducer.apply(old_delta).expect("reduce old message"));
        let new_snapshot = snapshot(&store, &session_id, "New generation", 20);
        let mut new_created = entry(
            1,
            SessionFeedEvent::SessionSnapshotCreated {
                snapshot: Box::new(new_snapshot),
            },
        );
        new_created.session_id = session_id.clone();
        new_created.runtime_id = None;
        new_created.event_id = "new-generation-root".to_string();

        assert!(reducer.apply(new_created).expect("reduce new generation"));
        assert_eq!(
            store
                .get_session(&session_id)
                .expect("new generation cache")
                .name
                .as_deref(),
            Some("New generation")
        );
        assert!(store.get_messages(&session_id).is_empty());
    }

    #[test]
    fn rejected_generation_root_is_not_recorded_as_applied() {
        let store = SessionStore::empty();
        let mut reducer = SessionFeedReducer::new(store);
        let malformed = entry(
            1,
            SessionFeedEvent::SessionSnapshotCreated {
                snapshot: Box::new(SessionSnapshot {
                    session_id: "different-session".to_string(),
                    workspace: "C:/workspace".to_string(),
                    name: None,
                    parent_id: None,
                    created_at: 1,
                    updated_at: 1,
                    last_user_message_at: None,
                    state: None,
                    status: None,
                    message_count: 0,
                    task_management: json!({}),
                    lifecycle_projection: lifecycle::SessionAggregate::new(
                        "different-session".to_string(),
                    )
                    .query(lifecycle::SessionQuery::Lifecycle),
                    management: json!({}),
                    session: json!({}),
                    todos: Vec::new(),
                }),
            },
        );

        assert!(reducer.apply(malformed.clone()).is_err());
        assert!(reducer.apply(malformed).is_err());
    }

    #[test]
    fn empty_projection_cache_selects_its_first_canonical_session() {
        let (source, session_id) = test_store();
        let created = snapshot(&source, &session_id, "First canonical", 10);
        let store = SessionStore::empty();
        let mut reducer = SessionFeedReducer::new(store.clone());
        let mut created_entry = entry(
            1,
            SessionFeedEvent::SessionSnapshotCreated {
                snapshot: Box::new(created),
            },
        );
        created_entry.session_id = session_id.clone();
        created_entry.runtime_id = None;

        assert!(reducer.apply(created_entry).expect("reduce first snapshot"));
        assert_eq!(
            store
                .get_current_session()
                .expect("first canonical session becomes current")
                .id,
            session_id
        );
        assert_eq!(store.session_count(), 1);
    }

    #[test]
    fn updated_snapshot_uses_cursor_order_and_deduplicates_public_events() {
        let (store, session_id) = test_store();
        let mut reducer = SessionFeedReducer::new(store.clone());
        let mut event_cursor = store.event_cursor();
        let updated = snapshot(&store, &session_id, "Remote update", 1);
        let mut first = entry(
            1,
            SessionFeedEvent::SessionSnapshotUpdated {
                snapshot: Box::new(updated.clone()),
            },
        );
        first.session_id = session_id.clone();
        first.runtime_id = None;
        assert!(reducer.apply(first).expect("reduce updated snapshot"));
        let session = store.get_session(&session_id).expect("updated cache entry");
        assert_eq!(session.name.as_deref(), Some("Remote update"));
        assert_eq!(session.model.as_deref(), Some("model-Remote update"));
        assert_eq!(session.agent.as_deref(), Some("agent-Remote update"));
        assert!(session.validator_enabled);
        assert!(session.force_planning);

        let mut duplicate_value = entry(
            2,
            SessionFeedEvent::SessionSnapshotUpdated {
                snapshot: Box::new(updated),
            },
        );
        duplicate_value.session_id = session_id;
        duplicate_value.runtime_id = None;
        assert!(reducer
            .apply(duplicate_value)
            .expect("advance cursor for equal snapshot"));
        let events = std::iter::from_fn(|| store.next_event(&mut event_cursor)).collect::<Vec<_>>();
        assert_eq!(
            events
                .iter()
                .filter(|event| matches!(event, GlobalEvent::SessionUpdated { .. }))
                .count(),
            1
        );
    }

    #[test]
    fn older_snapshot_cannot_rollback_todos_applied_by_a_direct_response() {
        let (store, session_id) = test_store();
        let mut reducer = SessionFeedReducer::new(store.clone());
        let mut event_cursor = store.event_cursor();
        let direct_todos = vec![json!({"id": "direct-todo", "status": "in_progress"})];
        store.apply_todos_projection(&session_id, 3, direct_todos.clone());
        let mut stale_snapshot = snapshot(&store, &session_id, "Stale snapshot", 1);
        stale_snapshot.todos = Vec::new();
        let mut first = entry(
            1,
            SessionFeedEvent::SessionSnapshotUpdated {
                snapshot: Box::new(stale_snapshot.clone()),
            },
        );
        first.session_id = session_id.clone();
        first.runtime_id = None;
        let mut second = entry(
            2,
            SessionFeedEvent::SessionSnapshotUpdated {
                snapshot: Box::new(stale_snapshot),
            },
        );
        second.session_id = session_id.clone();
        second.runtime_id = None;
        let mut own_feed = entry(
            3,
            SessionFeedEvent::TodosUpdated {
                todos: direct_todos.clone(),
                updated_at: 2,
            },
        );
        own_feed.session_id = session_id.clone();
        own_feed.runtime_id = None;

        assert!(reducer.apply(first).expect("reduce first stale snapshot"));
        assert!(reducer.apply(second).expect("reduce second stale snapshot"));
        assert!(reducer.apply(own_feed).expect("reduce own todo feed"));
        assert_eq!(store.get_todos(&session_id), direct_todos);
        let events = std::iter::from_fn(|| store.next_event(&mut event_cursor)).collect::<Vec<_>>();
        assert_eq!(
            events
                .iter()
                .filter(|event| matches!(event, GlobalEvent::TodoUpdated { .. }))
                .count(),
            1
        );
    }

    #[test]
    fn deleted_tombstone_crosses_cursor_gaps_and_emits_once() {
        let (store, session_id) = test_store();
        let mut reducer = SessionFeedReducer::new(store.clone());
        let mut event_cursor = store.event_cursor();
        let mut deleted = entry(9, SessionFeedEvent::SessionDeleted {});
        deleted.session_id = session_id.clone();
        deleted.runtime_id = None;

        assert!(reducer
            .apply(deleted.clone())
            .expect("reduce deletion tombstone across startup gap"));
        assert!(store.get_session(&session_id).is_none());
        assert!(matches!(
            store.next_event(&mut event_cursor),
            Some(GlobalEvent::SessionDeleted { properties })
                if properties.session_id == session_id && properties.info.id == session_id
        ));
        assert!(!reducer
            .apply(deleted)
            .expect("deduplicate deletion tombstone"));
        assert!(store.next_event(&mut event_cursor).is_none());
    }

    #[test]
    fn local_delete_and_feed_race_produces_one_public_event() {
        let (store, session_id) = test_store();
        let deleted = store
            .remove_session_projection(&session_id)
            .expect("local request wins projection deletion");
        let mut event_cursor = store.event_cursor();
        store.push_event(GlobalEvent::SessionDeleted {
            properties: crate::contracts::SessionDeletedProperties {
                session_id: session_id.clone(),
                info: deleted,
            },
        });
        let mut reducer = SessionFeedReducer::new(store.clone());
        let mut tombstone = entry(3, SessionFeedEvent::SessionDeleted {});
        tombstone.session_id = session_id;
        tombstone.runtime_id = None;

        assert!(reducer
            .apply(tombstone)
            .expect("feed wins cursor after local projection deletion"));
        let events = std::iter::from_fn(|| store.next_event(&mut event_cursor)).collect::<Vec<_>>();
        assert_eq!(
            events
                .iter()
                .filter(|event| matches!(event, GlobalEvent::SessionDeleted { .. }))
                .count(),
            1
        );
    }

    #[test]
    fn stale_deleted_tombstone_cannot_remove_a_newer_projection() {
        let (store, session_id) = test_store();
        let updated = snapshot(&store, &session_id, "Newer projection", 2);
        let mut reducer = SessionFeedReducer::new(store.clone());
        let mut update = entry(
            2,
            SessionFeedEvent::SessionSnapshotUpdated {
                snapshot: Box::new(updated),
            },
        );
        update.session_id = session_id.clone();
        update.runtime_id = None;
        let mut deleted = entry(1, SessionFeedEvent::SessionDeleted {});
        deleted.session_id = session_id.clone();
        deleted.runtime_id = None;

        let mut first = entry(
            1,
            SessionFeedEvent::TodosUpdated {
                todos: Vec::new(),
                updated_at: 1,
            },
        );
        first.session_id = session_id.clone();
        assert!(reducer.apply(first).is_ok());
        assert!(reducer.apply(update).expect("reduce newer projection"));
        assert!(!reducer.apply(deleted).expect("ignore stale tombstone"));
        assert!(store.get_session(&session_id).is_some());
    }

    #[test]
    fn terminal_feed_metadata_does_not_mutate_session_lifecycle() {
        let (store, session_id) = test_store();
        let before = store
            .session_lifecycle_projection(&session_id)
            .expect("initial lifecycle");
        let mut reducer = SessionFeedReducer::new(store.clone());
        let mut terminal = entry(
            1,
            SessionFeedEvent::AgentMessage {
                message_id: "feed-runtime.message".to_string(),
                part_id: "feed-runtime.message".to_string(),
                reply_message: "done".to_string(),
                new_learning: String::new(),
                runtime_status: Some(RuntimeProjection::new(
                    "feed-runtime".to_string(),
                    RuntimeState::Finished,
                )),
                context_tokens: None,
                usage: None,
                created_at: 1,
                updated_at: 2,
            },
        );
        terminal.session_id = session_id.clone();

        assert!(reducer.apply(terminal).expect("reduce terminal feed"));
        let after = store
            .session_lifecycle_projection(&session_id)
            .expect("lifecycle after feed");
        assert_eq!(after.state, before.state);
        assert!(store
            .get_messages(&session_id)
            .iter()
            .any(|message| message.id == "feed-runtime.message"));
    }

    #[test]
    fn reducer_merges_explicit_message_ids_and_deduplicates_each_session_cursor() {
        let (store, session_id) = test_store();
        let mut reducer = SessionFeedReducer::new(store.clone());
        let mut cursor = store.event_cursor();
        let mut delta = entry(
            1,
            SessionFeedEvent::AssistantTextDelta {
                message_id: "feed-runtime.message".to_string(),
                part_id: "feed-runtime.message".to_string(),
                delta: "hel".to_string(),
                created_at: 10,
                updated_at: 11,
            },
        );
        delta.session_id = session_id.clone();
        assert!(reducer.apply(delta.clone()).expect("reduce text delta"));
        assert!(!reducer.apply(delta).expect("deduplicate text delta"));

        let mut final_message = entry(
            2,
            SessionFeedEvent::AgentMessage {
                message_id: "feed-runtime.message".to_string(),
                part_id: "feed-runtime.message".to_string(),
                reply_message: "hello".to_string(),
                new_learning: String::new(),
                runtime_status: None,
                context_tokens: None,
                usage: None,
                created_at: 10,
                updated_at: 12,
            },
        );
        final_message.session_id = session_id.clone();
        assert!(reducer.apply(final_message).expect("reduce final message"));

        let messages = store.get_messages(&session_id);
        let message = messages
            .iter()
            .find(|message| message.id == "feed-runtime.message")
            .expect("feed message projection");
        assert_eq!(message.parts.len(), 1);
        assert_eq!(message.parts[0].text.as_deref(), Some("hello"));
        let events = std::iter::from_fn(|| store.next_event(&mut cursor)).collect::<Vec<_>>();
        assert_eq!(
            events
                .iter()
                .filter(|event| matches!(event, GlobalEvent::MessagePartDelta { .. }))
                .count(),
            1
        );
        assert_eq!(
            events
                .iter()
                .filter(|event| matches!(event, GlobalEvent::MessageUpdated { .. }))
                .count(),
            1
        );
    }

    #[test]
    fn repeated_message_projection_is_idempotent_across_distinct_feed_cursors() {
        let (store, session_id) = test_store();
        let mut reducer = SessionFeedReducer::new(store.clone());
        let mut event_cursor = store.event_cursor();
        let message = store.build_text_message_with_ids_and_times(
            &session_id,
            MessageRole::User,
            "persist once".to_string(),
            Some("feed-user-message".to_string()),
            Some("feed-user-part".to_string()),
            None,
            10,
            11,
        );
        let projection = SessionRecordProjection {
            session_id: session_id.clone(),
            message_id: message.id.clone(),
            role: "user".to_string(),
            created_at: message.created_at,
            updated_at: message.updated_at,
            record: serde_json::to_value(&message).expect("message projection JSON"),
        };

        for cursor in [1, 2] {
            let mut duplicate = entry(
                cursor,
                SessionFeedEvent::MessageUpserted {
                    message: projection.clone(),
                },
            );
            duplicate.session_id = session_id.clone();
            assert!(reducer.apply(duplicate).expect("reduce message projection"));
        }

        let messages = store.get_messages(&session_id);
        assert_eq!(
            messages
                .iter()
                .filter(|candidate| candidate.id == message.id)
                .count(),
            1
        );
        let info = store
            .get_session_info(&session_id)
            .expect("session projection cache");
        assert_eq!(info.message_count, messages.len());
        assert_eq!(info.management.session_log.len(), 1);
        let events = std::iter::from_fn(|| store.next_event(&mut event_cursor)).collect::<Vec<_>>();
        assert_eq!(
            events
                .iter()
                .filter(|event| matches!(event, GlobalEvent::MessageUpdated { .. }))
                .count(),
            1
        );
        assert_eq!(
            events
                .iter()
                .filter(|event| matches!(event, GlobalEvent::MessagePartUpdated { .. }))
                .count(),
            1
        );
    }

    #[test]
    fn reducer_preserves_tool_command_and_todo_event_shapes() {
        let (store, session_id) = test_store();
        let mut reducer = SessionFeedReducer::new(store.clone());
        let mut cursor = store.event_cursor();
        let mut tool = entry(
            1,
            SessionFeedEvent::ToolCallUpdated {
                message_id: "feed-runtime.message".to_string(),
                part_id: "feed-runtime.tool.command_run".to_string(),
                tool_name: "command_run".to_string(),
                call_id: "call-1".to_string(),
                state: json!({"status": "running"}),
                metadata: None,
                runtime_status: None,
                context_tokens: None,
                usage: None,
                command_updates: vec![SessionFeedCommandUpdate {
                    message_id: "feed-runtime.message".to_string(),
                    part_id: "feed-runtime.tool.command_run".to_string(),
                    command_run_id: "feed-runtime.tool.command_run".to_string(),
                    command_id: "command-1".to_string(),
                    provider_tool_call_id: Some("call-1".to_string()),
                    command_index: Some(0),
                    event_seq: Some(1),
                    status: "running".to_string(),
                    command: json!({"command_type": "shell_command"}),
                    result: serde_json::Value::Null,
                    created_at: 20,
                    updated_at: 21,
                }],
                created_at: 20,
                updated_at: 21,
            },
        );
        tool.session_id = session_id.clone();
        assert!(reducer.apply(tool).expect("reduce tool update"));

        let mut todos = entry(
            2,
            SessionFeedEvent::TodosUpdated {
                todos: vec![json!({"id": "todo-1", "status": "in_progress"})],
                updated_at: 22,
            },
        );
        todos.session_id = session_id.clone();
        assert!(reducer.apply(todos).expect("reduce todos"));

        let events = std::iter::from_fn(|| store.next_event(&mut cursor)).collect::<Vec<_>>();
        assert!(events
            .iter()
            .any(|event| matches!(event, GlobalEvent::MessageUpdated { .. })));
        assert!(events.iter().any(|event| matches!(
            event,
            GlobalEvent::CommandUpdated { properties }
                if properties.command_id == "command-1"
                    && properties.runtime_id == "feed-runtime"
        )));
        assert!(events
            .iter()
            .any(|event| matches!(event, GlobalEvent::TodoUpdated { .. })));
        assert_eq!(store.get_todos(&session_id)[0]["id"], "todo-1");
    }

    #[test]
    fn canonical_projection_uses_cursor_order_even_when_the_timestamp_moves_backwards() {
        let (store, session_id) = test_store();
        let mut reducer = SessionFeedReducer::new(store.clone());
        let mut projection = store
            .session_lifecycle_projection(&session_id)
            .expect("initial lifecycle");
        projection.state = lifecycle::SessionState::Completed;
        let mut update = entry(
            1,
            SessionFeedEvent::SessionProjectionUpdated {
                projection,
                session_name: None,
                updated_at: 1,
            },
        );
        update.session_id = session_id.clone();

        assert!(reducer.apply(update).expect("reduce canonical projection"));
        assert_eq!(
            store
                .session_lifecycle_projection(&session_id)
                .expect("updated lifecycle")
                .state,
            lifecycle::SessionState::Completed
        );
        assert_eq!(
            store
                .get_session(&session_id)
                .expect("updated canonical timestamp")
                .updated_at,
            1
        );
    }

    #[test]
    fn equal_projection_converges_timestamp_without_a_duplicate_public_event() {
        let (store, session_id) = test_store();
        let mut reducer = SessionFeedReducer::new(store.clone());
        let mut event_cursor = store.event_cursor();
        let projection = store
            .session_lifecycle_projection(&session_id)
            .expect("initial lifecycle");
        let mut update = entry(
            1,
            SessionFeedEvent::SessionProjectionUpdated {
                projection,
                session_name: None,
                updated_at: 1,
            },
        );
        update.session_id = session_id.clone();

        assert!(reducer.apply(update).expect("reduce equal projection"));
        assert_eq!(
            store
                .get_session(&session_id)
                .expect("timestamp-converged session")
                .updated_at,
            1
        );
        assert!(store.next_event(&mut event_cursor).is_none());
    }
}
