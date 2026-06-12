use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use tura_llm_rust::{
    extract_response_text, extract_tool_calls, normalize_command_run_tool_input,
    openai_compatible_usage_stream_supported, prompt_cache_key_supported,
    provider_latency_timeouts, provider_media_fallback, provider_unsupported_content_type,
    replace_unsupported_content_type_in_messages, strip_thought_blocks, CallOptions,
    ProviderConfig, ProviderLatencyTimeouts, ProviderMediaFallback, ProviderStreamEvent,
    RouteConfig, Settings, TuraConfig, TuraError,
};

#[derive(Debug)]
struct CapturedHttpRequest {
    method: String,
    path: String,
    headers: String,
    body: Value,
}

#[tokio::test]
async fn openai_compatible_business_flow_streams_local_tool_response_and_metrics() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind local provider");
    let addr = listener.local_addr().expect("local provider addr");
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept provider request");
        let request = read_http_request(&mut stream);

        let first = json!({
            "choices": [{
                "delta": {"content": "working "}
            }]
        });
        let tool_start = json!({
            "choices": [{
                "delta": {
                    "tool_calls": [{
                        "index": 0,
                        "id": "call_local_1",
                        "type": "function",
                        "function": {
                            "name": "command_run",
                            "arguments": "{\"commands\":[{\"step\":1,\"command\":\"pwd\""
                        }
                    }]
                }
            }]
        });
        let tool_end = json!({
            "choices": [{
                "delta": {
                    "tool_calls": [{
                        "index": 0,
                        "function": {
                            "arguments": ",\"command_line\":\"pwd\"}]}"
                        }
                    }]
                },
                "finish_reason": "tool_calls"
            }]
        });
        let usage = json!({
            "choices": [],
            "usage": {
                "prompt_tokens": 42,
                "completion_tokens": 5,
                "total_tokens": 47,
                "prompt_tokens_details": {"cached_tokens": 7}
            }
        });
        let body =
            format!("data: {first}\n\ndata: {tool_start}\n\ndata: {tool_end}\n\ndata: {usage}\n\ndata: [DONE]\n\n");
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\n\r\n{}",
            body.len(),
            body
        );
        stream
            .write_all(response.as_bytes())
            .expect("write provider response");
        request
    });

    let previous_key = std::env::var_os("LOCALTEST_API_KEY");
    std::env::set_var("LOCALTEST_API_KEY", "dummy-local-key");
    let config = ProviderConfig {
        provider: "localtest".to_string(),
        base_url: format!("http://{addr}"),
        model: "local-model".to_string(),
        temperature: 0.2,
    };
    let conf = TuraConfig::new(".env.provider-business-missing");
    let events = Arc::new(Mutex::new(Vec::<String>::new()));
    let sink_events = Arc::clone(&events);
    let sink = Arc::new(move |event: ProviderStreamEvent| {
        if let ProviderStreamEvent::TextDelta { text } = event {
            sink_events.lock().expect("events lock").push(text);
        }
    });

    let result = config
        .call_with_stream_events(
            &conf,
            vec![json!({"role": "user", "content": "run pwd locally"})],
            CallOptions {
                stream: Some(true),
                stream_options: Some(json!({"include_usage": true})),
                context_window: Some(100),
                tools: Some(vec![json!({
                    "type": "function",
                    "function": {
                        "name": "command_run",
                        "parameters": {"type": "object"}
                    }
                })]),
                tool_choice: Some(json!("auto")),
                ..CallOptions::default()
            },
            Some(sink),
        )
        .await;

    match previous_key {
        Some(value) => std::env::set_var("LOCALTEST_API_KEY", value),
        None => std::env::remove_var("LOCALTEST_API_KEY"),
    }

    let response = result.expect("local provider call");
    let captured = server.join().expect("server thread joins");

    assert_eq!(captured.method, "POST");
    assert_eq!(captured.path, "/chat/completions");
    assert!(
        captured
            .headers
            .contains("authorization: bearer dummy-local-key"),
        "provider key should come from LOCALTEST_API_KEY"
    );
    assert_eq!(captured.body["model"], "local-model");
    assert_eq!(captured.body["stream"], true);
    assert_eq!(captured.body["stream_options"]["include_usage"], true);
    assert_eq!(captured.body["messages"][0]["role"], "user");
    assert_eq!(captured.body["tools"][0]["function"]["name"], "command_run");
    assert_eq!(captured.body["tool_choice"], "auto");

    assert_eq!(
        events.lock().expect("events lock").as_slice(),
        &["working ".to_string()]
    );
    assert_eq!(response.content["text"], "working ");
    assert_eq!(
        response.content["tool_calls"][0]["function"]["name"],
        "command_run"
    );
    assert_eq!(
        response.content["tool_calls"][0]["function"]["arguments"]["commands"][0]["command_line"],
        "pwd"
    );
    let metrics = response.metrics.expect("metrics");
    assert_eq!(metrics.usage.input_tokens, Some(42));
    assert_eq!(metrics.usage.output_tokens, Some(5));
    assert_eq!(metrics.usage.total_tokens, Some(47));
    assert_eq!(metrics.usage.cached_input_tokens, Some(7));
    assert_eq!(metrics.usage.context_window, Some(100));
    assert_eq!(metrics.tool_call_count, 1);
    assert_eq!(metrics.finish_reason.as_deref(), Some("tool_calls"));
    assert!(metrics.cache_hit);
}

#[tokio::test]
async fn openai_compatible_business_flow_reports_missing_local_key_without_network() {
    let previous_key = std::env::var_os("LOCALTEST_MISSING_API_KEY");
    std::env::remove_var("LOCALTEST_MISSING_API_KEY");
    let config = ProviderConfig {
        provider: "localtest_missing".to_string(),
        base_url: "http://127.0.0.1:9".to_string(),
        model: "local-model".to_string(),
        temperature: 0.2,
    };

    let err = config
        .call(
            &TuraConfig::new(".env.provider-business-missing"),
            vec![json!({"role": "user", "content": "no network"})],
            CallOptions::default(),
        )
        .await
        .expect_err("missing key should fail before any network call");

    match previous_key {
        Some(value) => std::env::set_var("LOCALTEST_MISSING_API_KEY", value),
        None => std::env::remove_var("LOCALTEST_MISSING_API_KEY"),
    }

    match err {
        TuraError::Config { message } => {
            assert!(message.contains("localtest_missing"));
        }
        other => panic!("expected config error, got {other}"),
    }
}

#[tokio::test]
async fn openai_compatible_business_flow_embeds_local_vectors_and_rejects_bad_payloads() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind local embedding provider");
    let addr = listener
        .local_addr()
        .expect("local embedding provider addr");
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept embedding request");
        let request = read_http_request(&mut stream);
        let body = json!({
            "data": [{
                "embedding": [0.125, -1.5, 42.0]
            }],
            "usage": {
                "prompt_tokens": 3,
                "total_tokens": 3
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
            .expect("write embedding response");
        request
    });

    let previous_key = std::env::var_os("LOCALEMBED_API_KEY");
    std::env::set_var("LOCALEMBED_API_KEY", "dummy-embed-key");
    let config = ProviderConfig {
        provider: "localembed".to_string(),
        base_url: format!("http://{addr}"),
        model: "text-embedding-local".to_string(),
        temperature: 0.2,
    };

    let embedding = config
        .embed(
            "Embed this local business document",
            &TuraConfig::new(".env.provider-business-missing"),
        )
        .await
        .expect("local embedding call");
    let captured = server.join().expect("embedding server thread joins");

    assert_eq!(captured.method, "POST");
    assert_eq!(captured.path, "/embeddings");
    assert!(
        captured
            .headers
            .contains("authorization: bearer dummy-embed-key"),
        "embedding request should use the provider-specific env key"
    );
    assert_eq!(captured.body["model"], "text-embedding-local");
    assert_eq!(captured.body["input"], "Embed this local business document");
    assert_eq!(embedding, vec![0.125_f32, -1.5_f32, 42.0_f32]);

    let bad_listener = TcpListener::bind("127.0.0.1:0").expect("bind bad local embedding provider");
    let bad_addr = bad_listener
        .local_addr()
        .expect("bad local embedding provider addr");
    let bad_server = thread::spawn(move || {
        let (mut stream, _) = bad_listener.accept().expect("accept bad embedding request");
        let request = read_http_request(&mut stream);
        let body = json!({
            "data": [{
                "not_embedding": ["wrong shape"]
            }]
        })
        .to_string();
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
            body.len(),
            body
        );
        stream
            .write_all(response.as_bytes())
            .expect("write bad embedding response");
        request
    });
    let bad_config = ProviderConfig {
        provider: "localembed".to_string(),
        base_url: format!("http://{bad_addr}"),
        model: "text-embedding-local".to_string(),
        temperature: 0.2,
    };
    let err = bad_config
        .embed(
            "Bad provider payload must fail",
            &TuraConfig::new(".env.provider-business-missing"),
        )
        .await
        .expect_err("missing embedding vector should fail");
    let bad_captured = bad_server
        .join()
        .expect("bad embedding server thread joins");

    match previous_key {
        Some(value) => std::env::set_var("LOCALEMBED_API_KEY", value),
        None => std::env::remove_var("LOCALEMBED_API_KEY"),
    }

    assert_eq!(bad_captured.path, "/embeddings");
    match err {
        TuraError::ProviderRequest { provider, message } => {
            assert_eq!(provider, "openai-compatible");
            assert!(message.contains("missing embedding vector"));
        }
        other => panic!("expected provider request error, got {other}"),
    }
}

#[tokio::test]
async fn openai_compatible_business_flow_preserves_native_tool_conversation_shape() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind local provider");
    let addr = listener.local_addr().expect("local provider addr");
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept provider request");
        let request = read_http_request(&mut stream);
        let body = json!({
            "id": "chatcmpl-local-tool-conversation",
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": "tool result accepted"
                },
                "finish_reason": "stop"
            }],
            "usage": {
                "prompt_tokens": 31,
                "completion_tokens": 4,
                "total_tokens": 35
            }
        })
        .to_string();
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nx-request-id: req-local-tool-shape\r\nContent-Length: {}\r\n\r\n{}",
            body.len(),
            body
        );
        stream
            .write_all(response.as_bytes())
            .expect("write provider response");
        request
    });

    let previous_key = std::env::var_os("LOCALTEST_API_KEY");
    std::env::set_var("LOCALTEST_API_KEY", "dummy-local-key");
    let config = ProviderConfig {
        provider: "localtest".to_string(),
        base_url: format!("http://{addr}"),
        model: "local-model".to_string(),
        temperature: 0.2,
    };
    let result = config
        .call(
            &TuraConfig::new(".env.provider-business-missing"),
            vec![
                json!({"role": "system", "content": "keep native roles"}),
                json!({"role": "user", "content": "call a tool"}),
                json!({
                    "role": "assistant",
                    "content": null,
                    "tool_calls": [{
                        "id": "call_keep_roles",
                        "type": "function",
                        "function": {
                            "name": "command_run",
                            "arguments": "{\"commands\":[{\"step\":1,\"command_line\":\"pwd\"}]}"
                        }
                    }]
                }),
                json!({
                    "role": "tool",
                    "tool_call_id": "call_keep_roles",
                    "content": "{\"exit_code\":0,\"stdout\":\"/tmp/workspace\"}"
                }),
            ],
            CallOptions {
                tools: Some(vec![json!({
                    "type": "function",
                    "function": {
                        "name": "command_run",
                        "parameters": {"type": "object"}
                    }
                })]),
                ..CallOptions::default()
            },
        )
        .await;

    match previous_key {
        Some(value) => std::env::set_var("LOCALTEST_API_KEY", value),
        None => std::env::remove_var("LOCALTEST_API_KEY"),
    }

    let response = result.expect("local provider call");
    let captured = server.join().expect("server thread joins");
    let messages = captured.body["messages"]
        .as_array()
        .expect("messages array");
    assert_eq!(messages.len(), 4);
    assert_eq!(messages[0]["role"], "system");
    assert_eq!(messages[1]["role"], "user");
    assert_eq!(messages[2]["role"], "assistant");
    assert_eq!(messages[2]["content"], "");
    assert_eq!(messages[2]["tool_calls"][0]["id"], "call_keep_roles");
    assert_eq!(messages[3]["role"], "tool");
    assert_eq!(messages[3]["tool_call_id"], "call_keep_roles");
    assert_eq!(captured.body["tools"][0]["function"]["name"], "command_run");
    assert_eq!(response.content.as_str(), Some("tool result accepted"));
    let metrics = response.metrics.expect("metrics");
    assert_eq!(
        metrics.provider_request_id.as_deref(),
        Some("req-local-tool-shape")
    );
    assert_eq!(metrics.usage.total_tokens, Some(35));
}

#[tokio::test]
async fn openai_compatible_business_flow_non_stream_tool_calls_extract_structured_arguments_and_metrics(
) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind local provider");
    let addr = listener.local_addr().expect("local provider addr");
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept provider request");
        let request = read_http_request(&mut stream);
        let body = json!({
            "id": "chatcmpl-local-non-stream-tools",
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": "I will inspect the workspace.",
                    "tool_calls": [{
                        "id": "call_non_stream_1",
                        "type": "function",
                        "function": {
                            "name": "command_run",
                            "arguments": "{\"commands\":[{\"step\":1,\"command_line\":\"Get-ChildItem -Name\",\"command_type\":\"shell_command\"}]}"
                        },
                        "provider_metadata": {
                            "provider_call_index": 0
                        }
                    }]
                },
                "finish_reason": "tool_calls"
            }],
            "usage": {
                "prompt_tokens": 50,
                "completion_tokens": 12,
                "total_tokens": 62,
                "prompt_tokens_details": {"cached_tokens": 11}
            }
        })
        .to_string();
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nx-request-id: req-local-non-stream-tools\r\nContent-Length: {}\r\n\r\n{}",
            body.len(),
            body
        );
        stream
            .write_all(response.as_bytes())
            .expect("write provider response");
        request
    });

    let previous_key = std::env::var_os("LOCALTEST_API_KEY");
    std::env::set_var("LOCALTEST_API_KEY", "dummy-local-key");
    let config = ProviderConfig {
        provider: "localtest".to_string(),
        base_url: format!("http://{addr}"),
        model: "local-model".to_string(),
        temperature: 0.2,
    };
    let result = config
        .call(
            &TuraConfig::new(".env.provider-business-missing"),
            vec![json!({"role": "user", "content": "inspect with a non-stream tool call"})],
            CallOptions {
                tools: Some(vec![json!({
                    "type": "function",
                    "function": {
                        "name": "command_run",
                        "parameters": {"type": "object"}
                    }
                })]),
                tool_choice: Some(json!("auto")),
                context_window: Some(512),
                ..CallOptions::default()
            },
        )
        .await;

    match previous_key {
        Some(value) => std::env::set_var("LOCALTEST_API_KEY", value),
        None => std::env::remove_var("LOCALTEST_API_KEY"),
    }

    let response = result.expect("local non-stream tool call");
    let captured = server.join().expect("server thread joins");
    assert_eq!(captured.method, "POST");
    assert_eq!(captured.path, "/chat/completions");
    assert_eq!(captured.body["stream"], Value::Null);
    assert_eq!(captured.body["tools"][0]["function"]["name"], "command_run");
    assert_eq!(captured.body["tool_choice"], "auto");

    assert_eq!(
        response.content["text"], "I will inspect the workspace.",
        "visible assistant text should be preserved beside non-stream tool calls"
    );
    assert_eq!(response.content["tool_calls"][0]["id"], "call_non_stream_1");
    let calls = extract_tool_calls(&response.content);
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].tool_name, "command_run");
    assert_eq!(
        calls[0].arguments["commands"][0]["command_line"],
        "Get-ChildItem -Name"
    );
    assert_eq!(
        calls[0].provider_metadata.as_ref().expect("metadata")["provider_call_index"],
        0
    );

    let metrics = response.metrics.expect("metrics");
    assert_eq!(
        metrics.provider_request_id.as_deref(),
        Some("req-local-non-stream-tools")
    );
    assert_eq!(metrics.tool_call_count, 1);
    assert_eq!(metrics.finish_reason.as_deref(), Some("tool_calls"));
    assert_eq!(metrics.usage.input_tokens, Some(50));
    assert_eq!(metrics.usage.output_tokens, Some(12));
    assert_eq!(metrics.usage.total_tokens, Some(62));
    assert_eq!(metrics.usage.cached_input_tokens, Some(11));
    assert_eq!(metrics.usage.context_window, Some(512));
    assert!(metrics.cache_hit);
}

#[tokio::test]
async fn openai_compatible_business_flow_forwards_documented_request_options_and_metrics() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind local provider");
    let addr = listener.local_addr().expect("local provider addr");
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept provider request");
        let request = read_http_request(&mut stream);
        let body = json!({
            "id": "chatcmpl-local-options",
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": "options accepted"
                },
                "finish_reason": "stop"
            }],
            "usage": {
                "prompt_tokens": 21,
                "completion_tokens": 3,
                "total_tokens": 24
            }
        })
        .to_string();
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nx-request-id: req-local-options\r\nContent-Length: {}\r\n\r\n{}",
            body.len(),
            body
        );
        stream
            .write_all(response.as_bytes())
            .expect("write provider response");
        request
    });

    let previous_key = std::env::var_os("LOCALTEST_API_KEY");
    std::env::set_var("LOCALTEST_API_KEY", "dummy-local-key");
    let config = ProviderConfig {
        provider: "localtest".to_string(),
        base_url: format!("http://{addr}"),
        model: "local-model".to_string(),
        temperature: 0.2,
    };
    let result = config
        .call(
            &TuraConfig::new(".env.provider-business-missing"),
            vec![json!({"role": "user", "content": "verify request options"})],
            CallOptions {
                search: true,
                temperature: Some(0.4),
                top_p: Some(0.9),
                n: Some(1),
                stop: Some(json!(["STOP"])),
                max_completion_tokens: Some(128),
                max_tokens: Some(256),
                presence_penalty: Some(0.1),
                frequency_penalty: Some(0.2),
                logit_bias: Some(json!({"42": -1})),
                logprobs: Some(true),
                top_logprobs: Some(2),
                seed: Some(1234),
                user: Some("business-user".to_string()),
                safety_identifier: Some("safe-business-user".to_string()),
                prompt_cache_key: Some("tura:business:cache".to_string()),
                reasoning_effort: Some("highest".to_string()),
                prediction: Some(json!({"type": "content", "content": "expected"})),
                modalities: Some(vec!["text".to_string()]),
                store: Some(false),
                metadata: Some(HashMap::from([(
                    "flow".to_string(),
                    "provider-options".to_string(),
                )])),
                service_tier: Some("priority".to_string()),
                verbosity: Some("low".to_string()),
                web_search_options: Some(json!({"search_context_size": "low"})),
                parallel_tool_calls: Some(false),
                extra_body: Some(json!({
                    "custom_business_flag": true,
                    "metadata": {"extra": "merged"}
                })),
                context_window: Some(4096),
                ..CallOptions::default()
            },
        )
        .await;

    match previous_key {
        Some(value) => std::env::set_var("LOCALTEST_API_KEY", value),
        None => std::env::remove_var("LOCALTEST_API_KEY"),
    }

    let response = result.expect("local provider call");
    let captured = server.join().expect("server thread joins");
    assert_eq!(captured.method, "POST");
    assert_eq!(captured.path, "/chat/completions");
    assert!(captured
        .headers
        .contains("authorization: bearer dummy-local-key"));
    assert_eq!(captured.body["model"], "local-model");
    assert_eq!(
        captured.body["messages"][0]["content"],
        "verify request options"
    );
    assert_eq!(captured.body["temperature"], 0.4);
    assert_eq!(captured.body["top_p"], 0.9);
    assert_eq!(captured.body["n"], 1);
    assert_eq!(captured.body["stop"][0], "STOP");
    assert_eq!(captured.body["max_completion_tokens"], 128);
    assert_eq!(captured.body["max_tokens"], 256);
    assert_eq!(captured.body["presence_penalty"], 0.1);
    assert_eq!(captured.body["frequency_penalty"], 0.2);
    assert_eq!(captured.body["logit_bias"]["42"], -1);
    assert_eq!(captured.body["logprobs"], true);
    assert_eq!(captured.body["top_logprobs"], 2);
    assert_eq!(captured.body["seed"], 1234);
    assert_eq!(captured.body["user"], "business-user");
    assert_eq!(captured.body["safety_identifier"], "safe-business-user");
    assert_eq!(captured.body["prompt_cache_key"], "tura:business:cache");
    assert_eq!(captured.body["reasoning_effort"], "xhigh");
    assert_eq!(captured.body["prediction"]["content"], "expected");
    assert_eq!(captured.body["modalities"][0], "text");
    assert_eq!(captured.body["store"], false);
    assert_eq!(captured.body["metadata"]["flow"], "provider-options");
    assert_eq!(captured.body["metadata"]["extra"], "merged");
    assert!(
        captured.body.get("service_tier").is_none(),
        "service_tier is reserved for OpenAI-family chat models"
    );
    assert_eq!(captured.body["verbosity"], "low");
    assert_eq!(
        captured.body["web_search_options"]["search_context_size"],
        "low"
    );
    assert_eq!(captured.body["parallel_tool_calls"], false);
    assert_eq!(captured.body["custom_business_flag"], true);
    assert!(
        captured.body["tools"]
            .as_array()
            .expect("search should add a tool")
            .iter()
            .any(|tool| tool["type"] == "web_search"),
        "search=true should add the web_search tool"
    );

    assert_eq!(response.content.as_str(), Some("options accepted"));
    let metrics = response.metrics.expect("metrics");
    assert_eq!(
        metrics.provider_request_id.as_deref(),
        Some("req-local-options")
    );
    assert_eq!(metrics.usage.input_tokens, Some(21));
    assert_eq!(metrics.usage.output_tokens, Some(3));
    assert_eq!(metrics.usage.total_tokens, Some(24));
    assert_eq!(metrics.usage.context_window, Some(4096));
    assert_eq!(metrics.finish_reason.as_deref(), Some("stop"));
}

#[tokio::test]
async fn openai_compatible_business_flow_writes_success_and_error_logs_to_configured_root() {
    let success_listener = TcpListener::bind("127.0.0.1:0").expect("bind success log provider");
    let success_addr = success_listener.local_addr().expect("success log addr");
    let success_server = thread::spawn(move || {
        let (mut stream, _) = success_listener
            .accept()
            .expect("accept success log provider request");
        let request = read_http_request(&mut stream);
        let body = json!({
            "id": "chatcmpl-log-success",
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": "logged success"
                },
                "finish_reason": "stop"
            }],
            "usage": {
                "prompt_tokens": 9,
                "completion_tokens": 3,
                "total_tokens": 12,
                "prompt_tokens_details": {"cached_tokens": 2}
            }
        })
        .to_string();
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nx-request-id: req-log-success\r\nContent-Length: {}\r\n\r\n{}",
            body.len(),
            body
        );
        stream
            .write_all(response.as_bytes())
            .expect("write success log response");
        request
    });

    let error_listener = TcpListener::bind("127.0.0.1:0").expect("bind error log provider");
    let error_addr = error_listener.local_addr().expect("error log addr");
    let error_server = thread::spawn(move || {
        let (mut stream, _) = error_listener
            .accept()
            .expect("accept error log provider request");
        let request = read_http_request(&mut stream);
        let body = json!({
            "error": {
                "type": "server_overloaded",
                "message": "logged failure body"
            }
        })
        .to_string();
        let response = format!(
            "HTTP/1.1 503 Service Unavailable\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
            body.len(),
            body
        );
        stream
            .write_all(response.as_bytes())
            .expect("write error log response");
        request
    });

    let log_root = tempfile::tempdir().expect("provider log root");
    let previous_log_path = std::env::var_os("LOG_PATH");
    let previous_key = std::env::var_os("LOCALLOG_API_KEY");
    std::env::set_var("LOG_PATH", log_root.path());
    std::env::set_var("LOCALLOG_API_KEY", "dummy-log-key");

    let success_config = ProviderConfig {
        provider: "locallog".to_string(),
        base_url: format!("http://{success_addr}"),
        model: "log-success-model".to_string(),
        temperature: 0.2,
    };
    let success = success_config
        .call(
            &TuraConfig::new(".env.provider-business-missing"),
            vec![json!({"role": "user", "content": "write a success provider log"})],
            CallOptions {
                temperature: Some(0.31),
                metadata: Some(HashMap::from([(
                    "flow".to_string(),
                    "provider-log-success".to_string(),
                )])),
                context_window: Some(777),
                ..CallOptions::default()
            },
        )
        .await
        .expect("success log provider call");

    let error_config = ProviderConfig {
        provider: "locallog".to_string(),
        base_url: format!("http://{error_addr}"),
        model: "log-error-model".to_string(),
        temperature: 0.4,
    };
    let error = error_config
        .call(
            &TuraConfig::new(".env.provider-business-missing"),
            vec![json!({"role": "user", "content": "write an error provider log"})],
            CallOptions {
                metadata: Some(HashMap::from([(
                    "flow".to_string(),
                    "provider-log-error".to_string(),
                )])),
                context_window: Some(333),
                ..CallOptions::default()
            },
        )
        .await
        .expect_err("error log provider call should fail");

    match previous_log_path {
        Some(value) => std::env::set_var("LOG_PATH", value),
        None => std::env::remove_var("LOG_PATH"),
    }
    match previous_key {
        Some(value) => std::env::set_var("LOCALLOG_API_KEY", value),
        None => std::env::remove_var("LOCALLOG_API_KEY"),
    }

    let success_request = success_server.join().expect("success log server joins");
    let error_request = error_server.join().expect("error log server joins");
    assert_eq!(
        success_request.body["messages"][0]["content"],
        "write a success provider log"
    );
    assert_eq!(
        error_request.body["messages"][0]["content"],
        "write an error provider log"
    );
    assert!(success_request
        .headers
        .contains("authorization: bearer dummy-log-key"));
    assert!(error_request
        .headers
        .contains("authorization: bearer dummy-log-key"));
    assert_eq!(success.content.as_str(), Some("logged success"));
    match error {
        TuraError::HttpStatus { status, body } => {
            assert_eq!(status, 503);
            assert!(body.contains("logged failure body"));
        }
        other => panic!("expected logged HTTP status error, got {other}"),
    }

    let mut logs = read_llm_logs(log_root.path());
    logs.sort_by(|left, right| left["model"].as_str().cmp(&right["model"].as_str()));
    assert_eq!(logs.len(), 2, "expected one success log and one error log");

    let error_log = logs
        .iter()
        .find(|log| log["model"] == "log-error-model")
        .expect("error log");
    assert_eq!(error_log["type"], "llm_call");
    assert_eq!(error_log["success"], false);
    assert_eq!(error_log["provider"], "locallog");
    assert_eq!(error_log["base_url"], format!("http://{error_addr}"));
    assert_eq!(
        error_log["request"]["messages"][0]["content"],
        "write an error provider log"
    );
    assert_eq!(
        error_log["request"]["params"]["metadata"]["flow"],
        "provider-log-error"
    );
    assert_eq!(error_log["request"]["params"]["context_window"], 333);
    assert!(error_log.get("response").is_none());
    assert!(error_log
        .get("error")
        .and_then(Value::as_str)
        .is_some_and(|message| {
            message.contains("http status 503") && message.contains("logged failure body")
        }));
    assert!(error_log["call_id"]
        .as_str()
        .is_some_and(|call_id| call_id.len() >= 16));

    let success_log = logs
        .iter()
        .find(|log| log["model"] == "log-success-model")
        .expect("success log");
    assert_eq!(success_log["type"], "llm_call");
    assert_eq!(success_log["success"], true);
    assert_eq!(success_log["provider"], "locallog");
    assert_eq!(success_log["base_url"], format!("http://{success_addr}"));
    assert_eq!(
        success_log["request"]["messages"][0]["content"],
        "write a success provider log"
    );
    assert_eq!(
        success_log["request"]["params"]["metadata"]["flow"],
        "provider-log-success"
    );
    assert_eq!(success_log["request"]["params"]["temperature"], 0.31);
    assert_eq!(success_log["response"]["id"], "chatcmpl-log-success");
    assert_eq!(
        success_log["metrics"]["provider_request_id"],
        "req-log-success"
    );
    assert_eq!(success_log["metrics"]["usage"]["input_tokens"], 9);
    assert_eq!(success_log["metrics"]["usage"]["output_tokens"], 3);
    assert_eq!(success_log["metrics"]["usage"]["total_tokens"], 12);
    assert_eq!(success_log["metrics"]["usage"]["cached_input_tokens"], 2);
    assert_eq!(success_log["metrics"]["usage"]["context_window"], 777);
}

#[tokio::test]
async fn openai_compatible_business_flow_concurrent_calls_keep_request_and_response_isolated() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind concurrent local provider");
    let addr = listener.local_addr().expect("concurrent provider addr");
    let server = thread::spawn(move || {
        let mut captured = Vec::new();
        for _ in 0..2 {
            let (mut stream, _) = listener.accept().expect("accept provider request");
            let request = read_http_request(&mut stream);
            let prompt = request.body["messages"][0]["content"]
                .as_str()
                .unwrap_or_default()
                .to_string();
            let label = prompt
                .strip_prefix("concurrent ")
                .unwrap_or("unknown")
                .to_string();
            let body = json!({
                "id": format!("chatcmpl-{label}"),
                "choices": [{
                    "message": {
                        "role": "assistant",
                        "content": format!("response for {label}")
                    },
                    "finish_reason": "stop"
                }],
                "usage": {
                    "prompt_tokens": if label == "alpha" { 11 } else { 13 },
                    "completion_tokens": 2,
                    "total_tokens": if label == "alpha" { 13 } else { 15 }
                }
            })
            .to_string();
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nx-request-id: req-{label}\r\nContent-Length: {}\r\n\r\n{}",
                body.len(),
                body
            );
            stream
                .write_all(response.as_bytes())
                .expect("write provider response");
            captured.push(request);
        }
        captured
    });

    let previous_key = std::env::var_os("LOCALCONCURRENT_API_KEY");
    std::env::set_var("LOCALCONCURRENT_API_KEY", "dummy-concurrent-key");
    let config = ProviderConfig {
        provider: "localconcurrent".to_string(),
        base_url: format!("http://{addr}"),
        model: "local-concurrent-model".to_string(),
        temperature: 0.2,
    };

    let alpha_config = config.clone();
    let beta_config = config;
    let alpha_conf = TuraConfig::new(".env.provider-business-missing");
    let beta_conf = TuraConfig::new(".env.provider-business-missing");
    let (alpha, beta) = tokio::join!(
        alpha_config.call(
            &alpha_conf,
            vec![json!({"role": "user", "content": "concurrent alpha"})],
            CallOptions {
                metadata: Some(HashMap::from([("flow".to_string(), "alpha".to_string())])),
                context_window: Some(101),
                ..CallOptions::default()
            },
        ),
        beta_config.call(
            &beta_conf,
            vec![json!({"role": "user", "content": "concurrent beta"})],
            CallOptions {
                metadata: Some(HashMap::from([("flow".to_string(), "beta".to_string())])),
                context_window: Some(202),
                ..CallOptions::default()
            },
        )
    );

    match previous_key {
        Some(value) => std::env::set_var("LOCALCONCURRENT_API_KEY", value),
        None => std::env::remove_var("LOCALCONCURRENT_API_KEY"),
    }

    let alpha = alpha.expect("alpha concurrent provider call");
    let beta = beta.expect("beta concurrent provider call");
    let mut captured = server.join().expect("server thread joins");
    captured.sort_by(|left, right| {
        left.body["messages"][0]["content"]
            .as_str()
            .cmp(&right.body["messages"][0]["content"].as_str())
    });

    assert_eq!(captured.len(), 2);
    assert!(captured.iter().all(|request| {
        request.method == "POST"
            && request.path == "/chat/completions"
            && request
                .headers
                .contains("authorization: bearer dummy-concurrent-key")
            && request.body["model"] == "local-concurrent-model"
    }));
    assert_eq!(
        captured[0].body["messages"][0]["content"],
        "concurrent alpha"
    );
    assert_eq!(captured[0].body["metadata"]["flow"], "alpha");
    assert_eq!(
        captured[1].body["messages"][0]["content"],
        "concurrent beta"
    );
    assert_eq!(captured[1].body["metadata"]["flow"], "beta");

    assert_eq!(alpha.content.as_str(), Some("response for alpha"));
    let alpha_metrics = alpha.metrics.expect("alpha metrics");
    assert_eq!(
        alpha_metrics.provider_request_id.as_deref(),
        Some("req-alpha")
    );
    assert_eq!(alpha_metrics.usage.context_window, Some(101));
    assert_eq!(alpha_metrics.usage.total_tokens, Some(13));

    assert_eq!(beta.content.as_str(), Some("response for beta"));
    let beta_metrics = beta.metrics.expect("beta metrics");
    assert_eq!(
        beta_metrics.provider_request_id.as_deref(),
        Some("req-beta")
    );
    assert_eq!(beta_metrics.usage.context_window, Some(202));
    assert_eq!(beta_metrics.usage.total_tokens, Some(15));
}

#[tokio::test]
async fn openai_compatible_route_business_flow_falls_back_after_provider_error() {
    let failing_listener = TcpListener::bind("127.0.0.1:0").expect("bind failing provider");
    let failing_addr = failing_listener.local_addr().expect("failing addr");
    let failing_server = thread::spawn(move || {
        let (mut stream, _) = failing_listener
            .accept()
            .expect("accept failing provider request");
        let request = read_http_request(&mut stream);
        let body = json!({
            "error": {
                "type": "rate_limit_exceeded",
                "message": "first local route provider is unavailable"
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
            .expect("write failing provider response");
        request
    });

    let fallback_listener = TcpListener::bind("127.0.0.1:0").expect("bind fallback provider");
    let fallback_addr = fallback_listener.local_addr().expect("fallback addr");
    let fallback_server = thread::spawn(move || {
        let (mut stream, _) = fallback_listener
            .accept()
            .expect("accept fallback provider request");
        let request = read_http_request(&mut stream);
        let body = json!({
            "id": "chatcmpl-route-fallback",
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": "fallback route provider handled the request"
                },
                "finish_reason": "stop"
            }],
            "usage": {
                "prompt_tokens": 13,
                "completion_tokens": 7,
                "total_tokens": 20
            }
        })
        .to_string();
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nx-request-id: req-route-fallback\r\nContent-Length: {}\r\n\r\n{}",
            body.len(),
            body
        );
        stream
            .write_all(response.as_bytes())
            .expect("write fallback provider response");
        request
    });

    let previous_key = std::env::var_os("LOCALTEST_API_KEY");
    std::env::set_var("LOCALTEST_API_KEY", "dummy-local-key");
    let route = RouteConfig {
        default_temperature: 0.55,
        providers: vec![
            ProviderConfig {
                provider: "localtest".to_string(),
                base_url: format!("http://{failing_addr}"),
                model: "route-failing-model".to_string(),
                temperature: 0.1,
            },
            ProviderConfig {
                provider: "localtest".to_string(),
                base_url: format!("http://{fallback_addr}"),
                model: "route-fallback-model".to_string(),
                temperature: 0.55,
            },
        ],
    };
    let result = route
        .run(
            &TuraConfig::new(".env.provider-business-missing"),
            vec![json!({"role": "user", "content": "route should fallback locally"})],
            CallOptions {
                temperature: None,
                context_window: Some(2048),
                metadata: Some(HashMap::from([(
                    "flow".to_string(),
                    "route-fallback".to_string(),
                )])),
                ..CallOptions::default()
            },
        )
        .await;

    match previous_key {
        Some(value) => std::env::set_var("LOCALTEST_API_KEY", value),
        None => std::env::remove_var("LOCALTEST_API_KEY"),
    }

    let response = result.expect("route should fall back to the second local provider");
    let first = failing_server.join().expect("failing server joins");
    let second = fallback_server.join().expect("fallback server joins");

    assert_eq!(first.method, "POST");
    assert_eq!(first.path, "/chat/completions");
    assert_eq!(first.body["model"], "route-failing-model");
    assert_eq!(
        first.body["messages"][0]["content"],
        "route should fallback locally"
    );
    assert_eq!(
        first.body["metadata"]["flow"], "route-fallback",
        "route options should be preserved on the failed attempt"
    );

    assert_eq!(second.method, "POST");
    assert_eq!(second.path, "/chat/completions");
    assert_eq!(second.body["model"], "route-fallback-model");
    assert_eq!(
        second.body["messages"][0]["content"],
        "route should fallback locally"
    );
    assert_eq!(second.body["metadata"]["flow"], "route-fallback");
    assert_eq!(second.body["temperature"], 0.55);

    assert_eq!(
        response.content.as_str(),
        Some("fallback route provider handled the request")
    );
    let metrics = response.metrics.expect("metrics");
    assert_eq!(
        metrics.provider_request_id.as_deref(),
        Some("req-route-fallback")
    );
    assert_eq!(metrics.usage.input_tokens, Some(13));
    assert_eq!(metrics.usage.output_tokens, Some(7));
    assert_eq!(metrics.usage.total_tokens, Some(20));
    assert_eq!(metrics.usage.context_window, Some(2048));
    assert_eq!(metrics.finish_reason.as_deref(), Some("stop"));
}

#[tokio::test]
async fn openai_compatible_route_business_flow_uses_first_healthy_provider_without_touching_fallback(
) {
    let primary_listener = TcpListener::bind("127.0.0.1:0").expect("bind primary provider");
    let primary_addr = primary_listener.local_addr().expect("primary addr");
    let primary_server = thread::spawn(move || {
        let (mut stream, _) = primary_listener
            .accept()
            .expect("accept primary provider request");
        let request = read_http_request(&mut stream);
        let body = json!({
            "id": "chatcmpl-route-primary",
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": "primary route provider handled the request"
                },
                "finish_reason": "stop"
            }],
            "usage": {
                "prompt_tokens": 17,
                "completion_tokens": 6,
                "total_tokens": 23
            }
        })
        .to_string();
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nx-request-id: req-route-primary\r\nContent-Length: {}\r\n\r\n{}",
            body.len(),
            body
        );
        stream
            .write_all(response.as_bytes())
            .expect("write primary provider response");
        request
    });

    let fallback_listener = TcpListener::bind("127.0.0.1:0").expect("bind unused fallback");
    let fallback_addr = fallback_listener.local_addr().expect("fallback addr");
    let fallback_server =
        thread::spawn(move || accept_optional_provider_request(fallback_listener, 400));

    let previous_key = std::env::var_os("LOCALTEST_API_KEY");
    std::env::set_var("LOCALTEST_API_KEY", "dummy-local-key");
    let route = RouteConfig {
        default_temperature: 0.42,
        providers: vec![
            ProviderConfig {
                provider: "localtest".to_string(),
                base_url: format!("http://{primary_addr}"),
                model: "route-primary-model".to_string(),
                temperature: 0.33,
            },
            ProviderConfig {
                provider: "localtest".to_string(),
                base_url: format!("http://{fallback_addr}"),
                model: "route-unused-fallback-model".to_string(),
                temperature: 0.77,
            },
        ],
    };
    let result = route
        .run(
            &TuraConfig::new(".env.provider-business-missing"),
            vec![json!({"role": "user", "content": "route should use primary locally"})],
            CallOptions {
                temperature: None,
                context_window: Some(3072),
                metadata: Some(HashMap::from([(
                    "flow".to_string(),
                    "route-primary".to_string(),
                )])),
                ..CallOptions::default()
            },
        )
        .await;

    match previous_key {
        Some(value) => std::env::set_var("LOCALTEST_API_KEY", value),
        None => std::env::remove_var("LOCALTEST_API_KEY"),
    }

    let response = result.expect("route should use first healthy local provider");
    let primary = primary_server.join().expect("primary server joins");
    let fallback = fallback_server.join().expect("fallback server joins");

    assert_eq!(primary.method, "POST");
    assert_eq!(primary.path, "/chat/completions");
    assert_eq!(primary.body["model"], "route-primary-model");
    assert_eq!(
        primary.body["messages"][0]["content"],
        "route should use primary locally"
    );
    assert_eq!(primary.body["metadata"]["flow"], "route-primary");
    assert_eq!(
        primary.body["temperature"], 0.33,
        "route should apply the selected provider temperature when call options omit one"
    );
    assert!(
        fallback.is_none(),
        "fallback provider must not be contacted after the primary route succeeds"
    );

    assert_eq!(
        response.content.as_str(),
        Some("primary route provider handled the request")
    );
    let metrics = response.metrics.expect("metrics");
    assert_eq!(
        metrics.provider_request_id.as_deref(),
        Some("req-route-primary")
    );
    assert_eq!(metrics.usage.input_tokens, Some(17));
    assert_eq!(metrics.usage.output_tokens, Some(6));
    assert_eq!(metrics.usage.total_tokens, Some(23));
    assert_eq!(metrics.usage.context_window, Some(3072));
    assert_eq!(metrics.finish_reason.as_deref(), Some("stop"));
}

#[tokio::test]
async fn openai_compatible_route_business_flow_reports_all_provider_failures_with_attempt_context()
{
    let http_error_listener = TcpListener::bind("127.0.0.1:0").expect("bind http-error provider");
    let http_error_addr = http_error_listener.local_addr().expect("http-error addr");
    let http_error_server = thread::spawn(move || {
        let (mut stream, _) = http_error_listener
            .accept()
            .expect("accept http-error provider request");
        let request = read_http_request(&mut stream);
        let body = json!({
            "error": {
                "type": "server_overloaded",
                "message": "first provider failed locally"
            }
        })
        .to_string();
        let response = format!(
            "HTTP/1.1 503 Service Unavailable\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
            body.len(),
            body
        );
        stream
            .write_all(response.as_bytes())
            .expect("write http-error provider response");
        request
    });

    let invalid_json_listener =
        TcpListener::bind("127.0.0.1:0").expect("bind invalid-json provider");
    let invalid_json_addr = invalid_json_listener
        .local_addr()
        .expect("invalid-json addr");
    let invalid_json_server = thread::spawn(move || {
        let (mut stream, _) = invalid_json_listener
            .accept()
            .expect("accept invalid-json provider request");
        let request = read_http_request(&mut stream);
        let body = r#"{"choices":[{"message":{"content":"broken"}]"#;
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
            body.len(),
            body
        );
        stream
            .write_all(response.as_bytes())
            .expect("write invalid-json provider response");
        request
    });

    let previous_key = std::env::var_os("LOCALTEST_API_KEY");
    std::env::set_var("LOCALTEST_API_KEY", "dummy-local-key");
    let route = RouteConfig {
        default_temperature: 0.25,
        providers: vec![
            ProviderConfig {
                provider: "localtest".to_string(),
                base_url: format!("http://{http_error_addr}"),
                model: "route-http-error-model".to_string(),
                temperature: 0.25,
            },
            ProviderConfig {
                provider: "localtest".to_string(),
                base_url: format!("http://{invalid_json_addr}"),
                model: "route-invalid-json-model".to_string(),
                temperature: 0.5,
            },
        ],
    };
    let err = route
        .run(
            &TuraConfig::new(".env.provider-business-missing"),
            vec![json!({"role": "user", "content": "every route provider fails locally"})],
            CallOptions {
                temperature: None,
                context_window: Some(1024),
                metadata: Some(HashMap::from([(
                    "flow".to_string(),
                    "route-all-fail".to_string(),
                )])),
                ..CallOptions::default()
            },
        )
        .await
        .expect_err("all failing route providers should surface aggregate failure");

    match previous_key {
        Some(value) => std::env::set_var("LOCALTEST_API_KEY", value),
        None => std::env::remove_var("LOCALTEST_API_KEY"),
    }

    let first = http_error_server.join().expect("http-error server joins");
    let second = invalid_json_server
        .join()
        .expect("invalid-json server joins");
    for captured in [&first, &second] {
        assert_eq!(captured.method, "POST");
        assert_eq!(captured.path, "/chat/completions");
        assert_eq!(
            captured.body["messages"][0]["content"],
            "every route provider fails locally"
        );
        assert_eq!(captured.body["metadata"]["flow"], "route-all-fail");
    }
    assert_eq!(first.body["model"], "route-http-error-model");
    assert_eq!(first.body["temperature"], 0.25);
    assert_eq!(second.body["model"], "route-invalid-json-model");
    assert_eq!(second.body["temperature"], 0.5);

    match err {
        TuraError::AllProvidersFailed { message } => {
            assert!(
                message.contains("localtest:route-http-error-model")
                    && message.contains("http status 503")
                    && message.contains("server_overloaded"),
                "aggregate route error should include first provider status/body: {message}"
            );
            assert!(
                message.contains("localtest:route-invalid-json-model")
                    && (message.contains("decoding response body")
                        || message.contains("EOF")
                        || message.contains("expected")),
                "aggregate route error should include second provider decode context: {message}"
            );
        }
        other => panic!("expected all-providers failure, got {other}"),
    }
}

#[tokio::test]
async fn openai_compatible_route_business_flow_loads_named_route_from_provider_config() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind configured provider");
    let addr = listener.local_addr().expect("configured provider addr");
    let server = thread::spawn(move || {
        let (mut stream, _) = listener
            .accept()
            .expect("accept configured provider request");
        let request = read_http_request(&mut stream);
        let body = json!({
            "id": "chatcmpl-configured-route",
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": "configured route handled the local request"
                },
                "finish_reason": "stop"
            }],
            "usage": {
                "prompt_tokens": 19,
                "completion_tokens": 8,
                "total_tokens": 27
            }
        })
        .to_string();
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nx-request-id: req-configured-route\r\nContent-Length: {}\r\n\r\n{}",
            body.len(),
            body
        );
        stream
            .write_all(response.as_bytes())
            .expect("write configured provider response");
        request
    });

    let temp = tempfile::tempdir().expect("provider config tempdir");
    let config_path = temp.path().join("provider_config.json");
    std::fs::write(
        &config_path,
        json!({
            "provider_base_url": {
                "localtest": format!("http://{addr}")
            },
            "routes": {
                "business_configured_route": {
                    "default_temperature": 0.61,
                    "providers": [
                        {
                            "provider": "localtest",
                            "model": "localtest/configured-route-model"
                        }
                    ]
                }
            }
        })
        .to_string(),
    )
    .expect("write provider config");

    let previous_config = std::env::var_os("TURA_PROVIDER_CONFIG");
    let previous_key = std::env::var_os("LOCALTEST_API_KEY");
    std::env::set_var("TURA_PROVIDER_CONFIG", &config_path);
    std::env::set_var("LOCALTEST_API_KEY", "dummy-configured-key");

    let settings = Settings::default()
        .await
        .expect("load explicit provider config");
    let route = settings
        .route_by_name("business_configured_route")
        .expect("configured business route exists");
    let result = route
        .run(
            &TuraConfig::new(".env.provider-business-missing"),
            vec![json!({"role": "user", "content": "use configured route locally"})],
            CallOptions {
                temperature: None,
                context_window: Some(4096),
                metadata: Some(HashMap::from([(
                    "flow".to_string(),
                    "configured-route".to_string(),
                )])),
                ..CallOptions::default()
            },
        )
        .await;

    match previous_config {
        Some(value) => std::env::set_var("TURA_PROVIDER_CONFIG", value),
        None => std::env::remove_var("TURA_PROVIDER_CONFIG"),
    }
    match previous_key {
        Some(value) => std::env::set_var("LOCALTEST_API_KEY", value),
        None => std::env::remove_var("LOCALTEST_API_KEY"),
    }

    let response = result.expect("configured route should call local provider");
    let captured = server.join().expect("configured server joins");

    assert_eq!(captured.method, "POST");
    assert_eq!(captured.path, "/chat/completions");
    assert!(
        captured
            .headers
            .contains("authorization: bearer dummy-configured-key"),
        "configured route request should use provider-specific auth from env"
    );
    assert_eq!(captured.body["model"], "configured-route-model");
    assert_eq!(
        captured.body["messages"][0]["content"],
        "use configured route locally"
    );
    assert_eq!(captured.body["metadata"]["flow"], "configured-route");
    assert_eq!(
        captured.body["temperature"], 0.61,
        "route should apply configured default temperature when provider omits one"
    );
    assert_eq!(
        response.content.as_str(),
        Some("configured route handled the local request")
    );
    let metrics = response.metrics.expect("metrics");
    assert_eq!(
        metrics.provider_request_id.as_deref(),
        Some("req-configured-route")
    );
    assert_eq!(metrics.usage.input_tokens, Some(19));
    assert_eq!(metrics.usage.output_tokens, Some(8));
    assert_eq!(metrics.usage.total_tokens, Some(27));
    assert_eq!(metrics.usage.context_window, Some(4096));
    assert_eq!(metrics.finish_reason.as_deref(), Some("stop"));
}

#[tokio::test]
async fn openai_compatible_settings_business_flow_loads_explicit_config_catalog_and_latency() {
    let temp = tempfile::tempdir().expect("settings config tempdir");
    let preferred_config = temp.path().join("preferred_provider_config.json");
    std::fs::write(
        &preferred_config,
        json!({
            "provider_base_url": {
                "localalpha": "http://127.0.0.1:1111/v1",
                "localbeta": "http://127.0.0.1:2222/v1"
            },
            "provider_enums": {
                "domains": ["local", "business"],
                "capabilities": ["chat", "tool_call", "embedding"],
                "api_styles": ["openai-compatible"],
                "auth_methods": ["env_token", "oauth"],
                "statuses": ["available", "degraded"]
            },
            "provider_auth": {
                "localalpha": {
                    "type": "api_key",
                    "status": "available",
                    "provider": "localalpha",
                    "token_env": "LOCALALPHA_API_KEY"
                }
            },
            "provider_latency": {
                "active": "business-fast",
                "levels": {
                    "business-fast": {
                        "idle_output_timeout_ms": 1234,
                        "first_output_timeout_ms": 2345,
                        "total_timeout_ms": 3456
                    },
                    "x-high": {
                        "idle_output_timeout_ms": 9000,
                        "first_output_timeout_ms": 19000,
                        "total_timeout_ms": 29000
                    },
                    "high": {
                        "idle_output_timeout_ms": 8000,
                        "first_output_timeout_ms": 18000,
                        "total_timeout_ms": 28000
                    },
                    "fast": {
                        "idle_output_timeout_ms": 7000,
                        "first_output_timeout_ms": 17000,
                        "total_timeout_ms": 27000
                    }
                }
            },
            "model_catalog": {
                "tiers": ["fast", "thinking"],
                "providers": {
                    "localalpha": {
                        "display_name": "Local Alpha",
                        "runtime_provider": "openai-compatible",
                        "api_style": "openai-compatible",
                        "base_url": "http://127.0.0.1:1111/v1",
                        "token_env": "LOCALALPHA_API_KEY",
                        "env": ["LOCALALPHA_API_KEY"],
                        "domains": ["local"],
                        "capabilities": ["chat", "tool_call"],
                        "auth_methods": ["env_token"],
                        "status": "available",
                        "models": {
                            "fast": [
                                "localalpha/alpha-fast",
                                {
                                    "id": "alpha-reasoning",
                                    "name": "Alpha Reasoning",
                                    "family": "alpha",
                                    "release_date": "2026-01-01",
                                    "attachment": true,
                                    "reasoning": true,
                                    "temperature": false,
                                    "tool_call": true,
                                    "limit": {
                                        "context": 64000,
                                        "input": 32000,
                                        "output": 4096
                                    },
                                    "modalities": {
                                        "input": ["text", "image"],
                                        "output": ["text"]
                                    },
                                    "options": {
                                        "tier": "business"
                                    },
                                    "status": "available"
                                }
                            ]
                        }
                    }
                }
            },
            "routes": {
                "business_fast": {
                    "default_temperature": 0.42,
                    "providers": [
                        {
                            "provider": "localalpha",
                            "model": "localalpha/alpha-fast"
                        },
                        {
                            "provider": "localbeta",
                            "model": "localbeta/beta-fast",
                            "temperature": 0.77
                        }
                    ]
                },
                "business_thinking": {
                    "default_temperature": 0.12,
                    "providers": [
                        {
                            "provider": "localalpha",
                            "model": "alpha-reasoning"
                        }
                    ]
                }
            }
        })
        .to_string(),
    )
    .expect("write preferred provider config");

    let previous_provider_config = std::env::var_os("TURA_PROVIDER_CONFIG");
    std::env::set_var("TURA_PROVIDER_CONFIG", &preferred_config);

    let settings = Settings::default()
        .await
        .expect("explicit provider config should load");

    match previous_provider_config {
        Some(value) => std::env::set_var("TURA_PROVIDER_CONFIG", value),
        None => std::env::remove_var("TURA_PROVIDER_CONFIG"),
    }

    assert_eq!(settings.routes.len(), 2);
    assert_eq!(
        settings.provider_base_url("localalpha").as_deref(),
        Some("http://127.0.0.1:1111/v1")
    );
    assert_eq!(
        settings.provider_base_url("localbeta").as_deref(),
        Some("http://127.0.0.1:2222/v1")
    );
    assert_eq!(settings.provider_base_url("missing"), None);

    let fast = settings
        .route_by_name("business_fast")
        .expect("business fast route");
    assert_eq!(fast.default_temperature, 0.42);
    assert_eq!(fast.providers.len(), 2);
    assert_eq!(fast.providers[0].provider, "localalpha");
    assert_eq!(fast.providers[0].base_url, "http://127.0.0.1:1111/v1");
    assert_eq!(fast.providers[0].model, "alpha-fast");
    assert_eq!(fast.providers[0].temperature, 0.42);
    assert_eq!(fast.providers[1].provider, "localbeta");
    assert_eq!(fast.providers[1].base_url, "http://127.0.0.1:2222/v1");
    assert_eq!(fast.providers[1].model, "beta-fast");
    assert_eq!(fast.providers[1].temperature, 0.77);

    let thinking = settings
        .route_by_name("business_thinking")
        .expect("business thinking route");
    assert_eq!(thinking.default_temperature, 0.12);
    assert_eq!(thinking.providers[0].model, "alpha-reasoning");

    assert_eq!(settings.model_catalog.tiers, vec!["fast", "thinking"]);
    let alpha_catalog = settings
        .model_catalog
        .providers
        .get("localalpha")
        .expect("localalpha catalog");
    assert_eq!(alpha_catalog.display_name, "Local Alpha");
    assert_eq!(alpha_catalog.runtime_provider, "openai-compatible");
    assert_eq!(
        alpha_catalog.token_env.as_deref(),
        Some("LOCALALPHA_API_KEY")
    );
    assert_eq!(alpha_catalog.domains, vec!["local"]);
    assert_eq!(alpha_catalog.capabilities, vec!["chat", "tool_call"]);
    assert_eq!(alpha_catalog.auth_methods, vec!["env_token"]);
    let detailed = alpha_catalog.models["fast"][1]
        .detail()
        .expect("detailed catalog model");
    assert_eq!(detailed.id, "alpha-reasoning");
    assert!(detailed.attachment);
    assert!(detailed.reasoning);
    assert!(!detailed.temperature);
    assert!(detailed.tool_call);
    assert_eq!(detailed.limit.context, 64_000);
    assert_eq!(detailed.limit.output, 4_096);
    assert_eq!(detailed.modalities.input, vec!["text", "image"]);
    assert_eq!(detailed.options["tier"], "business");
    assert_eq!(detailed.status.as_deref(), Some("available"));

    let configured_catalog = settings.configured_model_catalog();
    assert_eq!(
        configured_catalog.get("localalpha"),
        Some(&vec![
            "alpha-fast".to_string(),
            "alpha-reasoning".to_string()
        ])
    );
    assert_eq!(
        configured_catalog.get("localbeta"),
        Some(&vec!["beta-fast".to_string()])
    );

    assert_eq!(settings.provider_enums.domains, vec!["local", "business"]);
    assert_eq!(
        settings.provider_enums.capabilities,
        vec!["chat", "tool_call", "embedding"]
    );
    assert_eq!(
        settings.provider_enums.api_styles,
        vec!["openai-compatible"]
    );
    assert_eq!(
        settings.provider_enums.auth_methods,
        vec!["env_token", "oauth"]
    );
    assert_eq!(
        provider_latency_timeouts(),
        ProviderLatencyTimeouts {
            idle_output_timeout_ms: 1234,
            first_output_timeout_ms: 2345,
            total_timeout_ms: 3456,
        }
    );
}

#[tokio::test]
async fn openai_compatible_settings_business_flow_rejects_route_provider_missing_base_url() {
    let temp = tempfile::tempdir().expect("bad settings config tempdir");
    let config_path = temp.path().join("bad_provider_config.json");
    std::fs::write(
        &config_path,
        json!({
            "provider_base_url": {
                "known": "http://127.0.0.1:3131/v1"
            },
            "routes": {
                "bad_route": {
                    "default_temperature": 0.2,
                    "providers": [
                        {
                            "provider": "missing",
                            "model": "missing/model"
                        }
                    ]
                }
            }
        })
        .to_string(),
    )
    .expect("write bad provider config");

    let previous_provider_config = std::env::var_os("TURA_PROVIDER_CONFIG");
    std::env::set_var("TURA_PROVIDER_CONFIG", &config_path);

    let err = Settings::default()
        .await
        .expect_err("missing provider base URL should fail route construction");

    match previous_provider_config {
        Some(value) => std::env::set_var("TURA_PROVIDER_CONFIG", value),
        None => std::env::remove_var("TURA_PROVIDER_CONFIG"),
    }

    match err {
        TuraError::UnknownProvider { provider } => {
            assert_eq!(provider, "missing");
        }
        other => panic!("expected unknown provider error, got {other}"),
    }
}

#[test]
fn openai_compatible_provider_boundary_business_flow_normalizes_runtime_visible_contracts() {
    let mut messages = vec![
        json!({
            "role": "user",
            "content": [
                {"type": "input_text", "text": "Review these artifacts."},
                {
                    "type": "input_file",
                    "filename": "report.pdf",
                    "file_data": "data:application/pdf;base64,ZmFrZQ=="
                },
                {
                    "type": "input_image",
                    "image_url": "data:image/png;base64,ZmFrZQ=="
                }
            ]
        }),
        json!({
            "role": "assistant",
            "content": [{
                "type": "input_audio",
                "audio_url": "data:audio/wav;base64,ZmFrZQ=="
            }]
        }),
    ];
    let retry_file = provider_media_fallback(
        "HTTP 400: Invalid value: 'input_file'. Supported values are: input_text, input_image",
    )
    .expect("input_file should be retryable");
    assert_eq!(
        retry_file,
        ProviderMediaFallback::RetryWithoutContent {
            content_type: "input_file"
        }
    );
    assert_eq!(retry_file.content_type(), "input_file");
    assert_eq!(retry_file.retry_content_type(), Some("input_file"));
    assert_eq!(
        provider_unsupported_content_type("unsupported mime type application/pdf"),
        Some("input_file")
    );
    assert_eq!(
        replace_unsupported_content_type_in_messages(
            &mut messages,
            retry_file.retry_content_type().expect("retry type")
        ),
        1
    );
    assert_eq!(messages[0]["content"][1]["type"], "input_text");
    assert!(messages[0]["content"][1]["text"]
        .as_str()
        .expect("file fallback text")
        .contains("input_file"));
    assert_eq!(
        messages[0]["content"][2]["type"], "input_image",
        "retrying file content must not remove required image content"
    );
    assert_eq!(
        messages[1]["content"][0]["type"], "input_audio",
        "retrying file content must not remove audio content"
    );

    let required_image = provider_media_fallback(
        "OpenRouter: No endpoints found that support image input for this route",
    )
    .expect("image endpoint rejection should be detected");
    assert_eq!(
        required_image,
        ProviderMediaFallback::UnsupportedRequiredContent {
            content_type: "input_image"
        }
    );
    assert_eq!(required_image.content_type(), "input_image");
    assert_eq!(required_image.retry_content_type(), None);
    assert_eq!(
        provider_unsupported_content_type(
            "OpenRouter: No endpoints found that support audio input for this route"
        ),
        None,
        "required media failures should not be downgraded into retryable replacement"
    );

    let normalized = json!({
        "text": "<thought>private chain</thought>Visible answer.",
        "tool_calls": [{
            "id": "call_openai",
            "type": "function",
            "function": {
                "name": "command_run",
                "arguments": "{\"commands\":\"<parameter name=\\\"command_line\\\">Get-ChildItem src</parameter><parameter name=\\\"step\\\">2</parameter>\",\"command_type\":\"shell_command\"}"
            },
            "provider_metadata": {"source": "openai"}
        }],
        "parts": [
            {"text": "Ignored because top-level text wins."},
            {
                "functionCall": {
                    "name": "command_run",
                    "args": {
                        "commands": [{
                            "command": "shell_command",
                            "command_line": "cargo test -p provider"
                        }]
                    }
                },
                "thoughtSignature": "google-sig-1"
            }
        ]
    });
    assert_eq!(
        extract_response_text(&normalized).as_deref(),
        Some("<thought>private chain</thought>Visible answer.")
    );
    assert_eq!(
        strip_thought_blocks(
            extract_response_text(&normalized)
                .as_deref()
                .expect("visible text")
        ),
        "Visible answer."
    );
    let calls = extract_tool_calls(&normalized);
    assert_eq!(calls.len(), 2);
    assert_eq!(calls[0].tool_name, "command_run");
    assert_eq!(
        calls[0]
            .provider_metadata
            .as_ref()
            .expect("openai metadata")["source"],
        "openai"
    );
    let openai_input =
        normalize_command_run_tool_input(&calls[0].tool_name, calls[0].arguments.clone());
    assert_eq!(openai_input["command_type"], "shell_command");
    assert_eq!(
        openai_input["commands"][0]["command_line"],
        "Get-ChildItem src"
    );
    assert_eq!(openai_input["commands"][0]["step"], 2);
    assert_eq!(openai_input["commands"][0]["command_type"], "shell_command");
    assert_eq!(openai_input["commands"][0]["command"], "shell_command");

    assert_eq!(calls[1].tool_name, "command_run");
    assert_eq!(
        calls[1]
            .provider_metadata
            .as_ref()
            .expect("google metadata")["google_thought_signature"],
        "google-sig-1"
    );
    let google_input =
        normalize_command_run_tool_input(&calls[1].tool_name, calls[1].arguments.clone());
    assert_eq!(google_input["commands"][0]["command"], "shell_command");
    assert_eq!(
        google_input["commands"][0]["command_line"],
        "cargo test -p provider"
    );

    let plain_text_input =
        normalize_command_run_tool_input("command_run", json!("Get-Content Cargo.toml"));
    assert_eq!(
        plain_text_input["commands"][0]["command_type"],
        "shell_command"
    );
    assert_eq!(
        plain_text_input["commands"][0]["command_line"],
        "Get-Content Cargo.toml"
    );
    let passthrough = normalize_command_run_tool_input("web_discover", json!({"query": "docs"}));
    assert_eq!(passthrough["query"], "docs");

    let previous_prompt_cache = std::env::var_os("TURA_DISABLE_PROMPT_CACHE");
    std::env::remove_var("TURA_DISABLE_PROMPT_CACHE");
    assert!(prompt_cache_key_supported(
        "openai",
        "https://api.openai.com/v1"
    ));
    assert!(prompt_cache_key_supported(
        "localtest",
        "https://api.openai.com/v1"
    ));
    assert!(!prompt_cache_key_supported(
        "localtest",
        "https://example.invalid/v1"
    ));
    std::env::set_var("TURA_DISABLE_PROMPT_CACHE", "true");
    assert!(!prompt_cache_key_supported(
        "openai",
        "https://api.openai.com/v1"
    ));
    match previous_prompt_cache {
        Some(value) => std::env::set_var("TURA_DISABLE_PROMPT_CACHE", value),
        None => std::env::remove_var("TURA_DISABLE_PROMPT_CACHE"),
    }

    assert!(openai_compatible_usage_stream_supported(
        "openrouter",
        "https://openrouter.ai/api/v1"
    ));
    assert!(openai_compatible_usage_stream_supported(
        "qwen",
        "https://dashscope.aliyuncs.com/compatible-mode/v1"
    ));
    assert!(openai_compatible_usage_stream_supported(
        "localtest",
        "https://api.minimax.io/v1"
    ));
    assert!(!openai_compatible_usage_stream_supported(
        "localtest",
        "https://example.invalid/v1"
    ));
}

#[tokio::test]
async fn openai_compatible_business_flow_normalizes_content_parts_tools_costs_and_usage_variants() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind local provider");
    let addr = listener.local_addr().expect("local provider addr");
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept provider request");
        let request = read_http_request(&mut stream);
        let body = json!({
            "id": "chatcmpl-shape-1",
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": {
                        "parts": [
                            {"text": "first part "},
                            {"text": "<thought>hidden</thought>second part"}
                        ]
                    },
                    "tool_calls": [{
                        "id": "call_shape_1",
                        "type": "function",
                        "function": {
                            "name": "command_run",
                            "arguments": "{\"commands\":\"<parameters><command_type>shell_command</command_type><command_line>Get-Content README.md</command_line></parameters>\"}"
                        },
                        "provider_metadata": {"native_id": "provider-call-1"}
                    }]
                },
                "finish_reason": "tool_calls"
            }],
            "usage": {
                "input_tokens": 80,
                "output_tokens": 9,
                "cache_read_tokens": 4,
                "cache_write_tokens": 3,
                "output_tokens_details": {"reasoning_tokens": 2}
            },
            "cost": {
                "input": 0.001,
                "output": 0.002,
                "cache_read": 0.0001,
                "cache_write": 0.0002,
                "reasoning": 0.0003,
                "total": 0.0036,
                "currency": "USD"
            }
        })
        .to_string();
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nx-request-id: req-shape-1\r\nContent-Length: {}\r\n\r\n{}",
            body.len(),
            body
        );
        stream
            .write_all(response.as_bytes())
            .expect("write provider response");
        request
    });

    let previous_key = std::env::var_os("LOCALTEST_API_KEY");
    std::env::set_var("LOCALTEST_API_KEY", "dummy-local-key");
    let config = ProviderConfig {
        provider: "localtest".to_string(),
        base_url: format!("http://{addr}"),
        model: "local-shape-model".to_string(),
        temperature: 0.2,
    };
    let response = config
        .call(
            &TuraConfig::new(".env.provider-business-missing"),
            vec![json!({"role": "user", "content": "normalize content parts and tool call"})],
            CallOptions {
                context_window: Some(8192),
                metadata: Some(HashMap::from([(
                    "shape".to_string(),
                    "content-parts".to_string(),
                )])),
                ..CallOptions::default()
            },
        )
        .await
        .expect("content-part provider call should succeed");

    match previous_key {
        Some(value) => std::env::set_var("LOCALTEST_API_KEY", value),
        None => std::env::remove_var("LOCALTEST_API_KEY"),
    }

    let captured = server.join().expect("server thread joins");
    assert_eq!(captured.method, "POST");
    assert_eq!(captured.path, "/chat/completions");
    assert_eq!(captured.body["model"], "local-shape-model");
    assert_eq!(captured.body["metadata"]["shape"], "content-parts");

    let text = extract_response_text(&response.content).expect("response text");
    assert_eq!(text, "first part <thought>hidden</thought>second part");
    assert_eq!(strip_thought_blocks(&text), "first part second part");
    let calls = extract_tool_calls(&response.content);
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].tool_name, "command_run");
    assert_eq!(
        calls[0].arguments["commands"][0]["command_line"],
        "Get-Content README.md"
    );
    assert_eq!(
        calls[0]
            .provider_metadata
            .as_ref()
            .and_then(|metadata| metadata.get("native_id"))
            .and_then(Value::as_str),
        Some("provider-call-1")
    );

    let metrics = response.metrics.expect("metrics");
    assert_eq!(metrics.provider_request_id.as_deref(), Some("req-shape-1"));
    assert_eq!(metrics.tool_call_count, 1);
    assert_eq!(metrics.finish_reason.as_deref(), Some("tool_calls"));
    assert_eq!(metrics.usage.input_tokens, Some(80));
    assert_eq!(metrics.usage.output_tokens, Some(9));
    assert_eq!(metrics.usage.cached_input_tokens, Some(4));
    assert_eq!(metrics.usage.cache_write_tokens, Some(3));
    assert_eq!(metrics.usage.reasoning_tokens, Some(2));
    assert_eq!(metrics.usage.total_tokens, Some(96));
    assert_eq!(metrics.usage.context_window, Some(8192));
    assert!(metrics.cache_hit);
    assert_eq!(metrics.cost.total_cost, Some(0.0036));
    assert_eq!(metrics.cost.currency.as_deref(), Some("USD"));
}

#[tokio::test]
async fn openai_compatible_business_flow_rejects_empty_success_shapes_without_silent_content() {
    for (case, payload) in [
        (
            "missing-choices",
            json!({"id": "empty-1", "usage": {"total_tokens": 1}}),
        ),
        ("empty-choices", json!({"choices": []})),
        (
            "missing-message",
            json!({"choices": [{"finish_reason": "stop"}]}),
        ),
    ] {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind local provider");
        let addr = listener.local_addr().expect("local provider addr");
        let body = payload.to_string();
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept provider request");
            let request = read_http_request(&mut stream);
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                body.len(),
                body
            );
            stream
                .write_all(response.as_bytes())
                .expect("write provider response");
            request
        });

        let previous_key = std::env::var_os("LOCALTEST_API_KEY");
        std::env::set_var("LOCALTEST_API_KEY", "dummy-local-key");
        let config = ProviderConfig {
            provider: "localtest".to_string(),
            base_url: format!("http://{addr}"),
            model: format!("local-empty-{case}"),
            temperature: 0.2,
        };
        let err = config
            .call(
                &TuraConfig::new(".env.provider-business-missing"),
                vec![json!({"role": "user", "content": format!("case {case}")})],
                CallOptions::default(),
            )
            .await
            .expect_err("empty provider success shapes should be rejected");

        match previous_key {
            Some(value) => std::env::set_var("LOCALTEST_API_KEY", value),
            None => std::env::remove_var("LOCALTEST_API_KEY"),
        }

        let captured = server.join().expect("server thread joins");
        assert_eq!(captured.method, "POST");
        assert_eq!(captured.path, "/chat/completions");
        assert_eq!(captured.body["model"], format!("local-empty-{case}"));
        match err {
            TuraError::ProviderRequest { provider, message } => {
                assert_eq!(provider, "localtest");
                assert!(
                    message.contains("missing response content")
                        || message.contains("missing choices")
                        || message.contains("missing message")
                        || message.contains("empty"),
                    "{case} should preserve response-shape context: {message}"
                );
            }
            TuraError::Network { message } => {
                assert!(
                    message.contains("missing response content")
                        || message.contains("missing choices")
                        || message.contains("missing message")
                        || message.contains("empty"),
                    "{case} should preserve response-shape context: {message}"
                );
            }
            other => panic!("expected response-shape error for {case}, got {other}"),
        }
    }
}

#[tokio::test]
async fn openai_compatible_business_flow_reports_provider_http_status_and_body() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind local provider");
    let addr = listener.local_addr().expect("local provider addr");
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept provider request");
        let request = read_http_request(&mut stream);
        let body = json!({
            "error": {
                "type": "rate_limit_exceeded",
                "message": "local provider quota exhausted"
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
            .expect("write provider response");
        request
    });

    let previous_key = std::env::var_os("LOCALTEST_API_KEY");
    std::env::set_var("LOCALTEST_API_KEY", "dummy-local-key");
    let config = ProviderConfig {
        provider: "localtest".to_string(),
        base_url: format!("http://{addr}"),
        model: "local-model".to_string(),
        temperature: 0.2,
    };
    let err = config
        .call(
            &TuraConfig::new(".env.provider-business-missing"),
            vec![json!({"role": "user", "content": "force provider error"})],
            CallOptions::default(),
        )
        .await
        .expect_err("provider HTTP error should fail the call");

    match previous_key {
        Some(value) => std::env::set_var("LOCALTEST_API_KEY", value),
        None => std::env::remove_var("LOCALTEST_API_KEY"),
    }

    let captured = server.join().expect("server thread joins");
    assert_eq!(captured.method, "POST");
    assert_eq!(captured.path, "/chat/completions");
    match err {
        TuraError::HttpStatus { status, body } => {
            assert_eq!(status, 429);
            assert!(
                body.contains("rate_limit_exceeded")
                    && body.contains("local provider quota exhausted"),
                "HTTP status error should preserve provider error body: {body}"
            );
        }
        other => panic!("expected HTTP status error, got {other}"),
    }
}

#[tokio::test]
async fn openai_compatible_business_flow_reports_invalid_json_response() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind local provider");
    let addr = listener.local_addr().expect("local provider addr");
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept provider request");
        let request = read_http_request(&mut stream);
        let body = r#"{"choices":[{"message":{"content":"unterminated"}]"#;
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
            body.len(),
            body
        );
        stream
            .write_all(response.as_bytes())
            .expect("write provider response");
        request
    });

    let previous_key = std::env::var_os("LOCALTEST_API_KEY");
    std::env::set_var("LOCALTEST_API_KEY", "dummy-local-key");
    let config = ProviderConfig {
        provider: "localtest".to_string(),
        base_url: format!("http://{addr}"),
        model: "local-model".to_string(),
        temperature: 0.2,
    };
    let err = config
        .call(
            &TuraConfig::new(".env.provider-business-missing"),
            vec![json!({"role": "user", "content": "return bad json"})],
            CallOptions::default(),
        )
        .await
        .expect_err("invalid provider JSON should fail the call");

    match previous_key {
        Some(value) => std::env::set_var("LOCALTEST_API_KEY", value),
        None => std::env::remove_var("LOCALTEST_API_KEY"),
    }

    let captured = server.join().expect("server thread joins");
    assert_eq!(captured.method, "POST");
    assert_eq!(captured.path, "/chat/completions");
    match err {
        TuraError::Network { message } => {
            assert!(
                message.contains("decoding response body") || message.contains("EOF"),
                "decode error should preserve body parsing context: {message}"
            );
        }
        other => panic!("expected decode network error, got {other}"),
    }
}

#[tokio::test]
async fn openai_compatible_business_flow_reports_invalid_stream_event_json() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind local provider");
    let addr = listener.local_addr().expect("local provider addr");
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept provider request");
        let request = read_http_request(&mut stream);
        let body = "data: {\"choices\":[{\"delta\":{\"content\":\"partial\"}}]\n\n";
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\n\r\n{}",
            body.len(),
            body
        );
        stream
            .write_all(response.as_bytes())
            .expect("write provider response");
        request
    });

    let previous_key = std::env::var_os("LOCALTEST_API_KEY");
    std::env::set_var("LOCALTEST_API_KEY", "dummy-local-key");
    let config = ProviderConfig {
        provider: "localtest".to_string(),
        base_url: format!("http://{addr}"),
        model: "local-model".to_string(),
        temperature: 0.2,
    };
    let err = config
        .call_with_stream_events(
            &TuraConfig::new(".env.provider-business-missing"),
            vec![json!({"role": "user", "content": "stream bad json"})],
            CallOptions {
                stream: Some(true),
                ..CallOptions::default()
            },
            None,
        )
        .await
        .expect_err("invalid stream JSON should fail the call");

    match previous_key {
        Some(value) => std::env::set_var("LOCALTEST_API_KEY", value),
        None => std::env::remove_var("LOCALTEST_API_KEY"),
    }

    let captured = server.join().expect("server thread joins");
    assert_eq!(captured.method, "POST");
    assert_eq!(captured.path, "/chat/completions");
    match err {
        TuraError::Json(error) => {
            assert!(
                error.to_string().contains("EOF")
                    || error.to_string().contains("expected")
                    || error.to_string().contains("trailing"),
                "stream JSON error should preserve parser context: {error}"
            );
        }
        other => panic!("expected JSON error, got {other}"),
    }
}

#[tokio::test]
async fn openai_compatible_business_flow_reports_truncated_stream_transport_error() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind local provider");
    let addr = listener.local_addr().expect("local provider addr");
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept provider request");
        let request = read_http_request(&mut stream);
        let body = "data: {\"choices\":[{\"delta\":{\"content\":\"partial stream text\"}}]}\n\n";
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\n\r\n{}",
            body.len() + 4096,
            body
        );
        stream
            .write_all(response.as_bytes())
            .expect("write truncated stream response");
        request
    });

    let previous_key = std::env::var_os("LOCALTEST_API_KEY");
    std::env::set_var("LOCALTEST_API_KEY", "dummy-local-key");
    let config = ProviderConfig {
        provider: "localtest".to_string(),
        base_url: format!("http://{addr}"),
        model: "local-model".to_string(),
        temperature: 0.2,
    };
    let events = Arc::new(Mutex::new(Vec::<String>::new()));
    let sink_events = Arc::clone(&events);
    let sink = Arc::new(move |event: ProviderStreamEvent| {
        if let ProviderStreamEvent::TextDelta { text } = event {
            sink_events.lock().expect("events lock").push(text);
        }
    });
    let err = config
        .call_with_stream_events(
            &TuraConfig::new(".env.provider-business-missing"),
            vec![json!({"role": "user", "content": "stream transport truncation"})],
            CallOptions {
                stream: Some(true),
                ..CallOptions::default()
            },
            Some(sink),
        )
        .await
        .expect_err("truncated provider stream must fail the call");

    match previous_key {
        Some(value) => std::env::set_var("LOCALTEST_API_KEY", value),
        None => std::env::remove_var("LOCALTEST_API_KEY"),
    }

    let captured = server.join().expect("server thread joins");
    assert_eq!(captured.method, "POST");
    assert_eq!(captured.path, "/chat/completions");
    assert_eq!(captured.body["stream"], true);
    assert_eq!(
        events.lock().expect("events lock").as_slice(),
        &["partial stream text".to_string()],
        "already received stream deltas may be emitted, but the final provider call must still fail"
    );
    match err {
        TuraError::Network { message } => {
            assert!(
                message.contains("end of file")
                    || message.contains("connection")
                    || message.contains("body")
                    || message.contains("incomplete")
                    || message.contains("reset"),
                "truncated stream should preserve transport error context: {message}"
            );
        }
        other => panic!("expected network error for truncated stream, got {other}"),
    }
}

#[tokio::test]
async fn openai_compatible_business_flow_rejects_empty_stream_success_without_silent_null() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind local provider");
    let addr = listener.local_addr().expect("local provider addr");
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept provider request");
        let request = read_http_request(&mut stream);
        let usage = json!({
            "choices": [],
            "usage": {
                "prompt_tokens": 9,
                "completion_tokens": 0,
                "total_tokens": 9
            }
        });
        let body = format!("data: {usage}\n\ndata: [DONE]\n\n");
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\n\r\n{}",
            body.len(),
            body
        );
        stream
            .write_all(response.as_bytes())
            .expect("write empty stream response");
        request
    });

    let previous_key = std::env::var_os("LOCALTEST_API_KEY");
    std::env::set_var("LOCALTEST_API_KEY", "dummy-local-key");
    let config = ProviderConfig {
        provider: "localtest".to_string(),
        base_url: format!("http://{addr}"),
        model: "local-model".to_string(),
        temperature: 0.2,
    };
    let err = config
        .call_with_stream_events(
            &TuraConfig::new(".env.provider-business-missing"),
            vec![json!({"role": "user", "content": "stream empty success"})],
            CallOptions {
                stream: Some(true),
                stream_options: Some(json!({"include_usage": true})),
                ..CallOptions::default()
            },
            None,
        )
        .await
        .expect_err("empty stream success must not become a null assistant turn");

    match previous_key {
        Some(value) => std::env::set_var("LOCALTEST_API_KEY", value),
        None => std::env::remove_var("LOCALTEST_API_KEY"),
    }

    let captured = server.join().expect("server thread joins");
    assert_eq!(captured.method, "POST");
    assert_eq!(captured.path, "/chat/completions");
    assert_eq!(captured.body["stream"], true);
    assert_eq!(captured.body["stream_options"]["include_usage"], true);
    match err {
        TuraError::ProviderRequest { provider, message } => {
            assert_eq!(provider, "localtest");
            assert!(
                message.contains("missing response content"),
                "empty stream should preserve response-shape context: {message}"
            );
        }
        other => panic!("expected stream response-shape error, got {other}"),
    }
}

#[tokio::test]
async fn openai_compatible_business_flow_preserves_reasoning_only_stream_as_content() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind local provider");
    let addr = listener.local_addr().expect("local provider addr");
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept provider request");
        let request = read_http_request(&mut stream);
        let first = json!({
            "choices": [{
                "delta": {"reasoning_content": "thinking through local state "}
            }]
        });
        let second = json!({
            "choices": [{
                "delta": {"reasoning": "before final answer"},
                "finish_reason": "stop"
            }]
        });
        let body = format!("data: {first}\n\ndata: {second}\n\ndata: [DONE]\n\n");
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\n\r\n{}",
            body.len(),
            body
        );
        stream
            .write_all(response.as_bytes())
            .expect("write provider response");
        request
    });

    let previous_key = std::env::var_os("LOCALTEST_API_KEY");
    std::env::set_var("LOCALTEST_API_KEY", "dummy-local-key");
    let config = ProviderConfig {
        provider: "localtest".to_string(),
        base_url: format!("http://{addr}"),
        model: "local-model".to_string(),
        temperature: 0.2,
    };
    let events = Arc::new(Mutex::new(Vec::<String>::new()));
    let sink_events = Arc::clone(&events);
    let sink = Arc::new(move |event: ProviderStreamEvent| {
        if let ProviderStreamEvent::TextDelta { text } = event {
            sink_events.lock().expect("events lock").push(text);
        }
    });
    let result = config
        .call_with_stream_events(
            &TuraConfig::new(".env.provider-business-missing"),
            vec![json!({"role": "user", "content": "reason without final text"})],
            CallOptions {
                stream: Some(true),
                context_window: Some(128),
                ..CallOptions::default()
            },
            Some(sink),
        )
        .await;

    match previous_key {
        Some(value) => std::env::set_var("LOCALTEST_API_KEY", value),
        None => std::env::remove_var("LOCALTEST_API_KEY"),
    }

    let response = result.expect("reasoning-only stream should produce content");
    let captured = server.join().expect("server thread joins");
    assert_eq!(captured.method, "POST");
    assert_eq!(captured.path, "/chat/completions");
    assert_eq!(captured.body["stream"], true);
    assert_eq!(
        response.content.as_str(),
        Some("thinking through local state before final answer")
    );
    assert!(
        events.lock().expect("events lock").is_empty(),
        "reasoning-only chunks should not be emitted as visible text deltas"
    );
    let metrics = response.metrics.expect("metrics");
    assert_eq!(metrics.finish_reason.as_deref(), Some("stop"));
    assert_eq!(metrics.usage.context_window, Some(128));
    assert!(
        metrics.usage.total_tokens.unwrap_or_default() > 0,
        "stream without provider usage should receive estimated usage"
    );
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
    let path = parts.next().expect("path").to_string();
    let headers_lower = headers.to_ascii_lowercase();
    let body_text = String::from_utf8(buffer[body_start..body_start + content_length].to_vec())
        .expect("utf8 request body");
    let body = serde_json::from_str(&body_text).expect("json request body");

    CapturedHttpRequest {
        method,
        path,
        headers: headers_lower,
        body,
    }
}

fn accept_optional_provider_request(
    listener: TcpListener,
    timeout_ms: u64,
) -> Option<CapturedHttpRequest> {
    listener
        .set_nonblocking(true)
        .expect("set fallback listener nonblocking");
    let started = Instant::now();
    while started.elapsed() < Duration::from_millis(timeout_ms) {
        match listener.accept() {
            Ok((mut stream, _)) => {
                let request = read_http_request(&mut stream);
                let body = json!({
                    "choices": [{
                        "message": {
                            "role": "assistant",
                            "content": "unexpected fallback"
                        }
                    }]
                })
                .to_string();
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                    body.len(),
                    body
                );
                stream
                    .write_all(response.as_bytes())
                    .expect("write unexpected fallback response");
                return Some(request);
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(10));
            }
            Err(error) => panic!("fallback provider listener failed: {error}"),
        }
    }
    None
}

fn read_llm_logs(root: &std::path::Path) -> Vec<Value> {
    let mut logs = Vec::new();
    for day in std::fs::read_dir(root).expect("read log root") {
        let day = day.expect("read day entry");
        if !day.path().is_dir() {
            continue;
        }
        for entry in std::fs::read_dir(day.path()).expect("read log day") {
            let entry = entry.expect("read log entry");
            if entry.path().extension().and_then(|value| value.to_str()) != Some("json") {
                continue;
            }
            let content = std::fs::read_to_string(entry.path()).expect("read llm log");
            logs.push(serde_json::from_str(&content).expect("parse llm log"));
        }
    }
    logs
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
