use super::helpers::*;

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
