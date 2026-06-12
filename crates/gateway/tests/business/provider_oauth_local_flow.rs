use axum::extract::{Json, Path, Query};
use gateway::api::provider::{
    oauth_callback, oauth_callback_info, provider_auth_logout, provider_auth_refresh,
    provider_auth_validate, set_auth, OAuthCallbackParams, OAuthCallbackPayload,
    OAuthRedirectCallbackParams, ProviderAuthActionDetail,
};
use gateway::api::types::ProviderAuth;
use gateway::mock::global_store;
use serde_json::json;
use std::collections::HashMap;
use std::ffi::OsString;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path as FsPath, PathBuf};
use std::process::Command;
use std::thread;

const AUTH_CHILD_PROVIDER_ENV: &str = "TURA_PROVIDER_AUTH_BUSINESS_CHILD_PROVIDER";
const AUTH_CHILD_KEY_ENV: &str = "TURA_PROVIDER_AUTH_BUSINESS_CHILD_KEY";
const AUTH_CHILD_ACTION_ENV: &str = "TURA_PROVIDER_AUTH_BUSINESS_CHILD_ACTION";

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

    let Json(valid) = provider_auth_validate(Path("business-local".to_string())).await;

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
    assert_detail(&valid.details, "provider.request.no_paid_model");
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
    let Json(invalid) = provider_auth_validate(Path("business-local".to_string())).await;
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
    assert_detail(&invalid.details, "provider.request.no_paid_model");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn oauth_business_concurrent_auth_writes_keep_env_and_config_complete() {
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
            "churn" => {
                let key = std::env::var(AUTH_CHILD_KEY_ENV).expect("child auth key env");
                for round in 0..4 {
                    assert!(
                        write_api_auth(&provider_id, &format!("{key}-round-{round}")).await,
                        "child process churn write should be saved"
                    );
                    let Json(response) = provider_auth_logout(Path(provider_id.clone())).await;
                    assert!(
                        response.ok,
                        "child process churn logout should succeed: {}",
                        response.message
                    );
                }
                assert!(
                    write_api_auth(&provider_id, &key).await,
                    "child process final churn write should be saved"
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
    let churn_provider_ids = (0..6usize)
        .map(|index| format!("business_churn_{index}"))
        .collect::<Vec<_>>();
    let dynamic_env_keys = provider_ids
        .iter()
        .chain(process_provider_ids.iter())
        .chain(logout_provider_ids.iter())
        .chain(churn_provider_ids.iter())
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
    for (index, provider_id) in churn_provider_ids.iter().enumerate() {
        children.push((
            provider_id.clone(),
            format!("business-churn-final-key-{index}"),
            Command::new(&current_exe)
                .arg("--exact")
                .arg("oauth_business_concurrent_auth_writes_keep_env_and_config_complete")
                .arg("--nocapture")
                .arg("--test-threads=1")
                .env("TURA_ENV_PATH", &env_path)
                .env("TURA_PROVIDER_CONFIG", &provider_config)
                .env(AUTH_CHILD_PROVIDER_ENV, provider_id)
                .env(AUTH_CHILD_ACTION_ENV, "churn")
                .env(
                    AUTH_CHILD_KEY_ENV,
                    format!("business-churn-final-key-{index}"),
                )
                .spawn()
                .expect("spawn auth churn child"),
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
    for (index, provider_id) in churn_provider_ids.iter().enumerate() {
        assert_persisted_api_auth(
            &env_content,
            provider_auth,
            provider_id,
            &format!("business-churn-final-key-{index}"),
        );
    }
}

async fn write_api_auth(provider_id: &str, key: &str) -> bool {
    let token_env = format!("{}_API_KEY", provider_id.to_ascii_uppercase());
    let mut metadata = HashMap::new();
    metadata.insert("login".to_string(), json!("api"));
    metadata.insert("token_env".to_string(), json!(token_env));
    let auth = ProviderAuth {
        auth_type: "api".to_string(),
        key: Some(key.to_string()),
        access: None,
        refresh: None,
        expires: None,
        account_id: None,
        metadata: Some(metadata),
    };
    let Json(saved) = set_auth(Path(provider_id.to_string()), Json(auth)).await;
    saved
}

fn assert_persisted_api_auth(
    env_content: &str,
    provider_auth: &serde_json::Map<String, serde_json::Value>,
    provider_id: &str,
    key: &str,
) {
    let token_env = format!("{}_API_KEY", provider_id.to_ascii_uppercase());
    let login_env = format!("{}_LOGIN", provider_id.to_ascii_uppercase());
    assert!(
        env_content.contains(&format!("{token_env}=\"{key}\"")),
        "env file should keep token for {provider_id}; content:\n{env_content}"
    );
    assert!(
        env_content.contains(&format!("{login_env}=\"api\"")),
        "env file should keep login for {provider_id}; content:\n{env_content}"
    );
    let entry = provider_auth
        .get(provider_id)
        .unwrap_or_else(|| panic!("missing provider auth entry for {provider_id}"));
    assert_eq!(entry["login"], "api");
    assert_eq!(entry["status"], "connected");
    assert_eq!(entry["token_env"], token_env);
    assert_eq!(entry["login_env"], login_env);
}

fn assert_revoked_api_auth(
    env_content: &str,
    provider_auth: &serde_json::Map<String, serde_json::Value>,
    provider_id: &str,
) {
    let token_env = format!("{}_API_KEY", provider_id.to_ascii_uppercase());
    let login_env = format!("{}_LOGIN", provider_id.to_ascii_uppercase());
    assert!(
        env_content.contains(&format!("{token_env}=\"\"")),
        "env file should clear token for logged out {provider_id}; content:\n{env_content}"
    );
    assert!(
        env_content.contains(&format!("{login_env}=\"\"")),
        "env file should clear login for logged out {provider_id}; content:\n{env_content}"
    );
    let entry = provider_auth
        .get(provider_id)
        .unwrap_or_else(|| panic!("missing provider auth entry for logged out {provider_id}"));
    assert_eq!(entry["status"], "revoked");
    assert_eq!(entry["token_env"], token_env);
    assert_eq!(entry["login_env"], login_env);
}

#[derive(Debug)]
struct CapturedTokenRequest {
    method: String,
    path: String,
    form: HashMap<String, String>,
}

struct LocalTokenServer {
    addr: std::net::SocketAddr,
    handle: thread::JoinHandle<CapturedTokenRequest>,
}

impl LocalTokenServer {
    fn start(body: serde_json::Value) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind token server");
        let addr = listener.local_addr().expect("token server addr");
        let handle = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept token request");
            let request = read_token_request(&mut stream);
            let body = body.to_string();
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                body.len(),
                body
            );
            stream
                .write_all(response.as_bytes())
                .expect("write token response");
            request
        });
        Self { addr, handle }
    }

    fn url(&self) -> String {
        format!("http://{}/token", self.addr)
    }

    fn join(self) -> CapturedTokenRequest {
        self.handle.join().expect("token server joins")
    }
}

#[derive(Debug)]
struct CapturedModelRequest {
    method: String,
    path: String,
    headers: String,
}

struct LocalModelServer {
    addr: std::net::SocketAddr,
    handle: thread::JoinHandle<CapturedModelRequest>,
}

impl LocalModelServer {
    fn start(status: u16, body: String) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind model server");
        let addr = listener.local_addr().expect("model server addr");
        let handle = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept model request");
            let request = read_model_request(&mut stream);
            let response = format!(
                "HTTP/1.1 {status} OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                body.len(),
                body
            );
            stream
                .write_all(response.as_bytes())
                .expect("write model response");
            request
        });
        Self { addr, handle }
    }

    fn base_url(&self) -> String {
        format!("http://{}", self.addr)
    }

    fn join(self) -> CapturedModelRequest {
        self.handle.join().expect("model server joins")
    }
}

fn read_model_request(stream: &mut TcpStream) -> CapturedModelRequest {
    let mut buffer = Vec::new();
    let mut temp = [0u8; 4096];
    loop {
        let read = stream.read(&mut temp).expect("read model request");
        if read == 0 {
            break;
        }
        buffer.extend_from_slice(&temp[..read]);
        if let Some(headers_end) = find_headers_end(&buffer) {
            let headers = String::from_utf8_lossy(&buffer[..headers_end]).to_string();
            let request_line = headers.lines().next().unwrap_or_default();
            let mut parts = request_line.split_whitespace();
            return CapturedModelRequest {
                method: parts.next().unwrap_or_default().to_string(),
                path: parts.next().unwrap_or_default().to_string(),
                headers: headers.to_ascii_lowercase(),
            };
        }
    }
    panic!("model request did not contain complete headers")
}

fn read_token_request(stream: &mut TcpStream) -> CapturedTokenRequest {
    let mut buffer = Vec::new();
    let mut temp = [0u8; 4096];
    loop {
        let read = stream.read(&mut temp).expect("read token request");
        if read == 0 {
            break;
        }
        buffer.extend_from_slice(&temp[..read]);
        if let Some(headers_end) = find_headers_end(&buffer) {
            let headers = String::from_utf8_lossy(&buffer[..headers_end]).to_string();
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
            let body_start = headers_end + 4;
            while buffer.len() < body_start + content_length {
                let read = stream.read(&mut temp).expect("read token body");
                if read == 0 {
                    break;
                }
                buffer.extend_from_slice(&temp[..read]);
            }
            let request_line = headers.lines().next().unwrap_or_default();
            let mut parts = request_line.split_whitespace();
            let method = parts.next().unwrap_or_default().to_string();
            let path = parts.next().unwrap_or_default().to_string();
            let body = String::from_utf8_lossy(&buffer[body_start..body_start + content_length]);
            return CapturedTokenRequest {
                method,
                path,
                form: parse_form_body(&body),
            };
        }
    }
    panic!("token request did not contain complete headers")
}

fn find_headers_end(buffer: &[u8]) -> Option<usize> {
    buffer.windows(4).position(|window| window == b"\r\n\r\n")
}

fn parse_form_body(body: &str) -> HashMap<String, String> {
    body.split('&')
        .filter_map(|pair| pair.split_once('='))
        .map(|(key, value)| (percent_decode(key), percent_decode(value)))
        .collect()
}

fn percent_decode(value: &str) -> String {
    let mut out = Vec::new();
    let bytes = value.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'+' => {
                out.push(b' ');
                index += 1;
            }
            b'%' if index + 2 < bytes.len() => {
                if let Ok(hex) = u8::from_str_radix(&value[index + 1..index + 3], 16) {
                    out.push(hex);
                    index += 3;
                } else {
                    out.push(bytes[index]);
                    index += 1;
                }
            }
            byte => {
                out.push(byte);
                index += 1;
            }
        }
    }
    String::from_utf8(out).expect("form value utf8")
}

fn assert_detail(details: &[ProviderAuthActionDetail], code: &str) {
    assert!(
        details.iter().any(|detail| detail.code == code),
        "missing detail {code}; got {:?}",
        details
            .iter()
            .map(|detail| detail.code.as_str())
            .collect::<Vec<_>>()
    );
}

fn assert_detail_value(details: &[ProviderAuthActionDetail], code: &str, expected_value: &str) {
    let detail = details
        .iter()
        .find(|detail| detail.code == code)
        .unwrap_or_else(|| panic!("missing detail {code}"));
    assert_eq!(detail.value.as_deref(), Some(expected_value));
}

fn copy_provider_config(path: &FsPath) {
    std::fs::copy(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("workspace crates dir")
            .join("provider")
            .join("config")
            .join("provider_config.json"),
        path,
    )
    .expect("copy provider config");
}

fn write_local_provider_config(path: &FsPath, base_url: &str) {
    let config = json!({
        "provider_base_url": {},
        "routes": {},
        "provider_auth": {},
        "model_catalog": {
            "providers": {
                "business-local": {
                    "display_name": "Business Local Provider",
                    "runtime_provider": "openai",
                    "api_style": "openapi",
                    "base_url": base_url,
                    "token_env": "BUSINESS_LOCAL_API_KEY",
                    "env": ["BUSINESS_LOCAL_API_KEY"],
                    "domains": ["llm"],
                    "models": {
                        "fast": ["business-local-model"]
                    }
                }
            }
        }
    });
    std::fs::write(
        path,
        format!(
            "{}\n",
            serde_json::to_string_pretty(&config).expect("provider config json")
        ),
    )
    .expect("write local provider config");
}

struct EnvGuard {
    previous: Vec<(&'static str, Option<std::ffi::OsString>)>,
}

impl EnvGuard {
    fn new(values: &[(&'static str, Option<String>)]) -> Self {
        let previous = values
            .iter()
            .map(|(key, _)| (*key, std::env::var_os(key)))
            .collect::<Vec<_>>();
        for (key, value) in values {
            match value {
                Some(value) => std::env::set_var(key, value),
                None => std::env::remove_var(key),
            }
        }
        Self { previous }
    }
}

struct DynamicEnvGuard {
    previous: Vec<(String, Option<OsString>)>,
}

impl DynamicEnvGuard {
    fn capture(keys: Vec<String>) -> Self {
        let previous = keys
            .into_iter()
            .map(|key| {
                let value = std::env::var_os(&key);
                std::env::remove_var(&key);
                (key, value)
            })
            .collect();
        Self { previous }
    }
}

impl Drop for DynamicEnvGuard {
    fn drop(&mut self) {
        for (key, value) in self.previous.drain(..) {
            match value {
                Some(value) => std::env::set_var(key, value),
                None => std::env::remove_var(key),
            }
        }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        for (key, value) in self.previous.drain(..) {
            match value {
                Some(value) => std::env::set_var(key, value),
                None => std::env::remove_var(key),
            }
        }
    }
}

#[tokio::test]
async fn oauth_business_redirect_flow_rejects_non_pkce_pending_without_network_exchange() {
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
