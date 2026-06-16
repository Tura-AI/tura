//! Deterministic end-to-end check for the `claude-code` compat layer.
//!
//! Unlike `claude_code_live_e2e` (which talks to the real Anthropic API and is
//! gated behind a subscription quota), this test stands up a local mock of the
//! native Anthropic Messages API and drives the full gateway session engine
//! through it. It verifies, without any network access or credentials:
//!
//! * the compat layer emits a well-formed native Messages payload — `system`
//!   string (with the Claude Code identity on the OAuth route), alternating
//!   `messages`, Anthropic-shaped `tools` carrying `input_schema`, and crucially
//!   **no** `temperature` (current Claude models reject it);
//! * an Anthropic `tool_use` block is normalized into the OpenAI-shaped
//!   `tool_calls` the runtime state machine consumes, the tool actually runs,
//!   and the session reaches `Completed`.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{Shutdown, SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread;

use runtime::mano;
use runtime::state_machine::session_management::{SessionInput, SessionState};
use serde_json::{json, Value};

#[path = "../support/session_db_support.rs"]
mod session_db_support;

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

#[test]
#[ignore = "Claude compatibility coverage is run explicitly; global business runners skip claude targets"]
fn claude_code_gateway_session_tool_calling_mock_e2e() {
    debug_e2e("test entered");
    let _session_db = session_db_support::SessionDbTestService::start(&ENV_LOCK);
    debug_e2e("session_db test service started");
    let workspace = create_rust_workspace();
    debug_e2e(&format!("workspace created at {}", workspace.display()));
    let provider = MockAnthropic::start();
    debug_e2e(&format!("mock anthropic listening at {}", provider.addr));
    let llm_config = write_llm_config(&workspace, provider.addr);
    debug_e2e(&format!("llm config written at {}", llm_config.display()));
    let _env = EnvGuard::set(&[
        (
            "TURA_PROVIDER_CONFIG",
            llm_config.to_string_lossy().as_ref(),
        ),
        // An `sk-ant-oat...` token forces the OAuth subscription route so the
        // request shape (Bearer + system identity) is the one we assert on.
        ("CLAUDE_CODE_OAUTH_TOKEN", "sk-ant-oat01-mock-token"),
        ("ANTHROPIC_LOGIN", "oauth"),
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
    ]);

    debug_e2e("starting runtime session");
    let result = mano::process_from_gateway_session_in_directory(
        "claude-code-mock-e2e".to_string(),
        SessionInput {
            user_input: "Use command_run to run pwd, then finish with a normal assistant message."
                .to_string(),
            file_input: vec![],
            agent: None,
            runtime_context: None,
            planning_mode_override: None,
        },
        workspace,
    )
    .expect("claude-code mock gateway session should complete");
    debug_e2e("runtime session returned");

    assert_eq!(result.agents.len(), 1);
    assert_eq!(result.agents[0].agent_name, "fast");
    assert_eq!(
        result.session.state,
        SessionState::Completed,
        "session should reach Completed; log={:#?}",
        result.session.session_log
    );

    // Tool calling round-tripped through the state machine and executed.
    let tool_results = tool_results(&result.session.session_log);
    assert!(
        tool_results
            .iter()
            .any(|result| result.get("tool_name").and_then(Value::as_str) == Some("command_run")),
        "missing tool result for command_run; tool_results={tool_results:#?}; session_log={:#?}; requests={:#?}",
        result.session.session_log,
        provider.requests.lock().expect("requests lock")
    );
    assert_tool_success(&tool_results, "command_run");

    // The final assistant turn completed normally.
    assert!(
        result
            .session
            .session_log
            .iter()
            .filter_map(|entry| serde_json::from_str::<Value>(entry).ok())
            .any(|entry| entry.get("role").and_then(Value::as_str) == Some("assistant")),
        "expected a final assistant message; log={:#?}",
        result.session.session_log
    );

    // The compat layer must have produced a well-formed native Messages payload.
    let requests = provider.requests.lock().expect("requests lock");
    assert!(!requests.is_empty(), "mock received no requests");
    let first = &requests[0];

    // No `temperature`: current Claude models reject it as deprecated. This is
    // the regression the live run surfaced.
    assert!(
        first.get("temperature").is_none(),
        "native payload must not forward temperature; got {first}"
    );
    // OAuth route prepends the Claude Code identity as typed system blocks. The
    // first block carries that identity plus a prompt-cache breakpoint
    // (`cache_control: ephemeral`), mirroring the real claude-code CLI.
    let system_blocks = first
        .get("system")
        .and_then(Value::as_array)
        .expect("payload should carry typed system blocks");
    let first_block = system_blocks
        .first()
        .expect("system blocks must not be empty");
    let identity = first_block
        .get("text")
        .and_then(Value::as_str)
        .expect("first system block must carry text");
    assert!(
        identity.starts_with("You are Claude Code, Anthropic's official CLI for Claude."),
        "OAuth route must prepend the Claude Code system identity; got {identity:?}"
    );
    assert_eq!(
        first_block
            .get("cache_control")
            .and_then(|cache| cache.get("type"))
            .and_then(Value::as_str),
        Some("ephemeral"),
        "first system block must carry the prompt-cache breakpoint; got {first_block}"
    );
    // Tools are converted to the Anthropic shape with `input_schema`.
    let tools = first
        .get("tools")
        .and_then(Value::as_array)
        .expect("payload should carry tools");
    let command_run = tools
        .iter()
        .find(|tool| tool.get("name").and_then(Value::as_str) == Some("command_run"))
        .expect("command_run tool should be advertised in native shape");
    assert!(
        command_run.get("input_schema").is_some(),
        "Anthropic tools must carry input_schema; got {command_run}"
    );
    assert!(
        first.get("max_tokens").and_then(Value::as_u64).is_some(),
        "native payload must include max_tokens; got {first}"
    );
    // The follow-up turn must echo the prior tool result back as a user
    // tool_result block (proving conversation translation round-trips).
    assert!(
        requests.len() >= 2,
        "expected a follow-up turn after the tool result"
    );
    let follow_up = &requests[1];
    let has_tool_result = follow_up
        .get("messages")
        .and_then(Value::as_array)
        .is_some_and(|messages| {
            messages.iter().any(|message| {
                message
                    .get("content")
                    .and_then(Value::as_array)
                    .is_some_and(|blocks| {
                        blocks.iter().any(|block| {
                            block.get("type").and_then(Value::as_str) == Some("tool_result")
                        })
                    })
            })
        });
    assert!(
        has_tool_result,
        "follow-up turn should contain a tool_result block; got {follow_up}"
    );
}

struct MockAnthropic {
    addr: SocketAddr,
    requests: Arc<Mutex<Vec<Value>>>,
}

impl MockAnthropic {
    fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("mock should bind");
        let addr = listener.local_addr().expect("mock address");
        let requests = Arc::new(Mutex::new(Vec::new()));
        let thread_requests = Arc::clone(&requests);
        thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                handle_connection(stream, &thread_requests);
            }
        });
        Self { addr, requests }
    }
}

fn handle_connection(stream: TcpStream, requests: &Arc<Mutex<Vec<Value>>>) {
    debug_e2e("mock received connection");
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

    requests.lock().expect("requests lock").push(request);
    debug_e2e(&format!(
        "mock recorded request {}",
        requests.lock().expect("requests lock").len()
    ));

    let response_text = anthropic_stream_response(
        requests
            .lock()
            .expect("requests lock")
            .last()
            .expect("request should be recorded"),
    );
    let stream = reader.get_mut();
    let _ = write!(
        stream,
        "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        response_text.len(),
        response_text
    );
    let _ = stream.flush();
    let _ = stream.shutdown(Shutdown::Both);
    debug_e2e("mock response flushed");
}

fn debug_e2e(message: &str) {
    if std::env::var_os("TURA_DEBUG_RUNTIME").is_some() {
        eprintln!(
            "claude_code_mock_e2e [{}]: {message}",
            chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
        );
    }
}

/// Native Anthropic Messages SSE: issue `tool_use` until a tool_result is
/// present, then produce final text.
fn anthropic_stream_response(request: &Value) -> String {
    if request_has_tool_result(request) {
        sse_lines([
            json!({"type":"message_start","message":{"id":"msg_final","type":"message","role":"assistant","model":"claude-opus-4-8","usage":{"input_tokens":30}}}),
            json!({"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}}),
            json!({"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"done."}}),
            json!({"type":"content_block_stop","index":0}),
            json!({"type":"message_delta","delta":{"stop_reason":"end_turn"},"usage":{"output_tokens":8}}),
            json!({"type":"message_stop"}),
        ])
    } else {
        let input = json!({
            "commands": [{
                "step": 1,
                "command_type": "shell_command",
                "command_line": json!({"command": "pwd", "timeout_ms": MOCK_COMMAND_TIMEOUT_MS}).to_string()
            }],
            "step_summary": "Run pwd via command_run as requested."
        });
        sse_lines([
            json!({"type":"message_start","message":{"id":"msg_tool_use","type":"message","role":"assistant","model":"claude-opus-4-8","usage":{"input_tokens":24}}}),
            json!({"type":"content_block_start","index":0,"content_block":{"type":"tool_use","id":"toolu_pwd","name":"command_run","input":{}}}),
            json!({"type":"content_block_delta","index":0,"delta":{"type":"input_json_delta","partial_json":input.to_string()}}),
            json!({"type":"content_block_stop","index":0}),
            json!({"type":"message_delta","delta":{"stop_reason":"tool_use"},"usage":{"output_tokens":12}}),
            json!({"type":"message_stop"}),
        ])
    }
}

fn sse_lines<const N: usize>(events: [Value; N]) -> String {
    let mut output = String::new();
    for event in events {
        output.push_str("data: ");
        output.push_str(&serde_json::to_string(&event).expect("event should serialize"));
        output.push_str("\n\n");
    }
    output
}

fn request_has_tool_result(request: &Value) -> bool {
    request
        .get("messages")
        .and_then(Value::as_array)
        .is_some_and(|messages| {
            messages.iter().any(|message| {
                message
                    .get("content")
                    .and_then(Value::as_array)
                    .is_some_and(|content| {
                        content.iter().any(|item| {
                            item.get("type").and_then(Value::as_str) == Some("tool_result")
                        })
                    })
            })
        })
}

fn write_llm_config(workspace: &Path, addr: SocketAddr) -> PathBuf {
    let mut routes = serde_json::Map::new();
    for route in ROUTES {
        routes.insert(
            (*route).to_string(),
            json!({
                "default_temperature": 0.0,
                "providers": [{
                    "provider": "claude-code",
                    "model": "claude-opus-4-8",
                    "temperature": 0.0
                }]
            }),
        );
    }
    let config = json!({
        "provider_base_url": {
            "claude-code": format!("http://{}", addr),
            "anthropic": format!("http://{}", addr)
        },
        "routes": routes
    });
    let path = workspace.join("provider_config.json");
    std::fs::write(
        &path,
        serde_json::to_string_pretty(&config).expect("config should serialize"),
    )
    .expect("provider_config.json should be written");
    path
}

fn create_rust_workspace() -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "tura-claude-code-mock-e2e-{}",
        chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default()
    ));
    let src = root.join("src");
    std::fs::create_dir_all(&src).expect("test workspace src should be created");
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"tura-claude-code-mock-e2e\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .expect("Cargo.toml should be written");
    std::fs::write(
        src.join("lib.rs"),
        "pub fn run() -> String {\n    \"demo\".to_string()\n}\n",
    )
    .expect("lib.rs should be written");
    root
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
