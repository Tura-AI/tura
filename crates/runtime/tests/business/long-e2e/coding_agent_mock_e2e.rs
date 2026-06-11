use std::io::{BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

use runtime::mano;
use runtime::state_machine::session_management::{SessionInput, SessionState};
use serde_json::{json, Value};

const ROUTES: &[&str] = &[
    "flagship_thinking",
    "thinking",
    "fast",
    "instant",
    "embedding_high",
    "embedding_low",
];

static ENV_LOCK: Mutex<()> = Mutex::new(());
const MOCK_COMMAND_TIMEOUT_MS: u64 = 3_000;
const MOCK_PROVIDER_TIMEOUT_MS: &str = "30000";
const MOCK_PROVIDER_STREAM_TIMEOUT_MS: &str = "1000";
const MOCK_POST_COMMAND_TIMEOUT_MS: &str = "250";

#[test]
fn coding_agent_can_call_command_run_tool_e2e() {
    let _lock = ENV_LOCK.lock().expect("e2e env lock should be available");
    let workspace = create_rust_workspace();
    let provider = MockProvider::start_command_run();
    let llm_config = write_llm_config(&workspace, provider.addr);
    let _env = EnvGuard::set(&[
        ("TURALLM_CONFIG", llm_config.to_string_lossy().as_ref()),
        ("OPENAI_API_KEY", "test-key"),
        ("TURA_GATEWAY_CALLBACKS", "0"),
        ("TURA_MANAS_MAX_TURNS", "4"),
        ("TURA_NO_TOOL_RETRY_LIMIT", "0"),
        ("TURA_PROVIDER_TOTAL_TIMEOUT_MS", MOCK_PROVIDER_TIMEOUT_MS),
        (
            "TURA_PROVIDER_FIRST_OUTPUT_TIMEOUT_MS",
            MOCK_PROVIDER_STREAM_TIMEOUT_MS,
        ),
        (
            "TURA_PROVIDER_IDLE_OUTPUT_TIMEOUT_MS",
            MOCK_PROVIDER_STREAM_TIMEOUT_MS,
        ),
        (
            "TURA_STREAMED_COMMAND_RUN_POST_RESULT_TIMEOUT_MS",
            MOCK_POST_COMMAND_TIMEOUT_MS,
        ),
    ]);

    let result = mano::process_from_gateway_session_in_directory(
        "e2e-run-command-tool".to_string(),
        SessionInput {
            user_input: "Run pwd with command_run, then patch src/lib.rs with command_run apply_patch, verify it with shell_command, and finish with normal assistant text."
                .to_string(),
            file_input: vec![],
            agent: None,
            runtime_context: None,
            planning_mode_override: None,
        },
        workspace.clone(),
    )
    .expect("coding agent should complete the command_run e2e flow");

    assert_eq!(result.agents.len(), 1);
    assert_eq!(result.agents[0].agent_name, "fast");
    assert_eq!(result.session.state, SessionState::Completed);

    let tool_results = tool_results(&result.session.session_log);
    assert_tool_success(&tool_results, "command_run");
    assert!(!tool_results
        .iter()
        .any(|result| result.get("tool_name").and_then(Value::as_str)
            == Some("send_message_to_user")));
    assert!(result
        .session
        .session_log
        .iter()
        .filter_map(|entry| serde_json::from_str::<Value>(entry).ok())
        .any(|entry| entry.get("role").and_then(Value::as_str) == Some("assistant")));

    let run_output = tool_results
        .iter()
        .find(|result| result.get("tool_name").and_then(Value::as_str) == Some("command_run"))
        .and_then(|result| result.get("output"))
        .cloned()
        .unwrap_or(Value::Null);
    assert!(run_output
        .pointer("/results/0/output")
        .and_then(Value::as_str)
        .is_some_and(|output| output.starts_with("Exit code: 0\n")));
    assert!(run_output.pointer("/results/0/exit_code").is_none());
    assert!(run_output.pointer("/results/0/display_command").is_none());

    let patched_content = std::fs::read_to_string(workspace.join("src/lib.rs"))
        .expect("patched file should be readable");
    assert!(
        !patched_content.trim().is_empty(),
        "patched file was empty; tool_results={tool_results:#?}"
    );

    let requests = provider
        .requests
        .lock()
        .expect("mock provider requests lock");
    let first_tools = requests
        .iter()
        .find(|request| request.get("tools").and_then(Value::as_array).is_some())
        .and_then(|request| request.get("tools"))
        .and_then(Value::as_array)
        .expect("at least one provider request should include tools");
    let first_tool_names = first_tools
        .iter()
        .filter_map(|tool| {
            tool.pointer("/function/name")
                .or_else(|| tool.get("name"))
                .and_then(Value::as_str)
        })
        .collect::<Vec<_>>();
    assert!(first_tool_names.contains(&"command_run"));
}

#[test]
fn coding_agent_executes_command_run_command_before_stream_finishes() {
    let _lock = ENV_LOCK.lock().expect("e2e env lock should be available");
    let workspace = create_rust_workspace();
    let provider = MockProvider::start_codex_streaming_probe(workspace.clone());
    let llm_config = write_codex_llm_config(&workspace);
    let endpoint = format!("http://{}", provider.addr);
    let _env = EnvGuard::set(&[
        ("TURALLM_CONFIG", llm_config.to_string_lossy().as_ref()),
        ("OPENAI_LOGIN", "oauth"),
        ("OPENAI_API_KEY", "test-key"),
        ("OPENAI_CODEX_ENDPOINT", endpoint.as_str()),
        ("TURA_GATEWAY_CALLBACKS", "0"),
        ("TURA_MANAS_MAX_TURNS", "2"),
        ("TURA_NO_TOOL_RETRY_LIMIT", "0"),
        ("TURA_PROVIDER_TOTAL_TIMEOUT_MS", MOCK_PROVIDER_TIMEOUT_MS),
        (
            "TURA_PROVIDER_FIRST_OUTPUT_TIMEOUT_MS",
            MOCK_PROVIDER_STREAM_TIMEOUT_MS,
        ),
        (
            "TURA_PROVIDER_IDLE_OUTPUT_TIMEOUT_MS",
            MOCK_PROVIDER_STREAM_TIMEOUT_MS,
        ),
        (
            "TURA_STREAMED_COMMAND_RUN_POST_RESULT_TIMEOUT_MS",
            MOCK_POST_COMMAND_TIMEOUT_MS,
        ),
    ]);

    let result = mano::process_from_gateway_session_in_directory(
        "e2e-stream-command-before-message-done".to_string(),
        SessionInput {
            user_input: "Use command_run in this code file workspace to create streamed-first.txt, then create streamed-second.txt."
                .to_string(),
            file_input: vec![],
            agent: None,
            runtime_context: None,
            planning_mode_override: None,
        },
        workspace.clone(),
    )
    .expect("coding agent should complete the streaming command_run e2e flow");

    assert!(
        provider
            .first_command_observed_before_response_finished
            .load(Ordering::SeqCst),
        "first streamed command did not execute before the provider finished sending the response"
    );
    assert_eq!(result.session.state, SessionState::Completed);
    assert!(
        workspace.join("streamed-first.txt").exists(),
        "first streamed command should create streamed-first.txt"
    );
    assert!(
        workspace.join("streamed-second.txt").exists(),
        "second streamed command should create streamed-second.txt"
    );
}

struct MockProvider {
    addr: SocketAddr,
    requests: Arc<Mutex<Vec<Value>>>,
    first_command_observed_before_response_finished: Arc<AtomicBool>,
}

#[derive(Debug, Clone, Copy)]
enum MockMode {
    CommandRun,
    CodexStreamingProbe,
}

impl MockProvider {
    fn start_command_run() -> Self {
        Self::start_with_mode(MockMode::CommandRun, None)
    }

    fn start_codex_streaming_probe(workspace: PathBuf) -> Self {
        Self::start_with_mode(MockMode::CodexStreamingProbe, Some(workspace))
    }

    fn start_with_mode(mode: MockMode, workspace: Option<PathBuf>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("mock provider should bind");
        let addr = listener.local_addr().expect("mock provider address");
        let requests = Arc::new(Mutex::new(Vec::new()));
        let counter = Arc::new(AtomicUsize::new(0));
        let first_command_observed_before_response_finished = Arc::new(AtomicBool::new(false));
        let thread_requests = Arc::clone(&requests);
        let thread_counter = Arc::clone(&counter);
        let thread_first_command_observed =
            Arc::clone(&first_command_observed_before_response_finished);

        thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                handle_provider_connection(
                    stream,
                    &thread_counter,
                    &thread_requests,
                    mode,
                    workspace.as_deref(),
                    &thread_first_command_observed,
                );
            }
        });

        Self {
            addr,
            requests,
            first_command_observed_before_response_finished,
        }
    }
}

fn handle_provider_connection(
    stream: TcpStream,
    counter: &AtomicUsize,
    requests: &Arc<Mutex<Vec<Value>>>,
    mode: MockMode,
    workspace: Option<&Path>,
    first_command_observed_before_response_finished: &AtomicBool,
) {
    let mut reader = BufReader::new(stream);
    let mut content_length = 0usize;
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).is_err() || line == "\r\n" || line == "\n" {
            break;
        }
        if let Some((name, value)) = line.split_once(':') {
            if name.eq_ignore_ascii_case("content-length") {
                content_length = value.trim().parse::<usize>().unwrap_or(0);
            }
        }
    }

    let mut body = vec![0; content_length];
    let _ = reader.read_exact(&mut body);
    let request = serde_json::from_slice::<Value>(&body).unwrap_or_else(|_| json!({}));

    let index = counter.fetch_add(1, Ordering::SeqCst);
    let response = provider_response(index, mode);
    requests
        .lock()
        .expect("mock provider requests lock")
        .push(request);
    let stream = reader.get_mut();
    match mode {
        MockMode::CommandRun => {
            write_command_run_responses(stream, &response);
        }
        MockMode::CodexStreamingProbe => {
            if index == 0 {
                write_codex_streaming_probe_response(
                    stream,
                    workspace.expect("codex streaming probe workspace"),
                    first_command_observed_before_response_finished,
                );
            } else {
                write_codex_final_response(stream, "streaming command probe completed.");
            }
        }
    }
}

fn provider_response(index: usize, mode: MockMode) -> Value {
    match mode {
        MockMode::CommandRun => command_run_provider_response(index),
        MockMode::CodexStreamingProbe => assistant_response("streaming probe completed."),
    }
}

fn write_codex_streaming_probe_response(
    stream: &mut TcpStream,
    workspace: &Path,
    first_command_observed_before_response_finished: &AtomicBool,
) {
    let _ = write!(
        stream,
        "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n"
    );
    write_codex_sse(
        stream,
        json!({
            "type": "response.output_item.added",
            "item": {
                "id": "fc_stream_probe",
                "type": "function_call",
                "call_id": "call_stream_probe",
                "name": "command_run",
                "arguments": ""
            }
        }),
    );
    write_codex_sse(
        stream,
        json!({
            "type": "response.function_call_arguments.delta",
            "item_id": "fc_stream_probe",
            "delta": "{\"commands\":["
        }),
    );
    write_codex_sse(
        stream,
        json!({
            "type": "response.function_call_arguments.delta",
            "item_id": "fc_stream_probe",
            "delta": json!({
                "step": 1,
                "command_type": "shell_command",
                "command_line": json!({
                    "command": "python -c \"from pathlib import Path; Path('streamed-first.txt').write_text('first')\"",
                    "timeout_ms": MOCK_COMMAND_TIMEOUT_MS
                }).to_string()
            }).to_string() + ","
        }),
    );
    let first_path = workspace.join("streamed-first.txt");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    while std::time::Instant::now() < deadline {
        if first_path.exists() {
            first_command_observed_before_response_finished.store(true, Ordering::SeqCst);
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
    write_codex_sse(
        stream,
        json!({
            "type": "response.function_call_arguments.delta",
            "item_id": "fc_stream_probe",
            "delta": json!({
                "step": 2,
                "command_type": "shell_command",
                "command_line": json!({
                    "command": "python -c \"from pathlib import Path; Path('streamed-second.txt').write_text('second')\"",
                    "timeout_ms": MOCK_COMMAND_TIMEOUT_MS
                }).to_string()
            }).to_string() + "]}"
        }),
    );
    write_codex_sse(
        stream,
        json!({
            "type": "response.function_call_arguments.done",
            "item_id": "fc_stream_probe",
            "arguments": json!({
                "commands": [
                    {
                        "step": 1,
                        "command_type": "shell_command",
                        "command_line": json!({
                            "command": "python -c \"from pathlib import Path; Path('streamed-first.txt').write_text('first')\"",
                            "timeout_ms": MOCK_COMMAND_TIMEOUT_MS
                        }).to_string()
                    },
                    {
                        "step": 2,
                        "command_type": "shell_command",
                        "command_line": json!({
                            "command": "python -c \"from pathlib import Path; Path('streamed-second.txt').write_text('second')\"",
                            "timeout_ms": MOCK_COMMAND_TIMEOUT_MS
                        }).to_string()
                    }
                ]
            }).to_string()
        }),
    );
    write_codex_sse(
        stream,
        json!({
            "type": "response.completed",
            "response": {
                "id": "resp_stream_probe",
                "output": [{
                    "id": "fc_stream_probe",
                    "type": "function_call",
                    "call_id": "call_stream_probe",
                    "name": "command_run",
                    "arguments": json!({
                        "commands": [
                            {
                                "step": 1,
                                "command_type": "shell_command",
                                "command_line": json!({
                                    "command": "python -c \"from pathlib import Path; Path('streamed-first.txt').write_text('first')\"",
                                    "timeout_ms": MOCK_COMMAND_TIMEOUT_MS
                                }).to_string()
                            },
                            {
                                "step": 2,
                                "command_type": "shell_command",
                                "command_line": json!({
                                    "command": "python -c \"from pathlib import Path; Path('streamed-second.txt').write_text('second')\"",
                                    "timeout_ms": MOCK_COMMAND_TIMEOUT_MS
                                }).to_string()
                            }
                        ]
                    }).to_string()
                }],
                "usage": {
                    "input_tokens": 1,
                    "output_tokens": 1,
                    "total_tokens": 2
                }
            }
        }),
    );
    write_codex_sse_raw(stream, "data: [DONE]\n\n");
    let _ = write!(stream, "0\r\n\r\n");
    let _ = stream.flush();
}

/// Translate the chat.completion-shaped mock `response` into the OpenAI
/// Responses-API SSE stream that the runtime now consumes for the `openai`
/// provider (non-OAuth OpenAI rides the shared Responses core).
fn write_command_run_responses(stream: &mut TcpStream, response: &Value) {
    let _ = write!(
        stream,
        "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n"
    );

    let message = response
        .pointer("/choices/0/message")
        .cloned()
        .unwrap_or_else(|| json!({}));

    if let Some(tool_calls) = message.get("tool_calls").and_then(Value::as_array) {
        let mut output_items = Vec::new();
        for (index, call) in tool_calls.iter().enumerate() {
            let call_id = call
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or("call_mock")
                .to_string();
            let item_id = format!("fc_cmd_{index}");
            let name = call
                .pointer("/function/name")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            let arguments = call
                .pointer("/function/arguments")
                .and_then(Value::as_str)
                .unwrap_or("{}")
                .to_string();
            write_codex_sse(
                stream,
                json!({
                    "type": "response.output_item.added",
                    "item": {
                        "id": item_id,
                        "type": "function_call",
                        "call_id": call_id,
                        "name": name,
                        "arguments": ""
                    }
                }),
            );
            write_codex_sse(
                stream,
                json!({
                    "type": "response.function_call_arguments.delta",
                    "item_id": item_id,
                    "delta": arguments
                }),
            );
            write_codex_sse(
                stream,
                json!({
                    "type": "response.function_call_arguments.done",
                    "item_id": item_id,
                    "arguments": arguments
                }),
            );
            output_items.push(json!({
                "id": item_id,
                "type": "function_call",
                "call_id": call_id,
                "name": name,
                "arguments": arguments
            }));
        }
        write_codex_sse(
            stream,
            json!({
                "type": "response.completed",
                "response": {
                    "id": "resp_cmd_run",
                    "output": output_items,
                    "usage": { "input_tokens": 1, "output_tokens": 1, "total_tokens": 2 }
                }
            }),
        );
        write_codex_sse_raw(stream, "data: [DONE]\n\n");
        let _ = write!(stream, "0\r\n\r\n");
        let _ = stream.flush();
        return;
    }

    let content = message
        .get("content")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    write_command_run_final_text(stream, &content);
}

fn write_command_run_final_text(stream: &mut TcpStream, content: &str) {
    write_codex_sse(
        stream,
        json!({
            "type": "response.output_text.delta",
            "delta": content
        }),
    );
    write_codex_sse(
        stream,
        json!({
            "type": "response.completed",
            "response": {
                "id": "resp_cmd_run_final",
                "output": [{
                    "id": "msg_cmd_run_final",
                    "type": "message",
                    "role": "assistant",
                    "content": [{
                        "type": "output_text",
                        "text": content
                    }]
                }],
                "usage": { "input_tokens": 1, "output_tokens": 1, "total_tokens": 2 }
            }
        }),
    );
    write_codex_sse_raw(stream, "data: [DONE]\n\n");
    let _ = write!(stream, "0\r\n\r\n");
    let _ = stream.flush();
}

fn write_codex_sse(stream: &mut TcpStream, value: Value) {
    write_codex_sse_raw(stream, &format!("data: {}\n\n", value));
}

fn write_codex_sse_raw(stream: &mut TcpStream, data: &str) {
    let _ = write!(stream, "{:X}\r\n{}\r\n", data.len(), data);
    let _ = stream.flush();
}

fn write_codex_final_response(stream: &mut TcpStream, content: &str) {
    let _ = write!(
        stream,
        "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n"
    );
    write_codex_sse(
        stream,
        json!({
            "type": "response.output_text.delta",
            "delta": content
        }),
    );
    write_codex_sse(
        stream,
        json!({
            "type": "response.completed",
            "response": {
                "id": "resp_stream_probe_final",
                "output": [{
                    "id": "msg_stream_probe_final",
                    "type": "message",
                    "role": "assistant",
                    "content": [{
                        "type": "output_text",
                        "text": content
                    }]
                }],
                "usage": {
                    "input_tokens": 1,
                    "output_tokens": 1,
                    "total_tokens": 2
                }
            }
        }),
    );
    write_codex_sse_raw(stream, "data: [DONE]\n\n");
    let _ = write!(stream, "0\r\n\r\n");
    let _ = stream.flush();
}

fn command_run_provider_response(index: usize) -> Value {
    match index {
        0 => tool_response(
            "call_command_run",
            "command_run",
            json!({
                "commands": [
                    { "command": "shell_command", "command_line": json!({"command":"pwd","timeout_ms":MOCK_COMMAND_TIMEOUT_MS}).to_string(), "step": 1 },
                    { "command": "shell_command", "command_line": json!({"command":"Write-Output 2","timeout_ms":MOCK_COMMAND_TIMEOUT_MS}).to_string(), "step": 1 }
                ],
                "step_summary": "Call the command_run console tool as requested."
            }),
        ),
        1 => tool_response(
            "call_apply_patch",
            "command_run",
            json!({
                "commands": [
                    {
                        "step": 1,
                        "command": "apply_patch",
                        "command_line": "*** Begin Patch\n*** Update File: src/lib.rs\n@@\n-pub fn process_manas_internal(input: &str) -> String {\n-    format!(\"processed {input}\")\n+pub fn process_manas_internal(input: &str) -> String {\n+    format!(\"processed verified {input}\")\n }\n*** End Patch"
                    },
                    {
                        "step": 2,
                        "command": "shell_command",
                        "command_line": json!({"command":"Get-Content src/lib.rs","timeout_ms":MOCK_COMMAND_TIMEOUT_MS}).to_string()
                    }
                ],
                "step_summary": "Patch src/lib.rs and verify the edited content."
            }),
        ),
        _ => assistant_response("done."),
    }
}

fn assistant_response(content: &str) -> Value {
    json!({
        "id": "chatcmpl-final",
        "object": "chat.completion",
        "choices": [{
            "index": 0,
            "message": {
                "role": "assistant",
                "content": content
            },
            "finish_reason": "stop"
        }],
        "usage": { "prompt_tokens": 1, "completion_tokens": 1, "total_tokens": 2 }
    })
}

fn tool_response(id: &str, name: &str, arguments: Value) -> Value {
    json!({
        "id": format!("chatcmpl-{id}"),
        "object": "chat.completion",
        "choices": [{
            "index": 0,
            "message": {
                "role": "assistant",
                "content": Value::Null,
                "tool_calls": [{
                    "id": id,
                    "type": "function",
                    "function": {
                        "name": name,
                        "arguments": arguments.to_string()
                    }
                }]
            },
            "finish_reason": "tool_calls"
        }],
        "usage": { "prompt_tokens": 1, "completion_tokens": 1, "total_tokens": 2 }
    })
}

fn create_rust_workspace() -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "tura-agent-lsp-e2e-{}",
        chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default()
    ));
    let src = root.join("src");
    std::fs::create_dir_all(&src).expect("test workspace src should be created");
    write_fixture(
        &root.join("Cargo.toml"),
        r#"[package]
name = "tura-agent-lsp-e2e"
version = "0.1.0"
edition = "2021"
"#,
    );
    write_fixture(
        &src.join("lib.rs"),
        r#"pub mod extra;
pub mod worker;

pub fn process_manas_internal(input: &str) -> String {
    format!("processed {input}")
}

pub fn run() -> String {
    worker::call_process("demo")
}
"#,
    );
    write_fixture(
        &src.join("worker.rs"),
        r#"use crate::process_manas_internal;

pub fn call_process(value: &str) -> String {
    process_manas_internal(value)
}
"#,
    );
    write_fixture(
        &src.join("extra.rs"),
        r#"pub fn second() -> String {
    crate::process_manas_internal("second")
}
"#,
    );
    root
}

fn write_fixture(path: &Path, content: &str) {
    std::fs::write(path, content)
        .unwrap_or_else(|err| panic!("failed to write {}: {err}", path.display()));
}

fn write_llm_config(workspace: &Path, addr: SocketAddr) -> PathBuf {
    let mut routes = serde_json::Map::new();
    for route in ROUTES {
        routes.insert(
            (*route).to_string(),
            json!({
                "default_temperature": 0.0,
                "providers": [{
                    "provider": "openai",
                    "model": "mock-coder",
                    "temperature": 0.0
                }]
            }),
        );
    }
    let config = json!({
        "provider_base_url": {
            "openai": format!("http://{}", addr)
        },
        "routes": routes
    });
    let path = workspace.join("provider_config.json");
    write_fixture(
        &path,
        &serde_json::to_string_pretty(&config).expect("config should serialize"),
    );
    path
}

fn write_codex_llm_config(workspace: &Path) -> PathBuf {
    let mut routes = serde_json::Map::new();
    for route in ROUTES {
        routes.insert(
            (*route).to_string(),
            json!({
                "default_temperature": 0.0,
                "providers": [{
                    "provider": "openai",
                    "model": "mock-codex-stream",
                    "temperature": 0.0
                }]
            }),
        );
    }
    let config = json!({
        "provider_base_url": {
            "openai": "https://api.openai.com/v1"
        },
        "routes": routes
    });
    let path = workspace.join("tura_codex_llm_config.json");
    write_fixture(
        &path,
        &serde_json::to_string_pretty(&config).expect("config should serialize"),
    );
    path
}

fn tool_results(log: &[String]) -> Vec<Value> {
    log.iter()
        .filter_map(|entry| serde_json::from_str::<Value>(entry).ok())
        .filter(|value| value.get("type").and_then(Value::as_str) == Some("tool_result"))
        .collect()
}

fn assert_tool_success(tool_results: &[Value], tool_name: &str) {
    let result = tool_results
        .iter()
        .find(|result| result.get("tool_name").and_then(Value::as_str) == Some(tool_name))
        .unwrap_or_else(|| panic!("missing tool result for {tool_name}; saw {tool_results:#?}"));
    assert_eq!(
        result.get("success").and_then(Value::as_bool),
        Some(true),
        "tool {tool_name} should succeed: {result}"
    );
}

struct EnvGuard {
    previous: Vec<(String, Option<String>)>,
}

impl EnvGuard {
    fn set(values: &[(&str, &str)]) -> Self {
        let previous = values
            .iter()
            .map(|(key, _)| ((*key).to_string(), std::env::var(key).ok()))
            .collect::<Vec<_>>();
        for (key, value) in values {
            std::env::set_var(key, value);
        }
        Self { previous }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        for (key, value) in &self.previous {
            if let Some(value) = value {
                std::env::set_var(key, value);
            } else {
                std::env::remove_var(key);
            }
        }
    }
}
