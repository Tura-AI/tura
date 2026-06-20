use serde_json::{json, Value};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::thread;
use tokio::sync::Mutex;
use tura_llm_rust::{
    extract_response_text, extract_tool_calls, CallOptions, ProviderConfig, TuraConfig, TuraError,
};

#[derive(Debug)]
struct CapturedHttpRequest {
    method: String,
    path: String,
    query: String,
    headers: String,
    body: Value,
}

static ENV_LOCK: Mutex<()> = Mutex::const_new(());

#[tokio::test]
async fn google_business_flow_generates_content_with_tools_system_usage_and_request_shape() {
    let _env_guard = ENV_LOCK.lock().await;
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind local google provider");
    let addr = listener.local_addr().expect("local google provider addr");
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept google request");
        let request = read_http_request(&mut stream);
        let body = json!({
            "candidates": [{
                "content": {
                    "role": "model",
                    "parts": [
                        {"text": "I will inspect the workspace."},
                        {
                            "thoughtSignature": "google-thought-sig-1",
                            "functionCall": {
                                "name": "command_run",
                                "args": {
                                    "commands": [{
                                        "step": 1,
                                        "command_type": "shell_command",
                                        "command_line": "Get-ChildItem -Name"
                                    }]
                                }
                            }
                        }
                    ]
                },
                "finishReason": "STOP"
            }],
            "usageMetadata": {
                "promptTokenCount": 33,
                "candidatesTokenCount": 12,
                "totalTokenCount": 45,
                "cachedContentTokenCount": 5
            }
        })
        .to_string();
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nx-request-id: req-google-local-1\r\nContent-Length: {}\r\n\r\n{}",
            body.len(),
            body
        );
        stream
            .write_all(response.as_bytes())
            .expect("write google response");
        request
    });

    let previous_key = std::env::var_os("GOOGLE_API_KEY");
    std::env::set_var("GOOGLE_API_KEY", "dummy-google-key");
    let config = ProviderConfig {
        provider: "google".to_string(),
        base_url: format!("http://{addr}/v1beta"),
        model: "models/gemini-local".to_string(),
        temperature: 0.2,
    };
    let result = config
        .call(
            &TuraConfig::new(".env.provider-business-missing"),
            vec![
                json!({"role": "system", "content": "Keep Google system turns separate."}),
                json!({"role": "developer", "content": [{"text": "Prefer shell commands."}]}),
                json!({"role": "user", "content": [
                    {"type": "input_text", "text": "Inspect the workspace"},
                    {"type": "input_image", "image_url": "data:image/png;base64,QUJD"}
                ]}),
            ],
            CallOptions {
                tools: Some(vec![json!({
                    "type": "function",
                    "function": {
                        "name": "command_run",
                        "description": "Run a local command",
                        "parameters": {
                            "type": "object",
                            "additionalProperties": false,
                            "properties": {
                                "commands": {
                                    "type": "array",
                                    "items": {
                                        "type": "object",
                                        "additionalProperties": false,
                                        "properties": {
                                            "step": {"type": "integer"},
                                            "command_line": {"type": "string"}
                                        }
                                    }
                                }
                            }
                        }
                    }
                })]),
                tool_choice: Some(json!({
                    "type": "function",
                    "function": {"name": "command_run"}
                })),
                max_tokens: Some(64),
                top_p: Some(0.7),
                context_window: Some(1024),
                extra_body: Some(json!({
                    "generationConfig": {
                        "responseMimeType": "application/json"
                    }
                })),
                ..CallOptions::default()
            },
        )
        .await;

    match previous_key {
        Some(value) => std::env::set_var("GOOGLE_API_KEY", value),
        None => std::env::remove_var("GOOGLE_API_KEY"),
    }

    let response = result.expect("local google provider call");
    let captured = server.join().expect("google server thread joins");
    assert_eq!(captured.method, "POST");
    assert_eq!(captured.path, "/v1beta/models/gemini-local:generateContent");
    assert_eq!(captured.query, "key=dummy-google-key");
    assert!(
        captured.headers.contains("authorization:"),
        "google request deliberately clears bearer auth and uses the key query parameter"
    );
    assert_eq!(
        captured.body["systemInstruction"]["parts"][0]["text"],
        "Keep Google system turns separate."
    );
    assert_eq!(
        captured.body["systemInstruction"]["parts"][1]["text"],
        "Prefer shell commands."
    );
    assert_eq!(captured.body["contents"][0]["role"], "user");
    assert_eq!(
        captured.body["contents"][0]["parts"][0]["text"],
        "Inspect the workspace"
    );
    assert_eq!(
        captured.body["contents"][0]["parts"][1]["inlineData"]["mimeType"],
        "image/png"
    );
    assert_eq!(
        captured.body["tools"][0]["functionDeclarations"][0]["name"],
        "command_run"
    );
    assert!(
        captured.body["tools"][0]["functionDeclarations"][0]["parameters"]
            .get("additionalProperties")
            .is_none(),
        "Gemini schemas must be sanitized before leaving the provider boundary"
    );
    assert_eq!(
        captured.body["tools"][0]["functionDeclarations"][0]["parameters"]["properties"]
            ["commands"]["items"]
            .get("additionalProperties"),
        None
    );
    assert_eq!(
        captured.body["toolConfig"]["functionCallingConfig"]["allowedFunctionNames"][0],
        "command_run"
    );
    assert_eq!(captured.body["generationConfig"]["temperature"], 0.2);
    assert_eq!(captured.body["generationConfig"]["maxOutputTokens"], 64);
    assert_eq!(captured.body["generationConfig"]["topP"], 0.7);
    assert_eq!(
        captured.body["generationConfig"]["responseMimeType"],
        "application/json"
    );

    assert_eq!(
        extract_response_text(&response.content).as_deref(),
        Some("I will inspect the workspace.")
    );
    let calls = extract_tool_calls(&response.content);
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].tool_name, "command_run");
    assert_eq!(
        calls[0].arguments["commands"][0]["command_line"],
        "Get-ChildItem -Name"
    );
    assert_eq!(
        calls[0]
            .provider_metadata
            .as_ref()
            .and_then(|metadata| metadata.get("google_thought_signature"))
            .and_then(Value::as_str),
        Some("google-thought-sig-1")
    );
    let metrics = response.metrics.expect("google metrics");
    assert_eq!(
        metrics.provider_request_id.as_deref(),
        Some("req-google-local-1")
    );
    assert_eq!(metrics.usage.input_tokens, Some(33));
    assert_eq!(metrics.usage.output_tokens, Some(12));
    assert_eq!(metrics.usage.total_tokens, Some(45));
    assert_eq!(metrics.usage.cached_input_tokens, Some(5));
    assert_eq!(metrics.usage.context_window, Some(1024));
    assert_eq!(metrics.tool_call_count, 1);
    assert_eq!(metrics.finish_reason.as_deref(), Some("STOP"));
    assert!(metrics.cache_hit);
}

#[tokio::test]
async fn google_business_flow_replays_function_call_output_and_media_sidecar() {
    let _env_guard = ENV_LOCK.lock().await;
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind local google provider");
    let addr = listener.local_addr().expect("local google provider addr");
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept google request");
        let request = read_http_request(&mut stream);
        let body = json!({
            "candidates": [{
                "content": {
                    "role": "model",
                    "parts": [{"text": "The image sidecar and command output were accepted."}]
                },
                "finishReason": "STOP"
            }],
            "usageMetadata": {
                "promptTokenCount": 21,
                "candidatesTokenCount": 9,
                "totalTokenCount": 30
            }
        })
        .to_string();
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
            body.len(),
            body
        );
        stream
            .write_all(response.as_bytes())
            .expect("write google replay response");
        request
    });

    let previous_key = std::env::var_os("GOOGLE_API_KEY");
    std::env::set_var("GOOGLE_API_KEY", "dummy-google-key");
    let config = ProviderConfig {
        provider: "google".to_string(),
        base_url: format!("http://{addr}"),
        model: "gemini-local".to_string(),
        temperature: 0.2,
    };
    let result = config
        .call(
            &TuraConfig::new(".env.provider-business-missing"),
            vec![
                json!({"role": "user", "content": "Run and then inspect generated media"}),
                json!({
                    "type": "function_call",
                    "call_id": "call_media",
                    "name": "command_run",
                    "arguments": "{\"commands\":[{\"step\":1,\"command_line\":\"python draw.py\"}]}",
                    "provider_metadata": {"thoughtSignature": "sig-replay"}
                }),
                json!({
                    "type": "function_call_output",
                    "call_id": "call_media",
                    "output": [
                        {"type": "input_text", "text": "created output.png"},
                        {"type": "input_image", "image_url": "data:image/png;base64,WFla"}
                    ]
                }),
            ],
            CallOptions::default(),
        )
        .await;

    match previous_key {
        Some(value) => std::env::set_var("GOOGLE_API_KEY", value),
        None => std::env::remove_var("GOOGLE_API_KEY"),
    }

    let response = result.expect("local google replay call");
    let captured = server.join().expect("google server thread joins");
    assert_eq!(
        captured.path, "/models/gemini-local:generateContent",
        "models without the prefix are still sent as Google model paths"
    );
    assert_eq!(
        captured.body["contents"][1]["parts"][0]["functionCall"]["name"],
        "command_run"
    );
    assert_eq!(
        captured.body["contents"][1]["parts"][0]["functionCall"]["args"]["commands"][0]
            ["command_line"],
        "python draw.py"
    );
    assert_eq!(
        captured.body["contents"][1]["parts"][0]["thoughtSignature"],
        "sig-replay"
    );
    assert_eq!(
        captured.body["contents"][2]["parts"][0]["functionResponse"]["name"],
        "command_run"
    );
    assert_eq!(
        captured.body["contents"][2]["parts"][0]["functionResponse"]["response"]["output"],
        "created output.png"
    );
    assert_eq!(
        captured.body["contents"][3]["parts"][1]["inlineData"]["data"],
        "WFla"
    );
    assert_eq!(
        extract_response_text(&response.content).as_deref(),
        Some("The image sidecar and command output were accepted.")
    );
    let metrics = response.metrics.expect("google replay metrics");
    assert_eq!(metrics.usage.total_tokens, Some(30));
    assert_eq!(metrics.tool_call_count, 0);
}

#[tokio::test]
async fn google_business_flow_embeds_local_vectors_and_rejects_missing_values() {
    let _env_guard = ENV_LOCK.lock().await;
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind local google embed provider");
    let addr = listener.local_addr().expect("local google embed addr");
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept google embed request");
        let request = read_http_request(&mut stream);
        let body = json!({
            "embedding": {
                "values": [0.25, -0.5, 1.75]
            }
        })
        .to_string();
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
            body.len(),
            body
        );
        stream
            .write_all(response.as_bytes())
            .expect("write google embed response");
        request
    });

    let previous_key = std::env::var_os("GOOGLE_API_KEY");
    std::env::set_var("GOOGLE_API_KEY", "dummy-google-key");
    let config = ProviderConfig {
        provider: "google".to_string(),
        base_url: format!("http://{addr}"),
        model: "models/text-embedding-local".to_string(),
        temperature: 0.2,
    };
    let embedding = config
        .embed(
            "local document for google embeddings",
            &TuraConfig::new(".env.provider-business-missing"),
        )
        .await
        .expect("local google embedding");
    let captured = server.join().expect("google embed server joins");
    assert_eq!(captured.path, "/models/text-embedding-local:embedContent");
    assert_eq!(captured.query, "key=dummy-google-key");
    assert_eq!(
        captured.body["model"], "models/text-embedding-local",
        "embedding payload keeps the Google models/ prefix exactly once"
    );
    assert_eq!(
        captured.body["content"]["parts"][0]["text"],
        "local document for google embeddings"
    );
    assert_eq!(captured.body["taskType"], "RETRIEVAL_DOCUMENT");
    assert_eq!(embedding, vec![0.25_f32, -0.5_f32, 1.75_f32]);

    let bad_listener =
        TcpListener::bind("127.0.0.1:0").expect("bind bad local google embed provider");
    let bad_addr = bad_listener.local_addr().expect("bad google embed addr");
    let bad_server = thread::spawn(move || {
        let (mut stream, _) = bad_listener
            .accept()
            .expect("accept bad google embed request");
        let request = read_http_request(&mut stream);
        let body = json!({"embedding": {"notValues": ["wrong"]}}).to_string();
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
            body.len(),
            body
        );
        stream
            .write_all(response.as_bytes())
            .expect("write bad google embed response");
        request
    });
    let bad_config = ProviderConfig {
        provider: "google".to_string(),
        base_url: format!("http://{bad_addr}"),
        model: "text-embedding-local".to_string(),
        temperature: 0.2,
    };
    let err = bad_config
        .embed(
            "bad embedding payload",
            &TuraConfig::new(".env.provider-business-missing"),
        )
        .await
        .expect_err("missing embedding values should fail");
    let bad_captured = bad_server.join().expect("bad google embed server joins");

    match previous_key {
        Some(value) => std::env::set_var("GOOGLE_API_KEY", value),
        None => std::env::remove_var("GOOGLE_API_KEY"),
    }

    assert_eq!(
        bad_captured.path,
        "/models/text-embedding-local:embedContent"
    );
    match err {
        TuraError::ProviderRequest { provider, message } => {
            assert_eq!(provider, "google");
            assert!(message.contains("missing embedding values"));
        }
        other => panic!("expected google embedding shape error, got {other}"),
    }
}

#[tokio::test]
async fn google_business_flow_reports_http_status_body_and_invalid_json() {
    let _env_guard = ENV_LOCK.lock().await;
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind local google provider");
    let addr = listener.local_addr().expect("local google provider addr");
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept google request");
        let request = read_http_request(&mut stream);
        let body = json!({
            "error": {
                "code": 429,
                "status": "RESOURCE_EXHAUSTED",
                "message": "local google quota exhausted"
            }
        })
        .to_string();
        let response = format!(
            "HTTP/1.1 429 Too Many Requests\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
            body.len(),
            body
        );
        stream
            .write_all(response.as_bytes())
            .expect("write google error response");
        request
    });

    let previous_key = std::env::var_os("GOOGLE_API_KEY");
    std::env::set_var("GOOGLE_API_KEY", "dummy-google-key");
    let config = ProviderConfig {
        provider: "google".to_string(),
        base_url: format!("http://{addr}"),
        model: "gemini-local".to_string(),
        temperature: 0.2,
    };
    let err = config
        .call(
            &TuraConfig::new(".env.provider-business-missing"),
            vec![json!({"role": "user", "content": "force google status error"})],
            CallOptions::default(),
        )
        .await
        .expect_err("google HTTP status should fail");
    let captured = server.join().expect("google error server joins");
    assert_eq!(captured.path, "/models/gemini-local:generateContent");
    match err {
        TuraError::HttpStatus { status, body } => {
            assert_eq!(status, 429);
            assert!(
                body.contains("RESOURCE_EXHAUSTED")
                    && body.contains("local google quota exhausted")
            );
        }
        other => panic!("expected google HTTP status error, got {other}"),
    }

    let invalid_listener = TcpListener::bind("127.0.0.1:0").expect("bind invalid google provider");
    let invalid_addr = invalid_listener.local_addr().expect("invalid google addr");
    let invalid_server = thread::spawn(move || {
        let (mut stream, _) = invalid_listener
            .accept()
            .expect("accept invalid google request");
        let request = read_http_request(&mut stream);
        let body = r#"{"candidates":[{"content":{"parts":[{"text":"unterminated"}]}]"#;
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
            body.len(),
            body
        );
        stream
            .write_all(response.as_bytes())
            .expect("write invalid google response");
        request
    });
    let invalid_config = ProviderConfig {
        provider: "google".to_string(),
        base_url: format!("http://{invalid_addr}"),
        model: "gemini-local".to_string(),
        temperature: 0.2,
    };
    let err = invalid_config
        .call(
            &TuraConfig::new(".env.provider-business-missing"),
            vec![json!({"role": "user", "content": "return invalid json"})],
            CallOptions::default(),
        )
        .await
        .expect_err("invalid google JSON should fail");
    let invalid_captured = invalid_server.join().expect("invalid google server joins");

    match previous_key {
        Some(value) => std::env::set_var("GOOGLE_API_KEY", value),
        None => std::env::remove_var("GOOGLE_API_KEY"),
    }

    assert_eq!(
        invalid_captured.path,
        "/models/gemini-local:generateContent"
    );
    match err {
        TuraError::Network { message } => {
            assert!(
                message.contains("decoding response body")
                    || message.contains("EOF")
                    || message.contains("expected"),
                "invalid JSON error should keep parser context: {message}"
            );
        }
        other => panic!("expected google decode error, got {other}"),
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
    let method = parts.next().expect("method").to_string();
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
        method,
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
