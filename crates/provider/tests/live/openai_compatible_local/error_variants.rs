use super::helpers::*;

#[tokio::test]
async fn openai_compatible_business_flow_rejects_empty_success_shapes_without_silent_content() {
    let _env_guard = ENV_LOCK.lock().await;
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
    let _env_guard = ENV_LOCK.lock().await;
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
    let _env_guard = ENV_LOCK.lock().await;
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
    let _env_guard = ENV_LOCK.lock().await;
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
    let _env_guard = ENV_LOCK.lock().await;
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
    let _env_guard = ENV_LOCK.lock().await;
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
    let _env_guard = ENV_LOCK.lock().await;
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
