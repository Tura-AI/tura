use super::helpers::*;

#[test]
fn openai_compatible_provider_boundary_business_flow_normalizes_runtime_visible_contracts() {
    let _env_guard = ENV_LOCK.blocking_lock();
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
    assert!(prompt_cache_key_supported(
        "codex",
        "https://chatgpt.com/backend-api/codex"
    ));
    assert!(prompt_cache_key_supported(
        "chatgpt",
        "https://chatgpt.com/backend-api"
    ));
    assert!(prompt_cache_key_supported(
        "openai-api",
        "https://example.invalid/v1"
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
    let _env_guard = ENV_LOCK.lock().await;
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
