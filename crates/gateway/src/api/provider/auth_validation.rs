use std::time::Duration;

use super::catalog::{provider_api_from_settings, provider_env_from_settings};
use super::{
    config_value, ProviderAuthActionDetail, ProviderAuthStatusResponse,
    ProviderAuthValidationRequest,
};

pub(super) struct ProviderValidationReceipt {
    pub(super) ok: bool,
    pub(super) level: String,
    pub(super) code: String,
    pub(super) message: String,
    pub(super) details: Vec<ProviderAuthActionDetail>,
}

pub(super) async fn validate_provider_auth_config(
    provider_id: &str,
    status: &ProviderAuthStatusResponse,
    request: Option<&ProviderAuthValidationRequest>,
) -> ProviderValidationReceipt {
    let settings = tura_llm_rust::Settings::default().await.ok();
    let provider_config = settings
        .as_deref()
        .and_then(|settings| settings.model_catalog.providers.get(provider_id));
    let mut env_keys = Vec::new();
    let api_key_validation = uses_api_key_validation(provider_id, provider_config, status, request);
    push_unique_env(
        &mut env_keys,
        request
            .and_then(|payload| payload.token_env.as_deref())
            .or(status.token_env.as_deref()),
    );
    if !api_key_validation {
        push_unique_env(&mut env_keys, status.login_env.as_deref());
        push_unique_env(&mut env_keys, status.refresh_env.as_deref());
    }
    if let Some(settings) = settings.as_ref() {
        for env in provider_env_from_settings(Some(settings), provider_id) {
            push_unique_env(&mut env_keys, Some(&env));
        }
    }
    if let Some(entry) = tura_llm_rust::provider_auth_registry_entry(provider_id) {
        push_unique_env(&mut env_keys, entry.token_env);
        if !api_key_validation {
            push_unique_env(&mut env_keys, entry.login_env);
            push_unique_env(&mut env_keys, entry.refresh_env);
        }
    }

    let request_token_env = request.and_then(|payload| payload.token_env.as_deref());
    let request_token = request.and_then(validation_request_token);

    let present: Vec<String> = env_keys
        .iter()
        .filter(|key| {
            config_value(key).is_some()
                || (request_token.is_some() && request_token_env == Some(key.as_str()))
        })
        .cloned()
        .collect();
    let missing: Vec<String> = env_keys
        .iter()
        .filter(|key| {
            config_value(key).is_none()
                && !(request_token.is_some() && request_token_env == Some(key.as_str()))
        })
        .cloned()
        .collect();

    let base_url = provider_api_from_settings(settings.as_deref(), provider_id).or_else(|| {
        tura_llm_rust::provider_auth_registry_entry(provider_id)
            .map(|entry| entry.default_base_url.to_string())
            .filter(|url| !url.trim().is_empty())
    });
    let url_ok = base_url
        .as_deref()
        .map(|url| reqwest::Url::parse(url).is_ok())
        .unwrap_or(true);
    let token_env = status
        .token_env
        .as_deref()
        .or_else(|| provider_config.and_then(|provider| provider.token_env.as_deref()))
        .or_else(|| {
            tura_llm_rust::provider_auth_registry_entry(provider_id)
                .and_then(|entry| entry.token_env)
        });
    let token = request_token
        .map(ToString::to_string)
        .or_else(|| token_env.and_then(config_value));
    let external_validation = validate_provider_credentials_remotely(
        provider_id,
        provider_config,
        base_url.as_deref(),
        token.as_deref(),
        api_key_validation,
    )
    .await;
    let warning = matches!(
        external_validation,
        ProviderCredentialValidation::Warning(_)
    );
    let unsupported = matches!(
        external_validation,
        ProviderCredentialValidation::Unsupported(_)
    );
    let configured = status.authenticated || status.configured || !present.is_empty();
    let ok = if unsupported {
        url_ok && configured
    } else {
        url_ok
            && matches!(
                external_validation,
                ProviderCredentialValidation::Passed(_) | ProviderCredentialValidation::Warning(_)
            )
    };

    let mut parts = Vec::new();
    let mut details = Vec::new();
    if unsupported {
        parts.push("credential validation unavailable".to_string());
        details.push(validation_detail("provider.validation.unavailable", None));
    } else if ok {
        parts.push("credential validation passed".to_string());
        details.push(validation_detail("provider.validation.passed", None));
    } else {
        parts.push("credential validation failed".to_string());
        details.push(validation_detail("provider.validation.failed", None));
    }
    if let Some(base_url) = base_url {
        details.push(validation_detail(
            if url_ok {
                "provider.base_url.ok"
            } else {
                "provider.base_url.invalid"
            },
            Some(base_url.clone()),
        ));
        parts.push(format!(
            "base_url {} ({})",
            if url_ok { "ok" } else { "invalid" },
            base_url
        ));
    }
    if !present.is_empty() {
        details.push(validation_detail(
            "provider.env.present",
            Some(present.join(", ")),
        ));
        parts.push(format!("present env: {}", present.join(", ")));
    }
    if !missing.is_empty() {
        details.push(validation_detail(
            "provider.env.missing",
            Some(missing.join(", ")),
        ));
        parts.push(format!("missing env: {}", missing.join(", ")));
    }
    if env_keys.is_empty() {
        details.push(validation_detail("provider.env.none_registered", None));
        parts.push("no credential env is registered for this provider".to_string());
    }
    match external_validation {
        ProviderCredentialValidation::Passed(detail)
        | ProviderCredentialValidation::Warning(detail)
        | ProviderCredentialValidation::Failed(detail)
        | ProviderCredentialValidation::Unsupported(detail) => {
            parts.push(detail.message.clone());
            details.push(detail);
        }
    }
    if unsupported {
        details.push(validation_detail("provider.request.no_paid_model", None));
        parts.push("no paid model request was sent".to_string());
    }
    let level = if unsupported {
        "unsupported"
    } else if ok && warning {
        "warning"
    } else if ok {
        "valid"
    } else {
        "invalid"
    };
    let code = match level {
        "valid" => "provider.validation.valid",
        "warning" => "provider.validation.warning",
        "unsupported" => "provider.validation.unsupported",
        _ => "provider.validation.invalid",
    };
    ProviderValidationReceipt {
        ok,
        level: level.to_string(),
        code: code.to_string(),
        message: parts.join("; "),
        details,
    }
}

pub(super) enum ProviderCredentialValidation {
    Passed(ProviderAuthActionDetail),
    Warning(ProviderAuthActionDetail),
    Failed(ProviderAuthActionDetail),
    Unsupported(ProviderAuthActionDetail),
}

pub(super) async fn validate_provider_credentials_remotely(
    provider_id: &str,
    provider_config: Option<&tura_llm_rust::ProviderCatalogConfig>,
    base_url: Option<&str>,
    token: Option<&str>,
    api_key_validation: bool,
) -> ProviderCredentialValidation {
    let api_style = provider_config
        .map(|provider| provider.api_style.as_str())
        .unwrap_or_default();
    let registry_entry = tura_llm_rust::provider_auth_registry_entry(provider_id);
    let has_llm_domain = provider_config
        .map(|provider| provider_has_domain(provider, "llm"))
        .unwrap_or_else(|| {
            registry_entry
                .map(|entry| entry.capabilities.supports_model_validation)
                .unwrap_or(false)
                || is_openai_compatible_provider(provider_id)
        });
    let runtime_provider = provider_config
        .map(|provider| provider.runtime_provider.as_str())
        .filter(|value| !value.trim().is_empty())
        .or_else(|| registry_entry.map(|entry| entry.runtime_provider_id))
        .unwrap_or(provider_id);
    let token = token.map(str::trim).filter(|value| !value.is_empty());
    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(12))
        .build()
    {
        Ok(client) => client,
        Err(error) => {
            return ProviderCredentialValidation::Failed(validation_detail(
                "provider.validation.client_setup_failed",
                Some(error.to_string()),
            ));
        }
    };

    if !api_key_validation
        && (matches!(api_style, "codex" | "claude_code")
            || matches!(provider_id, "codex" | "claude-code" | "antigravity"))
    {
        let Some(token) = token else {
            return ProviderCredentialValidation::Failed(validation_detail(
                "provider.credential.oauth_token_missing",
                None,
            ));
        };
        if !looks_like_bearer_token(token) {
            return ProviderCredentialValidation::Failed(validation_detail(
                "provider.credential.oauth_token_invalid_format",
                None,
            ));
        }
    }

    if api_key_validation && (api_style == "codex" || provider_id == "codex") {
        let Some(token) = token else {
            return ProviderCredentialValidation::Failed(validation_detail(
                "provider.credential.api_key_missing",
                Some("OpenAI".to_string()),
            ));
        };
        let Some(url) = validation_url(Some("https://api.openai.com/v1"), "models") else {
            return ProviderCredentialValidation::Failed(validation_detail(
                "provider.base_url.invalid",
                Some("OpenAI".to_string()),
            ));
        };
        return validate_openai_compatible_models(&client, &url, Some(token)).await;
    }

    if !api_key_validation && (api_style == "codex" || provider_id == "codex") {
        let Some(token) = token else {
            return ProviderCredentialValidation::Failed(validation_detail(
                "provider.credential.oauth_token_missing",
                Some("Codex".to_string()),
            ));
        };
        return validate_bearer_get(
            &client,
            "https://chatgpt.com/backend-api/models",
            token,
            &[],
            "Codex subscription model list",
        )
        .await;
    }

    if !api_key_validation && (provider_id == "antigravity" || api_style == "antigravity") {
        let Some(token) = token else {
            return ProviderCredentialValidation::Failed(validation_detail(
                "provider.credential.oauth_token_missing",
                Some("Antigravity".to_string()),
            ));
        };
        return validate_bearer_get(
            &client,
            "https://www.googleapis.com/oauth2/v3/userinfo",
            token,
            &[],
            "Google OAuth userinfo",
        )
        .await;
    }

    if matches!(provider_id, "github" | "github-copilot") {
        let Some(token) = token else {
            return ProviderCredentialValidation::Failed(validation_detail(
                "provider.credential.token_missing",
                Some("GitHub".to_string()),
            ));
        };
        return validate_bearer_get(
            &client,
            "https://api.github.com/user",
            token,
            &[("user-agent", "tura-gateway")],
            "GitHub /user",
        )
        .await;
    }

    if provider_id == "openrouter" {
        let Some(token) = token else {
            return ProviderCredentialValidation::Failed(validation_detail(
                "provider.credential.api_key_missing",
                Some("OpenRouter".to_string()),
            ));
        };
        return validate_bearer_get(
            &client,
            "https://openrouter.ai/api/v1/key",
            token,
            &[],
            "OpenRouter current key",
        )
        .await;
    }

    if provider_id == "huggingface" {
        let Some(token) = token else {
            return ProviderCredentialValidation::Failed(validation_detail(
                "provider.credential.token_missing",
                Some("Hugging Face".to_string()),
            ));
        };
        return validate_bearer_get(
            &client,
            "https://huggingface.co/api/whoami-v2",
            token,
            &[],
            "Hugging Face whoami-v2",
        )
        .await;
    }

    if provider_id == "replicate" {
        let Some(token) = token else {
            return ProviderCredentialValidation::Failed(validation_detail(
                "provider.credential.token_missing",
                Some("Replicate".to_string()),
            ));
        };
        return validate_bearer_get(
            &client,
            "https://api.replicate.com/v1/account",
            token,
            &[],
            "Replicate account",
        )
        .await;
    }

    if provider_id == "fireworks" {
        let Some(token) = token else {
            return ProviderCredentialValidation::Failed(validation_detail(
                "provider.credential.api_key_missing",
                Some("Fireworks".to_string()),
            ));
        };
        return validate_bearer_get(
            &client,
            "https://api.fireworks.ai/v1/accounts",
            token,
            &[],
            "Fireworks accounts",
        )
        .await;
    }

    if provider_id == "cohere" {
        let Some(token) = token else {
            return ProviderCredentialValidation::Failed(validation_detail(
                "provider.credential.token_missing",
                Some("Cohere".to_string()),
            ));
        };
        return validate_bearer_get(
            &client,
            "https://api.cohere.com/v1/models",
            token,
            &[],
            "Cohere /models",
        )
        .await;
    }

    if provider_id == "perplexity" {
        return ProviderCredentialValidation::Unsupported(validation_detail(
            "provider.validation.public_model_list_unsupported",
            Some("Perplexity".to_string()),
        ));
    }

    if provider_id == "line" {
        let Some(token) = token else {
            return ProviderCredentialValidation::Failed(validation_detail(
                "provider.credential.token_missing",
                Some("LINE".to_string()),
            ));
        };
        return validate_bearer_get(
            &client,
            "https://api.line.me/v2/bot/info",
            token,
            &[],
            "LINE bot info",
        )
        .await;
    }

    if provider_id == "slack" {
        let Some(token) = token else {
            return ProviderCredentialValidation::Failed(validation_detail(
                "provider.credential.token_missing",
                Some("Slack".to_string()),
            ));
        };
        return validate_bearer_get(
            &client,
            "https://slack.com/api/auth.test",
            token,
            &[],
            "Slack auth.test",
        )
        .await;
    }

    if provider_id == "claude-code" || api_style == "claude_code" {
        let Some(token) = token else {
            return ProviderCredentialValidation::Failed(validation_detail(
                "provider.credential.oauth_token_missing",
                Some("Claude Code".to_string()),
            ));
        };
        let Some(url) = validation_url(base_url, "models") else {
            return ProviderCredentialValidation::Failed(validation_detail(
                "provider.base_url.invalid",
                Some("Claude Code".to_string()),
            ));
        };
        return validate_anthropic_oauth_models(&client, &url, token).await;
    }

    if (has_llm_domain && runtime_provider == "anthropic")
        || provider_id.contains("anthropic")
        || provider_id.contains("claude")
        || base_url
            .map(|url| url.contains("anthropic.com"))
            .unwrap_or(false)
    {
        let Some(token) = token else {
            return ProviderCredentialValidation::Failed(validation_detail(
                "provider.credential.token_missing",
                Some("Anthropic".to_string()),
            ));
        };
        let Some(url) = validation_url(base_url, "models") else {
            return ProviderCredentialValidation::Failed(validation_detail(
                "provider.base_url.invalid",
                Some("Anthropic".to_string()),
            ));
        };
        return validate_anthropic_models(&client, &url, token).await;
    }

    if has_llm_domain && (api_style == "google" || runtime_provider == "google") {
        let Some(token) = token else {
            return ProviderCredentialValidation::Failed(validation_detail(
                "provider.credential.api_key_missing",
                Some("Google".to_string()),
            ));
        };
        let Some(url) = validation_url(base_url, "models") else {
            return ProviderCredentialValidation::Failed(validation_detail(
                "provider.base_url.invalid",
                Some("Google".to_string()),
            ));
        };
        return validate_google_models(&client, &url, token).await;
    }

    if api_key_validation && has_llm_domain && api_style == "antigravity" {
        let Some(token) = token else {
            return ProviderCredentialValidation::Failed(validation_detail(
                "provider.credential.api_key_missing",
                Some("Antigravity".to_string()),
            ));
        };
        let Some(url) = validation_url(base_url, "models") else {
            return ProviderCredentialValidation::Failed(validation_detail(
                "provider.base_url.invalid",
                Some("Antigravity".to_string()),
            ));
        };
        return validate_openai_compatible_models(&client, &url, Some(token)).await;
    }

    if has_llm_domain
        && (matches!(api_style, "openapi" | "ollama") || is_openai_compatible_provider(provider_id))
    {
        let Some(url) = validation_url(base_url, "models") else {
            return ProviderCredentialValidation::Failed(validation_detail(
                "provider.base_url.invalid",
                Some("OpenAI-compatible".to_string()),
            ));
        };
        if !is_local_no_token_provider(provider_id, base_url) && token.is_none() {
            return ProviderCredentialValidation::Failed(validation_detail(
                "provider.credential.api_key_missing",
                Some("OpenAI-compatible".to_string()),
            ));
        }
        return validate_openai_compatible_models(&client, &url, token).await;
    }

    ProviderCredentialValidation::Unsupported(validation_detail(
        "provider.validation.gateway_not_configured",
        None,
    ))
}

fn provider_has_domain(provider: &tura_llm_rust::ProviderCatalogConfig, domain: &str) -> bool {
    provider
        .domains
        .iter()
        .any(|value| value.eq_ignore_ascii_case(domain))
}

fn uses_api_key_validation(
    provider_id: &str,
    provider_config: Option<&tura_llm_rust::ProviderCatalogConfig>,
    status: &ProviderAuthStatusResponse,
    request: Option<&ProviderAuthValidationRequest>,
) -> bool {
    if let Some(request) = request {
        if request_field_eq(request.auth_type.as_deref(), "api")
            || request_field_eq(request.kind.as_deref(), "api_key")
            || request_field_eq(request.login.as_deref(), "api")
        {
            return true;
        }
        if request_field_eq(request.auth_type.as_deref(), "oauth")
            || request_field_eq(request.kind.as_deref(), "oauth")
            || request_field_eq(request.login.as_deref(), "oauth")
            || request_field_eq(request.login.as_deref(), "browser")
        {
            return false;
        }
    }
    if status
        .login
        .as_deref()
        .is_some_and(|login| login.eq_ignore_ascii_case("api"))
    {
        return true;
    }
    provider_config
        .map(|provider| {
            provider
                .auth_methods
                .iter()
                .any(|method| method.eq_ignore_ascii_case("api_key"))
                && !provider
                    .auth_methods
                    .iter()
                    .any(|method| method.eq_ignore_ascii_case("oauth"))
        })
        .unwrap_or_else(|| {
            tura_llm_rust::provider_auth_registry_entry(provider_id)
                .map(|entry| {
                    entry.capabilities.supports_api_key && !entry.capabilities.supports_subscription
                })
                .unwrap_or(false)
        })
}

fn request_field_eq(value: Option<&str>, expected: &str) -> bool {
    value
        .map(str::trim)
        .is_some_and(|value| value.eq_ignore_ascii_case(expected))
}

fn validation_request_token(request: &ProviderAuthValidationRequest) -> Option<&str> {
    request
        .key
        .as_deref()
        .or(request.access.as_deref())
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

pub(super) fn validation_detail(code: &str, value: Option<String>) -> ProviderAuthActionDetail {
    let english = validation_detail_english(code);
    let message = match value.as_deref() {
        Some(value) if !value.is_empty() => format!("{english}: {value}"),
        _ => english.to_string(),
    };
    ProviderAuthActionDetail {
        code: code.to_string(),
        message,
        value,
    }
}

fn validation_detail_english(code: &str) -> &'static str {
    match code {
        "provider.validation.passed" => "credential validation passed",
        "provider.validation.failed" => "credential validation failed",
        "provider.validation.unavailable" => "credential validation unavailable",
        "provider.base_url.ok" => "base URL is valid",
        "provider.base_url.invalid" => "base URL is invalid",
        "provider.env.present" => "configured environment variables",
        "provider.env.missing" => "missing environment variables",
        "provider.env.none_registered" => "no credential environment variable is registered",
        "provider.remote.accepted" => "remote validation accepted credentials",
        "provider.remote.permission_limited" => {
            "remote validation authenticated credentials but reported limited permission"
        }
        "provider.remote.rejected" => "remote validation rejected credentials",
        "provider.remote.request_failed" => "remote validation request failed",
        "provider.validation.client_setup_failed" => "validation client setup failed",
        "provider.credential.oauth_token_missing" => "OAuth access token is missing",
        "provider.credential.oauth_token_invalid_format" => "OAuth access token format is invalid",
        "provider.credential.token_missing" => "token is missing",
        "provider.credential.api_key_missing" => "API key is missing",
        "provider.validation.public_model_list_unsupported" => {
            "public model list cannot validate credentials"
        }
        "provider.validation.gateway_not_configured" => {
            "validation gateway is not configured for this provider"
        }
        "provider.request.no_paid_model" => "no paid model request was sent",
        "provider.auth.refresh.unsupported" => "auth refresh is unsupported",
        "provider.auth.refresh.failed" => "auth refresh failed",
        "provider.auth.refresh.succeeded" => "auth refresh succeeded",
        "provider.auth.not_configured" => "provider auth is not configured",
        "provider.auth.logout.succeeded" => "provider auth logged out",
        "provider.auth.logout.failed" => "provider auth logout failed",
        _ => "provider validation detail",
    }
}

fn is_local_no_token_provider(provider_id: &str, base_url: Option<&str>) -> bool {
    matches!(provider_id, "ollama" | "lmstudio")
        || base_url
            .map(|url| url.contains("localhost") || url.contains("127.0.0.1"))
            .unwrap_or(false)
}

fn is_openai_compatible_provider(provider_id: &str) -> bool {
    matches!(
        provider_id,
        "openai"
            | "openai-api"
            | "gpt"
            | "openrouter"
            | "deepseek"
            | "qwen"
            | "qwen-cn"
            | "mistral"
            | "xai"
            | "grok"
            | "kimi"
            | "moonshot"
            | "huggingface"
            | "replicate"
            | "together"
            | "fireworks"
            | "groq"
            | "perplexity"
            | "volcengine"
            | "baidu_qianfan"
    )
}

fn validation_url(base_url: Option<&str>, suffix: &str) -> Option<String> {
    let base_url = base_url?.trim();
    if base_url.is_empty() || base_url.contains('{') || base_url.contains('}') {
        return None;
    }
    reqwest::Url::parse(base_url).ok()?;
    Some(format!(
        "{}/{}",
        base_url.trim_end_matches('/'),
        suffix.trim_start_matches('/')
    ))
}

async fn validate_openai_compatible_models(
    client: &reqwest::Client,
    url: &str,
    token: Option<&str>,
) -> ProviderCredentialValidation {
    let mut request = client.get(url);
    if let Some(token) = token {
        request = request.bearer_auth(token);
    }
    validate_response(request, "OpenAI-compatible /models").await
}

async fn validate_anthropic_models(
    client: &reqwest::Client,
    url: &str,
    token: &str,
) -> ProviderCredentialValidation {
    validate_response(
        client
            .get(url)
            .header("x-api-key", token)
            .header("anthropic-version", "2023-06-01"),
        "Anthropic /models",
    )
    .await
}

async fn validate_anthropic_oauth_models(
    client: &reqwest::Client,
    url: &str,
    token: &str,
) -> ProviderCredentialValidation {
    validate_response(
        client
            .get(url)
            .bearer_auth(token)
            .header("anthropic-version", "2023-06-01")
            .header("anthropic-beta", "oauth-2025-04-20"),
        "Claude Code OAuth /models",
    )
    .await
}

async fn validate_google_models(
    client: &reqwest::Client,
    url: &str,
    token: &str,
) -> ProviderCredentialValidation {
    validate_response(
        client.get(url).query(&[("key", token)]),
        "Google list models",
    )
    .await
}

async fn validate_bearer_get(
    client: &reqwest::Client,
    url: &str,
    token: &str,
    headers: &[(&str, &str)],
    label: &str,
) -> ProviderCredentialValidation {
    let mut request = client.get(url).bearer_auth(token);
    for (key, value) in headers {
        request = request.header(*key, *value);
    }
    validate_response(request, label).await
}

async fn validate_response(
    request: reqwest::RequestBuilder,
    label: &str,
) -> ProviderCredentialValidation {
    match request.send().await {
        Ok(response) => {
            let status = response.status();
            let text = if status.is_success() {
                String::new()
            } else {
                response.text().await.unwrap_or_default()
            };
            if status.is_success() {
                ProviderCredentialValidation::Passed(validation_detail(
                    "provider.remote.accepted",
                    Some(label.to_string()),
                ))
            } else if status == reqwest::StatusCode::FORBIDDEN
                && response_forbidden_but_authenticated(&text)
            {
                ProviderCredentialValidation::Warning(validation_detail(
                    "provider.remote.permission_limited",
                    Some(label.to_string()),
                ))
            } else {
                ProviderCredentialValidation::Failed(validation_detail(
                    "provider.remote.rejected",
                    Some(format!(
                        "{label} HTTP {status}: {}",
                        truncate_validation_body(&text)
                    )),
                ))
            }
        }
        Err(error) => ProviderCredentialValidation::Failed(validation_detail(
            "provider.remote.request_failed",
            Some(format!("{label}: {error}")),
        )),
    }
}

fn response_forbidden_but_authenticated(body: &str) -> bool {
    let body = body.to_ascii_lowercase();
    body.contains("insufficient permissions")
        || body.contains("missing scopes")
        || body.contains("permission")
}

fn truncate_validation_body(body: &str) -> String {
    let compact = body.split_whitespace().collect::<Vec<_>>().join(" ");
    if compact.len() > 240 {
        format!("{}...", &compact[..240])
    } else if compact.is_empty() {
        "<empty response>".to_string()
    } else {
        compact
    }
}

fn looks_like_bearer_token(token: &str) -> bool {
    token.split('.').count() >= 3 || token.len() >= 24
}

fn push_unique_env(keys: &mut Vec<String>, key: Option<&str>) {
    let Some(key) = key.map(str::trim).filter(|value| !value.is_empty()) else {
        return;
    };
    if !keys.iter().any(|item| item == key) {
        keys.push(key.to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};

    #[test]
    fn validation_url_rejects_unsafe_or_invalid_bases_and_joins_suffixes() {
        assert_eq!(
            validation_url(Some("https://api.example.test/v1/"), "/models"),
            Some("https://api.example.test/v1/models".to_string())
        );
        assert_eq!(
            validation_url(Some(" http://localhost:11434 "), "api/tags"),
            Some("http://localhost:11434/api/tags".to_string())
        );
        assert_eq!(validation_url(None, "models"), None);
        assert_eq!(validation_url(Some("   "), "models"), None);
        assert_eq!(
            validation_url(Some("https://{workspace}.example.test"), "models"),
            None
        );
        assert_eq!(validation_url(Some("not a url"), "models"), None);
    }

    #[test]
    fn provider_validation_helpers_classify_tokens_domains_and_local_providers() {
        let provider = tura_llm_rust::ProviderCatalogConfig {
            domains: vec!["LLM".to_string(), "productivity".to_string()],
            ..Default::default()
        };

        assert!(provider_has_domain(&provider, "llm"));
        assert!(provider_has_domain(&provider, "PRODUCTIVITY"));
        assert!(!provider_has_domain(&provider, "browser"));
        assert!(looks_like_bearer_token("aaa.bbb.ccc"));
        assert!(looks_like_bearer_token("abcdefghijklmnopqrstuvwxyz"));
        assert!(!looks_like_bearer_token("short-token"));
        assert!(is_local_no_token_provider("ollama", None));
        assert!(is_local_no_token_provider(
            "custom",
            Some("http://127.0.0.1:1234/v1")
        ));
        assert!(is_local_no_token_provider(
            "custom",
            Some("http://localhost:1234/v1")
        ));
        assert!(!is_local_no_token_provider(
            "custom",
            Some("https://api.example.test/v1")
        ));
        assert!(is_openai_compatible_provider("qwen-cn"));
        assert!(!is_openai_compatible_provider("feishu"));
    }

    #[test]
    fn push_unique_env_trims_skips_blank_and_preserves_first_seen_order() {
        let mut keys = vec!["OPENAI_API_KEY".to_string()];

        push_unique_env(&mut keys, None);
        push_unique_env(&mut keys, Some("   "));
        push_unique_env(&mut keys, Some(" OPENAI_API_KEY "));
        push_unique_env(&mut keys, Some("OPENAI_REFRESH_TOKEN"));
        push_unique_env(&mut keys, Some("OPENAI_REFRESH_TOKEN"));
        push_unique_env(&mut keys, Some("OPENAI_LOGIN"));

        assert_eq!(
            keys,
            vec![
                "OPENAI_API_KEY".to_string(),
                "OPENAI_REFRESH_TOKEN".to_string(),
                "OPENAI_LOGIN".to_string()
            ]
        );
    }

    #[test]
    fn validation_detail_messages_are_stable_for_known_and_unknown_codes() {
        let detail = validation_detail(
            "provider.remote.rejected",
            Some("OpenAI-compatible /models HTTP 401".to_string()),
        );
        assert_eq!(detail.code, "provider.remote.rejected");
        assert_eq!(
            detail.message,
            "remote validation rejected credentials: OpenAI-compatible /models HTTP 401"
        );
        assert_eq!(
            detail.value.as_deref(),
            Some("OpenAI-compatible /models HTTP 401")
        );

        let unknown = validation_detail("provider.future.code", None);
        assert_eq!(unknown.message, "provider validation detail");
        assert_eq!(unknown.value, None);
    }

    #[test]
    fn response_body_helpers_compact_permission_and_empty_cases() {
        assert!(response_forbidden_but_authenticated(
            r#"{"error":"missing scopes for this token"}"#
        ));
        assert!(response_forbidden_but_authenticated(
            "Insufficient permissions for model list"
        ));
        assert!(!response_forbidden_but_authenticated(
            "invalid or expired token"
        ));
        assert_eq!(truncate_validation_body(" \n\t "), "<empty response>");
        assert_eq!(
            truncate_validation_body("one\n two\tthree"),
            "one two three"
        );

        let long = "x".repeat(260);
        let truncated = truncate_validation_body(&long);
        assert_eq!(truncated.len(), 243);
        assert!(truncated.ends_with("..."));
    }

    #[tokio::test]
    async fn validate_response_maps_success_warning_rejection_and_request_errors() {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(1))
            .build()
            .expect("client");

        let ok_url = serve_http_once(200, "{}", None);
        let ok = validate_response(client.get(ok_url), "test success").await;
        assert!(matches!(
            ok,
            ProviderCredentialValidation::Passed(detail)
                if detail.code == "provider.remote.accepted"
                    && detail.value.as_deref() == Some("test success")
        ));

        let warning_url = serve_http_once(403, "missing scopes", None);
        let warning = validate_response(client.get(warning_url), "test warning").await;
        assert!(matches!(
            warning,
            ProviderCredentialValidation::Warning(detail)
                if detail.code == "provider.remote.permission_limited"
                    && detail.value.as_deref() == Some("test warning")
        ));

        let rejected_url = serve_http_once(401, "bad\n token", None);
        let rejected = validate_response(client.get(rejected_url), "test reject").await;
        assert!(matches!(
            rejected,
            ProviderCredentialValidation::Failed(detail)
                if detail.code == "provider.remote.rejected"
                    && detail.value.as_deref().is_some_and(|value| value.contains("HTTP 401 Unauthorized: bad token"))
        ));

        let dropped_url = serve_http_once(200, "", Some(ServerBehavior::DropWithoutResponse));
        let failed = validate_response(client.get(dropped_url), "test drop").await;
        assert!(matches!(
            failed,
            ProviderCredentialValidation::Failed(detail)
                if detail.code == "provider.remote.request_failed"
                    && detail.value.as_deref().is_some_and(|value| value.contains("test drop"))
        ));
    }

    #[tokio::test]
    async fn remote_validation_fails_locally_before_network_for_missing_credentials() {
        let codex_missing =
            validate_provider_credentials_remotely("codex", None, None, None, false).await;
        assert!(matches!(
            codex_missing,
            ProviderCredentialValidation::Failed(detail)
                if detail.code == "provider.credential.oauth_token_missing"
        ));

        let codex_short =
            validate_provider_credentials_remotely("codex", None, None, Some("short"), false).await;
        assert!(matches!(
            codex_short,
            ProviderCredentialValidation::Failed(detail)
                if detail.code == "provider.credential.oauth_token_invalid_format"
        ));

        let openrouter_missing =
            validate_provider_credentials_remotely("openrouter", None, None, None, true).await;
        assert!(matches!(
            openrouter_missing,
            ProviderCredentialValidation::Failed(detail)
                if detail.code == "provider.credential.api_key_missing"
                    && detail.value.as_deref() == Some("OpenRouter")
        ));

        let unsupported =
            validate_provider_credentials_remotely("perplexity", None, None, Some("token"), true)
                .await;
        assert!(matches!(
            unsupported,
            ProviderCredentialValidation::Unsupported(detail)
                if detail.code == "provider.validation.public_model_list_unsupported"
        ));
    }

    #[tokio::test]
    async fn openai_compatible_validation_uses_local_base_url_and_optional_bearer() {
        let provider = tura_llm_rust::ProviderCatalogConfig {
            api_style: "openapi".to_string(),
            base_url: "http://127.0.0.1:0/v1".to_string(),
            domains: vec!["llm".to_string()],
            ..Default::default()
        };
        let url = serve_http_once(
            200,
            r#"{"data":[]}"#,
            Some(ServerBehavior::AssertBearer("secret-key")),
        );
        let base_url = url.trim_end_matches("/models").to_string();

        let passed = validate_provider_credentials_remotely(
            "custom-openai",
            Some(&provider),
            Some(&base_url),
            Some("secret-key"),
            true,
        )
        .await;

        assert!(matches!(
            passed,
            ProviderCredentialValidation::Passed(detail)
                if detail.code == "provider.remote.accepted"
                    && detail.value.as_deref() == Some("OpenAI-compatible /models")
        ));

        let no_token = validate_provider_credentials_remotely(
            "custom-openai",
            Some(&provider),
            Some("https://api.example.test/v1"),
            None,
            true,
        )
        .await;
        assert!(matches!(
            no_token,
            ProviderCredentialValidation::Failed(detail)
                if detail.code == "provider.credential.api_key_missing"
        ));

        let no_token_url =
            serve_http_once(200, r#"{"data":[]}"#, Some(ServerBehavior::AssertNoBearer));
        let no_token_base_url = no_token_url.trim_end_matches("/models").to_string();
        let local = validate_provider_credentials_remotely(
            "ollama",
            Some(&provider),
            Some(&no_token_base_url),
            None,
            true,
        )
        .await;
        assert!(matches!(
            local,
            ProviderCredentialValidation::Passed(detail)
                if detail.code == "provider.remote.accepted"
        ));
    }

    #[tokio::test]
    async fn antigravity_api_key_validation_uses_configured_models_endpoint_not_oauth() {
        let provider = tura_llm_rust::ProviderCatalogConfig {
            api_style: "antigravity".to_string(),
            base_url: "http://127.0.0.1:0/v1".to_string(),
            domains: vec!["llm".to_string()],
            auth_methods: vec!["api_key".to_string()],
            ..Default::default()
        };
        let url = serve_http_once(
            200,
            r#"{"data":[]}"#,
            Some(ServerBehavior::AssertBearer("secret-key")),
        );
        let base_url = url.trim_end_matches("/models").to_string();

        let passed = validate_provider_credentials_remotely(
            "antigravity",
            Some(&provider),
            Some(&base_url),
            Some("secret-key"),
            true,
        )
        .await;

        assert!(matches!(
            passed,
            ProviderCredentialValidation::Passed(detail)
                if detail.code == "provider.remote.accepted"
                    && detail.value.as_deref() == Some("OpenAI-compatible /models")
        ));
    }

    enum ServerBehavior {
        DropWithoutResponse,
        AssertBearer(&'static str),
        AssertNoBearer,
    }

    fn serve_http_once(
        status: u16,
        body: &'static str,
        behavior: Option<ServerBehavior>,
    ) -> String {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind local server");
        let addr = listener.local_addr().expect("local addr");
        std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept request");
            let mut buffer = [0_u8; 4096];
            let size = stream.read(&mut buffer).expect("read request");
            let request = String::from_utf8_lossy(&buffer[..size]);
            match behavior {
                Some(ServerBehavior::DropWithoutResponse) => return,
                Some(ServerBehavior::AssertBearer(expected)) => {
                    assert!(
                        request.contains(&format!("authorization: Bearer {expected}"))
                            || request.contains(&format!("Authorization: Bearer {expected}")),
                        "{request}"
                    );
                }
                Some(ServerBehavior::AssertNoBearer) => {
                    assert!(
                        !request.to_ascii_lowercase().contains("authorization:"),
                        "{request}"
                    );
                }
                None => {}
            }
            let reason = match status {
                200 => "OK",
                401 => "Unauthorized",
                403 => "Forbidden",
                _ => "Test",
            };
            let response = format!(
                "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            stream
                .write_all(response.as_bytes())
                .expect("write response");
        });
        format!("http://{addr}/models")
    }
}
