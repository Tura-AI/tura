use super::*;
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

async fn serve_raw_http_once(writes: Vec<Vec<u8>>) -> (String, tokio::task::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind local HTTP fixture");
    let address = listener.local_addr().expect("fixture address");
    let task = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.expect("accept fixture request");
        let mut request = Vec::new();
        let mut buffer = [0_u8; 1024];
        loop {
            let read = socket
                .read(&mut buffer)
                .await
                .expect("read fixture request");
            if read == 0 {
                break;
            }
            request.extend_from_slice(&buffer[..read]);
            let Some(header_end) = request.windows(4).position(|window| window == b"\r\n\r\n")
            else {
                continue;
            };
            let header_end = header_end + 4;
            let headers = String::from_utf8_lossy(&request[..header_end]);
            let content_length = headers
                .lines()
                .find_map(|line| {
                    line.split_once(':').and_then(|(name, value)| {
                        name.eq_ignore_ascii_case("content-length")
                            .then(|| value.trim().parse::<usize>().ok())
                            .flatten()
                    })
                })
                .unwrap_or(0);
            if request.len() >= header_end + content_length {
                break;
            }
        }

        for write in writes {
            socket
                .write_all(&write)
                .await
                .expect("write fixture response");
        }
        socket.shutdown().await.expect("close fixture response");
    });
    (format!("http://{address}/v1"), task)
}

fn chunked_response_writes(status: &str, request_id: &str, chunks: Vec<Vec<u8>>) -> Vec<Vec<u8>> {
    let mut writes = vec![
        format!(
            "HTTP/1.1 {status}\r\nContent-Type: text/event-stream; charset=utf-8\r\n\
             Transfer-Encoding: chunked\r\nX-Request-Id: {request_id}\r\n\
             Connection: close\r\n\r\n"
        )
        .into_bytes(),
    ];
    for chunk in chunks {
        writes.push(format!("{:X}\r\n", chunk.len()).into_bytes());
        writes.push(chunk);
        writes.push(b"\r\n".to_vec());
    }
    writes.push(b"0\r\n\r\n".to_vec());
    writes
}

fn stream_options() -> CallOptions {
    CallOptions {
        stream: Some(true),
        stream_options: Some(json!({"include_usage": true})),
        ..Default::default()
    }
}

#[test]
fn chat_stream_completes_when_a_fresh_tokio_runtime_is_created_per_turn() {
    for turn in 0..8 {
        let runtime = tokio::runtime::Runtime::new().expect("per-turn runtime");
        runtime.block_on(async {
            let chunks = vec![
                format!(
                    "data: {}\n\n",
                    json!({"choices":[{"delta":{"content":format!("turn-{turn}")}}]})
                )
                .into_bytes(),
                b"data: [DONE]\n\n".to_vec(),
            ];
            let (base_url, server) = serve_raw_http_once(chunked_response_writes(
                "200 OK",
                &format!("req-runtime-turn-{turn}"),
                chunks,
            ))
            .await;

            let response = call(
                &base_url,
                "mimo-pro",
                "minimax",
                "test-key",
                &[json!({"role":"user","content":"test"})],
                &stream_options(),
            )
            .await
            .expect("stream inside per-turn runtime");
            server.await.expect("fixture task");

            assert_eq!(response.content, format!("turn-{turn}"));
        });
    }
}

#[tokio::test]
async fn chat_stream_preserves_utf8_split_across_transport_chunks_and_emits_text_events() {
    let event = format!(
        "data: {}\n\n",
        json!({"choices":[{"delta":{"content":"MiMo mówi: żółw"}}]})
    );
    let bytes = event.into_bytes();
    let utf8_start = bytes
        .windows("ż".len())
        .position(|window| window == "ż".as_bytes())
        .expect("UTF-8 content");
    let chunks = vec![
        bytes[..utf8_start + 1].to_vec(),
        bytes[utf8_start + 1..].to_vec(),
        b"data: [DONE]\n\n".to_vec(),
    ];
    let (base_url, server) = serve_raw_http_once(chunked_response_writes(
        "200 OK",
        "req-fragmented-utf8",
        chunks,
    ))
    .await;
    let events = Arc::new(Mutex::new(Vec::new()));
    let captured = Arc::clone(&events);
    let sink: ProviderStreamEventSink = Arc::new(move |event| {
        captured.lock().expect("event lock").push(event);
    });

    let response = call_with_stream_events(
        &base_url,
        "mimo-pro",
        "minimax",
        "test-key",
        &[json!({"role":"user","content":"test"})],
        &stream_options(),
        Some(sink),
    )
    .await
    .expect("fragmented UTF-8 stream");
    server.await.expect("fixture task");

    assert_eq!(response.content, "MiMo mówi: żółw");
    assert!(events.lock().expect("event lock").iter().any(|event| {
        matches!(
            event,
            ProviderStreamEvent::TextDelta { text } if text == "MiMo mówi: żółw"
        )
    }));
}

#[tokio::test]
async fn chat_completion_unwraps_clinepass_success_envelope() {
    let body = json!({
        "success": true,
        "data": {
            "id": "cline-fixture",
            "object": "chat.completion",
            "model": "xiaomi/mimo-v2.5-pro",
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": "ClinePass envelope works"
                },
                "finish_reason": "stop"
            }],
            "usage": {
                "prompt_tokens": 11,
                "completion_tokens": 5,
                "total_tokens": 16
            }
        }
    })
    .to_string();
    let response = vec![
        format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\
             Content-Length: {}\r\nX-Request-Id: req-cline-envelope\r\n\
             Connection: close\r\n\r\n",
            body.len()
        )
        .into_bytes(),
        body.into_bytes(),
    ];
    let (base_url, server) = serve_raw_http_once(response).await;

    let result = call(
        &base_url,
        "cline-pass/mimo-v2.5-pro",
        "cline-pass",
        "test-key",
        &[json!({"role":"user","content":"test"})],
        &CallOptions {
            stream: Some(false),
            ..Default::default()
        },
    )
    .await
    .expect("ClinePass envelope should normalize");
    server.await.expect("fixture task");

    assert_eq!(result.content, "ClinePass envelope works");
    let metrics = result.metrics.expect("response metrics");
    assert_eq!(metrics.usage.input_tokens, Some(11));
    assert_eq!(metrics.usage.output_tokens, Some(5));
    assert_eq!(metrics.usage.total_tokens, Some(16));
}

#[test]
fn clinepass_envelope_detection_requires_the_exact_api_host() {
    let envelope = json!({
        "success": true,
        "data": {"choices": [{"message": {"content": "ok"}}]}
    });

    let normalized =
        unwrap_chat_completion_response("openai-compatible", "https://api.cline.bot/v1", &envelope);
    assert!(normalized.get("choices").is_some());

    let attacker = unwrap_chat_completion_response(
        "openai-compatible",
        "https://api.cline.bot.attacker.example/v1",
        &envelope,
    );
    assert_eq!(attacker, envelope);
}

#[tokio::test]
async fn chat_stream_reassembles_tool_calls_and_usage_events() {
    let events = [
        json!({"choices":[{"delta":{"tool_calls":[{
            "index":0,
            "id":"call_fixture",
            "function":{"name":"command_run","arguments":"{\"commands\":["}
        }]}}]}),
        json!({"choices":[{"delta":{"tool_calls":[{
            "index":0,
            "function":{"arguments":"{\"step\":1,\"command_type\":\"shell_command\",\"command_line\":\"pwd\"}]}"}
        }]}}]}),
        json!({"choices":[{"delta":{},"finish_reason":"tool_calls"}]}),
        json!({"choices":[],"usage":{
            "prompt_tokens":12,
            "completion_tokens":7,
            "total_tokens":19
        }}),
    ];
    let chunks = events
        .into_iter()
        .map(|event| format!("data: {event}\n\n").into_bytes())
        .chain([b"data: [DONE]\n\n".to_vec()])
        .collect();
    let (base_url, server) =
        serve_raw_http_once(chunked_response_writes("200 OK", "req-tool-usage", chunks)).await;

    let response = call(
        &base_url,
        "mimo-pro",
        "minimax",
        "test-key",
        &[json!({"role":"user","content":"run pwd"})],
        &stream_options(),
    )
    .await
    .expect("tool and usage stream");
    server.await.expect("fixture task");

    assert_eq!(
        response.content.pointer("/tool_calls/0/id"),
        Some(&json!("call_fixture"))
    );
    assert_eq!(
        response
            .content
            .pointer("/tool_calls/0/function/arguments/commands/0/command_line"),
        Some(&json!("pwd"))
    );
    let metrics = response.metrics.expect("stream metrics");
    assert_eq!(metrics.usage.input_tokens, Some(12));
    assert_eq!(metrics.usage.output_tokens, Some(7));
    assert_eq!(metrics.usage.total_tokens, Some(19));
    assert_eq!(metrics.tool_call_count, 1);
    assert_eq!(metrics.finish_reason.as_deref(), Some("tool_calls"));
}

#[tokio::test]
async fn chat_stream_preserves_provider_error_body_and_status() {
    let body = br#"{"error":{"message":"provider rejected the request","code":"bad_request"}}"#;
    let response = vec![
        format!(
            "HTTP/1.1 422 Unprocessable Entity\r\nContent-Type: application/json\r\n\
             Content-Length: {}\r\nX-Request-Id: req-provider-error\r\n\
             Connection: close\r\n\r\n",
            body.len()
        )
        .into_bytes(),
        body.to_vec(),
    ];
    let (base_url, server) = serve_raw_http_once(response).await;

    let error = call(
        &base_url,
        "mimo-pro",
        "minimax",
        "test-key",
        &[json!({"role":"user","content":"test"})],
        &stream_options(),
    )
    .await
    .expect_err("provider status must fail");
    server.await.expect("fixture task");

    match error {
        TuraError::HttpStatus {
            status,
            body: actual,
        } => {
            assert_eq!(status, 422);
            assert_eq!(actual, String::from_utf8_lossy(body));
        }
        other => panic!("expected HTTP status error, got {other}"),
    }
}

#[tokio::test]
async fn chat_stream_reports_transfer_termination_with_response_context() {
    let response = vec![
        b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\n\
          Transfer-Encoding: chunked\r\nX-Request-Id: req-truncated-stream\r\n\
          Connection: close\r\n\r\n"
            .to_vec(),
        b"40\r\ndata: {\"choices\":[{\"delta\":{\"content\":\"partial\"}}]}\n".to_vec(),
    ];
    let (base_url, server) = serve_raw_http_once(response).await;

    let error = call(
        &base_url,
        "mimo-pro",
        "minimax",
        "test-key",
        &[json!({"role":"user","content":"test"})],
        &stream_options(),
    )
    .await
    .expect_err("truncated transfer must fail");
    server.await.expect("fixture task");

    let TuraError::Network { message } = error else {
        panic!("expected retryable network error");
    };
    assert!(message.contains("OpenAI-compatible chat stream body failed"));
    assert!(message.contains("provider 'minimax'"));
    assert!(message.contains("request_id=req-truncated-stream"));
    assert!(message.contains("content_type=text/event-stream"));
    assert!(message.contains("transfer_encoding=chunked"));
    assert!(message.contains("http_version=HTTP/1.1"));
    assert!(message.contains("error decoding response body"));
}

#[tokio::test]
async fn chat_stream_rejects_invalid_utf8_with_response_context() {
    let chunks = vec![
        b"data: {\"choices\":[{\"delta\":{\"content\":\"".to_vec(),
        vec![0xFF],
        b"\"}}]}\n\n".to_vec(),
    ];
    let (base_url, server) = serve_raw_http_once(chunked_response_writes(
        "200 OK",
        "req-invalid-utf8",
        chunks,
    ))
    .await;

    let error = call(
        &base_url,
        "mimo-pro",
        "minimax",
        "test-key",
        &[json!({"role":"user","content":"test"})],
        &stream_options(),
    )
    .await
    .expect_err("invalid UTF-8 must fail");
    server.await.expect("fixture task");

    let TuraError::Network { message } = error else {
        panic!("expected retryable network error");
    };
    assert!(message.contains("invalid UTF-8"));
    assert!(message.contains("provider 'minimax'"));
    assert!(message.contains("request_id=req-invalid-utf8"));
}

#[test]
fn openai_compatible_function_output_media_gets_sidecar_user_image() {
    let messages = vec![
        json!({
            "type": "function_call",
            "name": "command_run",
            "call_id": "call_media",
            "arguments": "{\"commands\":[]}"
        }),
        json!({
            "type": "function_call_output",
            "call_id": "call_media",
            "output": [
                { "type": "input_text", "text": "read_media returned image" },
                { "type": "input_image", "image_url": "data:image/jpeg;base64,AAA" }
            ]
        }),
    ];

    let normalized = normalize_messages_for_provider("openrouter", &messages);

    assert_eq!(normalized[1]["role"], "tool");
    assert_eq!(normalized[1]["content"], "read_media returned image");
    assert_eq!(normalized[2]["role"], "user");
    assert_eq!(normalized[2]["content"][1]["type"], "image_url");
    assert_eq!(
        normalized[2]["content"][1]["image_url"]["url"],
        "data:image/jpeg;base64,AAA"
    );
}

#[test]
fn openrouter_qwen_thinking_omits_object_tool_choice() {
    let payload = build_chat_payload(
        "openrouter",
        "qwen3.7-max",
        &[json!({"role": "user", "content": "hi"})],
        &CallOptions {
            tools: Some(vec![json!({
                "type": "function",
                "function": {
                    "name": "command_run",
                    "parameters": {"type": "object"}
                }
            })]),
            tool_choice: Some(json!({
                "type": "function",
                "function": {"name": "command_run"}
            })),
            reasoning_effort: Some("low".to_string()),
            ..Default::default()
        },
    );

    assert!(payload.get("tool_choice").is_none());
    assert!(payload.get("tools").is_some());
    assert_eq!(payload["model"], "qwen/qwen3.7-max");
}

#[test]
fn openrouter_user_facing_models_are_mapped_to_router_ids() {
    let payload = build_chat_payload(
        "openrouter",
        "deepseek-v4-pro",
        &[json!({"role": "user", "content": "hi"})],
        &CallOptions::default(),
    );

    assert_eq!(payload["model"], "deepseek/deepseek-v4-pro");

    let legacy_payload = build_chat_payload(
        "openrouter",
        "qwen/qwen3.6-flash",
        &[json!({"role": "user", "content": "hi"})],
        &CallOptions::default(),
    );

    assert_eq!(legacy_payload["model"], "qwen/qwen3.6-flash");
}
