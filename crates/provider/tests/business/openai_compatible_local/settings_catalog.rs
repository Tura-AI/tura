use super::helpers::*;

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
