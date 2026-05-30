use chrono::Utc;

use super::config::config_value;
use super::oauth_support::{
    anthropic_oauth_client_id, anthropic_oauth_token_url, google_oauth_client_id,
    google_oauth_client_secret, google_oauth_token_url, openai_oauth_client_id,
    openai_oauth_token_url,
};
use super::{
    auth_registry, auth_update, auth_validator, build_provider_auth_status,
    extract_account_id_from_jwt, persist_provider_auth, ProviderAuthStatusResponse,
};

pub(super) async fn refresh_provider_auth_if_needed(
    provider_id: &str,
    force: bool,
) -> Result<bool, String> {
    let Some(entry) = auth_registry::entry(provider_id) else {
        return Ok(false);
    };
    if !entry.capabilities.supports_oauth_refresh {
        return Ok(false);
    }
    let status = build_provider_auth_status(provider_id);
    let has_refresh_token = status
        .refresh_env
        .as_deref()
        .and_then(config_value)
        .is_some_and(|value| !value.trim().is_empty());
    if !matches!(status.login.as_deref(), Some("oauth"))
        && !(provider_id == "claude-code" && has_refresh_token)
    {
        return Ok(false);
    }
    if !force && !auth_validator::provider_auth_expires_soon(&status) {
        return Ok(false);
    }
    match entry.oauth_callback_kind {
        Some(tura_llm_rust::OAuthAuthorizeKind::OpenAiPkce) => {
            refresh_openai_provider_auth(provider_id, &status)
                .await
                .map(|_| true)
        }
        Some(tura_llm_rust::OAuthAuthorizeKind::AnthropicPkce) => {
            refresh_anthropic_provider_auth(provider_id, &status)
                .await
                .map(|_| true)
        }
        Some(tura_llm_rust::OAuthAuthorizeKind::GooglePkce) => {
            refresh_google_provider_auth(provider_id, &status)
                .await
                .map(|_| true)
        }
        _ => Ok(false),
    }
}

async fn refresh_openai_provider_auth(
    provider_id: &str,
    status: &ProviderAuthStatusResponse,
) -> Result<(), String> {
    refresh_oauth_provider_auth(
        provider_id,
        status,
        openai_oauth_token_url(),
        vec![("client_id".to_string(), openai_oauth_client_id())],
        "OpenAI",
    )
    .await
}

async fn refresh_google_provider_auth(
    provider_id: &str,
    status: &ProviderAuthStatusResponse,
) -> Result<(), String> {
    let mut extra_params = Vec::new();
    if let Some(client_id) = google_oauth_client_id(provider_id) {
        extra_params.push(("client_id".to_string(), client_id));
    }
    if let Some(client_secret) = google_oauth_client_secret(provider_id) {
        extra_params.push(("client_secret".to_string(), client_secret));
    }
    refresh_oauth_provider_auth(
        provider_id,
        status,
        google_oauth_token_url(),
        extra_params,
        "Google",
    )
    .await
}

async fn refresh_anthropic_provider_auth(
    provider_id: &str,
    status: &ProviderAuthStatusResponse,
) -> Result<(), String> {
    let refresh_env = status
        .refresh_env
        .as_deref()
        .ok_or_else(|| "Anthropic refresh env is not configured".to_string())?;
    let refresh = config_value(refresh_env)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| format!("{refresh_env} is not configured"))?;
    let client_id = anthropic_oauth_client_id();
    let form = vec![
        ("grant_type", "refresh_token"),
        ("refresh_token", refresh.as_str()),
        ("client_id", client_id.as_str()),
    ];
    let response = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .map_err(|error| error.to_string())?
        .post(anthropic_oauth_token_url())
        .header("content-type", "application/x-www-form-urlencoded")
        .form(&form)
        .send()
        .await
        .map_err(|error| error.to_string())?;
    let http_status = response.status();
    let body: serde_json::Value = response.json().await.map_err(|error| error.to_string())?;
    if !http_status.is_success() {
        return Err(format!(
            "Anthropic token endpoint returned {http_status}: {body}"
        ));
    }
    persist_oauth_refresh_response(provider_id, status, body, "Anthropic").await
}

async fn refresh_oauth_provider_auth(
    provider_id: &str,
    status: &ProviderAuthStatusResponse,
    token_url: String,
    extra_params: Vec<(String, String)>,
    display_name: &str,
) -> Result<(), String> {
    let refresh_env = status
        .refresh_env
        .as_deref()
        .ok_or_else(|| format!("{display_name} refresh env is not configured"))?;
    let refresh = config_value(refresh_env)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| format!("{refresh_env} is not configured"))?;
    let mut form = vec![
        ("grant_type".to_string(), "refresh_token".to_string()),
        ("refresh_token".to_string(), refresh.clone()),
    ];
    form.extend(extra_params);
    let response = reqwest::Client::new()
        .post(token_url)
        .header("content-type", "application/x-www-form-urlencoded")
        .form(&form)
        .send()
        .await
        .map_err(|error| error.to_string())?;
    let http_status = response.status();
    let body: serde_json::Value = response.json().await.map_err(|error| error.to_string())?;
    if !http_status.is_success() {
        return Err(format!(
            "{display_name} token endpoint returned {http_status}: {body}"
        ));
    }
    persist_oauth_refresh_response(provider_id, status, body, display_name).await
}

async fn persist_oauth_refresh_response(
    provider_id: &str,
    status: &ProviderAuthStatusResponse,
    body: serde_json::Value,
    display_name: &str,
) -> Result<(), String> {
    let access = body
        .get("access_token")
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| format!("{display_name} refresh response did not include access_token"))?
        .to_string();
    let refresh = body
        .get("refresh_token")
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(ToString::to_string)
        .or_else(|| status.refresh_env.as_deref().and_then(config_value))
        .ok_or_else(|| format!("{display_name} refresh response did not include refresh_token"))?;
    let expires = Utc::now().timestamp_millis()
        + body
            .get("expires_in")
            .and_then(serde_json::Value::as_i64)
            .unwrap_or(3600)
            * 1000;
    let account_id = body
        .get("id_token")
        .and_then(serde_json::Value::as_str)
        .and_then(extract_account_id_from_jwt)
        .or_else(|| extract_account_id_from_jwt(&access))
        .or_else(|| status.account_id.clone());
    let auth = auth_update::oauth_auth(
        access,
        Some(refresh),
        Some(expires),
        account_id,
        "oauth",
        None,
    );
    persist_provider_auth(provider_id, &auth).map_err(|error| error.to_string())
}
