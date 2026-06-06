use super::auth_validation::{
    validate_provider_credentials_remotely, ProviderCredentialValidation,
};
use super::catalog::{looks_like_claude_model, provider_list_for_route};
use super::*;
use axum::extract::{Path, Query};
use std::io::{Read, Write};
use tokio::sync::Mutex;

static ENV_LOCK: Mutex<()> = Mutex::const_new(());

#[test]
fn provider_auth_methods_are_projected_from_registry() {
    let codex = provider_auth_methods("codex");
    assert_eq!(codex.len(), 1);
    assert_eq!(codex[0].kind, AuthMethodKind::OAuthPkce);
    assert_eq!(codex[0].method_type, "oauth");
    assert_eq!(codex[0].token_env.as_deref(), Some("OPENAI_API_KEY"));

    let openai = provider_auth_methods("openai");
    assert_eq!(openai[0].kind, AuthMethodKind::ApiKey);
    assert_eq!(openai[0].method_type, "api");

    let anthropic = provider_auth_methods("anthropic");
    assert_eq!(anthropic[0].kind, AuthMethodKind::ApiKey);
    assert_eq!(anthropic[0].method_type, "api");
    assert_eq!(anthropic[0].login, "api");

    let claude_code = provider_auth_methods("claude-code");
    assert_eq!(claude_code[0].kind, AuthMethodKind::LocalCliToken);
    assert_eq!(claude_code[0].method_type, "oauth");
    assert_eq!(
        claude_code[0].token_env.as_deref(),
        Some("CLAUDE_CODE_OAUTH_TOKEN")
    );

    let openrouter = provider_auth_methods("openrouter");
    assert_eq!(openrouter[0].kind, AuthMethodKind::ApiKey);
    assert_eq!(openrouter[0].method_type, "api");
    assert_eq!(
        openrouter[0].token_env.as_deref(),
        Some("OPENROUTER_API_KEY")
    );
}

#[test]
fn provider_env_keys_use_registry_compatibility_aliases() {
    assert_eq!(provider_env_key("openai-api"), "OPENAI_API_KEY");
    assert_eq!(provider_login_key("anthropic-api"), "ANTHROPIC_LOGIN");
    assert_eq!(provider_env_key("gemini-api"), "GEMINI_API_KEY");
}

#[tokio::test]
async fn provider_auth_method_value_is_available_for_hover_reveal() {
    let _guard = ENV_LOCK.lock().await;
    clear_openai_refresh_test_env();
    set_env("OPENAI_LOGIN", "oauth");
    set_env("OPENAI_API_KEY", "sk-test-hover-reveal");

    let openai = provider_auth_methods("openai");
    assert_eq!(
        openai[0].configured_value.as_deref(),
        Some("sk-test-hover-reveal")
    );

    let openai_api = provider_auth_methods("openai-api");
    assert_eq!(
        openai_api[0].configured_value.as_deref(),
        Some("sk-test-hover-reveal")
    );

    clear_openai_refresh_test_env();
}

#[tokio::test]
async fn non_llm_openapi_catalog_provider_does_not_use_llm_models_validator() {
    let provider = tura_llm_rust::ProviderCatalogConfig {
        api_style: "openapi".to_string(),
        base_url: "https://example.com/v1".to_string(),
        domains: vec!["infrastructure".to_string()],
        ..Default::default()
    };

    let validation = validate_provider_credentials_remotely(
        "example_infrastructure",
        Some(&provider),
        Some(&provider.base_url),
        Some("fake-token"),
    )
    .await;

    assert!(matches!(
        validation,
        ProviderCredentialValidation::Unsupported(detail)
            if detail.code == "provider.validation.gateway_not_configured"
    ));
}

#[tokio::test]
async fn provider_list_projects_non_llm_catalog_entries() {
    let _guard = ENV_LOCK.lock().await;
    let settings = tura_llm_rust::Settings::default()
        .await
        .expect("load settings");
    let route = settings
        .route_by_name("fast")
        .expect("fast route should be configured");
    let response = provider_list_for_route(settings.as_ref(), route);
    let feishu = response
        .all
        .iter()
        .find(|provider| provider.id == "feishu")
        .expect("Feishu service provider should be listed");
    assert!(response
        .enums
        .domains
        .iter()
        .any(|domain| domain == "communication"));
    assert!(response.enums.api_styles.iter().any(|style| style == "mcp"));
    assert!(feishu.models.is_empty());
    assert_eq!(
        feishu.api.as_deref(),
        Some("https://open.feishu.cn/open-apis")
    );
    assert_eq!(
        feishu
            .options
            .get("domains")
            .and_then(|value| value.as_array())
            .and_then(|items| items.first())
            .and_then(|value| value.as_str()),
        Some("productivity")
    );

    let line = response
        .all
        .iter()
        .find(|provider| provider.id == "line")
        .expect("LINE service provider should be listed");
    assert!(line.models.is_empty());
    assert_eq!(line.api.as_deref(), Some("https://api.line.me/v2/bot"));
    assert!(line
        .env
        .iter()
        .any(|env| env == "LINE_CHANNEL_ACCESS_TOKEN"));
    assert_eq!(
        line.options
            .get("auth_methods")
            .and_then(|value| value.as_array())
            .and_then(|items| items.first())
            .and_then(|value| value.as_str()),
        Some("channel_access_token")
    );

    let duckduckgo = response
        .all
        .iter()
        .find(|provider| provider.id == "duckduckgo_search")
        .expect("DuckDuckGo search provider should be listed");
    assert!(duckduckgo.models.is_empty());
    assert_eq!(
        duckduckgo.api.as_deref(),
        Some("https://html.duckduckgo.com/html/")
    );
    assert!(duckduckgo
        .env
        .iter()
        .any(|env| env == "TURA_DUCKDUCKGO_SEARCH_ENDPOINT"));
    assert_eq!(
        duckduckgo
            .options
            .get("api_style")
            .and_then(|value| value.as_str()),
        Some("duckduckgo_html")
    );

    let exa = response
        .all
        .iter()
        .find(|provider| provider.id == "exa_search")
        .expect("Exa search provider should be listed");
    assert!(exa.models.is_empty());
    assert_eq!(exa.api.as_deref(), Some("https://mcp.exa.ai/mcp"));
    assert!(exa.env.iter().any(|env| env == "TURA_EXA_MCP_ENDPOINT"));
    assert_eq!(
        exa.options
            .get("api_style")
            .and_then(|value| value.as_str()),
        Some("mcp")
    );
}

#[tokio::test]
async fn provider_list_hides_claude_models_from_picker_catalog() {
    let _guard = ENV_LOCK.lock().await;
    let settings = tura_llm_rust::Settings::default()
        .await
        .expect("load settings");
    let route = settings
        .route_by_name("flagship_thinking")
        .expect("flagship_thinking route should be configured");
    let response = provider_list_for_route(settings.as_ref(), route);

    for (provider_id, model_id) in &response.default {
        assert!(
            !looks_like_claude_model(provider_id, model_id),
            "hidden Claude model leaked into provider default: {provider_id}/{model_id}"
        );
    }
    for provider in &response.all {
        for model in provider.models.values() {
            let id = model.id.to_ascii_lowercase();
            let family = model.family.to_ascii_lowercase();
            assert!(
                !id.contains("claude") && family != "claude",
                "hidden Claude model leaked into provider picker: {}/{}",
                provider.id,
                model.id
            );
        }
    }
}

#[tokio::test]
async fn provider_list_returns_configured_key_value() {
    let _guard = ENV_LOCK.lock().await;
    std::env::set_var("LINE_CHANNEL_ACCESS_TOKEN", "line-test-token");

    let settings = tura_llm_rust::Settings::default()
        .await
        .expect("load settings");
    let route = settings
        .route_by_name("fast")
        .expect("fast route should be configured");
    let response = provider_list_for_route(settings.as_ref(), route);
    let line = response
        .all
        .iter()
        .find(|provider| provider.id == "line")
        .expect("LINE service provider should be listed");
    assert_eq!(line.key.as_deref(), Some("line-test-token"));

    std::env::remove_var("LINE_CHANNEL_ACCESS_TOKEN");
}

#[test]
fn login_value_for_auth_prefers_metadata_and_registry_defaults() {
    let auth = ProviderAuth {
        auth_type: "oauth".to_string(),
        key: Some("secret".to_string()),
        access: None,
        refresh: None,
        expires: None,
        account_id: None,
        metadata: None,
    };
    assert_eq!(login_value_for_auth("anthropic", &auth), "api");

    let with_metadata = ProviderAuth {
        metadata: Some(HashMap::from([(
            "login".to_string(),
            serde_json::Value::String("oauth".to_string()),
        )])),
        ..auth
    };
    assert_eq!(login_value_for_auth("anthropic", &with_metadata), "oauth");
}

#[tokio::test]
async fn provider_auth_refresh_updates_expired_openai_oauth_env_and_config() {
    let _guard = ENV_LOCK.lock().await;
    let temp_dir = std::env::temp_dir().join(format!(
        "tura-openai-oauth-refresh-test-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).expect("temp dir");
    let env_path = temp_dir.join(".env");
    let config_path = temp_dir.join("provider_config.json");
    std::fs::write(
            &env_path,
            "OPENAI_LOGIN=oauth\nOPENAI_API_KEY=old-access\nOPENAI_REFRESH_TOKEN=old-refresh\nOPENAI_TOKEN_EXPIRES=0\n",
        )
        .expect("env");
    std::fs::write(&config_path, r#"{"provider_auth":{}}"#).expect("config");

    let (addr, server) = spawn_openai_token_server(
        "old-refresh",
        r#"{"access_token":"new-access","refresh_token":"new-refresh","expires_in":3600}"#,
    );

    set_env("TURA_ENV_PATH", env_path.to_string_lossy().as_ref());
    set_env("TURALLM_CONFIG", config_path.to_string_lossy().as_ref());
    set_env(
        "OPENAI_OAUTH_TOKEN_URL",
        &format!("http://{addr}/oauth/token"),
    );
    set_env("OPENAI_LOGIN", "oauth");
    set_env("OPENAI_API_KEY", "old-access");
    set_env("OPENAI_REFRESH_TOKEN", "old-refresh");
    set_env("OPENAI_TOKEN_EXPIRES", "0");
    assert_eq!(
        std::env::var("OPENAI_REFRESH_TOKEN").as_deref(),
        Ok("old-refresh")
    );

    let Json(response) = provider_auth_refresh(Path("codex".to_string())).await;

    assert!(response.ok, "{}", response.message);
    assert_eq!(
        response.status.as_ref().map(|status| status.authenticated),
        Some(true)
    );
    assert_eq!(std::env::var("OPENAI_API_KEY").as_deref(), Ok("new-access"));
    assert_eq!(
        std::env::var("OPENAI_REFRESH_TOKEN").as_deref(),
        Ok("new-refresh")
    );
    assert!(std::env::var("OPENAI_TOKEN_EXPIRES")
        .ok()
        .and_then(|value| value.parse::<i64>().ok())
        .is_some_and(|expires| expires > Utc::now().timestamp_millis()));
    let config = std::fs::read_to_string(&config_path).expect("read config");
    assert!(
        config.contains("\"status\": \"connected\"") || config.contains("\"status\":\"connected\"")
    );
    assert!(config.contains("OPENAI_REFRESH_TOKEN"));
    server.join().expect("token server should finish");

    clear_openai_refresh_test_env();
    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[tokio::test]
async fn gateway_oauth_callback_get_completes_all_pkce_logins() {
    let _guard = ENV_LOCK.lock().await;
    for case in [
        PkceCallbackCase {
            provider_id: "codex",
            token_url_env: "OPENAI_OAUTH_TOKEN_URL",
            token_env: "OPENAI_API_KEY",
            refresh_env: "OPENAI_REFRESH_TOKEN",
            extra_env: &[],
        },
        PkceCallbackCase {
            provider_id: "claude-code",
            token_url_env: "ANTHROPIC_OAUTH_TOKEN_URL",
            token_env: "CLAUDE_CODE_OAUTH_TOKEN",
            refresh_env: "CLAUDE_CODE_REFRESH_TOKEN",
            extra_env: &[],
        },
        PkceCallbackCase {
            provider_id: "google",
            token_url_env: "GOOGLE_OAUTH_TOKEN_URL",
            token_env: "GOOGLE_API_KEY",
            refresh_env: "GOOGLE_REFRESH_TOKEN",
            extra_env: &[
                ("GOOGLE_OAUTH_CLIENT_ID", "google-client"),
                ("GOOGLE_OAUTH_CLIENT_SECRET", "google-secret"),
            ],
        },
    ] {
        run_pkce_callback_case(case).await;
    }
}

#[tokio::test]
async fn provider_auth_status_refreshes_expired_openai_oauth_before_reporting() {
    let _guard = ENV_LOCK.lock().await;
    let temp_dir = std::env::temp_dir().join(format!(
        "tura-openai-oauth-status-refresh-test-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).expect("temp dir");
    let env_path = temp_dir.join(".env");
    let config_path = temp_dir.join("provider_config.json");
    std::fs::write(
            &env_path,
            "OPENAI_LOGIN=oauth\nOPENAI_API_KEY=expired-access\nOPENAI_REFRESH_TOKEN=status-refresh-token\nOPENAI_TOKEN_EXPIRES=0\n",
        )
        .expect("env");
    std::fs::write(&config_path, r#"{"provider_auth":{}}"#).expect("config");

    let (addr, server) = spawn_openai_token_server(
        "status-refresh-token",
        r#"{"access_token":"status-access","expires_in":3600}"#,
    );

    set_env("TURA_ENV_PATH", env_path.to_string_lossy().as_ref());
    set_env("TURALLM_CONFIG", config_path.to_string_lossy().as_ref());
    set_env(
        "OPENAI_OAUTH_TOKEN_URL",
        &format!("http://{addr}/oauth/token"),
    );
    set_env("OPENAI_LOGIN", "oauth");
    set_env("OPENAI_API_KEY", "expired-access");
    set_env("OPENAI_REFRESH_TOKEN", "status-refresh-token");
    set_env("OPENAI_TOKEN_EXPIRES", "0");
    assert_eq!(
        std::env::var("OPENAI_REFRESH_TOKEN").as_deref(),
        Ok("status-refresh-token")
    );

    let Json(status) = provider_auth_status(Path("codex".to_string())).await;

    assert!(status.authenticated);
    assert_eq!(status.expired, Some(false));
    assert_eq!(status.auth_state, tura_llm_rust::AuthState::Authenticated);
    assert_eq!(
        status.runtime_state,
        tura_llm_rust::ProviderRuntimeState::Ready
    );
    assert_eq!(
        std::env::var("OPENAI_API_KEY").as_deref(),
        Ok("status-access")
    );
    assert_eq!(
        std::env::var("OPENAI_REFRESH_TOKEN").as_deref(),
        Ok("status-refresh-token")
    );
    server.join().expect("token server should finish");

    clear_openai_refresh_test_env();
    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[tokio::test]
async fn provider_auth_refresh_covers_google_and_gemini_oauth_methods() {
    let _guard = ENV_LOCK.lock().await;

    for case in [
        OAuthRefreshCase {
            provider_id: "google",
            login_env: "GOOGLE_LOGIN",
            token_env: "GOOGLE_API_KEY",
            refresh_env: "GOOGLE_REFRESH_TOKEN",
            expires_env: "GOOGLE_TOKEN_EXPIRES",
            old_access: "google-expired-access",
            new_access: "google-new-access",
        },
        OAuthRefreshCase {
            provider_id: "gemini",
            login_env: "GEMINI_LOGIN",
            token_env: "GEMINI_API_KEY",
            refresh_env: "GOOGLE_REFRESH_TOKEN",
            expires_env: "GOOGLE_TOKEN_EXPIRES",
            old_access: "gemini-expired-access",
            new_access: "gemini-new-access",
        },
        OAuthRefreshCase {
            provider_id: "antigravity",
            login_env: "ANTIGRAVITY_LOGIN",
            token_env: "ANTIGRAVITY_API_KEY",
            refresh_env: "ANTIGRAVITY_REFRESH_TOKEN",
            expires_env: "ANTIGRAVITY_TOKEN_EXPIRES",
            old_access: "antigravity-expired-access",
            new_access: "antigravity-new-access",
        },
    ] {
        clear_openai_refresh_test_env();
        let temp_dir = std::env::temp_dir().join(format!(
            "tura-{provider}-oauth-refresh-test-{}",
            std::process::id(),
            provider = case.provider_id
        ));
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).expect("temp dir");
        let env_path = temp_dir.join(".env");
        let config_path = temp_dir.join("provider_config.json");
        std::fs::write(
                &env_path,
                format!(
                    "{login_env}=oauth\n{token_env}={old_access}\n{refresh_env}={refresh}\n{expires_env}=0\n",
                    login_env = case.login_env,
                    token_env = case.token_env,
                    refresh_env = case.refresh_env,
                    expires_env = case.expires_env,
                    old_access = case.old_access,
                    refresh = case.refresh_token()
                ),
            )
            .expect("env");
        std::fs::write(&config_path, r#"{"provider_auth":{}}"#).expect("config");

        let (addr, server) = spawn_openai_token_server(
            case.refresh_token(),
            Box::leak(
                format!(
                    r#"{{"access_token":"{}","refresh_token":"{}","expires_in":3600}}"#,
                    case.new_access,
                    case.refresh_token()
                )
                .into_boxed_str(),
            ),
        );

        set_env("TURA_ENV_PATH", env_path.to_string_lossy().as_ref());
        set_env("TURALLM_CONFIG", config_path.to_string_lossy().as_ref());
        set_env(
            "GOOGLE_OAUTH_TOKEN_URL",
            &format!("http://{addr}/oauth/token"),
        );
        set_env(case.login_env, "oauth");
        set_env(case.token_env, case.old_access);
        set_env(case.refresh_env, case.refresh_token());
        set_env(case.expires_env, "0");

        let Json(response) = provider_auth_refresh(Path(case.provider_id.to_string())).await;

        assert!(response.ok, "{}", response.message);
        assert_eq!(
            std::env::var(case.token_env).as_deref(),
            Ok(case.new_access)
        );
        assert_eq!(
            std::env::var(case.refresh_env).as_deref(),
            Ok(case.refresh_token())
        );
        assert!(std::env::var(case.expires_env)
            .ok()
            .and_then(|value| value.parse::<i64>().ok())
            .is_some_and(|expires| expires > Utc::now().timestamp_millis()));
        let config = std::fs::read_to_string(&config_path).expect("read config");
        assert!(config.contains(case.provider_id));
        assert!(config.contains(case.refresh_env));
        server.join().expect("token server should finish");

        clear_openai_refresh_test_env();
        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}

#[tokio::test]
async fn provider_auth_refresh_covers_claude_code_oauth_method() {
    let _guard = ENV_LOCK.lock().await;
    clear_openai_refresh_test_env();
    let temp_dir = std::env::temp_dir().join(format!(
        "tura-claude-code-oauth-refresh-test-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).expect("temp dir");
    let env_path = temp_dir.join(".env");
    let config_path = temp_dir.join("provider_config.json");
    std::fs::write(
            &env_path,
            "ANTHROPIC_LOGIN=oauth\nCLAUDE_CODE_OAUTH_TOKEN=claude-old-access\nCLAUDE_CODE_REFRESH_TOKEN=claude-refresh-token\nCLAUDE_CODE_TOKEN_EXPIRES=0\n",
        )
        .expect("env");
    std::fs::write(&config_path, r#"{"provider_auth":{}}"#).expect("config");

    let (addr, server) = spawn_openai_token_server(
        "claude-refresh-token",
        r#"{"access_token":"claude-new-access","refresh_token":"claude-new-refresh","expires_in":3600}"#,
    );

    set_env("TURA_ENV_PATH", env_path.to_string_lossy().as_ref());
    set_env("TURALLM_CONFIG", config_path.to_string_lossy().as_ref());
    set_env(
        "ANTHROPIC_OAUTH_TOKEN_URL",
        &format!("http://{addr}/oauth/token"),
    );
    set_env("ANTHROPIC_LOGIN", "oauth");
    set_env("CLAUDE_CODE_OAUTH_TOKEN", "claude-old-access");
    set_env("CLAUDE_CODE_REFRESH_TOKEN", "claude-refresh-token");
    set_env("CLAUDE_CODE_TOKEN_EXPIRES", "0");

    let Json(response) = provider_auth_refresh(Path("claude-code".to_string())).await;

    assert!(response.ok, "{}", response.message);
    assert_eq!(
        std::env::var("CLAUDE_CODE_OAUTH_TOKEN").as_deref(),
        Ok("claude-new-access")
    );
    assert_eq!(
        std::env::var("CLAUDE_CODE_REFRESH_TOKEN").as_deref(),
        Ok("claude-new-refresh")
    );
    assert!(std::env::var("CLAUDE_CODE_TOKEN_EXPIRES")
        .ok()
        .and_then(|value| value.parse::<i64>().ok())
        .is_some_and(|expires| expires > Utc::now().timestamp_millis()));
    let config = std::fs::read_to_string(&config_path).expect("read config");
    assert!(config.contains("claude-code"));
    assert!(config.contains("CLAUDE_CODE_REFRESH_TOKEN"));
    server.join().expect("token server should finish");

    clear_openai_refresh_test_env();
    let _ = std::fs::remove_dir_all(&temp_dir);
}

struct OAuthRefreshCase {
    provider_id: &'static str,
    login_env: &'static str,
    token_env: &'static str,
    refresh_env: &'static str,
    expires_env: &'static str,
    old_access: &'static str,
    new_access: &'static str,
}

impl OAuthRefreshCase {
    fn refresh_token(&self) -> &'static str {
        match self.provider_id {
            "google" => "google-refresh-token",
            "gemini" => "gemini-refresh-token",
            "antigravity" => "antigravity-refresh-token",
            _ => "refresh-token",
        }
    }
}

#[derive(Clone, Copy)]
struct PkceCallbackCase {
    provider_id: &'static str,
    token_url_env: &'static str,
    token_env: &'static str,
    refresh_env: &'static str,
    extra_env: &'static [(&'static str, &'static str)],
}

async fn run_pkce_callback_case(case: PkceCallbackCase) {
    clear_openai_refresh_test_env();
    let temp_dir = std::env::temp_dir().join(format!(
        "tura-{}-oauth-callback-get-test-{}",
        case.provider_id,
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).expect("temp dir");
    let env_path = temp_dir.join(".env");
    let config_path = temp_dir.join("provider_config.json");
    std::fs::write(&env_path, "").expect("env");
    std::fs::write(&config_path, r#"{"provider_auth":{}}"#).expect("config");

    let access: &'static str =
        Box::leak(format!("{}-callback-access", case.provider_id).into_boxed_str());
    let refresh: &'static str =
        Box::leak(format!("{}-callback-refresh", case.provider_id).into_boxed_str());
    let body: &'static str = Box::leak(
        format!(r#"{{"access_token":"{access}","refresh_token":"{refresh}","expires_in":3600}}"#)
            .into_boxed_str(),
    );
    let (addr, server) = spawn_oauth_code_token_server("callback-code", "callback-verifier", body);
    set_env("TURA_ENV_PATH", env_path.to_string_lossy().as_ref());
    set_env("TURALLM_CONFIG", config_path.to_string_lossy().as_ref());
    set_env(case.token_url_env, &format!("http://{addr}/oauth/token"));
    for (key, value) in case.extra_env {
        set_env(key, value);
    }
    global_store().set_oauth_state(
        case.provider_id,
        "oauth_pkce".to_string(),
        None,
        "http://localhost:1455/auth/callback".to_string(),
        Some("callback-state".to_string()),
        Some("callback-verifier".to_string()),
    );

    let html = oauth_callback_info(
        Path(case.provider_id.to_string()),
        Query(OAuthRedirectCallbackParams {
            code: Some("callback-code".to_string()),
            state: Some("callback-state".to_string()),
            error: None,
        }),
    )
    .await;

    assert!(
        html.0.contains("OAuth connected"),
        "{} callback failed: {}",
        case.provider_id,
        html.0
    );
    assert_eq!(std::env::var(case.token_env).as_deref(), Ok(access));
    assert_eq!(std::env::var(case.refresh_env).as_deref(), Ok(refresh));
    server.join().expect("token server should finish");
    clear_openai_refresh_test_env();
    let _ = std::fs::remove_dir_all(&temp_dir);
}

fn spawn_openai_token_server(
    expected_refresh_token: &'static str,
    token_body: &'static str,
) -> (std::net::SocketAddr, std::thread::JoinHandle<()>) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind token server");
    let addr = listener.local_addr().expect("token server addr");
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept refresh request");
        let request = read_http_request(&mut stream);
        let (_, body) = request
            .split_once("\r\n\r\n")
            .expect("refresh request should include body separator");
        assert!(body.contains("grant_type=refresh_token"), "{body}");
        assert!(
            body.contains(&format!("refresh_token={expected_refresh_token}")),
            "{body}"
        );
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
            token_body.len(),
            token_body
        );
        stream
            .write_all(response.as_bytes())
            .expect("write refresh response");
    });
    (addr, server)
}

fn spawn_oauth_code_token_server(
    expected_code: &'static str,
    expected_verifier: &'static str,
    token_body: &'static str,
) -> (std::net::SocketAddr, std::thread::JoinHandle<()>) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind token server");
    let addr = listener.local_addr().expect("token server addr");
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept callback request");
        let request = read_http_request(&mut stream);
        let (_, body) = request
            .split_once("\r\n\r\n")
            .expect("callback request should include body separator");
        assert!(body.contains("grant_type=authorization_code"), "{body}");
        assert!(body.contains(&format!("code={expected_code}")), "{body}");
        assert!(
            body.contains(&format!("code_verifier={expected_verifier}")),
            "{body}"
        );
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
            token_body.len(),
            token_body
        );
        stream
            .write_all(response.as_bytes())
            .expect("write callback response");
    });
    (addr, server)
}

fn read_http_request(stream: &mut std::net::TcpStream) -> String {
    stream
        .set_read_timeout(Some(std::time::Duration::from_secs(2)))
        .expect("set read timeout");
    let mut data = Vec::new();
    let mut buffer = [0_u8; 1024];
    loop {
        let size = stream.read(&mut buffer).expect("read refresh request");
        assert!(
            size > 0,
            "refresh request stream closed before body completed"
        );
        data.extend_from_slice(&buffer[..size]);
        if http_request_complete(&data) {
            return String::from_utf8_lossy(&data).into_owned();
        }
    }
}

fn http_request_complete(data: &[u8]) -> bool {
    let request = String::from_utf8_lossy(data);
    let Some((headers, body)) = request.split_once("\r\n\r\n") else {
        return false;
    };
    let content_length = headers
        .lines()
        .find_map(|line| line.strip_prefix("content-length:"))
        .or_else(|| {
            headers
                .lines()
                .find_map(|line| line.strip_prefix("Content-Length:"))
        })
        .and_then(|value| value.trim().parse::<usize>().ok())
        .unwrap_or(0);
    body.len() >= content_length
}

fn clear_openai_refresh_test_env() {
    for key in [
        "TURA_ENV_PATH",
        "TURA_PROVIDER_CONFIG",
        "TURALLM_CONFIG",
        "OPENAI_OAUTH_TOKEN_URL",
        "OPENAI_LOGIN",
        "OPENAI_API_KEY",
        "OPENAI_REFRESH_TOKEN",
        "OPENAI_TOKEN_EXPIRES",
        "GOOGLE_OAUTH_TOKEN_URL",
        "GOOGLE_OAUTH_CLIENT_ID",
        "GOOGLE_OAUTH_CLIENT_SECRET",
        "GOOGLE_LOGIN",
        "GOOGLE_API_KEY",
        "GEMINI_LOGIN",
        "GEMINI_API_KEY",
        "GOOGLE_REFRESH_TOKEN",
        "GOOGLE_TOKEN_EXPIRES",
        "ANTIGRAVITY_LOGIN",
        "ANTIGRAVITY_API_KEY",
        "ANTIGRAVITY_REFRESH_TOKEN",
        "ANTIGRAVITY_TOKEN_EXPIRES",
        "ANTIGRAVITY_OAUTH_CLIENT_ID",
        "ANTIGRAVITY_OAUTH_CLIENT_SECRET",
        "ANTIGRAVITY_OAUTH_REDIRECT_URI",
        "ANTIGRAVITY_OAUTH_SCOPE",
        "ANTHROPIC_OAUTH_TOKEN_URL",
        "ANTHROPIC_LOGIN",
        "CLAUDE_CODE_OAUTH_TOKEN",
        "CLAUDE_CODE_REFRESH_TOKEN",
        "CLAUDE_CODE_TOKEN_EXPIRES",
    ] {
        std::env::remove_var(key);
    }
}

fn set_env(key: &str, value: &str) {
    std::env::set_var(key, value);
}
