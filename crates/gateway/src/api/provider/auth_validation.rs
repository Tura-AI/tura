use std::time::Duration;

use super::catalog::{provider_api_from_settings, provider_env_from_settings};
use super::{config_value, ProviderAuthActionDetail, ProviderAuthStatusResponse};

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
) -> ProviderValidationReceipt {
    let settings = tura_llm_rust::Settings::default().await.ok();
    let provider_config = settings
        .as_deref()
        .and_then(|settings| settings.model_catalog.providers.get(provider_id));
    let mut env_keys = Vec::new();
    push_unique_env(&mut env_keys, status.token_env.as_deref());
    push_unique_env(&mut env_keys, status.login_env.as_deref());
    push_unique_env(&mut env_keys, status.refresh_env.as_deref());
    if let Some(settings) = settings.as_ref() {
        for env in provider_env_from_settings(Some(settings), provider_id) {
            push_unique_env(&mut env_keys, Some(&env));
        }
    }
    if let Some(entry) = tura_llm_rust::provider_auth_registry_entry(provider_id) {
        push_unique_env(&mut env_keys, entry.token_env);
        push_unique_env(&mut env_keys, entry.login_env);
        push_unique_env(&mut env_keys, entry.refresh_env);
    }

    let present: Vec<String> = env_keys
        .iter()
        .filter(|key| config_value(key).is_some())
        .cloned()
        .collect();
    let missing: Vec<String> = env_keys
        .iter()
        .filter(|key| config_value(key).is_none())
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
    let token = token_env.and_then(config_value);
    let external_validation = validate_provider_credentials_remotely(
        provider_id,
        provider_config,
        base_url.as_deref(),
        token.as_deref(),
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
    details.push(validation_detail("provider.request.no_paid_model", None));
    parts.push("no paid model request was sent".to_string());
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

    if matches!(api_style, "codex" | "claude_code")
        || matches!(provider_id, "codex" | "claude-code" | "antigravity")
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

    if api_style == "codex" || provider_id == "codex" {
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

    if provider_id == "antigravity" || api_style == "antigravity" {
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
