use chrono::{DateTime, Utc};
use serde_json::Value;
use std::collections::{BTreeSet, VecDeque};
use std::path::PathBuf;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc::{self, RecvTimeoutError},
    Arc, Mutex,
};
use std::thread::JoinHandle;
use std::time::Duration;
use tracing::error;

use crate::gateway_events::{emit_cli_live_command_run_results, emit_cli_live_command_run_started};
use crate::provider_flow::checkpointing;
use crate::provider_flow::streamed_command_run::{
    command_run_live_delta_result, command_run_stream_event_command,
    publish_streamed_command_run_update, streamed_command_event_record,
    streamed_command_result_record, StreamedCommandEvent, StreamedCommandRunUpdate,
};
use crate::router_command_run::execute_command_value_results;
use crate::state_machine::runtime_management::{
    RuntimeManagement, RuntimeSessionSyncStatus, ToolCallRecord,
};

const COMMAND_RUN_TOOL_NAME: &str = "command_run";

#[derive(Clone)]
pub(crate) struct StreamedCommandRunState {
    pub(crate) results: Arc<Mutex<Vec<Value>>>,
    pub(crate) inputs: Arc<Mutex<Vec<Value>>>,
    pub(crate) events: Arc<Mutex<Vec<Value>>>,
    pub(crate) seen: Arc<AtomicBool>,
    pub(crate) cancelled: Arc<AtomicBool>,
}

impl StreamedCommandRunState {
    pub(crate) fn new() -> Self {
        Self {
            results: Arc::new(Mutex::new(Vec::new())),
            inputs: Arc::new(Mutex::new(Vec::new())),
            events: Arc::new(Mutex::new(Vec::new())),
            seen: Arc::new(AtomicBool::new(false)),
            cancelled: Arc::new(AtomicBool::new(false)),
        }
    }

    pub(crate) fn mark_seen(&self) {
        self.seen.store(true, Ordering::SeqCst);
    }

    pub(crate) fn seen(&self) -> bool {
        self.seen.load(Ordering::SeqCst)
    }

    pub(crate) fn should_cancel_after_results(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst) && !self.snapshot_results().is_empty()
    }

    pub(crate) fn snapshot(&self) -> StreamedCommandRunSnapshot {
        StreamedCommandRunSnapshot {
            commands: self
                .inputs
                .lock()
                .unwrap_or_else(|err| err.into_inner())
                .clone(),
            events: self
                .events
                .lock()
                .unwrap_or_else(|err| err.into_inner())
                .clone(),
            results: self.snapshot_results(),
        }
    }

    fn snapshot_results(&self) -> Vec<Value> {
        self.results
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .clone()
    }
}

pub(crate) struct StreamedCommandRunSnapshot {
    pub(crate) commands: Vec<Value>,
    pub(crate) events: Vec<Value>,
    pub(crate) results: Vec<Value>,
}

pub(crate) struct SpawnStreamedCommandRunTask {
    pub(crate) stream_rx: mpsc::Receiver<tura_llm_rust::ProviderStreamEvent>,
    pub(crate) session_directory: PathBuf,
    pub(crate) allowed_command_run_commands: Option<BTreeSet<String>>,
    pub(crate) session_id: String,
    pub(crate) runtime_id: String,
    pub(crate) provider: Value,
    pub(crate) call_id: String,
    pub(crate) started_at: DateTime<Utc>,
    pub(crate) state: StreamedCommandRunState,
    pub(crate) runtime_status: RuntimeSessionSyncStatus,
}

#[derive(Clone)]
struct QueuedStreamCommand {
    tool_call_id: String,
    command_index: usize,
    command: Value,
    step: u64,
    order: usize,
}

struct StreamCommandCompletion {
    order: usize,
    completed: Vec<Value>,
    halted: bool,
}

struct OrderedStreamResult {
    order: usize,
    offset: usize,
    result: Value,
}

struct StreamStepNormalizer;

impl StreamStepNormalizer {
    fn normalize(&mut self, command: &mut Value) -> u64 {
        let step = command_step(command);
        if let Some(object) = command.as_object_mut() {
            object.insert("step".to_string(), serde_json::json!(step));
        }
        step
    }
}

pub(crate) fn spawn_streamed_command_run_task(
    input: SpawnStreamedCommandRunTask,
) -> JoinHandle<Vec<Value>> {
    std::thread::spawn(move || {
        let mut results = Vec::new();
        let mut ordered_results = Vec::new();
        let mut streamed_commands = Vec::new();
        let mut command_run_started = false;
        let mut live_item_index = 0usize;
        let mut pending = VecDeque::new();
        let mut active_step = None;
        let mut running = 0usize;
        let mut receiver_open = true;
        let mut halted_before_finish = false;
        let mut next_order = 0usize;
        let mut step_normalizer = StreamStepNormalizer;
        let (completion_tx, completion_rx) = mpsc::channel::<StreamCommandCompletion>();

        loop {
            while let Ok(completion) = completion_rx.try_recv() {
                running = running.saturating_sub(1);
                append_ordered_results(&mut ordered_results, &completion);
                emit_cli_live_command_run_results(&completion.completed, &mut live_item_index);
                record_completed_results(
                    &input.state,
                    &mut results,
                    &completion.completed,
                    &input.session_id,
                    &input.runtime_id,
                    &input.call_id,
                );
                if completion.halted {
                    halted_before_finish = true;
                    input.state.cancelled.store(true, Ordering::SeqCst);
                }
                publish_streamed_command_run_update(StreamedCommandRunUpdate {
                    session_id: &input.session_id,
                    runtime_id: &input.runtime_id,
                    provider: &input.provider,
                    call_id: &input.call_id,
                    commands: &streamed_commands,
                    results: &results,
                    status: "running",
                    started_at: input.started_at,
                    ended_at: None,
                    runtime_status: input.runtime_status.clone(),
                });
            }

            if input.state.cancelled.load(Ordering::SeqCst) {
                receiver_open = false;
                pending.clear();
            }
            start_ready_stream_commands(
                &input,
                &completion_tx,
                &mut pending,
                &mut active_step,
                &mut running,
                &streamed_commands,
                &results,
            );

            if !receiver_open && running == 0 && pending.is_empty() {
                break;
            }

            let event = if receiver_open {
                poll_stream_event(&input.stream_rx, running == 0 && pending.is_empty())
            } else {
                StreamEventPoll::Timeout
            };
            match event {
                StreamEventPoll::Event(event) => {
                    let Some(command_event) = command_run_stream_event_command(event) else {
                        continue;
                    };
                    let queued = match prepare_stream_command(
                        &input,
                        command_event,
                        &mut command_run_started,
                        &mut streamed_commands,
                        &mut step_normalizer,
                        next_order,
                        &results,
                    ) {
                        Some(queued) => queued,
                        None => {
                            next_order += 1;
                            continue;
                        }
                    };
                    next_order += 1;
                    enqueue_or_start_stream_command(
                        &input,
                        &completion_tx,
                        queued,
                        &mut pending,
                        &mut active_step,
                        &mut running,
                        &streamed_commands,
                        &results,
                    );
                }
                StreamEventPoll::Closed => {
                    receiver_open = false;
                }
                StreamEventPoll::Timeout => {}
            }
        }
        let final_results = ordered_stream_results(ordered_results);
        let checkpoint_ack_failed = input.state.cancelled.load(Ordering::SeqCst);
        if !streamed_commands.is_empty() {
            let finished_at = Utc::now();
            publish_streamed_command_run_update(StreamedCommandRunUpdate {
                session_id: &input.session_id,
                runtime_id: &input.runtime_id,
                provider: &input.provider,
                call_id: &input.call_id,
                commands: &streamed_commands,
                results: &final_results,
                status: if halted_before_finish || checkpoint_ack_failed {
                    "error"
                } else {
                    "completed"
                },
                started_at: input.started_at,
                ended_at: Some(finished_at),
                runtime_status: input.runtime_status.clone(),
            });
            let command_run_status = if halted_before_finish || checkpoint_ack_failed {
                "error"
            } else {
                "completed"
            };
            if let Err(error) = checkpointing::command_run_finished(
                &input.session_id,
                &input.runtime_id,
                &input.call_id,
                command_run_status,
                final_results.len(),
                input.started_at,
                finished_at,
            ) {
                tracing::warn!(
                    session_id = %input.session_id,
                    runtime_id = %input.runtime_id,
                    error = %error,
                    "failed to persist command_run_finished checkpoint"
                );
            }
        }
        if halted_before_finish {
            input.state.cancelled.store(true, Ordering::SeqCst);
        }
        final_results
    })
}

enum StreamEventPoll {
    Event(tura_llm_rust::ProviderStreamEvent),
    Timeout,
    Closed,
}

fn poll_stream_event(
    stream_rx: &mpsc::Receiver<tura_llm_rust::ProviderStreamEvent>,
    block: bool,
) -> StreamEventPoll {
    if block {
        return match stream_rx.recv() {
            Ok(event) => StreamEventPoll::Event(event),
            Err(_) => StreamEventPoll::Closed,
        };
    }
    match stream_rx.recv_timeout(Duration::from_millis(20)) {
        Ok(event) => StreamEventPoll::Event(event),
        Err(RecvTimeoutError::Timeout) => StreamEventPoll::Timeout,
        Err(RecvTimeoutError::Disconnected) => StreamEventPoll::Closed,
    }
}

fn prepare_stream_command(
    input: &SpawnStreamedCommandRunTask,
    command_event: StreamedCommandEvent,
    command_run_started: &mut bool,
    streamed_commands: &mut Vec<Value>,
    step_normalizer: &mut StreamStepNormalizer,
    order: usize,
    results: &[Value],
) -> Option<QueuedStreamCommand> {
    let StreamedCommandEvent {
        tool_call_id,
        command_index,
        command,
    } = command_event;
    let original_command = command;
    let mut command = match code_tools::command_run::normalize_command_value_for_execution(
        original_command.clone(),
        command_index,
    ) {
        Ok(command) => command,
        Err(error) => {
            tracing::warn!(
                session_id = %input.session_id,
                runtime_id = %input.runtime_id,
                error = %error,
                "failed to normalize streamed command_run command before execution"
            );
            original_command
        }
    };
    let step = step_normalizer.normalize(&mut command);
    if !*command_run_started {
        if let Err(error) = checkpointing::command_run_started(
            &input.session_id,
            &input.runtime_id,
            &input.call_id,
            input.started_at,
        ) {
            tracing::warn!(
                session_id = %input.session_id,
                runtime_id = %input.runtime_id,
                error = %error,
                "failed to persist command_run_started checkpoint"
            );
        }
        *command_run_started = true;
    }
    streamed_commands.push(command.clone());
    let ready_at = Utc::now();
    input
        .state
        .inputs
        .lock()
        .unwrap_or_else(|err| err.into_inner())
        .push(command.clone());
    input
        .state
        .events
        .lock()
        .unwrap_or_else(|err| err.into_inner())
        .push(streamed_command_event_record(
            "ready",
            &input.runtime_id,
            &tool_call_id,
            command_index,
            &command,
            None,
            ready_at,
        ));
    if let Err(error) = checkpointing::command_ready(
        &input.session_id,
        &input.runtime_id,
        &input.call_id,
        &tool_call_id,
        command_index,
        &command,
        ready_at,
    ) {
        tracing::warn!(
            session_id = %input.session_id,
            runtime_id = %input.runtime_id,
            error = %error,
            "failed to persist command_ready checkpoint"
        );
    }
    publish_streamed_command_run_update(StreamedCommandRunUpdate {
        session_id: &input.session_id,
        runtime_id: &input.runtime_id,
        provider: &input.provider,
        call_id: &input.call_id,
        commands: streamed_commands,
        results,
        status: "running",
        started_at: input.started_at,
        ended_at: None,
        runtime_status: input.runtime_status.clone(),
    });
    Some(QueuedStreamCommand {
        tool_call_id,
        command_index,
        command,
        step,
        order,
    })
}

#[expect(
    clippy::too_many_arguments,
    reason = "scheduler entrypoint passes the mutable queue state explicitly"
)]
fn enqueue_or_start_stream_command(
    input: &SpawnStreamedCommandRunTask,
    completion_tx: &mpsc::Sender<StreamCommandCompletion>,
    command: QueuedStreamCommand,
    pending: &mut VecDeque<QueuedStreamCommand>,
    active_step: &mut Option<u64>,
    running: &mut usize,
    streamed_commands: &[Value],
    results: &[Value],
) {
    match *active_step {
        Some(step) if command.step <= step => start_stream_command(
            input,
            completion_tx,
            command,
            running,
            streamed_commands,
            results,
        ),
        Some(_) => pending.push_back(command),
        None => {
            *active_step = Some(command.step);
            start_stream_command(
                input,
                completion_tx,
                command,
                running,
                streamed_commands,
                results,
            );
        }
    }
}

fn start_ready_stream_commands(
    input: &SpawnStreamedCommandRunTask,
    completion_tx: &mpsc::Sender<StreamCommandCompletion>,
    pending: &mut VecDeque<QueuedStreamCommand>,
    active_step: &mut Option<u64>,
    running: &mut usize,
    streamed_commands: &[Value],
    results: &[Value],
) {
    if *running != 0 {
        return;
    }
    let Some(next_pending_step) = pending.iter().map(|command| command.step).min() else {
        return;
    };
    match *active_step {
        Some(step) if step >= next_pending_step => {}
        _ => *active_step = Some(next_pending_step),
    }
    let Some(step) = *active_step else {
        return;
    };
    let mut index = 0;
    while index < pending.len() {
        if pending[index].step > step {
            index += 1;
            continue;
        }
        let command = pending
            .remove(index)
            .expect("pending index should be valid while starting ready commands");
        start_stream_command(
            input,
            completion_tx,
            command,
            running,
            streamed_commands,
            results,
        );
    }
}

fn start_stream_command(
    input: &SpawnStreamedCommandRunTask,
    completion_tx: &mpsc::Sender<StreamCommandCompletion>,
    queued: QueuedStreamCommand,
    running: &mut usize,
    streamed_commands: &[Value],
    results: &[Value],
) {
    let command_started_at = Utc::now();
    emit_cli_live_command_run_started(&queued.command, &queued.tool_call_id, queued.command_index);
    if let Err(error) = checkpointing::command_started(
        &input.session_id,
        &input.runtime_id,
        &input.call_id,
        &queued.tool_call_id,
        queued.command_index,
        &queued.command,
        command_started_at,
    ) {
        tracing::warn!(
            session_id = %input.session_id,
            runtime_id = %input.runtime_id,
            error = %error,
            "failed to persist command_started checkpoint"
        );
    }
    let live_command = queued.command.clone();
    let command = queued.command;
    let session_directory = input.session_directory.clone();
    let allowed_commands = input.allowed_command_run_commands.clone();
    let session_id = input.session_id.clone();
    let runtime_id = input.runtime_id.clone();
    let order = queued.order;
    let completion_tx = completion_tx.clone();
    *running += 1;
    std::thread::spawn(move || {
        let result = match tokio::runtime::Runtime::new() {
            Ok(runtime) => runtime.block_on(execute_command_value_results(
                command,
                session_directory,
                Some(&session_id),
                Some(&runtime_id),
                allowed_commands,
            )),
            Err(error) => crate::router_command_run::RouterCommandRunCommandResult {
                results: vec![serde_json::json!({
                    "step": 1,
                    "command_type": "command_run",
                    "success": false,
                    "error": format!("failed to create streamed command runtime: {error}"),
                })],
                halted: false,
            },
        };
        let _ = completion_tx.send(StreamCommandCompletion {
            order,
            completed: result.results,
            halted: result.halted,
        });
    });

    let mut live_results = results.to_vec();
    live_results.push(command_run_live_delta_result(&live_command, "", ""));
    publish_streamed_command_run_update(StreamedCommandRunUpdate {
        session_id: &input.session_id,
        runtime_id: &input.runtime_id,
        provider: &input.provider,
        call_id: &input.call_id,
        commands: streamed_commands,
        results: &live_results,
        status: "running",
        started_at: input.started_at,
        ended_at: None,
        runtime_status: input.runtime_status.clone(),
    });
}

fn append_ordered_results(
    ordered_results: &mut Vec<OrderedStreamResult>,
    completion: &StreamCommandCompletion,
) {
    for (offset, result) in completion.completed.iter().cloned().enumerate() {
        ordered_results.push(OrderedStreamResult {
            order: completion.order,
            offset,
            result,
        });
    }
}

fn ordered_stream_results(mut ordered_results: Vec<OrderedStreamResult>) -> Vec<Value> {
    ordered_results.sort_by_key(|result| (result.order, result.offset));
    ordered_results
        .into_iter()
        .map(|result| result.result)
        .collect()
}

fn command_step(command: &Value) -> u64 {
    command
        .get("step")
        .and_then(Value::as_u64)
        .unwrap_or(1)
        .max(1)
}

fn record_completed_results(
    state: &StreamedCommandRunState,
    results: &mut Vec<Value>,
    completed: &[Value],
    session_id: &str,
    runtime_id: &str,
    call_id: &str,
) {
    let completed_at = Utc::now();
    if !completed.is_empty() {
        {
            let mut shared = state.results.lock().unwrap_or_else(|err| err.into_inner());
            shared.extend(completed.to_vec());
        }
    }
    for (offset, result) in completed.iter().enumerate() {
        if let Err(error) = checkpointing::streamed_command_finished(
            session_id,
            runtime_id,
            call_id,
            results.len() + offset,
            result,
            completed_at,
        ) {
            error!(
                session_id = %session_id,
                runtime_id = %runtime_id,
                error = %error,
                "session_db command checkpoint ACK failed"
            );
            state.cancelled.store(true, Ordering::SeqCst);
            break;
        }
        state
            .events
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .push(streamed_command_result_record(
                "completed",
                runtime_id,
                results.len() + offset,
                result,
                completed_at,
            ));
    }
    results.extend_from_slice(completed);
}

pub(crate) fn apply_cancelled_streamed_command_run_result(
    runtime: &mut RuntimeManagement,
    commands: &[Value],
    events: &[Value],
    results: &[Value],
    finished_at: DateTime<Utc>,
) {
    runtime.set_output(cancelled_streamed_command_run_output(
        commands, events, results,
    ));
    runtime.push_tool_call(streamed_command_run_tool_record(commands, finished_at));
}

fn cancelled_streamed_command_run_output(
    commands: &[Value],
    events: &[Value],
    results: &[Value],
) -> Value {
    serde_json::json!({
        "streamed_command_run_result": {
            "commands": commands,
            "command_events": events,
            "results": results,
            "early_finish_reason": "apply_patch_failed",
            "cancelled": true,
        }
    })
}

fn streamed_command_run_tool_record(
    commands: &[Value],
    finished_at: DateTime<Utc>,
) -> ToolCallRecord {
    ToolCallRecord {
        tool_called_name: COMMAND_RUN_TOOL_NAME.to_string(),
        tool_called_input: serde_json::json!({ "commands": commands }),
        provider_metadata: None,
        tool_received_at: finished_at,
        tool_executed_at: finished_at,
        tool_calldata_received_at: finished_at,
        tool_reported_success: false,
        agent_reported_success: false,
        agent_reported_helpful: false,
        agent_reported_summary: String::new(),
        validator_reported_success: None,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        apply_cancelled_streamed_command_run_result, spawn_streamed_command_run_task,
        SpawnStreamedCommandRunTask, StreamedCommandRunState,
    };
    use crate::state_machine::agent_management::{ProviderConfig, ToolChoice};
    use crate::state_machine::runtime_management::{
        RuntimeManagement, RuntimeProviderConfig, RuntimeState,
    };
    use chrono::Utc;
    use serde_json::json;
    use serde_json::Value;
    use std::sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        mpsc, Arc, Mutex,
    };
    use std::time::Duration;
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    use tokio::net::TcpListener;

    static STREAMING_TEST_ENV: Mutex<()> = Mutex::new(());

    fn runtime() -> RuntimeManagement {
        RuntimeManagement::new(
            "runtime-call-test".to_string(),
            "session-call-test".to_string(),
            "agent-call-test".to_string(),
            RuntimeProviderConfig {
                base: ProviderConfig {
                    tura_llm_name: "fast".to_string(),
                    stream: true,
                    temperature: 0.0,
                    max_tokens: 1024,
                    tool_choice: ToolChoice::Auto,
                    time_out_ms: 30_000,
                },
                thinking: false,
                provider_name: "openai".to_string(),
                model_name: "gpt-test".to_string(),
                provider_url_name: "openai".to_string(),
                llm_provider_name: "openai".to_string(),
            },
            Utc::now(),
        )
    }

    #[test]
    fn cancelled_streamed_command_run_result_marks_output_and_tool_record() {
        let mut runtime = runtime();
        let commands = vec![json!({ "command": "apply_patch failed" })];
        let events = vec![json!({ "status": "completed" })];
        let results = vec![json!({ "success": false, "error": "patch failed" })];
        let finished_at = runtime.created_at;

        apply_cancelled_streamed_command_run_result(
            &mut runtime,
            &commands,
            &events,
            &results,
            finished_at,
        );

        let output = runtime.output.as_ref().expect("output should be set");
        assert_eq!(
            output.pointer("/streamed_command_run_result/early_finish_reason"),
            Some(&json!("apply_patch_failed"))
        );
        assert_eq!(
            output.pointer("/streamed_command_run_result/cancelled"),
            Some(&json!(true))
        );
        assert_eq!(runtime.tool_call.len(), 1);
        assert_eq!(runtime.tool_call[0].tool_called_name, "command_run");
        assert_eq!(
            runtime.tool_call[0].tool_called_input,
            json!({ "commands": commands })
        );
        assert_eq!(runtime.tool_call[0].tool_received_at, finished_at);
        assert_eq!(runtime.state, RuntimeState::Created);
    }

    #[test]
    fn streaming_queue_runs_late_same_step_concurrently_and_waits_later_steps() {
        let _guard = STREAMING_TEST_ENV
            .lock()
            .unwrap_or_else(|err| err.into_inner());
        let router = MockStreamingRouter::start();
        let _router_env = EnvGuard::set("TURA_ROUTER_ADDR", &router.addr);
        let _gateway_env = EnvGuard::set("TURA_GATEWAY_CALLBACKS", "off");
        let (stream_tx, stream_rx) = mpsc::channel();
        let state = StreamedCommandRunState::new();
        let handle = spawn_streamed_command_run_task(SpawnStreamedCommandRunTask {
            stream_rx,
            session_directory: std::env::temp_dir(),
            allowed_command_run_commands: None,
            session_id: "stream-session".to_string(),
            runtime_id: "stream-runtime".to_string(),
            provider: json!({ "provider": "test" }),
            call_id: "stream-call".to_string(),
            started_at: Utc::now(),
            state,
            runtime_status: runtime().session_sync_status(),
        });

        stream_tx
            .send(stream_command_event("step1-a", 1, 0))
            .expect("first command event should send");
        router.wait_for_started(&["step1-a"], Duration::from_secs(2));

        stream_tx
            .send(stream_command_event("step1-b", 1, 1))
            .expect("second same-step command event should send");
        router.wait_for_started(&["step1-a", "step1-b"], Duration::from_secs(2));
        assert!(
            router.max_active() >= 2,
            "same-step streamed commands should reach the router concurrently"
        );

        stream_tx
            .send(stream_command_event("step2", 2, 2))
            .expect("later-step command event should send");
        std::thread::sleep(Duration::from_millis(150));
        assert!(
            !router.started().iter().any(|label| label == "step2"),
            "later-step streamed command must wait for the active step to finish"
        );

        router.release_step1();
        drop(stream_tx);
        let results = handle
            .join()
            .expect("streamed command task should not panic");

        router.wait_for_started(&["step1-a", "step1-b", "step2"], Duration::from_secs(2));
        let labels = results
            .iter()
            .map(|result| {
                result
                    .get("output")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string()
            })
            .collect::<Vec<_>>();
        assert_eq!(labels, vec!["step1-a", "step1-b", "step2"]);
        assert_eq!(results[0]["step"], 1);
        assert_eq!(results[1]["step"], 1);
        assert_eq!(results[2]["step"], 2);
    }

    #[test]
    fn streaming_queue_runs_late_lower_step_with_current_active_step() {
        let _guard = STREAMING_TEST_ENV
            .lock()
            .unwrap_or_else(|err| err.into_inner());
        let router = MockStreamingRouter::start();
        let _router_env = EnvGuard::set("TURA_ROUTER_ADDR", &router.addr);
        let _gateway_env = EnvGuard::set("TURA_GATEWAY_CALLBACKS", "off");
        let (stream_tx, stream_rx) = mpsc::channel();
        let state = StreamedCommandRunState::new();
        let handle = spawn_streamed_command_run_task(SpawnStreamedCommandRunTask {
            stream_rx,
            session_directory: std::env::temp_dir(),
            allowed_command_run_commands: None,
            session_id: "stream-session-late-lower".to_string(),
            runtime_id: "stream-runtime-late-lower".to_string(),
            provider: json!({ "provider": "test" }),
            call_id: "stream-call-late-lower".to_string(),
            started_at: Utc::now(),
            state,
            runtime_status: runtime().session_sync_status(),
        });

        stream_tx
            .send(stream_command_event("initial-step1", 1, 0))
            .expect("initial command event should send");
        router.wait_for_started(&["initial-step1"], Duration::from_secs(2));
        std::thread::sleep(Duration::from_millis(100));

        stream_tx
            .send(stream_command_event("step2-block", 2, 1))
            .expect("current active step command event should send");
        router.wait_for_started(&["step2-block"], Duration::from_secs(2));

        stream_tx
            .send(stream_command_event("late-lower-step1", 1, 2))
            .expect("late lower step command event should send");
        router.wait_for_started(
            &["initial-step1", "step2-block", "late-lower-step1"],
            Duration::from_secs(2),
        );
        assert!(
            router.max_active() >= 2,
            "late lower step should run alongside the current active step"
        );

        stream_tx
            .send(stream_command_event("step3", 3, 3))
            .expect("future step command event should send");
        std::thread::sleep(Duration::from_millis(150));
        assert!(
            !router.started().iter().any(|label| label == "step3"),
            "future step must still wait while the current active step is running"
        );

        router.release_step2();
        drop(stream_tx);
        let results = handle
            .join()
            .expect("streamed command task should not panic");

        router.wait_for_started(
            &["initial-step1", "step2-block", "late-lower-step1", "step3"],
            Duration::from_secs(2),
        );
        let labels = results
            .iter()
            .map(|result| {
                result
                    .get("output")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string()
            })
            .collect::<Vec<_>>();
        assert_eq!(
            labels,
            vec!["initial-step1", "step2-block", "late-lower-step1", "step3"]
        );
        assert_eq!(results[0]["step"], 1);
        assert_eq!(results[1]["step"], 2);
        assert_eq!(results[2]["step"], 1);
        assert_eq!(results[3]["step"], 3);
    }

    #[test]
    fn streaming_gateway_callbacks_do_not_delay_command_start() {
        let _guard = STREAMING_TEST_ENV
            .lock()
            .unwrap_or_else(|err| err.into_inner());
        let router = MockStreamingRouter::start();
        let gateway = HangingGateway::start(Duration::from_secs(5));
        let _router_env = EnvGuard::set("TURA_ROUTER_ADDR", &router.addr);
        let _gateway_enabled = EnvGuard::set("TURA_GATEWAY_CALLBACKS", "1");
        let _gateway_url = EnvGuard::set("TURA_GATEWAY_URL", &gateway.endpoint);
        let _gateway_timeout = EnvGuard::set("TURA_GATEWAY_CALLBACK_TIMEOUT_MS", "2000");
        let (stream_tx, stream_rx) = mpsc::channel();
        let state = StreamedCommandRunState::new();
        let handle = spawn_streamed_command_run_task(SpawnStreamedCommandRunTask {
            stream_rx,
            session_directory: std::env::temp_dir(),
            allowed_command_run_commands: None,
            session_id: "stream-session-callback".to_string(),
            runtime_id: "stream-runtime-callback".to_string(),
            provider: json!({ "provider": "test" }),
            call_id: "stream-call-callback".to_string(),
            started_at: Utc::now(),
            state,
            runtime_status: runtime().session_sync_status(),
        });

        stream_tx
            .send(stream_command_event("callback-fast", 1, 0))
            .expect("callback test command event should send");
        router.wait_for_started(&["callback-fast"], Duration::from_millis(750));

        drop(stream_tx);
        let results = handle
            .join()
            .expect("streamed command task should not panic");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0]["output"], "callback-fast");
    }

    fn stream_command_event(
        label: &str,
        step: u64,
        command_index: usize,
    ) -> tura_llm_rust::ProviderStreamEvent {
        tura_llm_rust::ProviderStreamEvent::CommandRunCommandReady {
            tool_call_id: "stream-tool-call".to_string(),
            command_index,
            command: json!({
                "step": step,
                "label": label,
                "command": "shell_command",
                "command_line": json!({
                    "command": "Test-Path .",
                    "timeout_ms": 5000
                }).to_string()
            }),
        }
    }

    struct MockStreamingRouter {
        addr: String,
        state: Arc<MockStreamingRouterState>,
    }

    struct MockStreamingRouterState {
        started: Mutex<Vec<String>>,
        release_step1: AtomicBool,
        release_step2: AtomicBool,
        active: AtomicUsize,
        max_active: AtomicUsize,
        release_notify: tokio::sync::Notify,
    }

    impl MockStreamingRouter {
        fn start() -> Self {
            let listener = std::net::TcpListener::bind("127.0.0.1:0")
                .expect("mock streaming router should bind");
            listener
                .set_nonblocking(true)
                .expect("mock streaming router should be nonblocking");
            let addr = listener
                .local_addr()
                .expect("mock streaming router should have addr")
                .to_string();
            let state = Arc::new(MockStreamingRouterState {
                started: Mutex::new(Vec::new()),
                release_step1: AtomicBool::new(false),
                release_step2: AtomicBool::new(false),
                active: AtomicUsize::new(0),
                max_active: AtomicUsize::new(0),
                release_notify: tokio::sync::Notify::new(),
            });
            let server_state = Arc::clone(&state);
            std::thread::spawn(move || {
                let runtime =
                    tokio::runtime::Runtime::new().expect("mock router runtime should start");
                runtime.block_on(async move {
                    let listener = TcpListener::from_std(listener)
                        .expect("mock router listener should convert to tokio");
                    while let Ok((stream, _)) = listener.accept().await {
                        let state = Arc::clone(&server_state);
                        tokio::spawn(async move {
                            let (read, mut write) = stream.into_split();
                            let mut reader = BufReader::new(read);
                            let mut line = String::new();
                            if reader.read_line(&mut line).await.unwrap_or(0) == 0 {
                                return;
                            }
                            let response = state.response_for(&line).await;
                            let _ = write.write_all(format!("{response}\n").as_bytes()).await;
                            let _ = write.flush().await;
                        });
                    }
                });
            });
            Self { addr, state }
        }

        fn started(&self) -> Vec<String> {
            self.state
                .started
                .lock()
                .unwrap_or_else(|err| err.into_inner())
                .clone()
        }

        fn wait_for_started(&self, labels: &[&str], timeout: Duration) {
            let deadline = std::time::Instant::now() + timeout;
            while std::time::Instant::now() < deadline {
                let started = self.started();
                if labels
                    .iter()
                    .all(|label| started.iter().any(|started| started == label))
                {
                    return;
                }
                std::thread::sleep(Duration::from_millis(20));
            }
            panic!(
                "timed out waiting for started labels {labels:?}; got {:?}",
                self.started()
            );
        }

        fn release_step1(&self) {
            self.state.release_step1.store(true, Ordering::SeqCst);
            self.state.release_notify.notify_waiters();
        }

        fn release_step2(&self) {
            self.state.release_step2.store(true, Ordering::SeqCst);
            self.state.release_notify.notify_waiters();
        }

        fn max_active(&self) -> usize {
            self.state.max_active.load(Ordering::SeqCst)
        }
    }

    impl MockStreamingRouterState {
        async fn response_for(&self, raw: &str) -> Value {
            let request: Value =
                serde_json::from_str(raw.trim()).expect("mock router request should be JSON");
            let request_id = request
                .get("request_id")
                .and_then(Value::as_str)
                .unwrap_or("missing");
            let command = request
                .pointer("/payload/arguments/commands/0")
                .cloned()
                .unwrap_or_else(|| json!({}));
            let label = command
                .get("label")
                .and_then(Value::as_str)
                .unwrap_or("missing-label")
                .to_string();
            self.record_started(label.clone());
            if label.starts_with("step1-") {
                while !self.release_step1.load(Ordering::SeqCst) {
                    self.release_notify.notified().await;
                }
            }
            if label == "step2-block" {
                while !self.release_step2.load(Ordering::SeqCst) {
                    self.release_notify.notified().await;
                }
            }
            self.active.fetch_sub(1, Ordering::SeqCst);
            json!({
                "request_id": request_id,
                "ok": true,
                "payload": {
                    "status": "finished",
                    "owner": "mock-router",
                    "result": {
                        "results": [{
                            "step": command.get("step").cloned().unwrap_or_else(|| json!(1)),
                            "command_type": command
                                .get("command_type")
                                .or_else(|| command.get("command"))
                                .cloned()
                                .unwrap_or_else(|| json!("command_run")),
                            "success": true,
                            "output": label
                        }]
                    }
                }
            })
        }

        fn record_started(&self, label: String) {
            {
                let mut started = self.started.lock().unwrap_or_else(|err| err.into_inner());
                started.push(label);
            }
            let active = self.active.fetch_add(1, Ordering::SeqCst) + 1;
            let mut current = self.max_active.load(Ordering::SeqCst);
            while active > current {
                match self.max_active.compare_exchange(
                    current,
                    active,
                    Ordering::SeqCst,
                    Ordering::SeqCst,
                ) {
                    Ok(_) => break,
                    Err(next) => current = next,
                }
            }
        }
    }

    struct HangingGateway {
        endpoint: String,
    }

    impl HangingGateway {
        fn start(delay: Duration) -> Self {
            let listener =
                std::net::TcpListener::bind("127.0.0.1:0").expect("hanging gateway should bind");
            let addr = listener
                .local_addr()
                .expect("hanging gateway should have addr");
            std::thread::spawn(move || {
                while let Ok((mut stream, _)) = listener.accept() {
                    std::thread::spawn(move || {
                        let mut buffer = [0_u8; 512];
                        let _ = std::io::Read::read(&mut stream, &mut buffer);
                        std::thread::sleep(delay);
                    });
                }
            });
            Self {
                endpoint: format!("http://{addr}"),
            }
        }
    }

    struct EnvGuard {
        key: &'static str,
        previous: Option<std::ffi::OsString>,
    }

    impl EnvGuard {
        fn set(key: &'static str, value: &str) -> Self {
            let previous = std::env::var_os(key);
            std::env::set_var(key, value);
            Self { key, previous }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            if let Some(previous) = self.previous.take() {
                std::env::set_var(self.key, previous);
            } else {
                std::env::remove_var(self.key);
            }
        }
    }
}
