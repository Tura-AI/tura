use axum::body::to_bytes;
use axum::http::{Method, Request, StatusCode};
use gateway::api::types::ProviderAuth;
use gateway::mock::store::global_store;
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use tokio::sync::Mutex;
use tower::ServiceExt;

static ENV_LOCK: Mutex<()> = Mutex::const_new(());

async fn json_request(method: Method, uri: &str, body: Value) -> Value {
    let request = Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json")
        .body(axum::body::Body::from(body.to_string()))
        .expect("request");
    let response = gateway::web::build_router()
        .oneshot(request)
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = to_bytes(response.into_body(), 1024 * 1024)
        .await
        .expect("body");
    serde_json::from_slice(&bytes).expect("json")
}

#[tokio::test]
async fn provider_auth_surface_exposes_login_and_key_urls() {
    let _guard = ENV_LOCK.lock().await;
    let body = json_request(Method::GET, "/provider/auth", Value::Null).await;

    let openai_api = &body["openai"][0];
    assert_eq!(openai_api["type"], "api");
    assert_eq!(openai_api["kind"], "api_key");
    assert_eq!(openai_api["token_env"], "OPENAI_API_KEY");
    assert_eq!(
        openai_api["api_key_url"],
        "https://platform.openai.com/api-keys"
    );

    let codex = &body["codex"][0];
    assert_eq!(codex["type"], "oauth");
    assert_eq!(codex["kind"], "oauth_pkce");
    assert_eq!(
        codex["authorize_url"],
        "https://auth.openai.com/oauth/authorize"
    );
    assert_eq!(codex["token_url"], "https://auth.openai.com/oauth/token");
    assert_eq!(codex["available"], true);
    assert_eq!(codex["supports_refresh"], true);

    let google = &body["google"][0];
    assert_eq!(google["type"], "oauth");
    assert_eq!(google["kind"], "oauth_pkce");
    assert_eq!(
        google["authorize_url"],
        "https://accounts.google.com/o/oauth2/v2/auth"
    );
    assert_eq!(google["token_url"], "https://oauth2.googleapis.com/token");

    let anthropic = &body["anthropic"][0];
    assert_eq!(anthropic["type"], "token");
    assert_eq!(anthropic["kind"], "browser_token");
    assert_eq!(anthropic["login"], "browser");
    assert_eq!(anthropic["available"], true);

    let claude_code = &body["claude-code"][0];
    assert_eq!(claude_code["type"], "oauth");
    assert_eq!(claude_code["kind"], "local_cli_token");
    assert_eq!(claude_code["login"], "local");
    assert_eq!(claude_code["token_env"], "CLAUDE_CODE_OAUTH_TOKEN");
    assert_eq!(claude_code["supports_refresh"], true);

    let mut oauth_providers = body
        .as_object()
        .expect("auth methods object")
        .iter()
        .filter_map(|(provider, methods)| {
            methods
                .as_array()
                .is_some_and(|methods| methods.iter().any(|method| method["type"] == "oauth"))
                .then_some(provider.as_str())
        })
        .collect::<Vec<_>>();
    oauth_providers.sort_unstable();
    assert_eq!(
        oauth_providers,
        vec![
            "Antigravity",
            "antigravity",
            "claude-code",
            "codex",
            "gemini",
            "google"
        ]
    );
}

#[tokio::test]
async fn claude_code_oauth_authorize_builds_pkce_url() {
    let _guard = ENV_LOCK.lock().await;
    let body = json_request(
        Method::POST,
        "/provider/claude-code/oauth/authorize",
        serde_json::json!({ "method": 0 }),
    )
    .await;

    assert_eq!(body["method"], "code");
    let url = body["url"].as_str().expect("authorize url");
    assert!(url.starts_with("https://claude.ai/oauth/authorize?"));
    assert!(url.contains("client_id=9d1c250a-e61b-44d9-88ed-5944d1962f5e"));
    assert!(url.contains("response_type=code"));
    assert!(url.contains("code=true"));
    assert!(url.contains("code_challenge_method=S256"));
    assert!(url.contains("user%3Ainference"));
    assert!(url.contains("user%3Asessions%3Aclaude_code"));
}

#[tokio::test]
async fn google_oauth_authorize_builds_pkce_url() {
    let _guard = ENV_LOCK.lock().await;
    let original_client_id = std::env::var("GOOGLE_OAUTH_CLIENT_ID").ok();
    let original_redirect_uri = std::env::var("GOOGLE_OAUTH_REDIRECT_URI").ok();
    std::env::set_var("GOOGLE_OAUTH_CLIENT_ID", "google-client-id");
    std::env::set_var(
        "GOOGLE_OAUTH_REDIRECT_URI",
        "http://localhost:1455/auth/callback",
    );

    let body = json_request(
        Method::POST,
        "/provider/google/oauth/authorize",
        serde_json::json!({ "method": 0 }),
    )
    .await;

    assert_eq!(body["method"], "auto");
    let url = body["url"].as_str().expect("authorize url");
    assert!(url.starts_with("https://accounts.google.com/o/oauth2/v2/auth?"));
    assert!(url.contains("client_id=google-client-id"));
    assert!(url.contains("response_type=code"));
    assert!(url.contains("access_type=offline"));
    assert!(url.contains("code_challenge_method=S256"));
    assert!(url.contains("https%3A%2F%2Fwww.googleapis.com%2Fauth%2Fcloud-platform"));

    restore_env("GOOGLE_OAUTH_CLIENT_ID", original_client_id);
    restore_env("GOOGLE_OAUTH_REDIRECT_URI", original_redirect_uri);
}

#[tokio::test]
async fn antigravity_oauth_authorize_builds_google_pkce_url() {
    let _guard = ENV_LOCK.lock().await;
    let original_client_id = std::env::var("ANTIGRAVITY_OAUTH_CLIENT_ID").ok();
    let original_redirect_uri = std::env::var("ANTIGRAVITY_OAUTH_REDIRECT_URI").ok();
    let original_scope = std::env::var("ANTIGRAVITY_OAUTH_SCOPE").ok();
    std::env::set_var("ANTIGRAVITY_OAUTH_CLIENT_ID", "antigravity-client-id");
    std::env::set_var(
        "ANTIGRAVITY_OAUTH_REDIRECT_URI",
        "http://localhost:1455/auth/callback",
    );
    std::env::set_var(
        "ANTIGRAVITY_OAUTH_SCOPE",
        "openid email profile https://www.googleapis.com/auth/cloud-platform",
    );

    let body = json_request(
        Method::POST,
        "/provider/antigravity/oauth/authorize",
        serde_json::json!({ "method": 0 }),
    )
    .await;

    assert_eq!(body["method"], "auto");
    let url = body["url"].as_str().expect("authorize url");
    assert!(url.starts_with("https://accounts.google.com/o/oauth2/v2/auth?"));
    assert!(url.contains("client_id=antigravity-client-id"));
    assert!(url.contains("response_type=code"));
    assert!(url.contains("access_type=offline"));
    assert!(url.contains("code_challenge_method=S256"));
    assert!(url.contains("https%3A%2F%2Fwww.googleapis.com%2Fauth%2Fcloud-platform"));

    restore_env("ANTIGRAVITY_OAUTH_CLIENT_ID", original_client_id);
    restore_env("ANTIGRAVITY_OAUTH_REDIRECT_URI", original_redirect_uri);
    restore_env("ANTIGRAVITY_OAUTH_SCOPE", original_scope);
}

#[tokio::test]
async fn google_oauth_without_client_id_returns_clear_unavailable_state() {
    let _guard = ENV_LOCK.lock().await;
    let original_client_id = std::env::var("GOOGLE_OAUTH_CLIENT_ID").ok();
    let original_env_path = std::env::var("TURA_ENV_PATH").ok();
    std::env::set_var("GOOGLE_OAUTH_CLIENT_ID", "");
    let env_path = empty_env_file("google-oauth-missing-client");
    std::env::set_var("TURA_ENV_PATH", &env_path);

    let auth = json_request(Method::GET, "/provider/auth", Value::Null).await;
    let google = &auth["google"][0];
    assert_eq!(google["type"], "oauth");
    assert_eq!(google["available"], false);
    assert!(google["unavailable_reason"]
        .as_str()
        .unwrap_or_default()
        .contains("GOOGLE_OAUTH_CLIENT_ID"));

    let authorize = json_request(
        Method::POST,
        "/provider/google/oauth/authorize",
        serde_json::json!({ "method": 0 }),
    )
    .await;
    assert_eq!(authorize["method"], "code");
    assert_eq!(authorize["url"], "");
    assert!(authorize["instructions"]
        .as_str()
        .unwrap_or_default()
        .contains("GOOGLE_OAUTH_CLIENT_ID"));

    restore_env("GOOGLE_OAUTH_CLIENT_ID", original_client_id);
    restore_env("TURA_ENV_PATH", original_env_path);
    let _ = fs::remove_file(env_path);
}

#[tokio::test]
async fn browser_token_providers_do_not_return_fake_oauth_authorize_urls() {
    let _guard = ENV_LOCK.lock().await;
    for provider in ["anthropic"] {
        let authorize = json_request(
            Method::POST,
            &format!("/provider/{provider}/oauth/authorize"),
            serde_json::json!({ "method": 0 }),
        )
        .await;
        assert_eq!(authorize["method"], "code");
        assert_eq!(authorize["url"], "");
        assert_eq!(authorize["instructions"], "Invalid auth method");
    }
}

#[tokio::test]
async fn empty_auto_oauth_callback_waits_without_consuming_codex_state() {
    let _guard = ENV_LOCK.lock().await;
    let authorize = json_request(
        Method::POST,
        "/provider/codex/oauth/authorize",
        serde_json::json!({ "method": 0 }),
    )
    .await;
    assert_eq!(authorize["method"], "auto");
    assert!(authorize["url"]
        .as_str()
        .unwrap_or_default()
        .contains("state="));

    global_store().set_oauth_completed(
        "codex",
        ProviderAuth {
            auth_type: "oauth".to_string(),
            key: Some("access-token".to_string()),
            access: Some("access-token".to_string()),
            refresh: Some("refresh-token".to_string()),
            expires: None,
            account_id: None,
            metadata: Some(HashMap::new()),
        },
    );

    let callback = json_request(
        Method::POST,
        "/provider/codex/oauth/callback",
        serde_json::json!({ "method": 0 }),
    )
    .await;
    assert_eq!(callback, Value::Bool(true));
}

fn restore_env(key: &str, value: Option<String>) {
    if let Some(value) = value {
        std::env::set_var(key, value);
    } else {
        std::env::remove_var(key);
    }
}

fn empty_env_file(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "tura-{name}-{}-{}.env",
        std::process::id(),
        chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default()
    ));
    fs::write(&path, "\n").expect("write empty env file");
    path
}
