use base64::Engine;
use tokio::time::{sleep, Duration, Instant};
use tura_llm_rust::OAuthAuthorizeKind;

use crate::mock::global_store;

use super::super::oauth_support::{
    anthropic_oauth_client_id, anthropic_oauth_redirect_uri, anthropic_oauth_token_url,
    github_copilot_oauth_client_id, github_copilot_oauth_scope, github_device_code_url,
    github_oauth_token_url, google_oauth_client_id, google_oauth_client_secret,
    google_oauth_token_url, openai_oauth_client_id, openai_oauth_redirect_uri,
    openai_oauth_token_url, provider_google_oauth_redirect_uri,
};
use super::super::provider_display_name;
use super::NormalizedOAuthCode;

#[derive(Debug, Clone, serde::Deserialize)]
pub(super) struct OAuthTokenResponse {
    pub(super) id_token: Option<String>,
    pub(super) access_token: String,
    pub(super) refresh_token: Option<String>,
    pub(super) expires_in: Option<i64>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub(super) struct GithubDeviceCodeResponse {
    pub(super) device_code: String,
    pub(super) user_code: String,
    pub(super) verification_uri: String,
}

pub(super) async fn start_github_copilot_device_flow(
    provider_id: &str,
) -> anyhow::Result<GithubDeviceCodeResponse> {
    let client_id = github_copilot_oauth_client_id()
        .ok_or_else(|| anyhow::anyhow!("GITHUB_COPILOT_CLIENT_ID is not configured"))?;
    let scope = github_copilot_oauth_scope();
    let response = reqwest::Client::new()
        .post(github_device_code_url())
        .header("accept", "application/json")
        .form(&[("client_id", client_id.as_str()), ("scope", scope.as_str())])
        .send()
        .await?;
    let status = response.status();
    let body: serde_json::Value = response.json().await?;
    if !status.is_success() {
        return Err(anyhow::anyhow!(
            "{} device-code endpoint returned {status}: {body}",
            provider_display_name(provider_id)
        ));
    }
    let device: GithubDeviceCodeResponse = serde_json::from_value(body)?;
    if device.device_code.trim().is_empty()
        || device.user_code.trim().is_empty()
        || device.verification_uri.trim().is_empty()
    {
        return Err(anyhow::anyhow!(
            "{} device-code response was incomplete",
            provider_display_name(provider_id)
        ));
    }
    Ok(device)
}

pub(super) async fn wait_for_oauth_completed(provider_id: &str) -> bool {
    let deadline = Instant::now() + Duration::from_secs(5 * 60);
    while Instant::now() < deadline {
        if let Some(auth) = global_store().consume_oauth_completed(provider_id) {
            let _ = global_store().set_auth(provider_id, auth);
            return true;
        }
        sleep(Duration::from_millis(500)).await;
    }
    false
}

pub(super) async fn exchange_oauth_code(
    provider_id: &str,
    normalized_code: &NormalizedOAuthCode,
    pending: &crate::mock::store::PendingOAuth,
) -> anyhow::Result<OAuthTokenResponse> {
    let kind = tura_llm_rust::provider_auth_registry_entry(provider_id)
        .and_then(|entry| entry.oauth_callback_kind)
        .unwrap_or(OAuthAuthorizeKind::Unsupported);
    match kind {
        OAuthAuthorizeKind::OpenAiPkce => {
            exchange_openai_oauth_code(&normalized_code.code, pending).await
        }
        OAuthAuthorizeKind::AnthropicPkce => {
            exchange_anthropic_oauth_code(normalized_code, pending).await
        }
        OAuthAuthorizeKind::GooglePkce => {
            exchange_google_oauth_code(provider_id, &normalized_code.code, pending).await
        }
        OAuthAuthorizeKind::GithubDevice => {
            exchange_github_copilot_device_code(provider_id, pending).await
        }
        OAuthAuthorizeKind::BrowserTokenPaste | OAuthAuthorizeKind::Unsupported => Err(
            anyhow::anyhow!("unsupported OAuth callback provider: {provider_id}"),
        ),
    }
}

async fn exchange_github_copilot_device_code(
    provider_id: &str,
    pending: &crate::mock::store::PendingOAuth,
) -> anyhow::Result<OAuthTokenResponse> {
    let client_id = github_copilot_oauth_client_id()
        .ok_or_else(|| anyhow::anyhow!("GITHUB_COPILOT_CLIENT_ID is not configured"))?;
    let device_code = pending
        .code_verifier
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("missing GitHub device code"))?;
    let deadline = Instant::now() + Duration::from_secs(5 * 60);
    let mut interval = Duration::from_secs(5);
    let body = loop {
        let response = reqwest::Client::new()
            .post(github_oauth_token_url())
            .header("accept", "application/json")
            .form(&[
                ("client_id", client_id.as_str()),
                ("device_code", device_code),
                ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
            ])
            .send()
            .await?;
        let status = response.status();
        let body: serde_json::Value = response.json().await?;
        if !status.is_success() {
            return Err(anyhow::anyhow!(
                "{} token endpoint returned {status}: {body}",
                provider_display_name(provider_id)
            ));
        }
        match body.get("error").and_then(serde_json::Value::as_str) {
            Some("authorization_pending") if Instant::now() < deadline => {
                sleep(interval).await;
                continue;
            }
            Some("slow_down") if Instant::now() < deadline => {
                interval += Duration::from_secs(5);
                sleep(interval).await;
                continue;
            }
            Some(error) => {
                return Err(anyhow::anyhow!(
                    "{} token endpoint returned {error}: {}",
                    provider_display_name(provider_id),
                    body.get("error_description")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or_default()
                ));
            }
            None => break body,
        }
    };
    let access_token = body
        .get("access_token")
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| {
            anyhow::anyhow!(
                "{} token response did not include access_token",
                provider_display_name(provider_id)
            )
        })?
        .to_string();
    Ok(OAuthTokenResponse {
        id_token: body
            .get("id_token")
            .and_then(serde_json::Value::as_str)
            .map(ToString::to_string),
        access_token,
        refresh_token: body
            .get("refresh_token")
            .and_then(serde_json::Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .map(ToString::to_string),
        expires_in: body.get("expires_in").and_then(serde_json::Value::as_i64),
    })
}

async fn exchange_openai_oauth_code(
    code: &str,
    pending: &crate::mock::store::PendingOAuth,
) -> anyhow::Result<OAuthTokenResponse> {
    let code_verifier = pending
        .code_verifier
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("missing PKCE code verifier"))?;
    let client_id = openai_oauth_client_id();
    let redirect_uri = openai_oauth_redirect_uri();
    let response = reqwest::Client::new()
        .post(openai_oauth_token_url())
        .form(&[
            ("grant_type", "authorization_code"),
            ("client_id", client_id.as_str()),
            ("redirect_uri", redirect_uri.as_str()),
            ("code", code),
            ("code_verifier", code_verifier),
        ])
        .send()
        .await?;
    let status = response.status();
    let body: serde_json::Value = response.json().await?;
    if !status.is_success() {
        return Err(anyhow::anyhow!(
            "OpenAI token endpoint returned {status}: {body}"
        ));
    }
    let tokens: OAuthTokenResponse = serde_json::from_value(body)?;
    if tokens.access_token.trim().is_empty() {
        return Err(anyhow::anyhow!(
            "OpenAI token response did not include access_token"
        ));
    }
    if tokens
        .refresh_token
        .as_deref()
        .unwrap_or_default()
        .trim()
        .is_empty()
    {
        return Err(anyhow::anyhow!(
            "OpenAI token response did not include refresh_token"
        ));
    }
    Ok(tokens)
}

pub(super) async fn exchange_anthropic_oauth_code(
    normalized_code: &NormalizedOAuthCode,
    pending: &crate::mock::store::PendingOAuth,
) -> anyhow::Result<OAuthTokenResponse> {
    let code_verifier = pending
        .code_verifier
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("missing PKCE code verifier"))?;
    let client_id = anthropic_oauth_client_id();
    let redirect_uri = anthropic_oauth_redirect_uri();
    let state = normalized_code
        .state
        .as_deref()
        .or(pending.state.as_deref());
    let mut form = vec![
        ("grant_type", "authorization_code"),
        ("client_id", client_id.as_str()),
        ("redirect_uri", redirect_uri.as_str()),
        ("code", normalized_code.code.as_str()),
        ("code_verifier", code_verifier),
    ];
    if let Some(state) = state {
        form.push(("state", state));
    }
    let response = reqwest::Client::builder()
        .timeout(Duration::from_secs(120))
        .build()?
        .post(anthropic_oauth_token_url())
        .header("content-type", "application/x-www-form-urlencoded")
        .form(&form)
        .send()
        .await?;
    let status = response.status();
    let body: serde_json::Value = response.json().await?;
    if !status.is_success() {
        return Err(anyhow::anyhow!(
            "Anthropic token endpoint returned {status}: {body}"
        ));
    }
    let tokens: OAuthTokenResponse = serde_json::from_value(body)?;
    if tokens.access_token.trim().is_empty() {
        return Err(anyhow::anyhow!(
            "Anthropic token response did not include access_token"
        ));
    }
    if tokens
        .refresh_token
        .as_deref()
        .unwrap_or_default()
        .trim()
        .is_empty()
    {
        return Err(anyhow::anyhow!(
            "Anthropic token response did not include refresh_token"
        ));
    }
    Ok(tokens)
}

async fn exchange_google_oauth_code(
    provider_id: &str,
    code: &str,
    pending: &crate::mock::store::PendingOAuth,
) -> anyhow::Result<OAuthTokenResponse> {
    let code_verifier = pending
        .code_verifier
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("missing PKCE code verifier"))?;
    let client_id = google_oauth_client_id(provider_id)
        .ok_or_else(|| anyhow::anyhow!("missing Google OAuth client id"))?;
    let redirect_uri = provider_google_oauth_redirect_uri(provider_id);
    let mut form = vec![
        ("grant_type".to_string(), "authorization_code".to_string()),
        ("client_id".to_string(), client_id),
        ("redirect_uri".to_string(), redirect_uri),
        ("code".to_string(), code.to_string()),
        ("code_verifier".to_string(), code_verifier.to_string()),
    ];
    if let Some(client_secret) = google_oauth_client_secret(provider_id) {
        form.push(("client_secret".to_string(), client_secret));
    }
    let response = reqwest::Client::new()
        .post(google_oauth_token_url())
        .form(&form)
        .send()
        .await?;
    let status = response.status();
    let body: serde_json::Value = response.json().await?;
    if !status.is_success() {
        return Err(anyhow::anyhow!(
            "Google token endpoint returned {status}: {body}"
        ));
    }
    let tokens: OAuthTokenResponse = serde_json::from_value(body)?;
    if tokens.access_token.trim().is_empty() {
        return Err(anyhow::anyhow!(
            "Google token response did not include access_token"
        ));
    }
    Ok(tokens)
}

pub(super) fn extract_account_id(tokens: &OAuthTokenResponse) -> Option<String> {
    tokens
        .id_token
        .as_deref()
        .and_then(extract_account_id_from_jwt)
        .or_else(|| extract_account_id_from_jwt(&tokens.access_token))
}

pub(in crate::api::provider) fn extract_account_id_from_jwt(token: &str) -> Option<String> {
    let payload = token.split('.').nth(1)?;
    let decoded = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(payload)
        .or_else(|_| base64::engine::general_purpose::URL_SAFE.decode(payload))
        .ok()?;
    let claims: serde_json::Value = serde_json::from_slice(&decoded).ok()?;
    claims
        .get("chatgpt_account_id")
        .and_then(|value| value.as_str())
        .or_else(|| {
            claims
                .pointer("/https://api.openai.com~1auth/chatgpt_account_id")
                .and_then(|value| value.as_str())
        })
        .or_else(|| {
            claims
                .get("organizations")
                .and_then(|value| value.as_array())
                .and_then(|organizations| organizations.first())
                .and_then(|organization| organization.get("id"))
                .and_then(|value| value.as_str())
        })
        .map(str::to_string)
}
