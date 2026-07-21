use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{mpsc, Arc};

use lifecycle::{RuntimeAggregate, RuntimeEvent};
use session_log_contract::{
    ActivateRuntimeLeaseRequest, AppendSessionFeedEventRequest, CommitRuntimeEventRequest,
    RegisterRuntimeRequest, RuntimeEventCommitOutcome, RuntimeLeaseOutcome,
    RuntimeRegistrationOutcome, SessionFeedAppendOutcome, SessionFeedEvent, SessionLogCommand,
    SessionLogResponse,
};

use crate::session_log_client::SessionLogClient;

#[derive(Debug, Clone)]
struct RuntimeCursor {
    lease_id: String,
    revision: u64,
    next_event_seq: u64,
    pending_terminal: Option<RuntimeEvent>,
}

enum FeedCommand {
    Append(Box<AppendSessionFeedEventRequest>),
    Barrier {
        runtime_id: String,
        response: mpsc::Sender<Result<(), String>>,
    },
}

#[derive(Debug, Clone)]
pub(crate) struct RuntimeFeedPublisher {
    runtime_id: String,
    target_session_id: String,
    lease_id: String,
    next_event_seq: Arc<AtomicU64>,
    sender: mpsc::Sender<FeedCommand>,
}

impl RuntimeFeedPublisher {
    pub(crate) fn publish(&self, event: SessionFeedEvent) -> Result<(), String> {
        let event_seq = self.next_event_seq.fetch_add(1, Ordering::Relaxed);
        self.sender
            .send(FeedCommand::Append(Box::new(
                AppendSessionFeedEventRequest {
                    runtime_id: self.runtime_id.clone(),
                    target_session_id: self.target_session_id.clone(),
                    lease_id: self.lease_id.clone(),
                    event_id: format!("{}:feed:{event_seq}", self.runtime_id),
                    event,
                },
            )))
            .map_err(|_| format!("runtime {} feed writer stopped", self.runtime_id))
    }
}

/// Synchronous ordered writer owned by one supervised runtime worker.
///
/// A worker may execute several provider runtimes sequentially. Each runtime
/// gets its own lease and cursor; events are acknowledged locally only after
/// the session service confirms that exact sequence position.
#[derive(Debug)]
pub(crate) struct RuntimeEventWriter {
    session_id: String,
    initial_runtime_id: String,
    initial_lease_id: String,
    cursors: HashMap<String, RuntimeCursor>,
    feed_sequences: HashMap<String, Arc<AtomicU64>>,
    feed_sender: Option<mpsc::Sender<FeedCommand>>,
    feed_worker: Option<std::thread::JoinHandle<()>>,
    client: SessionLogClient,
}

impl RuntimeEventWriter {
    pub(crate) fn new(
        session_id: String,
        initial_runtime_id: String,
        initial_lease_id: String,
    ) -> Result<Self, String> {
        if session_id.trim().is_empty()
            || initial_runtime_id.trim().is_empty()
            || initial_lease_id.trim().is_empty()
        {
            return Err("runtime event writer identifiers must be non-empty".to_string());
        }
        let client = SessionLogClient::discover()
            .map_err(|error| format!("failed to discover session service: {error}"))?;
        let (feed_sender, feed_receiver) = mpsc::channel();
        let feed_client = client.clone();
        let feed_worker = std::thread::spawn(move || run_feed_worker(feed_client, feed_receiver));
        Ok(Self {
            session_id,
            initial_runtime_id,
            initial_lease_id,
            cursors: HashMap::new(),
            feed_sequences: HashMap::new(),
            feed_sender: Some(feed_sender),
            feed_worker: Some(feed_worker),
            client,
        })
    }

    pub(crate) fn flush(&mut self, runtime: &mut RuntimeAggregate) -> Result<(), String> {
        if runtime.session_id != self.session_id {
            return Err(format!(
                "runtime {} belongs to session {}, not writer session {}",
                runtime.runtime_id, runtime.session_id, self.session_id
            ));
        }
        self.prepare_runtime(runtime)?;

        while let Some(event) = runtime.next_uncommitted_event().cloned() {
            if is_terminal_event(&event) {
                let cursor = self
                    .cursors
                    .get_mut(&runtime.runtime_id)
                    .ok_or_else(|| format!("runtime {} has no event cursor", runtime.runtime_id))?;
                if cursor.pending_terminal.is_some() {
                    return Err(format!(
                        "runtime {} produced more than one terminal event",
                        runtime.runtime_id
                    ));
                }
                cursor.pending_terminal = Some(event);
                runtime.acknowledge_uncommitted_event();
                continue;
            }
            let cursor = self
                .cursors
                .get(&runtime.runtime_id)
                .cloned()
                .ok_or_else(|| format!("runtime {} has no event cursor", runtime.runtime_id))?;
            let (revision, next_event_seq) =
                commit_runtime_event(&self.client, &runtime.runtime_id, &cursor, event)?;
            let stored_cursor = self
                .cursors
                .get_mut(&runtime.runtime_id)
                .expect("runtime cursor was prepared before commit");
            stored_cursor.revision = revision;
            stored_cursor.next_event_seq = next_event_seq;
            runtime.acknowledge_uncommitted_event();
        }
        Ok(())
    }

    pub(crate) fn feed_publisher(
        &mut self,
        runtime_id: &str,
        target_session_id: &str,
    ) -> Result<RuntimeFeedPublisher, String> {
        let cursor = self
            .cursors
            .get(runtime_id)
            .ok_or_else(|| format!("runtime {runtime_id} has no event cursor"))?;
        let next_event_seq = Arc::clone(
            self.feed_sequences
                .entry(runtime_id.to_string())
                .or_insert_with(|| Arc::new(AtomicU64::new(1))),
        );
        Ok(RuntimeFeedPublisher {
            runtime_id: runtime_id.to_string(),
            target_session_id: target_session_id.to_string(),
            lease_id: cursor.lease_id.clone(),
            next_event_seq,
            sender: self
                .feed_sender
                .as_ref()
                .ok_or_else(|| "runtime feed writer is closed".to_string())?
                .clone(),
        })
    }

    pub(crate) fn seal_runtime(&mut self, runtime_id: &str) -> Result<(), String> {
        self.feed_barrier(runtime_id)?;
        let cursor = self
            .cursors
            .get(runtime_id)
            .cloned()
            .ok_or_else(|| format!("runtime {runtime_id} has no event cursor"))?;
        let Some(event) = cursor.pending_terminal.clone() else {
            return Err(format!(
                "runtime {runtime_id} has no pending terminal event"
            ));
        };
        let (revision, next_event_seq) =
            commit_runtime_event(&self.client, runtime_id, &cursor, event)?;
        let cursor = self
            .cursors
            .get_mut(runtime_id)
            .expect("runtime cursor existed before terminal commit");
        cursor.revision = revision;
        cursor.next_event_seq = next_event_seq;
        cursor.pending_terminal = None;
        Ok(())
    }

    fn feed_barrier(&self, runtime_id: &str) -> Result<(), String> {
        let (response, receiver) = mpsc::channel();
        self.feed_sender
            .as_ref()
            .ok_or_else(|| "runtime feed writer is closed".to_string())?
            .send(FeedCommand::Barrier {
                runtime_id: runtime_id.to_string(),
                response,
            })
            .map_err(|_| format!("runtime {runtime_id} feed writer stopped"))?;
        receiver
            .recv()
            .map_err(|_| format!("runtime {runtime_id} feed barrier was dropped"))?
    }

    fn prepare_runtime(&mut self, runtime: &RuntimeAggregate) -> Result<(), String> {
        let runtime_id = runtime.runtime_id.as_str();
        if self.cursors.contains_key(runtime_id) {
            return Ok(());
        }
        let response =
            self.client
                .call_typed(SessionLogCommand::RegisterRuntime(RegisterRuntimeRequest {
                    runtime_id: runtime_id.to_string(),
                    session_id: self.session_id.clone(),
                    fallback_from_id: runtime.fallback_from_id.clone(),
                }))?;
        let (revision, next_event_seq) = match response {
            SessionLogResponse::RuntimeRegistered {
                result:
                    RuntimeRegistrationOutcome::Registered {
                        revision,
                        next_event_seq,
                        ..
                    }
                    | RuntimeRegistrationOutcome::AlreadyRegistered {
                        revision,
                        next_event_seq,
                        ..
                    },
            } => (revision, next_event_seq),
            SessionLogResponse::RuntimeRegistered { result } => {
                return Err(format!(
                    "session service rejected runtime {runtime_id} registration: {result:?}"
                ));
            }
            SessionLogResponse::Error { error } => {
                return Err(format!(
                    "session service failed to register runtime {runtime_id}: {error}"
                ));
            }
            other => {
                return Err(format!(
                    "unexpected session service runtime registration response: {other:?}"
                ));
            }
        };
        let lease_id = if runtime_id == self.initial_runtime_id {
            self.initial_lease_id.clone()
        } else {
            format!("lease-{}", uuid::Uuid::new_v4())
        };
        let response = self
            .client
            .call_typed(SessionLogCommand::ActivateRuntimeLease(
                ActivateRuntimeLeaseRequest {
                    runtime_id: runtime_id.to_string(),
                    lease_id: lease_id.clone(),
                },
            ))?;
        match response {
            SessionLogResponse::RuntimeLeaseActivated {
                result: RuntimeLeaseOutcome::Activated | RuntimeLeaseOutcome::AlreadyActive,
            } => {}
            SessionLogResponse::RuntimeLeaseActivated { result } => {
                return Err(format!(
                    "session service rejected runtime {runtime_id} lease: {result:?}"
                ));
            }
            SessionLogResponse::Error { error } => {
                return Err(format!(
                    "session service failed to activate runtime {runtime_id}: {error}"
                ));
            }
            other => {
                return Err(format!(
                    "unexpected session service runtime lease response: {other:?}"
                ));
            }
        }

        self.cursors.insert(
            runtime_id.to_string(),
            RuntimeCursor {
                lease_id,
                revision,
                next_event_seq,
                pending_terminal: None,
            },
        );
        Ok(())
    }
}

impl Drop for RuntimeEventWriter {
    fn drop(&mut self) {
        self.feed_sender.take();
        if let Some(worker) = self.feed_worker.take() {
            let _ = worker.join();
        }
    }
}

fn is_terminal_event(event: &RuntimeEvent) -> bool {
    matches!(
        event,
        RuntimeEvent::RuntimeFinished { .. } | RuntimeEvent::RuntimeFailed { .. }
    )
}

fn commit_runtime_event(
    client: &SessionLogClient,
    runtime_id: &str,
    cursor: &RuntimeCursor,
    event: RuntimeEvent,
) -> Result<(u64, u64), String> {
    let response = client.call_typed(SessionLogCommand::CommitRuntimeEvent(
        CommitRuntimeEventRequest {
            runtime_id: runtime_id.to_string(),
            event_seq: cursor.next_event_seq,
            expected_revision: cursor.revision,
            lease_id: cursor.lease_id.clone(),
            idempotency_key: format!("{runtime_id}:{}", cursor.next_event_seq),
            event,
        },
    ))?;
    match response {
        SessionLogResponse::RuntimeEventCommitted {
            result:
                RuntimeEventCommitOutcome::Applied {
                    revision,
                    next_event_seq,
                    ..
                }
                | RuntimeEventCommitOutcome::Duplicate {
                    revision,
                    next_event_seq,
                },
        } => Ok((revision, next_event_seq)),
        SessionLogResponse::RuntimeEventCommitted { result } => Err(format!(
            "session service rejected runtime {runtime_id} event {}: {result:?}",
            cursor.next_event_seq
        )),
        SessionLogResponse::Error { error } => Err(format!(
            "session service failed to commit runtime {runtime_id} event {}: {error}",
            cursor.next_event_seq
        )),
        other => Err(format!(
            "unexpected session service runtime event response: {other:?}"
        )),
    }
}

fn run_feed_worker(client: SessionLogClient, receiver: mpsc::Receiver<FeedCommand>) {
    let mut errors = HashMap::<String, String>::new();
    while let Ok(command) = receiver.recv() {
        match command {
            FeedCommand::Append(request) => {
                if errors.contains_key(&request.runtime_id) {
                    continue;
                }
                let runtime_id = request.runtime_id.clone();
                let result = client.call_typed(SessionLogCommand::AppendSessionFeedEvent(*request));
                let error = match result {
                    Ok(SessionLogResponse::SessionFeedEventAppended {
                        result:
                            SessionFeedAppendOutcome::Applied { .. }
                            | SessionFeedAppendOutcome::Duplicate { .. },
                    }) => None,
                    Ok(SessionLogResponse::SessionFeedEventAppended { result }) => Some(format!(
                        "session service rejected runtime {runtime_id} feed event: {result:?}"
                    )),
                    Ok(SessionLogResponse::Error { error }) => Some(format!(
                        "session service failed to append runtime {runtime_id} feed event: {error}"
                    )),
                    Ok(other) => Some(format!(
                        "unexpected session service feed response for runtime {runtime_id}: {other:?}"
                    )),
                    Err(error) => Some(error),
                };
                if let Some(error) = error {
                    errors.insert(runtime_id, error);
                }
            }
            FeedCommand::Barrier {
                runtime_id,
                response,
            } => {
                let result = errors.remove(&runtime_id).map_or(Ok(()), Err);
                let _ = response.send(result);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use lifecycle::{ProviderConfig, RuntimeProviderConfig, SessionCommand, TaskPlan, ToolChoice};
    use session_log_contract::{
        CreateSessionRequest, ReadSessionFeedRequest, ReplayRuntimeRequest, SessionFeedEvent,
    };
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn feed_barrier_precedes_terminal_runtime_commit() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|error| error.into_inner());
        let root = tempfile::tempdir().expect("session db root");
        let home = root.path().join("home");
        std::fs::create_dir_all(&home).expect("session db home");
        let previous_home = std::env::var_os("TURA_HOME");
        let previous_root = std::env::var_os("SESSION_LOG_DB_ROOT");
        std::env::set_var("TURA_HOME", &home);
        std::env::set_var("SESSION_LOG_DB_ROOT", root.path());
        let handle = std::thread::spawn(session_log::service::run_socket_service);
        let started = std::time::Instant::now();
        while !session_log_contract::client::service_is_running() {
            assert!(
                started.elapsed() < std::time::Duration::from_secs(10),
                "session service did not start"
            );
            std::thread::sleep(std::time::Duration::from_millis(25));
        }

        let session_id = "writer-feed-session".to_string();
        let runtime_id = "writer-feed-runtime".to_string();
        let workspace = root.path().join("workspace").to_string_lossy().to_string();
        let now = Utc::now();
        session_log_contract::client::call_service(&SessionLogCommand::CreateSession(
            CreateSessionRequest {
                command_id: format!("create:{session_id}"),
                session_id: session_id.clone(),
                creation_command: SessionCommand::CreateSession {
                    task_plan: TaskPlan::default(),
                },
                copy_context: false,
                workspace: workspace.clone(),
                session_directory: workspace,
                name: "writer feed ordering".to_string(),
                created_at: now.timestamp_millis(),
                model: None,
                agent: None,
                session_type: "coding".to_string(),
                kill_processes_on_start: false,
                validator_enabled: false,
                force_planning: false,
                model_variant: None,
                model_acceleration_enabled: false,
                disable_permission_restrictions: false,
                use_last_tool_call_response: false,
                auto_session_name: false,
            },
        ))
        .expect("create session");

        let provider = RuntimeProviderConfig {
            base: ProviderConfig {
                tura_llm_name: "test".to_string(),
                default_model_tier: None,
                current_model: None,
                stream: true,
                temperature: 0.0,
                max_tokens: 1024,
                tool_choice: ToolChoice::Auto,
                time_out_ms: 30_000,
            },
            thinking: false,
            provider_name: "test".to_string(),
            model_name: "test-model".to_string(),
            provider_url_name: "local".to_string(),
            llm_provider_name: "test".to_string(),
        };
        let mut runtime = RuntimeAggregate::new(
            runtime_id.clone(),
            session_id.clone(),
            "agent".to_string(),
            provider,
            now,
        );
        let mut writer = RuntimeEventWriter::new(
            session_id.clone(),
            runtime_id.clone(),
            "writer-feed-lease".to_string(),
        )
        .expect("writer");
        writer.flush(&mut runtime).expect("flush creation");
        let publisher = writer
            .feed_publisher(&runtime_id, &session_id)
            .expect("publisher");
        publisher
            .publish(SessionFeedEvent::AssistantTextDelta {
                message_id: format!("{runtime_id}.message"),
                part_id: format!("{runtime_id}.message"),
                delta: "first".to_string(),
                created_at: now.timestamp_millis(),
                updated_at: now.timestamp_millis(),
            })
            .expect("queue feed event");
        runtime
            .mark_called(now + chrono::Duration::milliseconds(1))
            .expect("mark called");
        runtime.mark_waiting_first_token().expect("mark waiting");
        runtime
            .mark_first_token(now + chrono::Duration::milliseconds(2))
            .expect("first token");
        runtime
            .finish_success(now + chrono::Duration::milliseconds(3), None)
            .expect("finish runtime");
        writer.flush(&mut runtime).expect("defer terminal event");

        let replay = session_log_contract::client::call_service(&SessionLogCommand::ReplayRuntime(
            ReplayRuntimeRequest {
                runtime_id: runtime_id.clone(),
            },
        ))
        .expect("replay before seal");
        let SessionLogResponse::RuntimeReplayed {
            runtime: Some(replay),
        } = replay
        else {
            panic!("runtime replay missing before seal");
        };
        assert!(!replay.aggregate.state.is_terminal());

        writer.seal_runtime(&runtime_id).expect("seal runtime");
        let response = session_log_contract::client::call_service(
            &SessionLogCommand::ReadSessionFeed(ReadSessionFeedRequest {
                session_id: session_id.clone(),
                after_cursor: 0,
                limit: 10,
            }),
        )
        .expect("read feed");
        let SessionLogResponse::SessionFeed {
            entries,
            next_cursor,
        } = response
        else {
            panic!("session feed response missing");
        };
        assert_eq!(next_cursor, 4);
        assert_eq!(
            entries.iter().map(|entry| entry.cursor).collect::<Vec<_>>(),
            vec![1, 2, 3, 4]
        );
        assert!(matches!(
            &entries[0].event,
            SessionFeedEvent::SessionSnapshotCreated { .. }
        ));
        assert!(matches!(
            &entries[1].event,
            SessionFeedEvent::SessionProjectionUpdated { projection, .. }
                if !projection.state.is_terminal()
        ));
        assert!(matches!(
            &entries[2].event,
            SessionFeedEvent::AssistantTextDelta { delta, .. } if delta == "first"
        ));
        assert!(matches!(
            &entries[3].event,
            SessionFeedEvent::SessionProjectionUpdated { projection, .. }
                if projection.state.is_terminal()
        ));
        let replay = session_log_contract::client::call_service(&SessionLogCommand::ReplayRuntime(
            ReplayRuntimeRequest { runtime_id },
        ))
        .expect("replay after seal");
        let SessionLogResponse::RuntimeReplayed {
            runtime: Some(replay),
        } = replay
        else {
            panic!("runtime replay missing after seal");
        };
        assert!(replay.aggregate.state.is_terminal());

        let _ = session_log_contract::client::call_service(&SessionLogCommand::Shutdown);
        let _ = handle.join();
        match previous_home {
            Some(value) => std::env::set_var("TURA_HOME", value),
            None => std::env::remove_var("TURA_HOME"),
        }
        match previous_root {
            Some(value) => std::env::set_var("SESSION_LOG_DB_ROOT", value),
            None => std::env::remove_var("SESSION_LOG_DB_ROOT"),
        }
    }
}
