use chrono::Utc;
use runtime::runtime::call_runtime::{call_runtime, CallRuntimeInput};
use runtime::state_machine::agent_management::{ProviderConfig, ToolChoice};
use runtime::state_machine::runtime_management::{
    RuntimeCallResultStatus, RuntimeManagement, RuntimeProviderConfig, RuntimeState,
};
use serde_json::{json, Value};
use std::collections::{BTreeSet, HashMap};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader as TokioBufReader};
use tokio::net::TcpListener as TokioTcpListener;
use tura_llm_rust::{
    ModelCatalog, ProviderConfig as LlmProviderConfig, ProviderEnumCatalog, RouteConfig, Settings,
    TuraConfig,
};

static MOCK_ROUTER_ADDR: OnceLock<String> = OnceLock::new();
static MOCK_ROUTER_INIT: Mutex<()> = Mutex::new(());

#[tokio::test]
async fn runtime_provider_timeout_business_flow_marks_runtime_timed_out_without_success_output() {
    let provider = DelayedProvider::start(Duration::from_millis(2_500));
    let _api_key = EnvGuard::set("LOCALTIMEOUT_API_KEY", "local-timeout-key");
    let settings = Arc::new(Settings {
        provider_base_url: HashMap::new(),
        routes: HashMap::from([(
            "timeout-route".to_string(),
            RouteConfig {
                default_temperature: 0.0,
                providers: vec![LlmProviderConfig {
                    provider: "localtimeout".to_string(),
                    base_url: provider.endpoint.clone(),
                    model: "local-timeout-model".to_string(),
                    temperature: 0.0,
                }],
            },
        )]),
        model_catalog: ModelCatalog::default(),
        provider_enums: ProviderEnumCatalog::default(),
    });
    let runtime = runtime_with_timeout("runtime-timeout-business", 1_000);

    let started = std::time::Instant::now();
    let result = call_runtime(
        CallRuntimeInput {
            runtime,
            messages: vec![json!({ "role": "user", "content": "trigger local provider timeout" })],
            tools: Vec::new(),
            provider_name: "timeout-route".to_string(),
            stream: false,
            max_tokens: 128,
            tool_choice: None,
            session_directory: std::env::temp_dir(),
            allowed_command_run_commands: Some(BTreeSet::new()),
        },
        settings,
        Arc::new(TuraConfig::new(".env.runtime-timeout-business-missing")),
    )
    .await
    .expect("timeout should be captured on the runtime");

    assert!(
        started.elapsed() < Duration::from_secs(2),
        "runtime timeout should bound a hanging provider call"
    );
    assert_eq!(result.state, RuntimeState::Failed);
    assert_eq!(result.call_result_status, RuntimeCallResultStatus::TimedOut);
    assert!(result.called_at.is_some());
    assert!(result.call_finished_at.is_some());
    assert!(result.first_token_at.is_none());
    assert_eq!(result.text, "");
    assert!(result.tool_call.is_empty());
    let output = result.output.as_ref().expect("timeout output");
    let output_error = output["error"].as_str().expect("timeout output text");
    assert!(
        output_error.contains("runtime call timed out after 1000 ms"),
        "unexpected timeout output: {output}"
    );
    let runtime_error = result.error.as_ref().expect("runtime error");
    assert_eq!(runtime_error.error_code.as_deref(), Some("CALL_TIMED_OUT"));
    assert!(runtime_error.retry_allowed);
    assert!(runtime_error.fallback_allowed);
    assert!(runtime_error
        .error_text
        .as_deref()
        .is_some_and(|text| text.contains("runtime call timed out after 1000 ms")));
    let usage = result.usage.as_ref().expect("estimated timeout usage");
    assert_eq!(usage.pricing_source, "runtime_estimate_timeout");
    assert!(usage.input_tokens > 0);
    assert!(usage.output_tokens > 0);
    assert_eq!(
        usage.total_tokens,
        usage.input_tokens + usage.output_tokens + usage.reasoning_tokens
    );
    assert!(usage.latency_ms >= 1_000);

    let request = provider.join();
    assert!(request.starts_with("POST /chat/completions "));
    assert!(request
        .to_ascii_lowercase()
        .contains("authorization: bearer local-timeout-key"));
}

#[tokio::test]
async fn streamed_command_run_post_result_timeout_advances_without_final_provider_event() {
    let workspace = tempfile::tempdir().expect("workspace");
    let output_path = workspace.path().join("post-result-timeout.txt");
    let provider = StalledResponsesProvider::start(stalled_command(&output_path));
    let router_addr = mock_command_run_router_addr();
    let _api_key = EnvGuard::set("OPENAI_API_KEY", "local-stream-key");
    let _post_result_timeout =
        EnvGuard::set("TURA_STREAMED_COMMAND_RUN_POST_RESULT_TIMEOUT_MS", "150");
    let _router_addr = EnvGuard::set("TURA_ROUTER_ADDR", router_addr.as_str());
    let settings = Arc::new(Settings {
        provider_base_url: HashMap::new(),
        routes: HashMap::from([(
            "stream-timeout-route".to_string(),
            RouteConfig {
                default_temperature: 0.0,
                providers: vec![LlmProviderConfig {
                    provider: "openai".to_string(),
                    base_url: provider.endpoint.clone(),
                    model: "local-stream-timeout-model".to_string(),
                    temperature: 0.0,
                }],
            },
        )]),
        model_catalog: ModelCatalog::default(),
        provider_enums: ProviderEnumCatalog::default(),
    });
    let runtime = runtime_for_provider(
        "runtime-post-command-timeout-business",
        30_000,
        true,
        "stream-timeout-route",
        "openai",
        "local-stream-timeout-model",
    );

    let started = std::time::Instant::now();
    let result = call_runtime(
        CallRuntimeInput {
            runtime,
            messages: vec![json!({
                "role": "user",
                "content": "run one streamed command then wait for provider finalization"
            })],
            tools: Vec::new(),
            provider_name: "stream-timeout-route".to_string(),
            stream: true,
            max_tokens: 128,
            tool_choice: None,
            session_directory: workspace.path().to_path_buf(),
            allowed_command_run_commands: Some(BTreeSet::from(["shell_command".to_string()])),
        },
        settings,
        Arc::new(TuraConfig::new(
            ".env.runtime-stream-timeout-business-missing",
        )),
    )
    .await
    .expect("post command_run stream timeout should finish with command results");

    let elapsed = started.elapsed();
    assert!(
        elapsed < Duration::from_secs(12),
        "post-result timeout should finish without waiting for the provider total timeout; elapsed={elapsed:?}, state={:?}, status={:?}, output={:?}, file_exists={}",
        result.state,
        result.call_result_status,
        result.output,
        output_path.exists()
    );
    assert_eq!(result.state, RuntimeState::Finished);
    assert_eq!(
        result.call_result_status,
        RuntimeCallResultStatus::Succeeded
    );
    assert!(result.called_at.is_some());
    assert!(result.call_finished_at.is_some());
    assert!(result.first_token_at.is_some());
    assert_eq!(
        std::fs::read_to_string(&output_path).expect("command output file"),
        "stream command completed"
    );

    let output = result.output.as_ref().expect("runtime output");
    assert_eq!(
        output.pointer("/streamed_command_run_result/early_finish_reason"),
        Some(&json!("post_command_run_stream_timeout"))
    );
    assert_eq!(
        output.pointer("/streamed_command_run_result/results/0/success"),
        Some(&json!(true))
    );
    assert!(output
        .pointer("/provider_content/text")
        .and_then(|value| value.as_str())
        .is_some_and(|text| text.contains("Provider stream did not finish")));
    assert_eq!(result.tool_call.len(), 1);
    assert_eq!(result.tool_call[0].tool_called_name, "command_run");

    let request = provider.request();
    assert!(request.starts_with("POST /responses "));
    assert!(request
        .to_ascii_lowercase()
        .contains("authorization: bearer local-stream-key"));
}

fn mock_command_run_router_addr() -> String {
    if let Some(addr) = MOCK_ROUTER_ADDR.get() {
        return addr.clone();
    }
    let _guard = MOCK_ROUTER_INIT
        .lock()
        .expect("mock command_run router init lock");
    if let Some(addr) = MOCK_ROUTER_ADDR.get() {
        return addr.clone();
    }

    let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock command_run router");
    listener
        .set_nonblocking(true)
        .expect("mock command_run router nonblocking");
    let addr = listener
        .local_addr()
        .expect("mock command_run router addr")
        .to_string();
    thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("mock command_run router runtime");
        runtime.block_on(async move {
            let listener = TokioTcpListener::from_std(listener).expect("tokio listener");
            while let Ok((stream, _)) = listener.accept().await {
                tokio::spawn(async move {
                    let (read, mut write) = stream.into_split();
                    let mut reader = TokioBufReader::new(read);
                    let mut line = String::new();
                    if reader.read_line(&mut line).await.unwrap_or(0) == 0 {
                        return;
                    }
                    let response = mock_command_run_router_response(&line).await;
                    let _ = write.write_all(format!("{response}\n").as_bytes()).await;
                    let _ = write.flush().await;
                });
            }
        });
    });
    MOCK_ROUTER_ADDR
        .set(addr.clone())
        .expect("mock command_run router addr set once");
    addr
}

async fn mock_command_run_router_response(raw: &str) -> Value {
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
        PathBuf::from(session_directory),
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

fn runtime_with_timeout(runtime_id: &str, timeout_ms: u64) -> RuntimeManagement {
    runtime_for_provider(
        runtime_id,
        timeout_ms,
        false,
        "timeout-route",
        "localtimeout",
        "local-timeout-model",
    )
}

fn runtime_for_provider(
    runtime_id: &str,
    timeout_ms: u64,
    stream: bool,
    route: &str,
    provider_url_name: &str,
    model: &str,
) -> RuntimeManagement {
    RuntimeManagement::new(
        runtime_id.to_string(),
        "session-timeout-business".to_string(),
        "agent-timeout-business".to_string(),
        RuntimeProviderConfig {
            base: ProviderConfig {
                tura_llm_name: route.to_string(),
                stream,
                temperature: 0.0,
                max_tokens: 128,
                tool_choice: ToolChoice::Auto,
                time_out_ms: timeout_ms,
            },
            thinking: false,
            provider_name: route.to_string(),
            model_name: model.to_string(),
            provider_url_name: provider_url_name.to_string(),
            llm_provider_name: provider_url_name.to_string(),
        },
        Utc::now(),
    )
}

struct DelayedProvider {
    endpoint: String,
    join: thread::JoinHandle<String>,
}

impl DelayedProvider {
    fn start(delay: Duration) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind delayed provider");
        let addr = listener.local_addr().expect("delayed provider addr");
        let join = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept delayed provider request");
            let request = read_request_head(&mut stream);
            thread::sleep(delay);
            let body = json!({
                "choices": [{
                    "message": {
                        "role": "assistant",
                        "content": "late provider response must not win"
                    },
                    "finish_reason": "stop"
                }],
                "usage": {
                    "prompt_tokens": 1,
                    "completion_tokens": 1,
                    "total_tokens": 2
                }
            })
            .to_string();
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                body.len(),
                body
            );
            let _ = stream.write_all(response.as_bytes());
            request
        });
        Self {
            endpoint: format!("http://{addr}"),
            join,
        }
    }

    fn join(self) -> String {
        self.join.join().expect("delayed provider joins")
    }
}

struct StalledResponsesProvider {
    endpoint: String,
    request: Arc<Mutex<Option<String>>>,
}

impl StalledResponsesProvider {
    fn start(command: serde_json::Value) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind stalled provider");
        let addr = listener.local_addr().expect("stalled provider addr");
        let request = Arc::new(Mutex::new(None));
        let thread_request = Arc::clone(&request);
        thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept stalled provider request");
            let request = read_request_head(&mut stream);
            *thread_request
                .lock()
                .expect("stalled provider request lock") = Some(request);
            write_stalled_command_run_stream(&mut stream, command);
            thread::sleep(Duration::from_secs(12));
        });
        Self {
            endpoint: format!("http://{addr}"),
            request,
        }
    }

    fn request(&self) -> String {
        self.request
            .lock()
            .expect("stalled provider request lock")
            .clone()
            .expect("stalled provider request captured")
    }
}

fn stalled_command(output_path: &std::path::Path) -> serde_json::Value {
    json!({
        "step": 1,
        "command": "shell_command",
        "command_type": "shell_command",
        "command_line": json!({
            "command": format!(
                "python -c \"from pathlib import Path; Path(r'{}').write_text('stream command completed')\"",
                output_path.display()
            ),
            "timeout_ms": 3_000
        }).to_string()
    })
}

fn write_stalled_command_run_stream(stream: &mut TcpStream, command: serde_json::Value) {
    let _ = write!(
        stream,
        "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nTransfer-Encoding: chunked\r\nConnection: keep-alive\r\n\r\n"
    );
    write_sse_chunk(
        stream,
        json!({
            "type": "response.output_item.added",
            "item": {
                "id": "fc_stalled_cmd",
                "type": "function_call",
                "call_id": "call_stalled_cmd",
                "name": "command_run",
                "arguments": ""
            }
        }),
    );
    let arguments = json!({
        "commands": [command],
        "step_summary": "Run a single local command before the provider stream stalls."
    })
    .to_string();
    write_sse_chunk(
        stream,
        json!({
            "type": "response.function_call_arguments.delta",
            "item_id": "fc_stalled_cmd",
            "delta": arguments
        }),
    );
    write_sse_chunk(
        stream,
        json!({
            "type": "response.function_call_arguments.done",
            "item_id": "fc_stalled_cmd",
            "arguments": arguments
        }),
    );
}

fn write_sse_chunk(stream: &mut TcpStream, value: serde_json::Value) {
    let data = format!("data: {value}\n\n");
    let _ = write!(stream, "{:X}\r\n{}\r\n", data.len(), data);
    let _ = stream.flush();
}

fn read_request_head(stream: &mut std::net::TcpStream) -> String {
    let mut buffer = Vec::new();
    let mut chunk = [0_u8; 512];
    loop {
        let read = stream.read(&mut chunk).expect("read request");
        assert!(read > 0, "client closed before request headers");
        buffer.extend_from_slice(&chunk[..read]);
        if buffer.windows(4).any(|window| window == b"\r\n\r\n") {
            break;
        }
    }
    String::from_utf8_lossy(&buffer).to_string()
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
        match self.previous.take() {
            Some(value) => std::env::set_var(self.key, value),
            None => std::env::remove_var(self.key),
        }
    }
}
