use std::io::{BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

use code_tools_suite::mano;
use code_tools_suite::state_machine::session_management::{SessionInput, SessionState};
use serde_json::{json, Value};

const ROUTES: &[&str] = &[
    "tura_general",
    "tura_office",
    "tura_creative",
    "tura_translator",
    "tura_validator",
    "tura_validator_advanced",
    "tura_classifier",
    "tura_embedding",
    "tura_coder",
    "tura_coder_advanced",
    "tura_planner",
    "tura_planner_advanced",
    "tura_roleplay",
    "tura_professional",
    "tura_math",
    "tura_academic",
];

static ENV_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn coding_agent_can_call_command_run_tool_e2e() {
    let _lock = ENV_LOCK.lock().expect("e2e env lock should be available");
    let workspace = create_rust_workspace();
    let provider = MockProvider::start_command_run();
    let llm_config = write_llm_config(&workspace, provider.addr);
    let _env = EnvGuard::set(&[
        ("TURALLM_CONFIG", llm_config.to_string_lossy().as_ref()),
        ("OPENAI_API_KEY", "test-key"),
        ("TURA_DISABLE_GATEWAY_CALLBACKS", "1"),
        ("TURA_MANAS_MAX_TURNS", "4"),
    ]);

    let result = mano::process_from_gateway_session_in_directory(
        "e2e-run-command-tool".to_string(),
        SessionInput {
            user_input: "Run pwd with command_run, then patch src/lib.rs with command_run apply_patch, verify it with shell_command, and finish with normal assistant text."
                .to_string(),
            file_input: vec![],
            agent: None,
            runtime_context: None,
        },
        workspace.clone(),
    )
    .expect("coding agent should complete the command_run e2e flow");

    assert_eq!(result.agents.len(), 1);
    assert_eq!(result.agents[0].agent_name, "coding_agent");
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
        .any(
            |entry| entry.get("role").and_then(Value::as_str) == Some("assistant")
                && entry.get("content").and_then(Value::as_str).is_some_and(
                    |text| text.contains("command_run shell and apply_patch e2e completed")
                )
        ));

    let run_output = tool_results
        .iter()
        .find(|result| result.get("tool_name").and_then(Value::as_str) == Some("command_run"))
        .and_then(|result| result.get("output"))
        .cloned()
        .unwrap_or(Value::Null);
    assert_eq!(
        run_output
            .pointer("/results/0/results/0/response/exit_code")
            .or_else(|| run_output.pointer("/results/0/exit_code"))
            .or_else(|| run_output.pointer("/results/0/output/metadata/exit_code"))
            .and_then(Value::as_i64),
        Some(0)
    );

    let patched_content = std::fs::read_to_string(workspace.join("src/lib.rs"))
        .expect("patched file should be readable");
    assert!(
        patched_content.contains("patched via command_run"),
        "patched file did not contain expected text; content={patched_content}; tool_results={tool_results:#?}"
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
        .filter_map(|tool| tool.pointer("/function/name").and_then(Value::as_str))
        .collect::<Vec<_>>();
    assert!(first_tool_names.contains(&"command_run"));
}

struct MockProvider {
    addr: SocketAddr,
    requests: Arc<Mutex<Vec<Value>>>,
}

#[derive(Debug, Clone, Copy)]
enum MockMode {
    CommandRun,
}

impl MockProvider {
    fn start_command_run() -> Self {
        Self::start_with_mode(MockMode::CommandRun)
    }

    fn start_with_mode(mode: MockMode) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("mock provider should bind");
        let addr = listener.local_addr().expect("mock provider address");
        let requests = Arc::new(Mutex::new(Vec::new()));
        let counter = Arc::new(AtomicUsize::new(0));
        let thread_requests = Arc::clone(&requests);
        let thread_counter = Arc::clone(&counter);

        thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                handle_provider_connection(stream, &thread_counter, &thread_requests, mode);
            }
        });

        Self { addr, requests }
    }
}

fn handle_provider_connection(
    stream: TcpStream,
    counter: &AtomicUsize,
    requests: &Arc<Mutex<Vec<Value>>>,
    mode: MockMode,
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
    let is_stream = request.get("stream").and_then(Value::as_bool) == Some(true);
    requests
        .lock()
        .expect("mock provider requests lock")
        .push(request);
    let stream = reader.get_mut();
    if is_stream {
        let response_text = response_to_sse(&response);
        let _ = write!(
            stream,
            "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            response_text.len(),
            response_text
        );
    } else {
        let response_text =
            serde_json::to_string(&response).expect("mock provider response should serialize");
        let _ = write!(
            stream,
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            response_text.len(),
            response_text
        );
    }
    let _ = stream.flush();
}

fn provider_response(index: usize, mode: MockMode) -> Value {
    match mode {
        MockMode::CommandRun => command_run_provider_response(index),
    }
}

fn command_run_provider_response(index: usize) -> Value {
    match index {
        0 => tool_response(
            "call_command_run",
            "command_run",
            json!({
                "commands": [
                    { "command": "shell_command", "command_line": json!({"command":"pwd","timeout_ms":20000}).to_string(), "step": 1 },
                    { "command": "shell_command", "command_line": json!({"command":"Write-Output 2","timeout_ms":20000}).to_string(), "step": 1 },
                    { "command": "shell_command", "command_line": json!({"command":"Write-Output 3","timeout_ms":20000}).to_string(), "step": 1 },
                    { "command": "shell_command", "command_line": json!({"command":"Write-Output 4","timeout_ms":20000}).to_string(), "step": 1 },
                    { "command": "shell_command", "command_line": json!({"command":"Write-Output 5","timeout_ms":20000}).to_string(), "step": 1 }
                ],
                "step_summary": "Call the command_run console tool as requested.",
                "previous_command_evaluations": []
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
                        "command_line": "*** Begin Patch\n*** Update File: src/lib.rs\n@@\n-pub fn process_manas_internal(input: &str) -> String {\n-    format!(\"processed {input}\")\n+pub fn process_manas_internal(input: &str) -> String {\n+    format!(\"patched via command_run {input}\")\n }\n*** End Patch"
                    },
                    {
                        "step": 2,
                        "command": "shell_command",
                        "command_line": json!({"command":"Get-Content src/lib.rs","timeout_ms":20000}).to_string()
                    }
                ],
                "step_summary": "Patch src/lib.rs and verify the edited content.",
                "previous_command_evaluations": [
                    { "command": "shell_command", "evaluation": "helpful_and_unreusable", "step": 1 },
                    { "command": "shell_command", "evaluation": "helpful_and_unreusable", "step": 1 },
                    { "command": "shell_command", "evaluation": "helpful_and_unreusable", "step": 1 },
                    { "command": "shell_command", "evaluation": "helpful_and_unreusable", "step": 1 },
                    { "command": "shell_command", "evaluation": "helpful_and_unreusable", "step": 1 }
                ]
            }),
        ),
        _ => assistant_response("command_run shell and apply_patch e2e completed."),
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

fn response_to_sse(response: &Value) -> String {
    let message = response
        .pointer("/choices/0/message")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let mut lines = Vec::new();
    if let Some(content) = message.get("content").and_then(Value::as_str) {
        lines.push(format!(
            "data: {}\n\n",
            json!({"choices":[{"index":0,"delta":{"content":content},"finish_reason":null}]}),
        ));
        lines.push(format!(
            "data: {}\n\n",
            json!({"choices":[{"index":0,"delta":{},"finish_reason":"stop"}],"usage":{"prompt_tokens":1,"completion_tokens":1,"total_tokens":2}}),
        ));
    }
    if let Some(tool_calls) = message.get("tool_calls").and_then(Value::as_array) {
        for (index, call) in tool_calls.iter().enumerate() {
            let id = call
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or("call_mock");
            let name = call
                .pointer("/function/name")
                .and_then(Value::as_str)
                .unwrap_or("");
            let arguments = call
                .pointer("/function/arguments")
                .and_then(Value::as_str)
                .unwrap_or("{}");
            lines.push(format!(
                "data: {}\n\n",
                json!({
                    "choices": [{
                        "index": 0,
                        "delta": {
                            "tool_calls": [{
                                "index": index,
                                "id": id,
                                "type": "function",
                                "function": {
                                    "name": name,
                                    "arguments": arguments
                                }
                            }]
                        },
                        "finish_reason": null
                    }]
                }),
            ));
        }
        lines.push(format!(
            "data: {}\n\n",
            json!({"choices":[{"index":0,"delta":{},"finish_reason":"tool_calls"}],"usage":{"prompt_tokens":1,"completion_tokens":1,"total_tokens":2}}),
        ));
    }
    lines.push("data: [DONE]\n\n".to_string());
    lines.concat()
}

fn create_rust_workspace() -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "tura-agent-lsp-e2e-{}",
        chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default()
    ));
    let src = root.join("src");
    std::fs::create_dir_all(&src).expect("test workspace src should be created");
    write_file(
        &root.join("Cargo.toml"),
        r#"[package]
name = "tura-agent-lsp-e2e"
version = "0.1.0"
edition = "2021"
"#,
    );
    write_file(
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
    write_file(
        &src.join("worker.rs"),
        r#"use crate::process_manas_internal;

pub fn call_process(value: &str) -> String {
    process_manas_internal(value)
}
"#,
    );
    write_file(
        &src.join("extra.rs"),
        r#"pub fn second() -> String {
    crate::process_manas_internal("second")
}
"#,
    );
    root
}

fn write_file(path: &Path, content: &str) {
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
    let path = workspace.join("tura_llm_config.json");
    write_file(
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
