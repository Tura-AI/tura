use axum::extract::{Json, Path, Query};
use axum::response::{Html, IntoResponse};
use chrono::Utc;
use tura_llm_rust::{AuthMethodKind, OAuthAuthorizeKind};
#[path = "oauth_exchange.rs"]
mod oauth_exchange;
pub(super) use oauth_exchange::extract_account_id_from_jwt;
use oauth_exchange::{
    exchange_anthropic_oauth_code, exchange_oauth_code, extract_account_id,
    start_github_copilot_device_flow, wait_for_oauth_completed,
};

use crate::contracts::{
    OAuthAuthorizeParams, OAuthAuthorizePayload, OAuthAuthorizeResponse, OAuthCallbackParams,
    OAuthCallbackPayload, OAuthMethod, OAuthRedirectCallbackParams, ProviderAuthActionResponse,
};
use crate::mock::global_store;

use super::oauth_support::{
    browser_login_token, browser_login_url, oauth_authorize_url, oauth_callback_html,
    oauth_code_challenge, oauth_code_verifier, oauth_state, random_confirmation_code,
};
use super::{
    auth_update, build_provider_auth_status, persist_provider_auth, provider_auth_methods,
    provider_display_name, validation_detail,
};

pub async fn oauth_authorize(
    Path(provider_id): Path<String>,
    Query(_params): Query<OAuthAuthorizeParams>,
    Json(payload): Json<OAuthAuthorizePayload>,
) -> Json<OAuthAuthorizeResponse> {
    Json(oauth_authorize_value(provider_id, payload).await)
}

pub async fn oauth_authorize_value(
    provider_id: String,
    payload: OAuthAuthorizePayload,
) -> OAuthAuthorizeResponse {
    let methods = provider_auth_methods(&provider_id);
    let selected_method = methods.get(payload.method).filter(|method| {
        matches!(
            method.kind,
            AuthMethodKind::OAuthPkce | AuthMethodKind::LocalCliToken | AuthMethodKind::DeviceCode
        )
    });

    let Some(selected_method) = selected_method else {
        return OAuthAuthorizeResponse {
            url: String::new(),
            method: OAuthMethod::Code,
            instructions: "Invalid auth method".to_string(),
        };
    };

    if let Some(reason) = selected_method.unavailable_reason.as_deref() {
        return OAuthAuthorizeResponse {
            url: String::new(),
            method: OAuthMethod::Code,
            instructions: reason.to_string(),
        };
    }

    let entry = tura_llm_rust::provider_auth_registry_entry(&provider_id);
    let authorize_kind = entry
        .and_then(|entry| entry.oauth_authorize_kind)
        .unwrap_or(OAuthAuthorizeKind::Unsupported);

    let (url, method, instructions) = if authorize_kind == OAuthAuthorizeKind::GithubDevice {
        match start_github_copilot_device_flow(&provider_id).await {
            Ok(device) => {
                global_store().set_oauth_state(
                    &provider_id,
                    "device_code".to_string(),
                    Some(device.user_code.clone()),
                    device.verification_uri.clone(),
                    Some(oauth_state()),
                    Some(device.device_code.clone()),
                );
                (
                    device.verification_uri.clone(),
                    OAuthMethod::Code,
                    format!(
                        "Open {} and enter code {}. After GitHub authorizes Copilot, submit the same code here.",
                        device.verification_uri, device.user_code
                    ),
                )
            }
            Err(error) => {
                return OAuthAuthorizeResponse {
                    url: String::new(),
                    method: OAuthMethod::Code,
                    instructions: format!("GitHub Copilot OAuth cannot start: {error}"),
                };
            }
        }
    } else if matches!(
        authorize_kind,
        OAuthAuthorizeKind::OpenAiPkce
            | OAuthAuthorizeKind::AnthropicPkce
            | OAuthAuthorizeKind::GooglePkce
    ) {
        let state = oauth_state();
        let code_verifier = oauth_code_verifier();
        let code_challenge = oauth_code_challenge(&code_verifier);
        let Some(url) = oauth_authorize_url(&provider_id, authorize_kind, &state, &code_challenge)
        else {
            return OAuthAuthorizeResponse {
                url: String::new(),
                method: OAuthMethod::Code,
                instructions: format!(
                    "{} OAuth cannot start because its OAuth client configuration is incomplete.",
                    provider_display_name(&provider_id)
                ),
            };
        };
        global_store().set_oauth_state(
            &provider_id,
            "oauth_pkce".to_string(),
            None,
            url.clone(),
            Some(state),
            Some(code_verifier),
        );
        (
            url,
            OAuthMethod::Auto,
            format!(
                "Complete {} authorization in your browser. This window will close automatically.",
                provider_display_name(&provider_id)
            ),
        )
    } else if authorize_kind == OAuthAuthorizeKind::BrowserTokenPaste {
        let code = random_confirmation_code(&provider_id, payload.method);
        let url = browser_login_url(&provider_id);
        global_store().set_oauth_state(
            &provider_id,
            "token".to_string(),
            Some(code.clone()),
            url.clone(),
            Some(oauth_state()),
            None,
        );
        (
            url,
            OAuthMethod::Code,
            format!("Open the login page, copy your browser token, and paste it here. Confirmation: {code}"),
        )
    } else if selected_method.kind == AuthMethodKind::OAuthPkce {
        (
            String::new(),
            OAuthMethod::Code,
            format!(
                "{} OAuth is listed but is not implemented yet.",
                provider_display_name(&provider_id)
            ),
        )
    } else {
        (
            String::new(),
            OAuthMethod::Code,
            "This provider does not support browser authorization.".to_string(),
        )
    };

    OAuthAuthorizeResponse {
        url,
        method,
        instructions,
    }
}

pub async fn oauth_callback(
    Path(provider_id): Path<String>,
    Query(_params): Query<OAuthCallbackParams>,
    Json(payload): Json<OAuthCallbackPayload>,
) -> Json<ProviderAuthActionResponse> {
    Json(oauth_callback_value(provider_id, payload).await)
}

pub async fn oauth_callback_value(
    provider_id: String,
    payload: OAuthCallbackPayload,
) -> ProviderAuthActionResponse {
    complete_oauth_callback(provider_id, payload).await
}

pub async fn oauth_callback_info(
    Path(provider_id): Path<String>,
    Query(params): Query<OAuthRedirectCallbackParams>,
) -> Html<String> {
    if params.has_callback_payload() {
        return finish_oauth_redirect_callback(params, Some(provider_id)).await;
    }
    Html(oauth_callback_html(
        false,
        &format!(
            "{} OAuth callback is waiting for the provider redirect.",
            provider_display_name(&provider_id)
        ),
    ))
}

async fn complete_oauth_callback(
    provider_id: String,
    payload: OAuthCallbackPayload,
) -> ProviderAuthActionResponse {
    if payload
        .code
        .as_deref()
        .unwrap_or_default()
        .trim()
        .is_empty()
    {
        let pending_method = global_store().pending_oauth_method(&provider_id);
        let ok = match pending_method.as_deref() {
            Some("oauth_pkce") => wait_for_oauth_completed(&provider_id).await,
            Some("device_code") => complete_pending_device_oauth(&provider_id).await,
            _ => false,
        };
        return oauth_callback_response(
            &provider_id,
            ok,
            if ok {
                "provider.oauth.completed"
            } else {
                "provider.oauth.code_missing"
            },
            if ok {
                "provider OAuth completed"
            } else {
                "Paste the copied authorization code before submitting"
            },
        );
    }

    let has_pending = global_store().peek_oauth_state(&provider_id);
    if has_pending.is_none() {
        let normalized_code = normalize_oauth_code(payload.code.as_deref().unwrap_or_default());
        if provider_id == "claude-code" && looks_like_claude_code_oauth_token(&normalized_code.code)
        {
            return persist_direct_claude_code_oauth_token(&provider_id, normalized_code.code);
        }
        if provider_id == "claude-code" && normalized_code.verifier.is_some() {
            return complete_claude_code_manual_oauth(&provider_id, &normalized_code).await;
        }
        return oauth_callback_response(
            &provider_id,
            false,
            "provider.oauth.pending_missing",
            "No pending OAuth login was found. Click OAuth login again, then paste the new code.",
        );
    }

    let Some(pending) = has_pending else {
        return oauth_callback_response(
            &provider_id,
            false,
            "provider.oauth.pending_missing",
            "No pending OAuth login was found. Click OAuth login again, then paste the new code.",
        );
    };
    if matches!(pending.method.as_str(), "code" | "token" | "oauth_pkce")
        && payload
            .code
            .as_deref()
            .unwrap_or_default()
            .trim()
            .is_empty()
    {
        return oauth_callback_response(
            &provider_id,
            false,
            "provider.oauth.code_missing",
            "Paste the copied authorization code before submitting",
        );
    }

    if pending.method == "oauth_pkce"
        && payload.state.is_some()
        && payload.state.as_deref() != pending.state.as_deref()
    {
        return oauth_callback_response(
            &provider_id,
            false,
            "provider.oauth.state_mismatch",
            "OAuth state did not match. Click OAuth login again and paste the new code.",
        );
    }

    if pending.method == "code" {
        let expected_code = pending.code.as_ref();
        if payload.code.as_ref() != expected_code {
            return oauth_callback_response(
                &provider_id,
                false,
                "provider.oauth.code_mismatch",
                "OAuth confirmation code did not match",
            );
        }
    }

    let normalized_code = normalize_oauth_code(payload.code.as_deref().unwrap_or_default());
    if provider_id == "claude-code" && looks_like_claude_code_oauth_token(&normalized_code.code) {
        let response = persist_direct_claude_code_oauth_token(&provider_id, normalized_code.code);
        if response.ok {
            let _ = global_store().consume_oauth_state(&provider_id);
        }
        return response;
    }
    if provider_id == "claude-code" && normalized_code.verifier.is_some() {
        let response = complete_claude_code_manual_oauth(&provider_id, &normalized_code).await;
        if response.ok {
            let _ = global_store().consume_oauth_state(&provider_id);
        }
        return response;
    }
    if pending.method == "oauth_pkce" {
        if let Some(state) = normalized_code.state.as_deref() {
            if pending.state.as_deref() != Some(state) {
                return oauth_callback_response(
                    &provider_id,
                    false,
                    "provider.oauth.state_mismatch",
                    "OAuth state does not match the pending authorization request. Start login again and paste the newest code.",
                );
            }
        }
    }
    let tokens = if matches!(pending.method.as_str(), "oauth_pkce" | "device_code") {
        match exchange_oauth_code(&provider_id, &normalized_code, &pending).await {
            Ok(tokens) => Some(tokens),
            Err(error) => {
                return oauth_callback_response(
                    &provider_id,
                    false,
                    "provider.oauth.exchange_failed",
                    &format!("OAuth token exchange failed: {error}"),
                );
            }
        }
    } else {
        None
    };

    let key = if matches!(pending.method.as_str(), "oauth_pkce" | "device_code") {
        tokens
            .as_ref()
            .map(|tokens| tokens.access_token.clone())
            .unwrap_or_default()
    } else if pending.method == "token" {
        payload
            .code
            .as_deref()
            .unwrap_or_default()
            .trim()
            .to_string()
    } else {
        browser_login_token(&provider_id, pending.code.as_deref())
    };

    let login = if pending.method == "oauth_pkce" {
        "oauth"
    } else if pending.method == "device_code" {
        "device"
    } else {
        "browser"
    };
    let auth = auth_update::oauth_auth(
        key,
        tokens
            .as_ref()
            .and_then(|tokens| tokens.refresh_token.clone()),
        tokens
            .as_ref()
            .map(|tokens| Utc::now().timestamp_millis() + tokens.expires_in.unwrap_or(3600) * 1000),
        tokens.as_ref().and_then(extract_account_id),
        login,
        Some(pending.url.clone()),
    );

    if let Err(error) = persist_provider_auth(&provider_id, &auth) {
        return oauth_callback_response(
            &provider_id,
            false,
            "provider.oauth.persist_failed",
            &format!("OAuth token was received but could not be saved: {error}"),
        );
    }

    let _ = global_store().consume_oauth_state(&provider_id);
    let _ = global_store().set_auth(&provider_id, auth);

    oauth_callback_response(
        &provider_id,
        true,
        "provider.oauth.completed",
        "provider OAuth completed",
    )
}

fn oauth_callback_response(
    provider_id: &str,
    ok: bool,
    code: &str,
    message: &str,
) -> ProviderAuthActionResponse {
    ProviderAuthActionResponse {
        ok,
        provider_id: provider_id.to_string(),
        code: code.to_string(),
        message: message.to_string(),
        level: Some(if ok { "valid" } else { "invalid" }.to_string()),
        details: vec![validation_detail(code, None)],
        status: Some(build_provider_auth_status(provider_id)),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct NormalizedOAuthCode {
    code: String,
    state: Option<String>,
    verifier: Option<String>,
}

fn normalize_oauth_code(code: &str) -> NormalizedOAuthCode {
    let trimmed = code.trim();
    if let Some(code) = trimmed.strip_prefix("code=") {
        let normalized = code
            .split('&')
            .next()
            .unwrap_or_default()
            .split('#')
            .next()
            .unwrap_or_default()
            .trim()
            .to_string();
        let state = trimmed
            .split('#')
            .nth(1)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToString::to_string);
        return NormalizedOAuthCode {
            code: normalized,
            state,
            verifier: None,
        };
    }
    if let Ok(url) = reqwest::Url::parse(trimmed) {
        if let Some(code) = url
            .query_pairs()
            .find_map(|(key, value)| (key == "code").then(|| value.to_string()))
        {
            let state = url
                .query_pairs()
                .find_map(|(key, value)| (key == "state").then(|| value.to_string()))
                .or_else(|| {
                    url.fragment()
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                        .map(ToString::to_string)
                });
            return NormalizedOAuthCode {
                code: code.trim().to_string(),
                state,
                verifier: None,
            };
        }
    }
    let mut parts = trimmed.splitn(2, '#');
    let code = parts.next().unwrap_or_default().trim().to_string();
    let fragment = parts
        .next()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string);
    let verifier = fragment
        .as_deref()
        .filter(|value| looks_like_pkce_verifier(value))
        .map(ToString::to_string);
    let state = if verifier.is_some() { None } else { fragment };
    NormalizedOAuthCode {
        code,
        state,
        verifier,
    }
}

fn looks_like_pkce_verifier(value: &str) -> bool {
    let len = value.len();
    (43..=128).contains(&len)
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~'))
}

fn looks_like_claude_code_oauth_token(value: &str) -> bool {
    let token = value.trim();
    token.starts_with("sk-ant-oat") || token.starts_with("sk-ant-ort")
}

fn persist_direct_claude_code_oauth_token(
    provider_id: &str,
    token: String,
) -> ProviderAuthActionResponse {
    let auth = auth_update::oauth_auth(token, None, None, None, "oauth", None);
    if let Err(error) = persist_provider_auth(provider_id, &auth) {
        return oauth_callback_response(
            provider_id,
            false,
            "provider.oauth.persist_failed",
            &format!("OAuth token could not be saved: {error}"),
        );
    }
    let _ = global_store().set_auth(provider_id, auth);
    oauth_callback_response(
        provider_id,
        true,
        "provider.oauth.completed",
        "provider OAuth token saved",
    )
}

async fn complete_claude_code_manual_oauth(
    provider_id: &str,
    normalized_code: &NormalizedOAuthCode,
) -> ProviderAuthActionResponse {
    let pending = crate::mock::store::PendingOAuth {
        method: "oauth_pkce".to_string(),
        code: None,
        url: String::new(),
        state: None,
        code_verifier: normalized_code.verifier.clone(),
        expires_at: Utc::now().timestamp_millis() + 15 * 60_000,
    };
    let tokens = match exchange_anthropic_oauth_code(normalized_code, &pending).await {
        Ok(tokens) => tokens,
        Err(error) => {
            return oauth_callback_response(
                provider_id,
                false,
                "provider.oauth.exchange_failed",
                &format!("OAuth token exchange failed: {error}"),
            );
        }
    };
    let auth = auth_update::oauth_auth(
        tokens.access_token.clone(),
        tokens.refresh_token.clone(),
        Some(Utc::now().timestamp_millis() + tokens.expires_in.unwrap_or(3600) * 1000),
        extract_account_id(&tokens),
        "oauth",
        None,
    );
    if let Err(error) = persist_provider_auth(provider_id, &auth) {
        return oauth_callback_response(
            provider_id,
            false,
            "provider.oauth.persist_failed",
            &format!("OAuth token was received but could not be saved: {error}"),
        );
    }
    let _ = global_store().set_auth(provider_id, auth);
    oauth_callback_response(
        provider_id,
        true,
        "provider.oauth.completed",
        "provider OAuth completed",
    )
}

async fn complete_pending_device_oauth(provider_id: &str) -> bool {
    let Some(pending) = global_store().consume_oauth_state(provider_id) else {
        return false;
    };
    if pending.method != "device_code" {
        return false;
    }
    let tokens = match exchange_oauth_code(
        provider_id,
        &NormalizedOAuthCode {
            code: String::new(),
            state: None,
            verifier: None,
        },
        &pending,
    )
    .await
    {
        Ok(tokens) => tokens,
        Err(_) => return false,
    };
    let auth = auth_update::oauth_auth(
        tokens.access_token.clone(),
        tokens.refresh_token.clone(),
        Some(Utc::now().timestamp_millis() + tokens.expires_in.unwrap_or(3600) * 1000),
        extract_account_id(&tokens),
        "device",
        Some(pending.url.clone()),
    );
    if persist_provider_auth(provider_id, &auth).is_err() {
        return false;
    }
    let _ = global_store().set_auth(provider_id, auth);
    true
}

pub async fn oauth_redirect_callback(
    Query(params): Query<OAuthRedirectCallbackParams>,
) -> impl IntoResponse {
    finish_oauth_redirect_callback(params, None).await
}

async fn finish_oauth_redirect_callback(
    params: OAuthRedirectCallbackParams,
    expected_provider_id: Option<String>,
) -> Html<String> {
    if let Some(error) = params
        .error
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        return Html(oauth_callback_html(
            false,
            &format!("OAuth provider returned an error: {error}"),
        ));
    }
    let Some(code) = params
        .code
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    else {
        return Html(oauth_callback_html(false, "Missing authorization code"));
    };
    let Some(state) = params
        .state
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    else {
        return Html(oauth_callback_html(false, "Missing OAuth state"));
    };
    let Some((provider_id, pending)) = global_store().consume_oauth_state_by_state(state) else {
        return Html(oauth_callback_html(
            false,
            "OAuth state expired or was not found",
        ));
    };
    if expected_provider_id
        .as_deref()
        .is_some_and(|expected| expected != provider_id)
    {
        return Html(oauth_callback_html(
            false,
            "OAuth callback provider did not match the pending login",
        ));
    }
    if pending.method != "oauth_pkce" {
        return Html(oauth_callback_html(
            false,
            "OAuth callback did not match a PKCE login",
        ));
    }
    let tokens = match exchange_oauth_code(
        &provider_id,
        &NormalizedOAuthCode {
            code: code.to_string(),
            state: pending.state.clone(),
            verifier: None,
        },
        &pending,
    )
    .await
    {
        Ok(tokens) => tokens,
        Err(error) => {
            return Html(oauth_callback_html(
                false,
                &format!("Token exchange failed: {error}"),
            ))
        }
    };
    let auth = auth_update::oauth_auth(
        tokens.access_token.clone(),
        tokens.refresh_token.clone(),
        Some(Utc::now().timestamp_millis() + tokens.expires_in.unwrap_or(3600) * 1000),
        extract_account_id(&tokens),
        "oauth",
        Some(pending.url.clone()),
    );
    if persist_provider_auth(&provider_id, &auth).is_err() {
        return Html(oauth_callback_html(
            false,
            "Token was received but could not be persisted",
        ));
    }
    let _ = global_store().set_auth(&provider_id, auth.clone());
    global_store().set_oauth_completed(&provider_id, auth);
    Html(oauth_callback_html(
        true,
        &format!(
            "{} OAuth connected. You can close this window.",
            provider_display_name(&provider_id)
        ),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock::global_store;
    use axum::extract::{Json, Path, Query};

    fn clear_oauth(provider_id: &str) {
        let store = global_store();
        store.pending_oauth.write().remove(provider_id);
        store.completed_oauth.write().remove(provider_id);
    }

    fn set_pending(
        provider_id: &str,
        method: &str,
        code: Option<&str>,
        state: Option<&str>,
        verifier: Option<&str>,
    ) {
        clear_oauth(provider_id);
        global_store().set_oauth_state(
            provider_id,
            method.to_string(),
            code.map(ToString::to_string),
            format!("https://auth.example.test/{provider_id}"),
            state.map(ToString::to_string),
            verifier.map(ToString::to_string),
        );
    }

    #[test]
    fn normalize_oauth_code_accepts_plain_code_query_url_and_fragment_state() {
        assert_eq!(
            normalize_oauth_code("  plain-code # callback-state "),
            NormalizedOAuthCode {
                code: "plain-code".to_string(),
                state: Some("callback-state".to_string()),
                verifier: None,
            }
        );
        assert_eq!(
            normalize_oauth_code("code=direct-code&ignored=1#fragment-state"),
            NormalizedOAuthCode {
                code: "direct-code".to_string(),
                state: Some("fragment-state".to_string()),
                verifier: None,
            }
        );
        assert_eq!(
            normalize_oauth_code(
                "https://localhost/callback?state=query-state&code=url%20code#ignored"
            ),
            NormalizedOAuthCode {
                code: "url code".to_string(),
                state: Some("query-state".to_string()),
                verifier: None,
            }
        );
    }

    #[test]
    fn normalize_oauth_code_distinguishes_pkce_verifier_from_state_fragment() {
        let verifier = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789-._~";
        assert_eq!(
            normalize_oauth_code(&format!("manual-code#{verifier}")),
            NormalizedOAuthCode {
                code: "manual-code".to_string(),
                state: None,
                verifier: Some(verifier.to_string()),
            }
        );

        for value in ["short", "contains space and is long enough to be a state"] {
            assert!(!looks_like_pkce_verifier(value));
        }
        assert!(looks_like_pkce_verifier(verifier));
    }

    #[test]
    fn claude_code_oauth_token_detection_is_prefix_only_after_trim() {
        assert!(looks_like_claude_code_oauth_token(" sk-ant-oat-token "));
        assert!(looks_like_claude_code_oauth_token("sk-ant-ort-token"));
        assert!(!looks_like_claude_code_oauth_token("x-sk-ant-oat-token"));
        assert!(!looks_like_claude_code_oauth_token("sk-ant-api-token"));
    }

    #[test]
    fn redirect_callback_payload_presence_ignores_whitespace() {
        assert!(!OAuthRedirectCallbackParams {
            code: Some(" ".to_string()),
            state: None,
            error: None,
        }
        .has_callback_payload());
        assert!(OAuthRedirectCallbackParams {
            code: None,
            state: Some("state".to_string()),
            error: None,
        }
        .has_callback_payload());
        assert!(OAuthRedirectCallbackParams {
            code: None,
            state: None,
            error: Some("access_denied".to_string()),
        }
        .has_callback_payload());
    }

    #[test]
    fn oauth_callback_response_uses_stable_provider_code_level_and_detail() {
        let response = oauth_callback_response("flow-response-provider", false, "flow.error", "no");
        assert!(!response.ok);
        assert_eq!(response.provider_id, "flow-response-provider");
        assert_eq!(response.code, "flow.error");
        assert_eq!(response.message, "no");
        assert_eq!(response.level.as_deref(), Some("invalid"));
        assert_eq!(response.details.len(), 1);
        assert_eq!(response.details[0].code, "flow.error");
        assert!(response.status.is_some());

        let response = oauth_callback_response("flow-response-provider", true, "flow.ok", "yes");
        assert!(response.ok);
        assert_eq!(response.level.as_deref(), Some("valid"));
    }

    #[tokio::test]
    async fn oauth_authorize_rejects_unknown_method_without_creating_pending_state() {
        let provider_id = "flow-authorize-invalid";
        clear_oauth(provider_id);

        let Json(response) = oauth_authorize(
            Path(provider_id.to_string()),
            Query(OAuthAuthorizeParams::default()),
            Json(OAuthAuthorizePayload {
                method: 99,
                inputs: None,
            }),
        )
        .await;

        assert!(response.url.is_empty());
        assert_eq!(response.instructions, "Invalid auth method");
        assert!(global_store().peek_oauth_state(provider_id).is_none());
    }

    #[tokio::test]
    async fn empty_callback_code_reports_missing_code_and_keeps_manual_pending_state() {
        let provider_id = "flow-empty-code-token";
        set_pending(provider_id, "token", Some("confirm"), Some("state"), None);

        let response = complete_oauth_callback(
            provider_id.to_string(),
            OAuthCallbackPayload {
                method: 0,
                state: Some("state".to_string()),
                code: Some("  ".to_string()),
            },
        )
        .await;

        assert!(!response.ok);
        assert_eq!(response.code, "provider.oauth.code_missing");
        assert!(global_store().peek_oauth_state(provider_id).is_some());
        clear_oauth(provider_id);
    }

    #[tokio::test]
    async fn pkce_state_mismatch_does_not_consume_pending_state() {
        let provider_id = "flow-pkce-state-mismatch";
        set_pending(
            provider_id,
            "oauth_pkce",
            None,
            Some("expected-state"),
            Some("verifier"),
        );

        let response = complete_oauth_callback(
            provider_id.to_string(),
            OAuthCallbackPayload {
                method: 0,
                state: Some("wrong-state".to_string()),
                code: Some("callback-code".to_string()),
            },
        )
        .await;

        assert!(!response.ok);
        assert_eq!(response.code, "provider.oauth.state_mismatch");
        assert_eq!(
            global_store()
                .peek_oauth_state(provider_id)
                .and_then(|pending| pending.state),
            Some("expected-state".to_string())
        );
        clear_oauth(provider_id);
    }

    #[tokio::test]
    async fn manual_confirmation_code_mismatch_keeps_pending_state_for_retry() {
        let provider_id = "flow-code-mismatch";
        set_pending(
            provider_id,
            "code",
            Some("expected-code"),
            Some("state"),
            None,
        );

        let response = complete_oauth_callback(
            provider_id.to_string(),
            OAuthCallbackPayload {
                method: 0,
                state: Some("state".to_string()),
                code: Some("wrong-code".to_string()),
            },
        )
        .await;

        assert!(!response.ok);
        assert_eq!(response.code, "provider.oauth.code_mismatch");
        assert_eq!(
            global_store()
                .peek_oauth_state(provider_id)
                .and_then(|pending| pending.code),
            Some("expected-code".to_string())
        );
        clear_oauth(provider_id);
    }

    #[tokio::test]
    async fn redirect_callback_error_and_missing_fields_return_html_without_state_lookup() {
        let error = finish_oauth_redirect_callback(
            OAuthRedirectCallbackParams {
                code: Some("code".to_string()),
                state: Some("state".to_string()),
                error: Some("access_denied".to_string()),
            },
            None,
        )
        .await;
        assert!(error.0.contains("OAuth provider returned an error"));

        let missing_code = finish_oauth_redirect_callback(
            OAuthRedirectCallbackParams {
                code: Some(" ".to_string()),
                state: Some("state".to_string()),
                error: None,
            },
            None,
        )
        .await;
        assert!(missing_code.0.contains("Missing authorization code"));

        let missing_state = finish_oauth_redirect_callback(
            OAuthRedirectCallbackParams {
                code: Some("code".to_string()),
                state: None,
                error: None,
            },
            None,
        )
        .await;
        assert!(missing_state.0.contains("Missing OAuth state"));
    }

    #[tokio::test]
    async fn redirect_callback_rejects_unknown_state_expected_provider_and_non_pkce() {
        let unknown_state = finish_oauth_redirect_callback(
            OAuthRedirectCallbackParams {
                code: Some("code".to_string()),
                state: Some("missing-state".to_string()),
                error: None,
            },
            None,
        )
        .await;
        assert!(unknown_state
            .0
            .contains("OAuth state expired or was not found"));

        let provider_id = "flow-redirect-provider";
        set_pending(
            provider_id,
            "oauth_pkce",
            None,
            Some("redirect-state"),
            Some("verifier"),
        );
        let wrong_provider = finish_oauth_redirect_callback(
            OAuthRedirectCallbackParams {
                code: Some("code".to_string()),
                state: Some("redirect-state".to_string()),
                error: None,
            },
            Some("other-provider".to_string()),
        )
        .await;
        assert!(wrong_provider
            .0
            .contains("OAuth callback provider did not match the pending login"));
        assert!(global_store().peek_oauth_state(provider_id).is_none());

        let provider_id = "flow-redirect-non-pkce";
        set_pending(
            provider_id,
            "token",
            Some("confirm"),
            Some("token-state"),
            None,
        );
        let non_pkce = finish_oauth_redirect_callback(
            OAuthRedirectCallbackParams {
                code: Some("code".to_string()),
                state: Some("token-state".to_string()),
                error: None,
            },
            None,
        )
        .await;
        assert!(non_pkce
            .0
            .contains("OAuth callback did not match a PKCE login"));
        assert!(global_store().peek_oauth_state(provider_id).is_none());
    }
}
