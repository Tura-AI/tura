use std::path::PathBuf;
use std::sync::Arc;

use chrono::Utc;
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;
use tracing::{error, info, warn};
use uuid::Uuid;

use crate::auth_registry::OAuthAuthorizeKind;
use crate::llm::providers;
use crate::logging::{build_call_log, write_llm_log};
use crate::tura_conf::TuraConfig;
use crate::utils::{strip_text_tool_calls, text_tool_calls_value};

#[path = "tura_llm_config.rs"]
mod tura_llm_config;
pub use tura_llm_config::{
    apply_latency_for_tier, latency_level_for_tier, provider_latency_config,
    provider_latency_timeouts, set_provider_latency_config, set_provider_latency_timeouts,
    CallOptions, CatalogModelConfig, CatalogModelDetail, CatalogModelLimit, CatalogModelModalities,
    ModelCatalog, ProviderAuthConfig, ProviderCatalogConfig, ProviderEnumCatalog,
    ProviderLatencyConfig, ProviderLatencyTimeouts, RawProviderConfig, RawRouteConfig, RootConfig,
    RouteConfig, Settings, SETTINGS,
};

#[derive(Debug, Clone)]
pub enum ProviderStreamEvent {
    ProviderOutputStarted,
    CommandRunCommandReady {
        tool_call_id: String,
        command_index: usize,
        command: Value,
    },
}

pub type ProviderStreamEventSink = Arc<dyn Fn(ProviderStreamEvent) + Send + Sync>;

#[derive(Debug, Error)]
pub enum TuraError {
    #[error("config error: {message}")]
    Config { message: String },

    #[error("unknown provider '{provider}'")]
    UnknownProvider { provider: String },

    #[error("validation error: {message}")]
    Validation { message: String },

    #[error("http status {status}: {body}")]
    HttpStatus { status: u16, body: String },

    #[error("network error: {message}")]
    Network { message: String },

    #[error("provider '{provider}' request failed: {message}")]
    ProviderRequest { provider: String, message: String },

    #[error("all providers failed: {message}")]
    AllProvidersFailed { message: String },

    #[error(transparent)]
    Json(#[from] serde_json::Error),

    #[error("io error: {message}")]
    Io { message: String },
}

impl TuraError {
    pub fn io(err: std::io::Error) -> Self {
        Self::Io {
            message: err.to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UsageDetails {
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub total_tokens: Option<u64>,
    pub cached_input_tokens: Option<u64>,
    pub cache_write_tokens: Option<u64>,
    pub reasoning_tokens: Option<u64>,
    pub audio_input_tokens: Option<u64>,
    pub audio_output_tokens: Option<u64>,
    pub context_window: Option<u64>,
    pub context_used_tokens: Option<u64>,
    pub context_utilization_ratio: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CostDetails {
    pub input_cost: Option<f64>,
    pub output_cost: Option<f64>,
    pub cache_read_cost: Option<f64>,
    pub cache_write_cost: Option<f64>,
    pub reasoning_cost: Option<f64>,
    pub total_cost: Option<f64>,
    pub currency: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CallMetrics {
    pub usage: UsageDetails,
    pub cost: CostDetails,
    pub cache_hit: bool,
    pub cache_triggered_at_input_tokens: Option<u64>,
    pub tool_call_count: usize,
    pub finish_reason: Option<String>,
    pub provider_request_id: Option<String>,
    pub raw_usage: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProviderResponse {
    pub content: Value,
    pub raw: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metrics: Option<CallMetrics>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub provider: String,
    pub base_url: String,
    pub model: String,
    pub temperature: f64,
}

impl ProviderConfig {
    pub fn validate(&self) -> Result<(), TuraError> {
        if self.model.trim().is_empty() {
            return Err(TuraError::Validation {
                message: "model must not be empty".into(),
            });
        }
        if !(0.0..=2.0).contains(&self.temperature) {
            return Err(TuraError::Validation {
                message: "temperature must be within [0.0, 2.0]".into(),
            });
        }
        Ok(())
    }

    fn get_api_key(&self, conf: &TuraConfig) -> Result<String, TuraError> {
        crate::auth_registry::provider_token_env(&self.provider)
            .and_then(|key| conf.get(key))
            .or_else(|| conf.get(&format!("{}_API_KEY", self.provider.to_uppercase())))
            .or_else(|| conf.get(&format!("{}_api_key", self.provider)))
            .or_else(|| conf.get(&self.provider))
            .ok_or_else(|| TuraError::Config {
                message: format!("API Key not found for provider '{}'", self.provider),
            })
    }

    pub async fn embed(&self, text: &str, conf: &TuraConfig) -> Result<Vec<f32>, TuraError> {
        self.validate()?;
        let _parameter_policy = providers::parameter_policy(&self.provider);
        let api_key = if should_use_openai_oauth(&self.provider, &self.base_url, conf) {
            refresh_openai_access_token_if_needed(conf).await?
        } else {
            self.get_api_key(conf)?
        };
        match self.provider.to_lowercase().as_str() {
            "codex" if self.model.starts_with("text-embedding-") => {
                providers::openai::embed("https://api.openai.com/v1", &self.model, &api_key, text)
                    .await
            }
            "cohere" => {
                crate::llm::cohere::embed(&self.base_url, &self.model, &api_key, text).await
            }
            "google" => providers::google::embed(&self.base_url, &self.model, &api_key, text).await,
            "minimax" => {
                providers::minimax::embed(&self.base_url, &self.model, &api_key, text).await
            }
            "openrouter" => {
                crate::llm::openapi::embed_for_provider(
                    "openrouter",
                    &self.base_url,
                    &self.model,
                    &api_key,
                    text,
                )
                .await
            }
            _ => providers::openai::embed(&self.base_url, &self.model, &api_key, text).await,
        }
    }

    pub async fn call(
        &self,
        conf: &TuraConfig,
        messages: Vec<Value>,
        options: CallOptions,
    ) -> Result<ProviderResponse, TuraError> {
        self.call_with_stream_events(conf, messages, options, None)
            .await
    }

    pub async fn call_with_stream_events(
        &self,
        conf: &TuraConfig,
        messages: Vec<Value>,
        options: CallOptions,
        stream_events: Option<ProviderStreamEventSink>,
    ) -> Result<ProviderResponse, TuraError> {
        self.validate()?;
        let _parameter_policy = providers::parameter_policy(&self.provider);
        let mut api_key = if should_use_openai_oauth(&self.provider, &self.base_url, conf) {
            refresh_openai_access_token_if_needed(conf).await?
        } else {
            self.get_api_key(conf)?
        };
        let call_id = Uuid::new_v4().simple().to_string();
        let started_at = Utc::now();
        let request_params = serde_json::to_value(&options).unwrap_or(Value::Null);

        // Reactive OAuth refresh: if the first attempt fails with an
        // authentication error (expired/invalid token), refresh the provider's
        // OAuth access token using its registered refresh token and retry once.
        let mut refreshed = false;
        let result = loop {
            let attempt = match self.provider.to_lowercase().as_str() {
                "google" => {
                    providers::google::call(
                        &self.base_url,
                        &self.model,
                        &api_key,
                        &messages,
                        &options,
                    )
                    .await
                }
                "bedrock" => {
                    providers::bedrock::call(
                        &self.base_url,
                        &self.model,
                        &api_key,
                        &messages,
                        &options,
                    )
                    .await
                }
                "codex" => {
                    providers::codex::call_with_stream_events(
                        &self.base_url,
                        &self.model,
                        &api_key,
                        &messages,
                        &options,
                        stream_events.clone(),
                    )
                    .await
                }
                "minimax" => {
                    providers::minimax::call_with_stream_events(
                        &self.base_url,
                        &self.model,
                        &api_key,
                        &messages,
                        &options,
                        stream_events.clone(),
                    )
                    .await
                }
                "anthropic" | "anthropic-api" | "claude-code" => {
                    providers::claude_code::call_with_stream_events(
                        &self.base_url,
                        &self.model,
                        &api_key,
                        &messages,
                        &options,
                        stream_events.clone(),
                    )
                    .await
                }
                "openai" if should_use_openai_oauth(&self.provider, &self.base_url, conf) => {
                    providers::codex::call_with_stream_events(
                        &self.base_url,
                        &self.model,
                        &api_key,
                        &messages,
                        &options,
                        stream_events.clone(),
                    )
                    .await
                }
                "openai" | "openai-api" | "chatgpt" => {
                    providers::openai::call_with_stream_events(
                        &self.base_url,
                        &self.model,
                        &api_key,
                        &messages,
                        &options,
                        stream_events.clone(),
                    )
                    .await
                }
                "xai" | "grok" => {
                    providers::xai::call_with_stream_events(
                        &self.base_url,
                        &self.model,
                        &api_key,
                        &messages,
                        &options,
                        stream_events.clone(),
                    )
                    .await
                }
                "qwen" | "qwen_cn" | "qwen-cn" => {
                    providers::qwen::call_with_stream_events(
                        &self.base_url,
                        &self.model,
                        &api_key,
                        &messages,
                        &options,
                        stream_events.clone(),
                    )
                    .await
                }
                other => {
                    crate::llm::openapi::call_with_stream_events(
                        &self.base_url,
                        &self.model,
                        other,
                        &api_key,
                        &messages,
                        &options,
                        stream_events.clone(),
                    )
                    .await
                }
            };

            if !refreshed && is_auth_expired_error(&attempt) {
                match try_refresh_oauth_access_token(&self.provider, conf).await {
                    Ok(Some(new_key)) => {
                        warn!(
                            provider = %self.provider,
                            "provider call hit auth error; refreshed OAuth token and retrying"
                        );
                        api_key = new_key;
                        refreshed = true;
                        continue;
                    }
                    Ok(None) => break attempt,
                    Err(refresh_err) => {
                        warn!(provider = %self.provider, error = %refresh_err, "OAuth token refresh failed");
                        break attempt;
                    }
                }
            }
            break attempt;
        };

        let finished_at = Utc::now();
        let duration_ms = (finished_at - started_at)
            .num_microseconds()
            .unwrap_or_default() as f64
            / 1000.0;

        match result {
            Ok(response) => {
                let log = build_call_log(
                    &self.provider,
                    &self.model,
                    &self.base_url,
                    Value::Array(messages.clone()),
                    Some(response.raw.clone()),
                    request_params,
                    options.response_format.clone(),
                    started_at,
                    finished_at,
                    duration_ms,
                    true,
                    &call_id,
                    response.metrics.clone(),
                    None,
                    None,
                );
                if let Ok(path) = write_llm_log(&log, Some(&call_id)).await {
                    info!(provider = %self.provider, model = %self.model, log_path = %path.display(), duration_ms = duration_ms, "provider call succeeded");
                }
                Ok(response)
            }
            Err(err) => {
                let log = build_call_log(
                    &self.provider,
                    &self.model,
                    &self.base_url,
                    Value::Array(messages.clone()),
                    None,
                    request_params,
                    options.response_format.clone(),
                    started_at,
                    finished_at,
                    duration_ms,
                    false,
                    &call_id,
                    None,
                    Some(err.to_string()),
                    None,
                );
                if let Ok(path) = write_llm_log(&log, Some(&call_id)).await {
                    error!(provider = %self.provider, model = %self.model, log_path = %path.display(), error = %err, "provider call failed");
                }
                Err(err)
            }
        }
    }
}

fn openai_login_is_oauth(conf: &TuraConfig) -> bool {
    if conf
        .get("OPENAI_LOGIN")
        .map(|value| value.eq_ignore_ascii_case("oauth"))
        .unwrap_or(false)
    {
        return true;
    }
    if openai_provider_auth_config_login_is_oauth() {
        return true;
    }
    conf.get("OPENAI_API_KEY")
        .filter(|value| !value.trim().is_empty())
        .is_none()
        && load_codex_auth_tokens().is_some()
}

fn should_use_openai_oauth(provider: &str, base_url: &str, conf: &TuraConfig) -> bool {
    if provider.eq_ignore_ascii_case("codex") {
        return true;
    }
    provider.eq_ignore_ascii_case("openai")
        && openai_login_is_oauth(conf)
        && openai_oauth_base_url_allowed(base_url)
}

fn openai_oauth_base_url_allowed(base_url: &str) -> bool {
    let normalized = base_url.trim().trim_end_matches('/');
    matches!(
        normalized,
        "" | "https://api.openai.com" | "https://api.openai.com/v1"
    )
}

async fn refresh_openai_access_token_if_needed(conf: &TuraConfig) -> Result<String, TuraError> {
    std::env::set_var("OPENAI_LOGIN", "oauth");
    let codex_auth = load_codex_auth_tokens();
    let access = conf
        .get("OPENAI_API_KEY")
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            codex_auth
                .as_ref()
                .map(|tokens| tokens.access_token.clone())
        })
        .ok_or_else(|| TuraError::Config {
            message: "Configuration key 'OPENAI_API_KEY' not found".to_string(),
        })?;
    let expires = conf
        .get("OPENAI_TOKEN_EXPIRES")
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or_default();
    let now = Utc::now().timestamp_millis();
    if expires > now + 60_000 {
        if let Some(tokens) = codex_auth.as_ref() {
            apply_codex_auth_env(tokens);
        }
        return Ok(access);
    }
    if let Some(tokens) = codex_auth.as_ref() {
        if tokens.access_token != access {
            apply_codex_auth_env(tokens);
            return Ok(tokens.access_token.clone());
        }
    }

    let refresh = conf
        .get("OPENAI_REFRESH_TOKEN")
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            codex_auth
                .as_ref()
                .map(|tokens| tokens.refresh_token.clone())
        })
        .ok_or_else(|| TuraError::Config {
            message: "Configuration key 'OPENAI_REFRESH_TOKEN' not found".to_string(),
        })?;
    let response = reqwest::Client::new()
        .post("https://auth.openai.com/oauth/token")
        .header(CONTENT_TYPE, "application/x-www-form-urlencoded")
        .form(&[
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh.as_str()),
            ("client_id", "app_EMoamEEZ73f0CkXaXp7hrann"),
        ])
        .send()
        .await
        .map_err(|err| TuraError::Network {
            message: err.to_string(),
        })?;
    let status = response.status();
    let body: Value = response.json().await.map_err(|err| TuraError::Network {
        message: err.to_string(),
    })?;
    if !status.is_success() {
        if let Some(access) = codex_auth
            .as_ref()
            .map(|tokens| tokens.access_token.clone())
        {
            if let Some(tokens) = codex_auth.as_ref() {
                apply_codex_auth_env(tokens);
            } else {
                std::env::set_var("OPENAI_API_KEY", &access);
                std::env::set_var("OPENAI_REFRESH_TOKEN", &refresh);
            }
            return Ok(access);
        }
        return Err(TuraError::HttpStatus {
            status: status.as_u16(),
            body: body.to_string(),
        });
    }
    let next_access = body
        .get("access_token")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| TuraError::ProviderRequest {
            provider: "openai".to_string(),
            message: "OpenAI OAuth refresh response did not include access_token".to_string(),
        })?
        .to_string();
    std::env::set_var("OPENAI_API_KEY", &next_access);
    if let Some(tokens) = codex_auth.as_ref() {
        if let Some(account_id) = tokens.account_id.as_deref() {
            std::env::set_var("OPENAI_ACCOUNT_ID", account_id);
        }
    }
    if let Some(refresh) = body.get("refresh_token").and_then(Value::as_str) {
        std::env::set_var("OPENAI_REFRESH_TOKEN", refresh);
    }
    let next_expires = now
        + body
            .get("expires_in")
            .and_then(Value::as_i64)
            .unwrap_or(3600)
            * 1000;
    std::env::set_var("OPENAI_TOKEN_EXPIRES", next_expires.to_string());
    Ok(next_access)
}

/// Whether a provider call result is an authentication failure that an OAuth
/// token refresh could plausibly fix (expired / invalid access token).
fn is_auth_expired_error<T>(result: &Result<T, TuraError>) -> bool {
    matches!(
        result,
        Err(TuraError::HttpStatus { status, .. }) if *status == 401 || *status == 403
    )
}

/// Reactively refresh an OAuth access token for `provider` using its registered
/// refresh token. Returns `Ok(Some(new_access))` on success, `Ok(None)` when the
/// provider does not support OAuth refresh or has no refresh token configured,
/// and `Err(_)` when the refresh attempt itself failed.
///
/// On success the new access token (and any rotated refresh token / expiry) is
/// written both to the process environment and persisted to the `.env` file so
/// it survives a restart. OpenAI/Codex, Anthropic (claude-code) and Google OAuth
/// providers are all supported via the provider auth registry.
async fn try_refresh_oauth_access_token(
    provider: &str,
    conf: &TuraConfig,
) -> Result<Option<String>, TuraError> {
    let Some(entry) = crate::auth_registry::provider_auth_registry_entry(provider) else {
        return Ok(None);
    };
    if !entry.capabilities.supports_oauth_refresh {
        return Ok(None);
    }
    let Some(refresh_env) = entry.refresh_env else {
        return Ok(None);
    };
    let Some(refresh) = conf
        .get(refresh_env)
        .filter(|value| !value.trim().is_empty())
    else {
        return Ok(None);
    };

    let env_default = |name: &str, fallback: &str| -> String {
        std::env::var(name)
            .ok()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| fallback.to_string())
    };

    let kind = entry.oauth_callback_kind.or(entry.oauth_authorize_kind);
    // (token_url, form params, send_cli_headers)
    let (token_url, form, send_cli_headers) = match kind {
        Some(OAuthAuthorizeKind::AnthropicPkce) => (
            env_default(
                "ANTHROPIC_OAUTH_TOKEN_URL",
                "https://platform.claude.com/v1/oauth/token",
            ),
            vec![
                ("grant_type".to_string(), "refresh_token".to_string()),
                ("refresh_token".to_string(), refresh.clone()),
                (
                    "client_id".to_string(),
                    env_default(
                        "ANTHROPIC_OAUTH_CLIENT_ID",
                        "9d1c250a-e61b-44d9-88ed-5944d1962f5e",
                    ),
                ),
            ],
            // platform.claude.com is behind Cloudflare and rejects the default
            // reqwest user-agent with HTTP 403 (error 1010); identify as the CLI.
            true,
        ),
        Some(OAuthAuthorizeKind::OpenAiPkce) => (
            env_default(
                "OPENAI_OAUTH_TOKEN_URL",
                "https://auth.openai.com/oauth/token",
            ),
            vec![
                ("grant_type".to_string(), "refresh_token".to_string()),
                ("refresh_token".to_string(), refresh.clone()),
                (
                    "client_id".to_string(),
                    env_default("OPENAI_OAUTH_CLIENT_ID", "app_EMoamEEZ73f0CkXaXp7hrann"),
                ),
            ],
            false,
        ),
        Some(OAuthAuthorizeKind::GooglePkce) => {
            let prefix = provider.to_uppercase().replace('-', "_");
            let mut form = vec![
                ("grant_type".to_string(), "refresh_token".to_string()),
                ("refresh_token".to_string(), refresh.clone()),
            ];
            if let Some(client_id) = conf
                .get(&format!("{prefix}_OAUTH_CLIENT_ID"))
                .or_else(|| conf.get("GOOGLE_OAUTH_CLIENT_ID"))
                .filter(|value| !value.trim().is_empty())
            {
                form.push(("client_id".to_string(), client_id));
            }
            if let Some(client_secret) = conf
                .get(&format!("{prefix}_OAUTH_CLIENT_SECRET"))
                .or_else(|| conf.get("GOOGLE_OAUTH_CLIENT_SECRET"))
                .filter(|value| !value.trim().is_empty())
            {
                form.push(("client_secret".to_string(), client_secret));
            }
            (
                env_default(
                    "GOOGLE_OAUTH_TOKEN_URL",
                    "https://oauth2.googleapis.com/token",
                ),
                form,
                false,
            )
        }
        _ => return Ok(None),
    };

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .map_err(|err| TuraError::Network {
            message: err.to_string(),
        })?;
    let mut request = client
        .post(&token_url)
        .header(CONTENT_TYPE, "application/x-www-form-urlencoded");
    if send_cli_headers {
        request = request
            .header(
                reqwest::header::USER_AGENT,
                "claude-cli/1.0 (external, cli)",
            )
            .header(reqwest::header::ACCEPT, "application/json");
    }
    let response = request
        .form(&form)
        .send()
        .await
        .map_err(|err| TuraError::Network {
            message: err.to_string(),
        })?;
    let status = response.status();
    let body: Value = response.json().await.map_err(|err| TuraError::Network {
        message: err.to_string(),
    })?;
    if !status.is_success() {
        return Err(TuraError::HttpStatus {
            status: status.as_u16(),
            body: body.to_string(),
        });
    }

    let access = body
        .get("access_token")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| TuraError::ProviderRequest {
            provider: provider.to_string(),
            message: "OAuth refresh response did not include access_token".to_string(),
        })?
        .to_string();

    let env_path = conf.env_path().to_path_buf();
    if let Some(token_env) = entry.token_env {
        std::env::set_var(token_env, &access);
        persist_env_var(&env_path, token_env, &access);
    }
    if let Some(new_refresh) = body
        .get("refresh_token")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
    {
        std::env::set_var(refresh_env, new_refresh);
        persist_env_var(&env_path, refresh_env, new_refresh);
    }
    if let Some(expires_env) = entry.expires_env {
        let expires_at = Utc::now().timestamp_millis()
            + body
                .get("expires_in")
                .and_then(Value::as_i64)
                .unwrap_or(3600)
                * 1000;
        let expires_at = expires_at.to_string();
        std::env::set_var(expires_env, &expires_at);
        persist_env_var(&env_path, expires_env, &expires_at);
    }

    Ok(Some(access))
}

/// Upsert `key=value` into the dotenv file at `env_path`, preserving the
/// existing newline style and leaving other entries untouched. Best-effort:
/// failures are ignored because the in-process `set_var` already keeps the
/// running process working.
fn persist_env_var(env_path: &std::path::Path, key: &str, value: &str) {
    let existing = std::fs::read_to_string(env_path).unwrap_or_default();
    let newline = if existing.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    };
    let mut out = String::new();
    let mut replaced = false;
    for line in existing.lines() {
        if line
            .split_once('=')
            .is_some_and(|(name, _)| name.trim() == key)
        {
            out.push_str(key);
            out.push('=');
            out.push_str(value);
            out.push_str(newline);
            replaced = true;
        } else {
            out.push_str(line);
            out.push_str(newline);
        }
    }
    if !replaced {
        out.push_str(key);
        out.push('=');
        out.push_str(value);
        out.push_str(newline);
    }
    let _ = std::fs::write(env_path, out);
}

#[derive(Debug, Clone)]
struct CodexAuthTokens {
    access_token: String,
    refresh_token: String,
    account_id: Option<String>,
}

fn load_codex_auth_tokens() -> Option<CodexAuthTokens> {
    let path = codex_auth_json_path()?;
    let value: Value = serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()?;
    let tokens = value.get("tokens")?;
    let access_token = tokens
        .get("access_token")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())?
        .to_string();
    let refresh_token = tokens
        .get("refresh_token")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())?
        .to_string();
    let account_id = tokens
        .get("account_id")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(ToString::to_string);
    Some(CodexAuthTokens {
        access_token,
        refresh_token,
        account_id,
    })
}

fn codex_auth_json_path() -> Option<PathBuf> {
    if let Some(home) = std::env::var_os("CODEX_HOME").filter(|value| !value.is_empty()) {
        return Some(PathBuf::from(home).join("auth.json"));
    }
    let home = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME"))?;
    Some(PathBuf::from(home).join(".codex").join("auth.json"))
}

fn apply_codex_auth_env(tokens: &CodexAuthTokens) {
    std::env::set_var("OPENAI_LOGIN", "oauth");
    std::env::set_var("OPENAI_API_KEY", &tokens.access_token);
    std::env::set_var("OPENAI_REFRESH_TOKEN", &tokens.refresh_token);
    if let Some(account_id) = tokens.account_id.as_deref() {
        std::env::set_var("OPENAI_ACCOUNT_ID", account_id);
    }
}

fn openai_provider_auth_config_login_is_oauth() -> bool {
    provider_config_json_path()
        .and_then(|path| std::fs::read_to_string(path).ok())
        .and_then(|content| serde_json::from_str::<Value>(&content).ok())
        .and_then(|value| {
            value
                .pointer("/provider_auth/openai/login")
                .and_then(Value::as_str)
                .map(|login| login.eq_ignore_ascii_case("oauth"))
        })
        .unwrap_or(false)
}

fn provider_config_json_path() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("TURA_PROVIDER_CONFIG").filter(|value| !value.is_empty()) {
        return Some(PathBuf::from(path));
    }
    if let Some(path) = std::env::var_os("TURALLM_CONFIG").filter(|value| !value.is_empty()) {
        return Some(PathBuf::from(path));
    }
    for path in default_provider_config_candidates() {
        if path.exists() {
            return Some(path);
        }
    }
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    Some(manifest_dir.join("src").join("provider_config.json"))
}

fn default_provider_config_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Ok(root) = std::env::var("TURA_PROJECT_ROOT") {
        let root = PathBuf::from(root);
        candidates.push(root.join("config").join("provider_config.json"));
        candidates.push(
            root.join("crates")
                .join("provider")
                .join("config")
                .join("provider_config.json"),
        );
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(bin_dir) = exe.parent() {
            candidates.push(bin_dir.join("config").join("provider_config.json"));
            candidates.push(bin_dir.join("provider_config.json"));
            if let Some(root) = bin_dir.parent() {
                candidates.push(root.join("config").join("provider_config.json"));
            }
        }
    }
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    candidates.push(manifest_dir.join("config").join("provider_config.json"));
    candidates.push(manifest_dir.join("config").join("tura_llm_config.json"));
    candidates
}

pub fn default_client(api_key: &str) -> Result<reqwest::Client, TuraError> {
    reqwest::Client::builder()
        .default_headers({
            let mut headers = reqwest::header::HeaderMap::new();
            let auth = format!("Bearer {api_key}");
            headers.insert(
                AUTHORIZATION,
                auth.parse()
                    .map_err(
                        |e: reqwest::header::InvalidHeaderValue| TuraError::Network {
                            message: e.to_string(),
                        },
                    )?,
            );
            headers.insert(
                CONTENT_TYPE,
                "application/json"
                    .parse()
                    .map_err(
                        |e: reqwest::header::InvalidHeaderValue| TuraError::Network {
                            message: e.to_string(),
                        },
                    )?,
            );
            headers
        })
        .build()
        .map_err(|e| TuraError::Network {
            message: e.to_string(),
        })
}

pub fn normalize_response_content(raw: &Value) -> Value {
    if let Some(message) = raw.pointer("/choices/0/message") {
        let content = message.get("content").cloned().unwrap_or(Value::Null);
        let tool_calls = message
            .get("tool_calls")
            .cloned()
            .or_else(|| content.as_str().map(text_tool_calls_value));
        if let Some(tool_calls) = tool_calls.filter(|value| !value.is_null()) {
            let mut object = serde_json::Map::new();
            if let Some(text) = content.as_str() {
                let stripped = strip_text_tool_calls(text);
                if !stripped.trim().is_empty() {
                    object.insert("text".to_string(), Value::String(stripped));
                }
            } else if !content.is_null() {
                object.insert("content".to_string(), content);
            }
            object.insert("tool_calls".to_string(), tool_calls);
            return Value::Object(object);
        }
        return content;
    }
    if let Some(output) = raw.get("output") {
        return output.clone();
    }
    if let Some(candidates) = raw.get("candidates") {
        return candidates.clone();
    }
    raw.clone()
}

pub fn estimate_context_utilization(metrics: &mut CallMetrics) {
    if let (Some(window), Some(input), maybe_output) = (
        metrics.usage.context_window,
        metrics.usage.input_tokens,
        metrics.usage.output_tokens,
    ) {
        let used = input + maybe_output.unwrap_or(0);
        metrics.usage.context_used_tokens = Some(used);
        if window > 0 {
            metrics.usage.context_utilization_ratio = Some(used as f64 / window as f64);
        }
    }
}

pub fn project_root() -> PathBuf {
    std::env::var("TURA_PROJECT_ROOT")
        .map(PathBuf::from)
        .ok()
        .filter(|path| path.exists())
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")))
}

#[cfg(test)]
#[path = "tura_llm_tests.rs"]
mod tests;
