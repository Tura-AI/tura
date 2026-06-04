use base64::Engine;
use reqwest::Url;
use sha2::{Digest, Sha256};
use tura_llm_rust::OAuthAuthorizeKind;
use uuid::Uuid;

use super::config::config_value;

pub(super) fn random_confirmation_code(provider: &str, method: usize) -> String {
    format!("{}-{}", provider, method)
}

pub(super) fn oauth_state() -> String {
    Uuid::new_v4().simple().to_string()
}

pub(super) fn oauth_code_verifier() -> String {
    format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple())
}

pub(super) fn oauth_code_challenge(verifier: &str) -> String {
    let digest = Sha256::digest(verifier.as_bytes());
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(digest)
}

pub(super) fn openai_oauth_client_id() -> String {
    std::env::var("OPENAI_OAUTH_CLIENT_ID")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "app_EMoamEEZ73f0CkXaXp7hrann".to_string())
}

pub(super) fn openai_oauth_redirect_uri() -> String {
    std::env::var("OPENAI_OAUTH_REDIRECT_URI")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "http://localhost:1455/auth/callback".to_string())
}

pub(super) fn provider_google_oauth_redirect_uri(provider_id: &str) -> String {
    let provider_prefix = provider_id.to_ascii_uppercase().replace('-', "_");
    config_value(&format!("{provider_prefix}_OAUTH_REDIRECT_URI"))
        .or_else(|| config_value("GOOGLE_OAUTH_REDIRECT_URI"))
        .unwrap_or_else(|| "http://localhost:1455/auth/callback".to_string())
}

pub(super) fn anthropic_oauth_client_id() -> String {
    std::env::var("ANTHROPIC_OAUTH_CLIENT_ID")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "9d1c250a-e61b-44d9-88ed-5944d1962f5e".to_string())
}

pub(super) fn anthropic_oauth_redirect_uri() -> String {
    std::env::var("ANTHROPIC_OAUTH_REDIRECT_URI")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "http://localhost:1455/callback".to_string())
}

pub(super) fn anthropic_oauth_token_url() -> String {
    std::env::var("ANTHROPIC_OAUTH_TOKEN_URL")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "https://platform.claude.com/v1/oauth/token".to_string())
}

pub(super) fn anthropic_oauth_scope() -> String {
    std::env::var("ANTHROPIC_OAUTH_SCOPE")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| {
            [
                "user:profile",
                "user:inference",
                "user:sessions:claude_code",
                "user:mcp_servers",
            ]
            .join(" ")
        })
}

pub(super) fn openai_oauth_token_url() -> String {
    std::env::var("OPENAI_OAUTH_TOKEN_URL")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "https://auth.openai.com/oauth/token".to_string())
}

pub(super) fn google_oauth_token_url() -> String {
    std::env::var("GOOGLE_OAUTH_TOKEN_URL")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "https://oauth2.googleapis.com/token".to_string())
}

pub(super) fn github_copilot_oauth_client_id() -> Option<String> {
    config_value("GITHUB_COPILOT_CLIENT_ID")
        .or_else(|| config_value("COPILOT_GITHUB_CLIENT_ID"))
        .filter(|value| !value.trim().is_empty())
}

pub(super) fn github_device_code_url() -> String {
    config_value("GITHUB_DEVICE_CODE_URL")
        .unwrap_or_else(|| "https://github.com/login/device/code".to_string())
}

pub(super) fn github_oauth_token_url() -> String {
    config_value("GITHUB_OAUTH_TOKEN_URL")
        .unwrap_or_else(|| "https://github.com/login/oauth/access_token".to_string())
}

pub(super) fn github_copilot_oauth_scope() -> String {
    config_value("GITHUB_COPILOT_OAUTH_SCOPE").unwrap_or_else(|| "read:user".to_string())
}

pub(super) fn google_oauth_client_id(provider_id: &str) -> Option<String> {
    let provider_prefix = provider_id.to_ascii_uppercase().replace('-', "_");
    [
        format!("{provider_prefix}_OAUTH_CLIENT_ID"),
        "GOOGLE_OAUTH_CLIENT_ID".to_string(),
    ]
    .into_iter()
    .find_map(|key| config_value(&key))
}

pub(super) fn google_oauth_client_secret(provider_id: &str) -> Option<String> {
    let provider_prefix = provider_id.to_ascii_uppercase().replace('-', "_");
    [
        format!("{provider_prefix}_OAUTH_CLIENT_SECRET"),
        "GOOGLE_OAUTH_CLIENT_SECRET".to_string(),
    ]
    .into_iter()
    .find_map(|key| config_value(&key))
}

pub(super) fn provider_google_oauth_scope(provider_id: &str) -> String {
    let provider_prefix = provider_id.to_ascii_uppercase().replace('-', "_");
    config_value(&format!("{provider_prefix}_OAUTH_SCOPE"))
        .or_else(|| config_value("GOOGLE_OAUTH_SCOPE"))
        .unwrap_or_else(|| {
            [
                "openid",
                "email",
                "profile",
                "https://www.googleapis.com/auth/cloud-platform",
                "https://www.googleapis.com/auth/generative-language.retriever",
            ]
            .join(" ")
        })
}

pub(super) fn oauth_authorize_url(
    provider_id: &str,
    kind: OAuthAuthorizeKind,
    state: &str,
    code_challenge: &str,
) -> Option<String> {
    match kind {
        OAuthAuthorizeKind::OpenAiPkce => Some(openai_oauth_authorize_url(state, code_challenge)),
        OAuthAuthorizeKind::AnthropicPkce => {
            Some(anthropic_oauth_authorize_url(state, code_challenge))
        }
        OAuthAuthorizeKind::GooglePkce => Some(google_oauth_authorize_url(
            provider_id,
            state,
            code_challenge,
        )?),
        OAuthAuthorizeKind::GithubDevice
        | OAuthAuthorizeKind::BrowserTokenPaste
        | OAuthAuthorizeKind::Unsupported => None,
    }
}

fn anthropic_oauth_authorize_url(state: &str, code_challenge: &str) -> String {
    let client_id = anthropic_oauth_client_id();
    let redirect_uri = anthropic_oauth_redirect_uri();
    let scope = anthropic_oauth_scope();
    Url::parse_with_params(
        "https://claude.ai/oauth/authorize",
        &[
            ("code", "true"),
            ("response_type", "code"),
            ("client_id", client_id.as_str()),
            ("redirect_uri", redirect_uri.as_str()),
            ("scope", scope.as_str()),
            ("code_challenge", code_challenge),
            ("code_challenge_method", "S256"),
            ("state", state),
        ],
    )
    .expect("static Anthropic OAuth authorize URL is valid")
    .to_string()
}

fn openai_oauth_authorize_url(state: &str, code_challenge: &str) -> String {
    let client_id = openai_oauth_client_id();
    let redirect_uri = openai_oauth_redirect_uri();
    Url::parse_with_params(
        "https://auth.openai.com/oauth/authorize",
        &[
            ("response_type", "code"),
            ("client_id", client_id.as_str()),
            ("redirect_uri", redirect_uri.as_str()),
            ("scope", "openid profile email offline_access"),
            ("code_challenge", code_challenge),
            ("code_challenge_method", "S256"),
            ("id_token_add_organizations", "true"),
            ("codex_cli_simplified_flow", "true"),
            ("state", state),
            ("originator", "opencode"),
        ],
    )
    .expect("static OpenAI OAuth authorize URL is valid")
    .to_string()
}

fn google_oauth_authorize_url(
    provider_id: &str,
    state: &str,
    code_challenge: &str,
) -> Option<String> {
    let client_id = google_oauth_client_id(provider_id)?;
    let redirect_uri = provider_google_oauth_redirect_uri(provider_id);
    let scope = provider_google_oauth_scope(provider_id);
    Some(
        Url::parse_with_params(
            "https://accounts.google.com/o/oauth2/v2/auth",
            &[
                ("response_type", "code"),
                ("client_id", client_id.as_str()),
                ("redirect_uri", redirect_uri.as_str()),
                ("scope", scope.as_str()),
                ("state", state),
                ("access_type", "offline"),
                ("prompt", "consent"),
                ("code_challenge", code_challenge),
                ("code_challenge_method", "S256"),
            ],
        )
        .expect("static Google OAuth authorize URL is valid")
        .to_string(),
    )
}

pub(super) fn oauth_callback_html(success: bool, message: &str) -> String {
    let title = if success {
        "OAuth connected"
    } else {
        "OAuth failed"
    };
    format!(
        r#"<!doctype html>
<html>
  <head><meta charset="utf-8"><title>{title}</title></head>
  <body style="font-family: system-ui, sans-serif; padding: 32px;">
    <h1>{title}</h1>
    <p>{}</p>
  </body>
</html>"#,
        html_escape(message)
    )
}

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

pub(super) fn browser_login_url(provider_id: &str) -> String {
    match provider_id {
        "openai" => "https://chatgpt.com/auth/login".to_string(),
        "anthropic" => "https://claude.ai/login".to_string(),
        "antigravity" => "https://antigravity.google.com/auth".to_string(),
        other => format!("https://auth.example.com/oauth/{other}"),
    }
}

pub(super) fn browser_login_token(provider_id: &str, code: Option<&str>) -> String {
    format!(
        "browser-login:{}:{}",
        provider_id,
        code.unwrap_or("confirmed")
    )
}
