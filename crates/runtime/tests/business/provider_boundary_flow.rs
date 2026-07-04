use chrono::Utc;
use runtime::runtime::call_runtime::{call_runtime, CallRuntimeInput};
use runtime::state_machine::agent_management::{ProviderConfig, ToolChoice};
use runtime::state_machine::runtime_management::{
    RuntimeCallResultStatus, RuntimeManagement, RuntimeProviderConfig, RuntimeState,
};
use serde_json::{json, Value};
use std::collections::{BTreeSet, HashMap};
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader as TokioBufReader};
use tokio::net::TcpListener as TokioTcpListener;
use tura_llm_rust::{
    ModelCatalog, ProviderConfig as LlmProviderConfig, ProviderEnumCatalog, RouteConfig, Settings,
    TuraConfig,
};

#[path = "../support/session_db_support.rs"]
mod session_db_support;

static ENV_LOCK: Mutex<()> = Mutex::new(());
static ASYNC_ENV_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
static MOCK_ROUTER_ADDR: OnceLock<String> = OnceLock::new();
static MOCK_ROUTER_INIT: Mutex<()> = Mutex::new(());

#[tokio::test]
async fn runtime_google_business_flow_extracts_text_tool_metadata_usage_and_request_options() {
    let _guard = ASYNC_ENV_LOCK.lock().await;
    let provider = LocalProvider::start(vec![ProviderReply::Json {
        status: "200 OK",
        request_id: Some("req-runtime-google"),
        body: json!({
            "candidates": [{
                "content": {
                    "role": "model",
                    "parts": [
                        {"text": "Runtime sees Google text."},
                        {
                            "thoughtSignature": "runtime-google-sig",
                            "functionCall": {
                                "name": "command_run",
                                "args": {
                                    "commands": [{
                                        "step": 1,
                                        "command_type": "shell_command",
                                        "command_line": "Get-Location"
                                    }]
                                }
                            }
                        }
                    ]
                },
                "finishReason": "STOP"
            }],
            "usageMetadata": {
                "promptTokenCount": 40,
                "candidatesTokenCount": 8,
                "totalTokenCount": 48,
                "cachedContentTokenCount": 6
            }
        }),
    }]);
    let _env = EnvGuard::set(&[
        ("GOOGLE_API_KEY", "runtime-google-key"),
        ("TURA_SESSION_MAX_TOKENS", "77"),
        ("TURA_SESSION_ACCELERATION_ENABLED", "true"),
        ("TURA_SESSION_REASONING_EFFORT", "high"),
    ]);
    let settings = settings_for_route(
        "google-runtime-route",
        "google",
        &provider.endpoint,
        "models/gemini-runtime",
    );
    let runtime = runtime_for_provider(
        "runtime-google-business",
        "session-google-business",
        "google-runtime-route",
        "google",
        "models/gemini-runtime",
        false,
    );

    let result = call_runtime(
        CallRuntimeInput {
            runtime,
            messages: vec![
                json!({"role": "system", "content": "Keep system instruction separate."}),
                json!({"role": "user", "content": [
                    {"type": "input_text", "text": "Inspect the workspace"},
                    {"type": "input_image", "image_url": "data:image/png;base64,QUJD"}
                ]}),
                json!({"role": "debug", "content": "debug context should become user text"}),
            ],
            tools: Vec::new(),
            provider_name: "google-runtime-route".to_string(),
            stream: false,
            max_tokens: 128,
            tool_choice: None,
            session_directory: std::env::temp_dir(),
            allowed_command_run_commands: None,
            require_startup_task_state: false,
        },
        Arc::new(settings),
        Arc::new(TuraConfig::new(".env.runtime-google-business-missing")),
    )
    .await
    .expect("runtime google call should succeed");

    let requests = provider.requests();
    assert_eq!(requests.len(), 1);
    let captured = &requests[0];
    assert_eq!(
        captured.path, "/models/gemini-runtime:generateContent",
        "runtime should use the Google provider family path"
    );
    assert_eq!(captured.query, "key=runtime-google-key");
    assert_eq!(
        captured.body["systemInstruction"]["parts"][0]["text"],
        "Keep system instruction separate."
    );
    assert_eq!(
        captured.body["contents"][0]["parts"][0]["text"],
        "Inspect the workspace"
    );
    assert_eq!(
        captured.body["contents"][0]["parts"][1]["inlineData"]["mimeType"],
        "image/png"
    );
    assert!(
        captured.body["contents"]
            .as_array()
            .expect("google contents")
            .iter()
            .any(|message| message["parts"][0]["text"]
                .as_str()
                .is_some_and(|text| text.contains("Runtime context (debug)"))),
        "unknown runtime roles should be preserved as user-visible context"
    );
    assert_eq!(captured.body["generationConfig"]["maxOutputTokens"], 77);
    assert_eq!(captured.body["generationConfig"]["temperature"], 0.0);

    assert_eq!(result.state, RuntimeState::Finished);
    assert_eq!(
        result.call_result_status,
        RuntimeCallResultStatus::Succeeded
    );
    assert_eq!(result.text, "Runtime sees Google text.");
    assert_eq!(result.tool_call.len(), 1);
    assert_eq!(result.tool_call[0].tool_called_name, "command_run");
    assert_eq!(
        result.tool_call[0].tool_called_input["commands"][0]["command_line"],
        "Get-Location"
    );
    assert_eq!(
        result.tool_call[0]
            .provider_metadata
            .as_ref()
            .and_then(|metadata| metadata.get("google_thought_signature"))
            .and_then(Value::as_str),
        Some("runtime-google-sig")
    );
    assert_eq!(
        result
            .output
            .as_ref()
            .expect("runtime output")
            .pointer("/parts/1/thoughtSignature"),
        Some(&json!("runtime-google-sig"))
    );
    let usage = result.usage.expect("google usage");
    assert_eq!(usage.input_tokens, 40);
    assert_eq!(usage.output_tokens, 8);
    assert_eq!(usage.total_tokens, 48);
    assert_eq!(usage.cached_input_tokens, 6);
    assert_eq!(usage.pricing_source, "provider");
    assert!(usage.latency_ms <= 30_000);
    assert!(result.called_at.is_some());
    assert!(result.first_token_at.is_some());
    assert!(result.call_finished_at.is_some());

    let runtime_input = result.input.expect("runtime input");
    assert_eq!(runtime_input["options"]["max_tokens"], 77);
    assert_eq!(runtime_input["options"]["reasoning_effort"], "high");
    assert_eq!(runtime_input["options"]["service_tier"], "priority");
    assert_eq!(runtime_input["options"]["stream"], false);
}

#[tokio::test]
async fn runtime_openai_business_flow_replays_final_command_run_once_and_records_output() {
    let _guard = ASYNC_ENV_LOCK.lock().await;
    let _session_db = session_db_support::SessionDbTestService::start(&ENV_LOCK);
    let workspace = tempfile::tempdir().expect("runtime workspace");
    let marker = workspace.path().join("runtime-final-command.txt");
    let command_line = create_marker_command("runtime-final-command.txt", "final command replayed");
    let router_addr = mock_command_run_router_addr();
    let provider = LocalProvider::start(vec![ProviderReply::Json {
        status: "200 OK",
        request_id: Some("req-runtime-openai-final"),
        body: json!({
            "id": "chatcmpl-runtime-final-command",
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": "I need one final command.",
                    "tool_calls": [{
                        "id": "call_runtime_final",
                        "type": "function",
                        "function": {
                            "name": "command_run",
                            "arguments": {
                                "commands": [{
                                    "step": 1,
                                    "command_type": "shell_command",
                                    "command_line": command_line
                                }]
                            }
                        },
                        "provider_metadata": {"id": "call_runtime_final"}
                    }]
                },
                "finish_reason": "tool_calls"
            }],
            "usage": {
                "prompt_tokens": 32,
                "completion_tokens": 6,
                "total_tokens": 38,
                "prompt_tokens_details": {"cached_tokens": 4}
            }
        }),
    }]);
    let _env = EnvGuard::set(&[
        ("LOCALRUNTIME_API_KEY", "runtime-openai-key"),
        ("TURA_GATEWAY_CALLBACKS", "0"),
        ("TURA_PARALLEL_TOOL_CALLS", "true"),
        ("TURA_DISABLE_PROMPT_CACHE", "0"),
        ("TURA_SESSION_MAX_TOKENS", "65"),
        ("TURA_ROUTER_ADDR", router_addr.as_str()),
    ]);
    let settings = settings_for_route(
        "openai-runtime-route",
        "localruntime",
        &provider.endpoint,
        "gpt-runtime-local",
    );
    let runtime = runtime_for_provider(
        "runtime-openai-final-command",
        "session-openai-final-command",
        "openai-runtime-route",
        "localruntime",
        "gpt-runtime-local",
        false,
    );

    let result = call_runtime(
        CallRuntimeInput {
            runtime,
            messages: vec![json!({
                "role": "user",
                "content": "Return a command_run tool call in the final response."
            })],
            tools: vec![command_run_tool_schema()],
            provider_name: "openai-runtime-route".to_string(),
            stream: false,
            max_tokens: 128,
            tool_choice: Some(json!("auto")),
            session_directory: workspace.path().to_path_buf(),
            allowed_command_run_commands: Some(BTreeSet::from(["shell_command".to_string()])),
            require_startup_task_state: false,
        },
        Arc::new(settings),
        Arc::new(TuraConfig::new(".env.runtime-openai-business-missing")),
    )
    .await
    .expect("runtime openai final command replay should succeed");

    assert!(
        marker.exists(),
        "marker file should be written; runtime state={:?}, status={:?}, output={:#?}, tool_call={:#?}",
        result.state,
        result.call_result_status,
        result.output,
        result.tool_call
    );
    assert_eq!(
        std::fs::read_to_string(&marker).expect("marker file should be readable"),
        "final command replayed"
    );
    let requests = provider.requests();
    assert_eq!(requests.len(), 1);
    let captured = &requests[0];
    assert_eq!(captured.path, "/chat/completions");
    assert!(captured
        .headers
        .contains("authorization: bearer runtime-openai-key"));
    assert_eq!(captured.body["stream"], false);
    assert_eq!(captured.body["parallel_tool_calls"], true);
    assert_eq!(captured.body["max_tokens"], 65);
    assert_eq!(captured.body["tools"][0]["function"]["name"], "command_run");
    assert_eq!(captured.body["tool_choice"], "auto");
    assert_eq!(
        captured.body["prompt_cache_key"],
        Value::Null,
        "local OpenAI-compatible routes should not get a prompt cache key"
    );

    assert_eq!(result.state, RuntimeState::Finished);
    assert_eq!(
        result.call_result_status,
        RuntimeCallResultStatus::Succeeded
    );
    assert_eq!(result.text, "I need one final command.");
    assert_eq!(result.tool_call.len(), 1);
    assert_eq!(result.tool_call[0].tool_called_name, "command_run");
    assert_eq!(
        result.tool_call[0].tool_called_input["commands"][0]["command_line"],
        command_line
    );
    let output = result.output.as_ref().expect("runtime output");
    assert_eq!(
        output.pointer("/provider_content/text"),
        Some(&json!("I need one final command."))
    );
    assert_eq!(
        output.pointer("/streamed_command_run_result/commands/0/command_line"),
        Some(&json!(command_line))
    );
    assert_eq!(
        output.pointer("/streamed_command_run_result/results/0/success"),
        Some(&json!(true))
    );
    assert_eq!(
        output.pointer("/streamed_command_run_result/results/0/output/exit_code"),
        Some(&json!(0))
    );
    assert_eq!(
        output.pointer("/streamed_command_run_result/results/0/output/stderr"),
        Some(&json!(""))
    );
    let usage = result.usage.expect("openai usage");
    assert_eq!(usage.input_tokens, 32);
    assert_eq!(usage.output_tokens, 6);
    assert_eq!(usage.total_tokens, 38);
    assert_eq!(usage.cached_input_tokens, 4);

    let runtime_input = result.input.expect("runtime input");
    assert_eq!(runtime_input["options"]["parallel_tool_calls"], true);
    assert_eq!(runtime_input["options"]["max_tokens"], 65);
    assert_eq!(runtime_input["options"]["tool_choice"], "auto");
    assert_eq!(
        runtime_input["options"]["prompt_cache_key"],
        Value::Null,
        "runtime diagnostics should agree with the outgoing provider payload"
    );
}

#[tokio::test]
async fn runtime_prompt_cache_key_reuses_root_session_for_forked_sessions() {
    let _guard = ASYNC_ENV_LOCK.lock().await;
    let _session_db = session_db_support::SessionDbTestService::start(&ENV_LOCK);
    let workspace = tempfile::tempdir().expect("runtime cache workspace");
    let workspace_text = workspace.path().to_string_lossy().to_string();
    let root_session_id = "cache-root-session";
    let child_session_id = "cache-child-session";
    let client = runtime::session_log_client::SessionLogClient::discover()
        .expect("session db client should be available");
    client
        .upsert_session(
            session_snapshot_json(root_session_id, &workspace_text),
            None,
            Vec::new(),
            Vec::new(),
        )
        .expect("root session should persist");
    client
        .upsert_session(
            session_snapshot_json(child_session_id, &workspace_text),
            Some(root_session_id.to_string()),
            Vec::new(),
            Vec::new(),
        )
        .expect("child session should persist");
    wait_for_session_parent(&client, child_session_id, root_session_id);

    let provider = LocalProvider::start(vec![ProviderReply::Json {
        status: "200 OK",
        request_id: Some("req-runtime-cache-root"),
        body: json!({
            "id": "chatcmpl-runtime-cache-root",
            "choices": [{
                "message": {"role": "assistant", "content": "cache root ok"},
                "finish_reason": "stop"
            }],
            "usage": {"prompt_tokens": 10, "completion_tokens": 3, "total_tokens": 13}
        }),
    }]);
    let _env = EnvGuard::set(&[
        ("OPENAI_API_KEY", "runtime-cache-key"),
        ("TURA_DISABLE_PROMPT_CACHE", "0"),
    ]);
    let settings = settings_for_route(
        "openai-cache-route",
        "openai",
        &provider.endpoint,
        "gpt-cache-local",
    );
    let runtime = runtime_for_provider(
        "runtime-cache-root",
        child_session_id,
        "openai-cache-route",
        "openai",
        "gpt-cache-local",
        false,
    );

    let result = call_runtime(
        CallRuntimeInput {
            runtime,
            messages: vec![json!({"role": "user", "content": "reuse cache"})],
            tools: Vec::new(),
            provider_name: "openai-cache-route".to_string(),
            stream: false,
            max_tokens: 128,
            tool_choice: None,
            session_directory: workspace.path().to_path_buf(),
            allowed_command_run_commands: None,
            require_startup_task_state: false,
        },
        Arc::new(settings),
        Arc::new(TuraConfig::new(".env.runtime-cache-business-missing")),
    )
    .await
    .expect("runtime cache call should succeed");

    let requests = provider.requests();
    let cache_key = requests[0].body["prompt_cache_key"]
        .as_str()
        .expect("prompt cache key should be sent for OpenAI providers");
    assert!(
        cache_key.starts_with("turaosv2:openai-cache-route:cache-root-session:"),
        "forked sessions should reuse the root session cache key, got {cache_key}"
    );
    assert_eq!(
        result.input.expect("runtime input")["options"]["prompt_cache_key"],
        cache_key
    );
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

fn runtime_for_provider(
    runtime_id: &str,
    session_id: &str,
    route: &str,
    provider_url_name: &str,
    model: &str,
    stream: bool,
) -> RuntimeManagement {
    RuntimeManagement::new(
        runtime_id.to_string(),
        session_id.to_string(),
        "agent-runtime-boundary".to_string(),
        RuntimeProviderConfig {
            base: ProviderConfig {
                tura_llm_name: route.to_string(),
                default_model_tier: None,
                current_model: None,
                stream,
                temperature: 0.0,
                max_tokens: 128,
                tool_choice: ToolChoice::Auto,
                time_out_ms: 30_000,
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

fn settings_for_route(
    route_name: &str,
    provider_name: &str,
    base_url: &str,
    model: &str,
) -> Settings {
    Settings {
        provider_base_url: HashMap::new(),
        routes: HashMap::from([(
            route_name.to_string(),
            RouteConfig {
                default_temperature: 0.0,
                providers: vec![LlmProviderConfig {
                    provider: provider_name.to_string(),
                    base_url: base_url.to_string(),
                    model: model.to_string(),
                    temperature: 0.0,
                }],
            },
        )]),
        model_catalog: ModelCatalog::default(),
        provider_enums: ProviderEnumCatalog::default(),
    }
}

fn session_snapshot_json(session_id: &str, workspace: &str) -> Value {
    json!({
        "id": session_id,
        "directory": workspace,
        "name": session_id,
        "created_at": 1,
        "updated_at": 1,
        "management": {
            "session_id": session_id,
            "session_name": session_id,
            "session_directory": workspace,
            "state": "created",
            "task_plan": {"plan_summary": "", "detailed_tasks": []}
        }
    })
}

fn wait_for_session_parent(
    client: &runtime::session_log_client::SessionLogClient,
    session_id: &str,
    parent_id: &str,
) {
    let started = Instant::now();
    while started.elapsed() < Duration::from_secs(10) {
        if client
            .get_session(session_id.to_string())
            .ok()
            .flatten()
            .and_then(|snapshot| snapshot.parent_id)
            .as_deref()
            == Some(parent_id)
        {
            return;
        }
        thread::sleep(Duration::from_millis(25));
    }
    let latest_parent = client
        .get_session(session_id.to_string())
        .ok()
        .flatten()
        .and_then(|snapshot| snapshot.parent_id);
    panic!(
        "session parent was not applied within 10s; session={session_id}; expected_parent={parent_id}; latest_parent={latest_parent:?}"
    );
}

fn command_run_tool_schema() -> Value {
    json!({
        "type": "function",
        "function": {
            "name": "command_run",
            "parameters": {
                "type": "object",
                "properties": {
                    "commands": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "step": {"type": "integer"},
                                "command_type": {"type": "string"},
                                "command_line": {"type": "string"}
                            }
                        }
                    }
                }
            }
        }
    })
}

fn create_marker_command(path: &str, content: &str) -> String {
    if cfg!(windows) {
        format!("Set-Content -LiteralPath {path:?} -Value {content:?} -NoNewline")
    } else {
        format!("printf %s {content:?} > {path:?}")
    }
}

#[derive(Debug, Clone)]
struct CapturedHttpRequest {
    path: String,
    query: String,
    headers: String,
    body: Value,
}

#[derive(Debug, Clone)]
enum ProviderReply {
    Json {
        status: &'static str,
        request_id: Option<&'static str>,
        body: Value,
    },
}

struct LocalProvider {
    endpoint: String,
    requests: Arc<Mutex<Vec<CapturedHttpRequest>>>,
}

impl LocalProvider {
    fn start(replies: Vec<ProviderReply>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind runtime provider");
        let addr: SocketAddr = listener.local_addr().expect("runtime provider addr");
        let requests = Arc::new(Mutex::new(Vec::new()));
        let thread_requests = Arc::clone(&requests);
        thread::spawn(move || {
            for reply in replies {
                let (mut stream, _) = listener.accept().expect("accept runtime provider request");
                let request = read_http_request(&mut stream);
                thread_requests.lock().expect("request lock").push(request);
                match reply {
                    ProviderReply::Json {
                        status,
                        request_id,
                        body,
                    } => {
                        let body = body.to_string();
                        let request_id_header = request_id
                            .map(|value| format!("x-request-id: {value}\r\n"))
                            .unwrap_or_default();
                        let response = format!(
                            "HTTP/1.1 {status}\r\nContent-Type: application/json\r\n{request_id_header}Content-Length: {}\r\n\r\n{}",
                            body.len(),
                            body
                        );
                        stream
                            .write_all(response.as_bytes())
                            .expect("write runtime provider response");
                    }
                }
            }
        });

        Self {
            endpoint: format!("http://{addr}"),
            requests,
        }
    }

    fn requests(&self) -> Vec<CapturedHttpRequest> {
        self.requests.lock().expect("request lock").clone()
    }
}

fn read_http_request(stream: &mut std::net::TcpStream) -> CapturedHttpRequest {
    let mut buffer = Vec::new();
    let mut chunk = [0_u8; 1024];
    let (header_end, content_length) = loop {
        let read = stream.read(&mut chunk).expect("read request");
        assert!(read > 0, "provider client closed before headers");
        buffer.extend_from_slice(&chunk[..read]);
        if let Some(header_end) = find_header_end(&buffer) {
            let headers = String::from_utf8_lossy(&buffer[..header_end]).to_string();
            let content_length = header_value(&headers, "content-length")
                .and_then(|value| value.parse::<usize>().ok())
                .expect("content-length header");
            break (header_end, content_length);
        }
    };
    let body_start = header_end + 4;
    while buffer.len() < body_start + content_length {
        let read = stream.read(&mut chunk).expect("read request body");
        assert!(read > 0, "provider client closed before body");
        buffer.extend_from_slice(&chunk[..read]);
    }

    let headers = String::from_utf8_lossy(&buffer[..header_end]).to_string();
    let request_line = headers.lines().next().expect("request line");
    let mut parts = request_line.split_whitespace();
    let _method = parts.next().expect("method");
    let target = parts.next().expect("path").to_string();
    let (path, query) = target
        .split_once('?')
        .map(|(path, query)| (path.to_string(), query.to_string()))
        .unwrap_or_else(|| (target, String::new()));
    let headers_lower = headers.to_ascii_lowercase();
    let body_text = String::from_utf8(buffer[body_start..body_start + content_length].to_vec())
        .expect("utf8 request body");
    let body = serde_json::from_str(&body_text).expect("json request body");

    CapturedHttpRequest {
        path,
        query,
        headers: headers_lower,
        body,
    }
}

fn find_header_end(buffer: &[u8]) -> Option<usize> {
    buffer.windows(4).position(|window| window == b"\r\n\r\n")
}

fn header_value(headers: &str, name: &str) -> Option<String> {
    headers.lines().find_map(|line| {
        let (key, value) = line.split_once(':')?;
        key.eq_ignore_ascii_case(name)
            .then(|| value.trim().to_string())
    })
}

struct EnvGuard {
    previous: Vec<(&'static str, Option<std::ffi::OsString>)>,
}

impl EnvGuard {
    fn set(vars: &[(&'static str, &str)]) -> Self {
        let previous = vars
            .iter()
            .map(|(key, value)| {
                let previous = std::env::var_os(key);
                std::env::set_var(key, value);
                (*key, previous)
            })
            .collect();
        Self { previous }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        for (key, value) in self.previous.drain(..).rev() {
            match value {
                Some(value) => std::env::set_var(key, value),
                None => std::env::remove_var(key),
            }
        }
    }
}
