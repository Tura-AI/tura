const AUTH_CHILD_PROVIDER_ENV: &str = "TURA_PROVIDER_AUTH_BUSINESS_CHILD_PROVIDER";
const AUTH_CHILD_KEY_ENV: &str = "TURA_PROVIDER_AUTH_BUSINESS_CHILD_KEY";
const AUTH_CHILD_ACTION_ENV: &str = "TURA_PROVIDER_AUTH_BUSINESS_CHILD_ACTION";

#[path = "helpers/provider_oauth_local.rs"]
mod helpers;
use helpers::*;
fn clear_provider(provider_id: &str) {
    let store = global_store();
    store.pending_oauth.write().remove(provider_id);
    store.completed_oauth.write().remove(provider_id);
}

fn set_pending(provider_id: &str, method: &str, state: &str, code: Option<&str>) {
    set_pending_with_verifier(provider_id, method, state, code, None);
}

fn set_pending_with_verifier(
    provider_id: &str,
    method: &str,
    state: &str,
    code: Option<&str>,
    verifier: Option<&str>,
) {
    clear_provider(provider_id);
    global_store().set_oauth_state(
        provider_id,
        method.to_string(),
        code.map(ToString::to_string),
        format!("https://auth.local.test/{provider_id}"),
        Some(state.to_string()),
        Some(verifier.unwrap_or("business-verifier").to_string()),
    );
}

#[tokio::test]
async fn oauth_business_retry_flow_keeps_pending_login_after_user_correctable_errors() {
    let _guard = ENV_LOCK.lock().await;
    let provider_id = "gateway-business-oauth-retry";

    set_pending(provider_id, "token", "token-state", Some("confirm-code"));
    let Json(blank_response) = oauth_callback(
        Path(provider_id.to_string()),
        Query(OAuthCallbackParams {
            directory: None,
            workspace: None,
        }),
        Json(OAuthCallbackPayload {
            method: 0,
            state: Some("token-state".to_string()),
            code: Some(" ".to_string()),
        }),
    )
    .await;
    assert!(!blank_response.ok);
    assert_eq!(blank_response.code, "provider.oauth.code_missing");
    assert!(global_store().peek_oauth_state(provider_id).is_some());

    set_pending(provider_id, "code", "manual-state", Some("confirm-code"));
    let Json(mismatch_response) = oauth_callback(
        Path(provider_id.to_string()),
        Query(OAuthCallbackParams {
            directory: None,
            workspace: None,
        }),
        Json(OAuthCallbackPayload {
            method: 0,
            state: Some("manual-state".to_string()),
            code: Some("wrong-code".to_string()),
        }),
    )
    .await;
    assert!(!mismatch_response.ok);
    assert_eq!(mismatch_response.code, "provider.oauth.code_mismatch");
    assert_eq!(
        global_store()
            .peek_oauth_state(provider_id)
            .and_then(|pending| pending.code),
        Some("confirm-code".to_string())
    );

    set_pending(provider_id, "oauth_pkce", "pkce-state", None);
    let Json(state_mismatch_response) = oauth_callback(
        Path(provider_id.to_string()),
        Query(OAuthCallbackParams {
            directory: None,
            workspace: None,
        }),
        Json(OAuthCallbackPayload {
            method: 0,
            state: Some("wrong-state".to_string()),
            code: Some("callback-code".to_string()),
        }),
    )
    .await;
    assert!(!state_mismatch_response.ok);
    assert_eq!(
        state_mismatch_response.code,
        "provider.oauth.state_mismatch"
    );
    assert_eq!(
        global_store()
            .peek_oauth_state(provider_id)
            .and_then(|pending| pending.state),
        Some("pkce-state".to_string())
    );

    clear_provider(provider_id);
}

#[tokio::test]
async fn oauth_business_pkce_flow_exchanges_local_token_and_persists_auth() {
    let _guard = ENV_LOCK.lock().await;
    let provider_id = "codex";
    let root = tempfile::tempdir().expect("temp oauth root");
    let env_path = root.path().join(".env.gateway-oauth-business");
    let provider_config = root.path().join("provider_config.json");
    copy_provider_config(&provider_config);
    let _env = EnvGuard::new(&[
        (
            "TURA_ENV_PATH",
            Some(env_path.to_string_lossy().to_string()),
        ),
        (
            "TURA_PROVIDER_CONFIG",
            Some(provider_config.to_string_lossy().to_string()),
        ),
        (
            "OPENAI_OAUTH_CLIENT_ID",
            Some("local-client-id".to_string()),
        ),
        (
            "OPENAI_OAUTH_REDIRECT_URI",
            Some("http://127.0.0.1/callback".to_string()),
        ),
        ("OPENAI_OAUTH_TOKEN_URL", None),
        ("OPENAI_API_KEY", None),
        ("OPENAI_LOGIN", None),
        ("OPENAI_REFRESH_TOKEN", None),
        ("OPENAI_TOKEN_EXPIRES", None),
        ("OPENAI_ACCOUNT_ID", None),
    ]);
    clear_provider(provider_id);
    set_pending_with_verifier(
        provider_id,
        "oauth_pkce",
        "pkce-local-state",
        None,
        Some("pkce-local-verifier"),
    );

    let token_server = LocalTokenServer::start(json!({
        "access_token": "local-access-token",
        "refresh_token": "local-refresh-token",
        "expires_in": 7200
    }));
    std::env::set_var("OPENAI_OAUTH_TOKEN_URL", token_server.url());

    let Json(response) = oauth_callback(
        Path(provider_id.to_string()),
        Query(OAuthCallbackParams {
            directory: None,
            workspace: None,
        }),
        Json(OAuthCallbackPayload {
            method: 0,
            state: Some("pkce-local-state".to_string()),
            code: Some("callback-local-code".to_string()),
        }),
    )
    .await;

    assert!(
        response.ok,
        "PKCE exchange should succeed against local token endpoint: {}",
        response.message
    );
    assert_eq!(response.code, "provider.oauth.completed");
    assert!(global_store().peek_oauth_state(provider_id).is_none());

    let request = token_server.join();
    assert_eq!(request.method, "POST");
    assert_eq!(request.path, "/token");
    assert_eq!(
        request.form.get("grant_type").map(String::as_str),
        Some("authorization_code")
    );
    assert_eq!(
        request.form.get("client_id").map(String::as_str),
        Some("local-client-id")
    );
    assert_eq!(
        request.form.get("redirect_uri").map(String::as_str),
        Some("http://127.0.0.1/callback")
    );
    assert_eq!(
        request.form.get("code").map(String::as_str),
        Some("callback-local-code")
    );
    assert_eq!(
        request.form.get("code_verifier").map(String::as_str),
        Some("pkce-local-verifier")
    );

    let env_content = std::fs::read_to_string(&env_path).expect("oauth env file");
    assert!(env_content.contains("OPENAI_API_KEY=\"local-access-token\""));
    assert!(env_content.contains("OPENAI_LOGIN=\"oauth\""));
    assert!(env_content.contains("OPENAI_REFRESH_TOKEN=\"local-refresh-token\""));
    assert!(env_content.contains("OPENAI_TOKEN_EXPIRES=\""));

    let provider_config: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(&provider_config).expect("provider auth config"),
    )
    .expect("provider config json");
    let codex_auth = &provider_config["provider_auth"]["codex"];
    assert_eq!(codex_auth["login"], "oauth");
    assert_eq!(codex_auth["status"], "connected");
    assert_eq!(codex_auth["token_env"], "OPENAI_API_KEY");
    assert_eq!(codex_auth["refresh_env"], "OPENAI_REFRESH_TOKEN");
    assert_eq!(codex_auth["expires_env"], "OPENAI_TOKEN_EXPIRES");
    assert_eq!(
        response
            .status
            .as_ref()
            .expect("auth status")
            .login
            .as_deref(),
        Some("oauth")
    );

    clear_provider(provider_id);
}

#[tokio::test]
async fn oauth_business_refresh_flow_uses_local_token_endpoint_and_updates_auth() {
    let _guard = ENV_LOCK.lock().await;
    let provider_id = "codex";
    let root = tempfile::tempdir().expect("temp oauth refresh root");
    let env_path = root.path().join(".env.gateway-oauth-refresh-business");
    let provider_config = root.path().join("provider_config.json");
    copy_provider_config(&provider_config);
    let _env = EnvGuard::new(&[
        (
            "TURA_ENV_PATH",
            Some(env_path.to_string_lossy().to_string()),
        ),
        (
            "TURA_PROVIDER_CONFIG",
            Some(provider_config.to_string_lossy().to_string()),
        ),
        (
            "OPENAI_OAUTH_CLIENT_ID",
            Some("refresh-client-id".to_string()),
        ),
        ("OPENAI_OAUTH_TOKEN_URL", None),
        ("OPENAI_API_KEY", Some("stale-access-token".to_string())),
        ("OPENAI_LOGIN", Some("oauth".to_string())),
        (
            "OPENAI_REFRESH_TOKEN",
            Some("stale-refresh-token".to_string()),
        ),
        ("OPENAI_TOKEN_EXPIRES", Some("1".to_string())),
        ("OPENAI_ACCOUNT_ID", Some("previous-account".to_string())),
    ]);
    clear_provider(provider_id);

    let token_server = LocalTokenServer::start(json!({
        "access_token": "refreshed-access-token",
        "refresh_token": "refreshed-refresh-token",
        "expires_in": 3600
    }));
    std::env::set_var("OPENAI_OAUTH_TOKEN_URL", token_server.url());

    let Json(response) = provider_auth_refresh(Path(provider_id.to_string())).await;

    assert!(
        response.ok,
        "forced refresh should succeed against local token endpoint: {}",
        response.message
    );
    assert_eq!(response.code, "provider.auth.refresh.succeeded");
    assert_eq!(response.level.as_deref(), Some("valid"));
    assert_eq!(
        response
            .status
            .as_ref()
            .expect("refresh status")
            .login
            .as_deref(),
        Some("oauth")
    );
    assert!(
        response
            .status
            .as_ref()
            .expect("refresh status")
            .authenticated
    );

    let request = token_server.join();
    assert_eq!(request.method, "POST");
    assert_eq!(request.path, "/token");
    assert_eq!(
        request.form.get("grant_type").map(String::as_str),
        Some("refresh_token")
    );
    assert_eq!(
        request.form.get("refresh_token").map(String::as_str),
        Some("stale-refresh-token")
    );
    assert_eq!(
        request.form.get("client_id").map(String::as_str),
        Some("refresh-client-id")
    );

    let env_content = std::fs::read_to_string(&env_path).expect("refresh env file");
    assert!(env_content.contains("OPENAI_API_KEY=\"refreshed-access-token\""));
    assert!(env_content.contains("OPENAI_LOGIN=\"oauth\""));
    assert!(env_content.contains("OPENAI_REFRESH_TOKEN=\"refreshed-refresh-token\""));
    assert!(env_content.contains("OPENAI_TOKEN_EXPIRES=\""));
    assert!(env_content.contains("OPENAI_ACCOUNT_ID=\"previous-account\""));

    let provider_config: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(&provider_config).expect("provider auth config"),
    )
    .expect("provider config json");
    let codex_auth = &provider_config["provider_auth"]["codex"];
    assert_eq!(codex_auth["login"], "oauth");
    assert_eq!(codex_auth["status"], "connected");
    assert_eq!(codex_auth["token_env"], "OPENAI_API_KEY");
    assert_eq!(codex_auth["refresh_env"], "OPENAI_REFRESH_TOKEN");
    assert_eq!(codex_auth["expires_env"], "OPENAI_TOKEN_EXPIRES");
    assert_eq!(codex_auth["account_id"], "previous-account");

    clear_provider(provider_id);
}

#[tokio::test]
async fn oauth_business_validate_flow_reports_local_success_and_missing_key_details() {
    let _guard = ENV_LOCK.lock().await;
    let root = tempfile::tempdir().expect("temp auth validation root");
    let env_path = root.path().join(".env.gateway-auth-validation-business");
    let provider_config = root.path().join("provider_config.json");
    let model_server = LocalModelServer::start(200, json!({ "data": [] }).to_string());
    write_local_provider_config(&provider_config, &model_server.base_url());
    let _env = EnvGuard::new(&[
        (
            "TURA_ENV_PATH",
            Some(env_path.to_string_lossy().to_string()),
        ),
        (
            "TURA_PROVIDER_CONFIG",
            Some(provider_config.to_string_lossy().to_string()),
        ),
        (
            "BUSINESS_LOCAL_API_KEY",
            Some("business-local-key".to_string()),
        ),
    ]);

    let Json(valid) = provider_auth_validate(
        Path("business-local".to_string()),
        Json(ProviderAuthValidationRequest::default()),
    )
    .await;

    assert!(valid.ok, "local validation should pass: {}", valid.message);
    assert_eq!(valid.code, "provider.validation.valid");
    assert_eq!(valid.level.as_deref(), Some("valid"));
    assert_detail(&valid.details, "provider.validation.passed");
    assert_detail_value(
        &valid.details,
        "provider.base_url.ok",
        &model_server.base_url(),
    );
    assert_detail_value(
        &valid.details,
        "provider.env.present",
        "BUSINESS_LOCAL_API_KEY",
    );
    assert_detail_value(
        &valid.details,
        "provider.remote.accepted",
        "OpenAI-compatible /models",
    );
    assert_no_detail(&valid.details, "provider.request.no_paid_model");
    let request = model_server.join();
    assert_eq!(request.method, "GET");
    assert_eq!(request.path, "/models");
    assert!(
        request
            .headers
            .contains("authorization: bearer business-local-key"),
        "validation should use the configured local API key, got headers: {}",
        request.headers
    );

    write_local_provider_config(&provider_config, "https://business.example.invalid/v1");
    std::env::remove_var("BUSINESS_LOCAL_API_KEY");
    let Json(invalid) = provider_auth_validate(
        Path("business-local".to_string()),
        Json(ProviderAuthValidationRequest::default()),
    )
    .await;
    assert!(!invalid.ok, "missing local key should fail validation");
    assert_eq!(invalid.code, "provider.validation.invalid");
    assert_eq!(invalid.level.as_deref(), Some("invalid"));
    assert_detail(&invalid.details, "provider.validation.failed");
    assert_detail_value(
        &invalid.details,
        "provider.env.missing",
        "BUSINESS_LOCAL_API_KEY",
    );
    assert_detail_value(
        &invalid.details,
        "provider.credential.api_key_missing",
        "OpenAI-compatible",
    );
    assert_no_detail(&invalid.details, "provider.request.no_paid_model");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn oauth_business_concurrent_auth_writes_keep_env_and_config_complete() {
    let _guard = ENV_LOCK.lock().await;
    if let Ok(provider_id) = std::env::var(AUTH_CHILD_PROVIDER_ENV) {
        match std::env::var(AUTH_CHILD_ACTION_ENV)
            .unwrap_or_else(|_| "write".to_string())
            .as_str()
        {
            "write" => {
                let key = std::env::var(AUTH_CHILD_KEY_ENV).expect("child auth key env");
                assert!(
                    write_api_auth(&provider_id, &key).await,
                    "child process provider auth write should be saved"
                );
            }
            "logout" => {
                let Json(response) = provider_auth_logout(Path(provider_id.clone())).await;
                assert!(
                    response.ok,
                    "child process provider auth logout should succeed: {}",
                    response.message
                );
            }
            action => panic!("unknown child auth action: {action}"),
        }
        return;
    }

    let root = tempfile::tempdir().expect("temp concurrent auth root");
    let env_path = root.path().join(".env.gateway-auth-concurrent-business");
    let provider_config = root.path().join("provider_config.json");
    copy_provider_config(&provider_config);
    let provider_count = 12usize;
    let provider_ids = (0..provider_count)
        .map(|index| format!("business_concurrent_{index}"))
        .collect::<Vec<_>>();
    let process_provider_ids = (0..8usize)
        .map(|index| format!("business_process_{index}"))
        .collect::<Vec<_>>();
    let logout_provider_ids = (0..6usize)
        .map(|index| format!("business_logout_{index}"))
        .collect::<Vec<_>>();
    let dynamic_env_keys = provider_ids
        .iter()
        .chain(process_provider_ids.iter())
        .chain(logout_provider_ids.iter())
        .flat_map(|provider_id| {
            [
                format!("{}_API_KEY", provider_id.to_ascii_uppercase()),
                format!("{}_LOGIN", provider_id.to_ascii_uppercase()),
            ]
        })
        .collect::<Vec<_>>();
    let _env = EnvGuard::new(&[
        (
            "TURA_ENV_PATH",
            Some(env_path.to_string_lossy().to_string()),
        ),
        (
            "TURA_PROVIDER_CONFIG",
            Some(provider_config.to_string_lossy().to_string()),
        ),
    ]);
    let _dynamic_env = DynamicEnvGuard::capture(dynamic_env_keys);

    let mut handles = Vec::new();
    for (index, provider_id) in provider_ids.iter().cloned().enumerate() {
        handles.push(tokio::spawn(async move {
            write_api_auth(&provider_id, &format!("business-concurrent-key-{index}")).await
        }));
    }
    for handle in handles {
        assert!(
            handle.await.expect("concurrent auth write joins"),
            "concurrent provider auth write should be saved"
        );
    }
    for (index, provider_id) in logout_provider_ids.iter().enumerate() {
        assert!(
            write_api_auth(provider_id, &format!("business-logout-key-{index}")).await,
            "seeded provider auth should be saved before logout race"
        );
    }

    let current_exe = std::env::current_exe().expect("current test exe");
    let mut children = Vec::new();
    for (index, provider_id) in process_provider_ids.iter().enumerate() {
        let key = format!("business-process-key-{index}");
        children.push((
            provider_id.clone(),
            key.clone(),
            Command::new(&current_exe)
                .arg("--exact")
                .arg("oauth_business_concurrent_auth_writes_keep_env_and_config_complete")
                .arg("--nocapture")
                .arg("--test-threads=1")
                .env("TURA_ENV_PATH", &env_path)
                .env("TURA_PROVIDER_CONFIG", &provider_config)
                .env(AUTH_CHILD_PROVIDER_ENV, provider_id)
                .env(AUTH_CHILD_ACTION_ENV, "write")
                .env(AUTH_CHILD_KEY_ENV, key)
                .spawn()
                .expect("spawn auth writer child"),
        ));
    }
    for (index, provider_id) in logout_provider_ids.iter().enumerate() {
        children.push((
            provider_id.clone(),
            format!("business-logout-key-{index}"),
            Command::new(&current_exe)
                .arg("--exact")
                .arg("oauth_business_concurrent_auth_writes_keep_env_and_config_complete")
                .arg("--nocapture")
                .arg("--test-threads=1")
                .env("TURA_ENV_PATH", &env_path)
                .env("TURA_PROVIDER_CONFIG", &provider_config)
                .env(AUTH_CHILD_PROVIDER_ENV, provider_id)
                .env(AUTH_CHILD_ACTION_ENV, "logout")
                .spawn()
                .expect("spawn auth logout child"),
        ));
    }
    for (provider_id, _key, mut child) in children {
        let status = child.wait().expect("wait auth writer child");
        assert!(
            status.success(),
            "auth child for {provider_id} should exit successfully: {status}"
        );
    }

    let env_content = std::fs::read_to_string(&env_path).expect("concurrent env file");
    let provider_config: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(&provider_config).expect("concurrent provider config"),
    )
    .expect("concurrent provider config json");
    let provider_auth = provider_config["provider_auth"]
        .as_object()
        .expect("provider_auth object");

    for (index, provider_id) in provider_ids.iter().enumerate() {
        assert_persisted_api_auth(
            &env_content,
            provider_auth,
            provider_id,
            &format!("business-concurrent-key-{index}"),
        );
    }
    for (index, provider_id) in process_provider_ids.iter().enumerate() {
        assert_persisted_api_auth(
            &env_content,
            provider_auth,
            provider_id,
            &format!("business-process-key-{index}"),
        );
    }
    for provider_id in &logout_provider_ids {
        assert_revoked_api_auth(&env_content, provider_auth, provider_id);
    }
}

#[tokio::test]
async fn oauth_business_redirect_flow_rejects_non_pkce_pending_without_network_exchange() {
    let _guard = ENV_LOCK.lock().await;
    let provider_id = "gateway-business-oauth-redirect";
    clear_provider(provider_id);

    let waiting = oauth_callback_info(
        Path(provider_id.to_string()),
        Query(OAuthRedirectCallbackParams {
            code: None,
            state: None,
            error: None,
        }),
    )
    .await;
    assert!(waiting.0.contains("OAuth callback is waiting"));
    assert!(global_store().peek_oauth_state(provider_id).is_none());

    set_pending(
        provider_id,
        "token",
        "redirect-token-state",
        Some("confirm-code"),
    );
    let non_pkce = oauth_callback_info(
        Path(provider_id.to_string()),
        Query(OAuthRedirectCallbackParams {
            code: Some("callback-code".to_string()),
            state: Some("redirect-token-state".to_string()),
            error: None,
        }),
    )
    .await;
    assert!(non_pkce
        .0
        .contains("OAuth callback did not match a PKCE login"));
    assert!(global_store().peek_oauth_state(provider_id).is_none());

    clear_provider(provider_id);
}
