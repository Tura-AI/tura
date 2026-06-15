use super::helpers::*;

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
