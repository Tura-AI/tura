use runtime::runtime::runtime_receive::{
    command_run_stream_event_command, execute_runtime_stream_command_batch,
    execute_runtime_stream_event,
};
use serde_json::{json, Value};
use std::fs;
use std::path::Path;
use std::sync::{Mutex, OnceLock};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;

static MOCK_ROUTER_ADDR: OnceLock<String> = OnceLock::new();
static MOCK_ROUTER_INIT: Mutex<()> = Mutex::new(());

fn ensure_mock_router() {
    if let Some(addr) = MOCK_ROUTER_ADDR.get() {
        std::env::set_var("TURA_ROUTER_ADDR", addr);
        return;
    }
    let _guard = MOCK_ROUTER_INIT.lock().expect("mock router init lock");
    if let Some(addr) = MOCK_ROUTER_ADDR.get() {
        std::env::set_var("TURA_ROUTER_ADDR", addr);
        return;
    }

    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind mock router");
    listener
        .set_nonblocking(true)
        .expect("mock router nonblocking");
    let addr = listener.local_addr().expect("mock router addr").to_string();
    std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("mock router runtime");
        runtime.block_on(async move {
            let listener = TcpListener::from_std(listener).expect("tokio listener");
            while let Ok((stream, _)) = listener.accept().await {
                tokio::spawn(async move {
                    let (read, mut write) = stream.into_split();
                    let mut reader = BufReader::new(read);
                    let mut line = String::new();
                    if reader.read_line(&mut line).await.unwrap_or(0) == 0 {
                        return;
                    }
                    let response = mock_router_response(&line).await;
                    let _ = write.write_all(format!("{response}\n").as_bytes()).await;
                    let _ = write.flush().await;
                });
            }
        });
    });
    MOCK_ROUTER_ADDR
        .set(addr.clone())
        .expect("mock router addr set once");
    std::env::set_var("TURA_ROUTER_ADDR", addr);
}

async fn mock_router_response(raw: &str) -> Value {
    let request: Value = match serde_json::from_str(raw.trim()) {
        Ok(request) => request,
        Err(error) => {
            return json!({
                "request_id": "invalid",
                "ok": false,
                "error": format!("invalid request: {error}")
            });
        }
    };
    let request_id = request
        .get("request_id")
        .and_then(Value::as_str)
        .unwrap_or("missing")
        .to_string();
    if request.get("method").and_then(Value::as_str) != Some("execution.command_run") {
        return json!({
            "request_id": request_id,
            "ok": false,
            "error": "unsupported mock router method"
        });
    }
    let payload = &request["payload"];
    let Some(session_directory) = payload.get("session_directory").and_then(Value::as_str) else {
        return json!({
            "request_id": request_id,
            "ok": false,
            "error": "session_directory missing"
        });
    };
    let output = code_tools::command_run::execute_async_value(
        payload
            .get("arguments")
            .cloned()
            .unwrap_or_else(|| json!({})),
        std::path::PathBuf::from(session_directory),
    )
    .await;
    json!({
        "request_id": request_id,
        "ok": true,
        "payload": {
            "status": "finished",
            "owner": "router",
            "result": output
        }
    })
}

async fn execute_runtime_stream_event_with_mock_router(
    event: tura_llm_rust::ProviderStreamEvent,
    session_directory: std::path::PathBuf,
) -> Option<Value> {
    ensure_mock_router();
    execute_runtime_stream_event(event, session_directory).await
}

async fn execute_runtime_stream_command_batch_with_mock_router(
    commands: Vec<Value>,
    session_directory: std::path::PathBuf,
) -> Option<Value> {
    ensure_mock_router();
    execute_runtime_stream_command_batch(commands, session_directory).await
}

fn shell_command(command: &str, step: u64) -> Value {
    shell_command_with_timeout(command, step, 5000)
}

fn shell_command_with_timeout(command: &str, step: u64, timeout_ms: u64) -> Value {
    json!({
        "step": step,
        "command": "shell_command",
        "command_line": json!({
            "command": command,
            "timeout_ms": timeout_ms
        }).to_string()
    })
}

fn text_at(path: &std::path::Path) -> String {
    fs::read_to_string(path).unwrap_or_else(|error| {
        panic!("failed to read {}: {error}", path.display());
    })
}

fn workspace_entries(path: &Path) -> Vec<String> {
    let mut entries = fs::read_dir(path)
        .unwrap_or_else(|error| panic!("workspace {} is readable: {error}", path.display()))
        .map(|entry| {
            entry
                .expect("workspace entry should be readable")
                .file_name()
                .to_string_lossy()
                .to_string()
        })
        .collect::<Vec<_>>();
    entries.sort();
    entries
}

#[tokio::test]
async fn streamed_provider_command_ready_event_executes_once_in_session_workspace() {
    let workspace = tempfile::tempdir().expect("workspace");
    let command = shell_command("echo runtime-stream-event-ok > streamed-event.txt", 1);
    let event = tura_llm_rust::ProviderStreamEvent::CommandRunCommandReady {
        tool_call_id: "call_runtime_stream_event".to_string(),
        command_index: 0,
        command: command.clone(),
    };

    assert_eq!(
        command_run_stream_event_command(event.clone()),
        Some(command)
    );

    let output =
        execute_runtime_stream_event_with_mock_router(event, workspace.path().to_path_buf())
            .await
            .expect("command ready event should produce command_run output");

    assert_eq!(output["results"][0]["command_type"], "shell_command");
    assert_eq!(output["results"][0]["success"], true);
    assert!(
        text_at(&workspace.path().join("streamed-event.txt")).contains("runtime-stream-event-ok")
    );
}

#[tokio::test]
async fn streamed_provider_non_command_events_do_not_execute_workspace_commands() {
    let workspace = tempfile::tempdir().expect("workspace");

    let output = execute_runtime_stream_event_with_mock_router(
        tura_llm_rust::ProviderStreamEvent::TextDelta {
            text: "not a command".to_string(),
        },
        workspace.path().to_path_buf(),
    )
    .await;

    assert_eq!(output, None);
    assert!(
        fs::read_dir(workspace.path())
            .expect("workspace is readable")
            .next()
            .is_none(),
        "text-only provider events must not produce workspace side effects"
    );
}

#[tokio::test]
async fn streamed_command_batch_executes_multiple_ready_commands_in_ordered_result_shape() {
    let workspace = tempfile::tempdir().expect("workspace");
    let commands = vec![
        shell_command("echo runtime-stream-batch-one > streamed-batch-one.txt", 1),
        shell_command("echo runtime-stream-batch-two > streamed-batch-two.txt", 1),
    ];

    let output = execute_runtime_stream_command_batch_with_mock_router(
        commands,
        workspace.path().to_path_buf(),
    )
    .await
    .expect("non-empty command batch should produce command_run output");

    let results = output["results"]
        .as_array()
        .expect("command_run output should contain results");
    assert_eq!(results.len(), 2);
    assert_eq!(results[0]["step"], 1);
    assert_eq!(
        results[1]["step"], 2,
        "command_run normalizes duplicate batch steps into a monotonic sequence"
    );
    assert_eq!(results[0]["command_type"], "shell_command");
    assert_eq!(results[1]["command_type"], "shell_command");
    assert_eq!(results[0]["success"], true);
    assert_eq!(results[1]["success"], true);
    assert!(text_at(&workspace.path().join("streamed-batch-one.txt"))
        .contains("runtime-stream-batch-one"));
    assert!(text_at(&workspace.path().join("streamed-batch-two.txt"))
        .contains("runtime-stream-batch-two"));
}

#[tokio::test]
async fn streamed_command_batch_empty_input_returns_none_without_workspace_side_effects() {
    let workspace = tempfile::tempdir().expect("workspace");

    let output = execute_runtime_stream_command_batch_with_mock_router(
        Vec::new(),
        workspace.path().to_path_buf(),
    )
    .await;

    assert_eq!(output, None);
    assert!(
        fs::read_dir(workspace.path())
            .expect("workspace is readable")
            .next()
            .is_none(),
        "empty streamed command batch must not touch the workspace"
    );
}

#[tokio::test]
async fn streamed_command_batch_reports_shell_failures_without_masking_result_shape() {
    let workspace = tempfile::tempdir().expect("workspace");
    let commands = vec![shell_command("exit 7", 1)];

    let output = execute_runtime_stream_command_batch_with_mock_router(
        commands,
        workspace.path().to_path_buf(),
    )
    .await
    .expect("failing command still returns command_run output");

    let results = output["results"]
        .as_array()
        .expect("command_run output should contain results");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0]["step"], 1);
    assert_eq!(results[0]["command_type"], "shell_command");
    assert_eq!(results[0]["success"], false);
    assert_ne!(
        results[0]["exit_code"], 0,
        "shell failure should preserve a non-zero exit code"
    );
    assert!(
        !workspace
            .path()
            .join("streamed-failure-should-not-exist.txt")
            .exists(),
        "failing command should not create unrelated success artifacts"
    );
}

#[tokio::test]
async fn streamed_command_batch_continues_after_failure_and_keeps_result_order() {
    let workspace = tempfile::tempdir().expect("workspace");
    let output = execute_runtime_stream_command_batch_with_mock_router(
        vec![
            shell_command("exit 9", 1),
            shell_command("echo runtime-after-failure > after-failure.txt", 1),
        ],
        workspace.path().to_path_buf(),
    )
    .await
    .expect("mixed failure/success command batch should produce command_run output");

    let results = output["results"]
        .as_array()
        .expect("command_run output should contain results");
    assert_eq!(results.len(), 2);
    assert_eq!(results[0]["step"], 1);
    assert_eq!(results[1]["step"], 2);
    assert_eq!(results[0]["command_type"], "shell_command");
    assert_eq!(results[1]["command_type"], "shell_command");
    assert_eq!(results[0]["success"], false);
    assert_eq!(results[1]["success"], true);
    assert_ne!(results[0]["exit_code"], 0);
    assert!(
        text_at(&workspace.path().join("after-failure.txt")).contains("runtime-after-failure"),
        "later commands in the streamed batch should still run after an earlier failure"
    );
}

#[tokio::test]
async fn streamed_command_batch_reports_timeouts_without_success_side_effects() {
    let workspace = tempfile::tempdir().expect("workspace");
    let command = if cfg!(windows) {
        "Start-Sleep -Milliseconds 1500; Set-Content streamed-timeout-should-not-exist.txt done"
    } else {
        "sleep 1.5; echo done > streamed-timeout-should-not-exist.txt"
    };
    let started = std::time::Instant::now();

    let output = execute_runtime_stream_command_batch_with_mock_router(
        vec![shell_command_with_timeout(command, 1, 150)],
        workspace.path().to_path_buf(),
    )
    .await
    .expect("timed out command still returns command_run output");

    assert!(
        started.elapsed() < std::time::Duration::from_secs(5),
        "timeout should bound the streamed command duration"
    );
    let results = output["results"]
        .as_array()
        .expect("command_run output should contain results");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0]["step"], 1);
    assert_eq!(results[0]["command_type"], "shell_command");
    assert_eq!(results[0]["success"], false);
    let timeout_text = results[0]["error"]
        .as_str()
        .or_else(|| results[0]["stderr"].as_str())
        .or_else(|| results[0]["output"].as_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    assert!(
        timeout_text.contains("timeout") || timeout_text.contains("timed out"),
        "timeout failure should be visible in the command result: {output}"
    );
    assert!(
        !workspace
            .path()
            .join("streamed-timeout-should-not-exist.txt")
            .exists(),
        "timed out command must not be reported as a successful workspace write"
    );
}

#[tokio::test]
async fn streamed_command_batch_runs_same_step_macro_commands_without_result_cross_talk() {
    let workspace = tempfile::tempdir().expect("workspace");
    let output = execute_runtime_stream_command_batch_with_mock_router(
        vec![
            shell_command("echo alpha-one > alpha.txt", 1),
            shell_command("echo beta-one > beta.txt", 1),
            shell_command("echo gamma-one > gamma.txt", 1),
        ],
        workspace.path().to_path_buf(),
    )
    .await
    .expect("same-step macro command batch should produce output");

    let results = output["results"]
        .as_array()
        .expect("command_run output should contain results");
    assert_eq!(results.len(), 3);
    assert_eq!(results[0]["step"], 1);
    assert_eq!(results[1]["step"], 2);
    assert_eq!(results[2]["step"], 3);
    assert!(results
        .iter()
        .all(|result| { result["command_type"] == "shell_command" && result["success"] == true }));
    assert!(text_at(&workspace.path().join("alpha.txt")).contains("alpha-one"));
    assert!(text_at(&workspace.path().join("beta.txt")).contains("beta-one"));
    assert!(text_at(&workspace.path().join("gamma.txt")).contains("gamma-one"));
}

#[tokio::test]
async fn streamed_command_batches_repeated_workspaces_do_not_cross_talk() {
    let batch_count = 5;
    let mut completed = Vec::new();

    for index in 0..batch_count {
        let workspace = tempfile::tempdir().expect("workspace");
        let marker = format!("runtime-repeated-stream-{index}");
        let first_file = format!("batch-{index}-first.txt");
        let second_file = format!("batch-{index}-second.txt");

        let output = execute_runtime_stream_command_batch_with_mock_router(
            vec![
                shell_command(&format!("echo {marker}-first > {first_file}"), 1),
                shell_command(&format!("echo {marker}-second > {second_file}"), 1),
            ],
            workspace.path().to_path_buf(),
        )
        .await
        .expect("repeated command batch should produce command_run output");

        let results = output["results"]
            .as_array()
            .expect("command_run output should contain results");
        assert_eq!(results.len(), 2);
        assert_eq!(results[0]["step"], 1);
        assert_eq!(results[1]["step"], 2);
        assert!(results.iter().all(|result| {
            result["command_type"] == "shell_command" && result["success"] == true
        }));
        assert!(text_at(&workspace.path().join(&first_file)).contains(&format!("{marker}-first")));
        assert!(text_at(&workspace.path().join(&second_file)).contains(&format!("{marker}-second")));
        let entries = workspace_entries(workspace.path());
        assert_eq!(
            entries.len(),
            2,
            "repeated stream batch workspace should only contain its own artifacts"
        );
        assert!(entries.iter().any(|name| name == &first_file));
        assert!(entries.iter().any(|name| name == &second_file));
        completed.push((index, output));
    }
    assert_eq!(completed.len(), batch_count);
    for (index, output) in completed {
        let serialized = output.to_string();
        assert!(
            serialized.contains("shell_command"),
            "batch {index} output should preserve command result shape: {output}"
        );
        for other in 0..batch_count {
            if other != index {
                assert!(
                    !serialized.contains(&format!("batch-{other}-")),
                    "repeated batch {index} result must not mention another workspace artifact: {output}"
                );
            }
        }
    }
}

#[tokio::test]
async fn streamed_command_batches_concurrent_workspaces_remain_isolated() {
    let completed = tokio::time::timeout(std::time::Duration::from_secs(20), async {
        streamed_command_batches_concurrent_workspaces_remain_isolated_inner().await
    })
    .await
    .expect("concurrent streamed command batches should finish before the business timeout");
    assert_eq!(completed, 4);
}

async fn streamed_command_batches_concurrent_workspaces_remain_isolated_inner() -> usize {
    let workspaces = (0..4)
        .map(|_| tempfile::tempdir().expect("workspace"))
        .collect::<Vec<_>>();
    let mut tasks = Vec::new();

    for (index, workspace) in workspaces.iter().enumerate() {
        let session_directory = workspace.path().to_path_buf();
        let marker = format!("runtime-concurrent-stream-{index}");
        let commands = vec![
            json!({
                "step": 1,
                "command": "task_status",
                "command_line": json!({
                    "status": "done",
                    "task_detail": format!("{marker}-first")
                }).to_string()
            }),
            json!({
                "step": 1,
                "command": "task_status",
                "command_line": json!({
                    "status": "question",
                    "task_detail": format!("{marker}-second")
                }).to_string()
            }),
        ];
        tasks.push(tokio::spawn(async move {
            let output =
                execute_runtime_stream_command_batch_with_mock_router(commands, session_directory)
                    .await
                    .expect("concurrent command batch should produce command_run output");
            (index, marker, output)
        }));
    }

    let mut completed = Vec::new();
    for task in tasks {
        completed.push(task.await.expect("concurrent command task should join"));
    }
    completed.sort_by_key(|(index, _, _)| *index);

    for (index, marker, output) in completed {
        let workspace = workspaces[index].path();
        let results = output["results"]
            .as_array()
            .expect("command_run output should contain results");
        assert_eq!(results.len(), 2);
        assert_eq!(results[0]["step"], 1);
        assert_eq!(results[1]["step"], 2);
        assert!(
            results.iter().all(|result| {
                result["command_type"] == "task_status" && result["success"] == true
            }),
            "task_status command batch should succeed with status result shape: {output}"
        );
        assert_eq!(
            results[0]["output"]["task_status"]["task_detail"],
            format!("{marker}-first")
        );
        assert_eq!(
            results[1]["output"]["task_status"]["task_detail"],
            format!("{marker}-second")
        );
        assert_eq!(results[0]["output"]["task_status"]["status"], "done");
        assert_eq!(results[1]["output"]["task_status"]["status"], "question");

        let entries = workspace_entries(workspace);
        assert!(
            entries.is_empty(),
            "planning-only concurrent batches should not create workspace artifacts"
        );
        let serialized = output.to_string();
        assert!(serialized.contains("task_status"));
        for other in 0..workspaces.len() {
            if other != index {
                assert!(
                    !serialized.contains(&format!("runtime-concurrent-stream-{other}")),
                    "concurrent batch {index} result must not mention another workspace artifact: {output}"
                );
            }
        }
    }
    workspaces.len()
}
