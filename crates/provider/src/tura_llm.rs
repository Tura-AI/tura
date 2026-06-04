use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, OnceLock, RwLock};

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

pub static SETTINGS: OnceLock<Arc<Settings>> = OnceLock::new();
static PROVIDER_LATENCY_TIMEOUTS: OnceLock<RwLock<ProviderLatencyTimeouts>> = OnceLock::new();
static PROVIDER_LATENCY_CONFIG: OnceLock<RwLock<ProviderLatencyConfig>> = OnceLock::new();

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
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let config_path = manifest_dir.join("config").join("provider_config.json");
    if config_path.exists() {
        return Some(config_path);
    }
    let legacy_config_path = manifest_dir.join("config").join("tura_llm_config.json");
    if legacy_config_path.exists() {
        return Some(legacy_config_path);
    }
    Some(manifest_dir.join("src").join("provider_config.json"))
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CallOptions {
    pub response_format: Option<Value>,
    pub search: bool,
    pub force_search: bool,
    pub temperature: Option<f64>,
    pub top_p: Option<f64>,
    pub n: Option<u64>,
    pub stop: Option<Value>,
    pub max_completion_tokens: Option<u64>,
    pub max_tokens: Option<u64>,
    pub presence_penalty: Option<f64>,
    pub frequency_penalty: Option<f64>,
    pub logit_bias: Option<Value>,
    pub logprobs: Option<bool>,
    pub top_logprobs: Option<u64>,
    pub seed: Option<u64>,
    pub user: Option<String>,
    pub safety_identifier: Option<String>,
    pub prompt_cache_key: Option<String>,
    pub codex_session_id: Option<String>,
    pub reasoning_effort: Option<String>,
    pub prediction: Option<Value>,
    pub modalities: Option<Vec<String>>,
    pub audio: Option<Value>,
    pub stream: Option<bool>,
    pub stream_options: Option<Value>,
    pub store: Option<bool>,
    pub metadata: Option<HashMap<String, String>>,
    pub service_tier: Option<String>,
    pub verbosity: Option<String>,
    pub web_search_options: Option<Value>,
    pub tools: Option<Vec<Value>>,
    pub tool_choice: Option<Value>,
    pub parallel_tool_calls: Option<bool>,
    pub extra_body: Option<Value>,
    pub context_window: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteConfig {
    pub default_temperature: f64,
    pub providers: Vec<ProviderConfig>,
}

impl RouteConfig {
    pub fn validate(&self) -> Result<(), TuraError> {
        if !(0.0..=2.0).contains(&self.default_temperature) {
            return Err(TuraError::Validation {
                message: "default_temperature must be within [0.0, 2.0]".into(),
            });
        }
        for p in &self.providers {
            p.validate()?;
        }
        Ok(())
    }

    pub fn provider(&self, name: &str) -> Result<&ProviderConfig, TuraError> {
        self.providers
            .iter()
            .find(|p| p.provider == name)
            .ok_or_else(|| TuraError::Config {
                message: format!("This route has no provider named '{}'", name),
            })
    }

    pub async fn embed(&self, text: &str, conf: &TuraConfig) -> Result<Vec<f32>, TuraError> {
        self.validate()?;
        self.providers[0].embed(text, conf).await
    }

    pub async fn run(
        &self,
        conf: &TuraConfig,
        messages: Vec<Value>,
        options: CallOptions,
    ) -> Result<ProviderResponse, TuraError> {
        self.run_with_stream_events(conf, messages, options, None)
            .await
    }

    pub async fn run_with_stream_events(
        &self,
        conf: &TuraConfig,
        messages: Vec<Value>,
        options: CallOptions,
        stream_events: Option<ProviderStreamEventSink>,
    ) -> Result<ProviderResponse, TuraError> {
        self.validate()?;

        let mut failures = Vec::new();
        for provider in &self.providers {
            let mut effective = options.clone();
            if effective.temperature.is_none() {
                effective.temperature = Some(provider.temperature);
            }
            match provider
                .call_with_stream_events(conf, messages.clone(), effective, stream_events.clone())
                .await
            {
                Ok(result) => return Ok(result),
                Err(err) => {
                    warn!(provider = %provider.provider, model = %provider.model, error = %err, "route fallback to next provider");
                    failures.push(format!(
                        "{}:{} => {}",
                        provider.provider, provider.model, err
                    ));
                }
            }
        }

        Err(TuraError::AllProvidersFailed {
            message: failures.join(" | "),
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub provider_base_url: HashMap<String, String>,
    pub routes: HashMap<String, RouteConfig>,
    #[serde(default)]
    pub model_catalog: ModelCatalog,
    #[serde(default)]
    pub provider_enums: ProviderEnumCatalog,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RootConfig {
    pub provider_base_url: HashMap<String, String>,
    pub routes: HashMap<String, RawRouteConfig>,
    #[serde(default)]
    pub model_catalog: ModelCatalog,
    #[serde(default)]
    pub provider_enums: ProviderEnumCatalog,
    #[serde(default)]
    pub provider_auth: HashMap<String, ProviderAuthConfig>,
    #[serde(default)]
    pub provider_latency: ProviderLatencyConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ModelCatalog {
    #[serde(default)]
    pub tiers: Vec<String>,
    #[serde(default)]
    pub providers: HashMap<String, ProviderCatalogConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProviderEnumCatalog {
    #[serde(default)]
    pub domains: Vec<String>,
    #[serde(default)]
    pub capabilities: Vec<String>,
    #[serde(default)]
    pub api_styles: Vec<String>,
    #[serde(default)]
    pub auth_methods: Vec<String>,
    #[serde(default)]
    pub statuses: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProviderCatalogConfig {
    #[serde(default)]
    pub display_name: String,
    #[serde(default)]
    pub runtime_provider: String,
    #[serde(default)]
    pub api_style: String,
    #[serde(default)]
    pub base_url: String,
    #[serde(default)]
    pub token_env: Option<String>,
    #[serde(default)]
    pub env: Vec<String>,
    #[serde(default)]
    pub domains: Vec<String>,
    #[serde(default)]
    pub capabilities: Vec<String>,
    #[serde(default)]
    pub auth_methods: Vec<String>,
    #[serde(default)]
    pub api_docs: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub models: HashMap<String, Vec<CatalogModelConfig>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum CatalogModelConfig {
    Id(String),
    Detailed(CatalogModelDetail),
}

impl CatalogModelConfig {
    pub fn id(&self) -> &str {
        match self {
            Self::Id(id) => id,
            Self::Detailed(detail) => &detail.id,
        }
    }

    pub fn detail(&self) -> Option<&CatalogModelDetail> {
        match self {
            Self::Id(_) => None,
            Self::Detailed(detail) => Some(detail),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CatalogModelDetail {
    pub id: String,
    #[serde(default = "default_model_visible")]
    pub visible: bool,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub family: String,
    #[serde(default)]
    pub release_date: String,
    #[serde(default)]
    pub attachment: bool,
    #[serde(default)]
    pub reasoning: bool,
    #[serde(default)]
    pub temperature: bool,
    #[serde(default)]
    pub tool_call: bool,
    #[serde(default)]
    pub limit: CatalogModelLimit,
    #[serde(default)]
    pub modalities: CatalogModelModalities,
    #[serde(default)]
    pub options: serde_json::Map<String, serde_json::Value>,
    #[serde(default)]
    pub status: Option<String>,
}

fn default_model_visible() -> bool {
    true
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct CatalogModelLimit {
    pub context: u32,
    pub input: u32,
    pub output: u32,
}

impl Default for CatalogModelLimit {
    fn default() -> Self {
        Self {
            context: 200_000,
            input: 200_000,
            output: 16_384,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogModelModalities {
    pub input: Vec<String>,
    pub output: Vec<String>,
}

impl Default for CatalogModelModalities {
    fn default() -> Self {
        Self {
            input: vec!["text".to_string()],
            output: vec!["text".to_string()],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderLatencyConfig {
    #[serde(default = "default_latency_level")]
    pub active: String,
    #[serde(default = "default_latency_levels")]
    pub levels: HashMap<String, ProviderLatencyTimeouts>,
}

impl Default for ProviderLatencyConfig {
    fn default() -> Self {
        Self {
            active: default_latency_level(),
            levels: default_latency_levels(),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ProviderLatencyTimeouts {
    pub idle_output_timeout_ms: u64,
    pub first_output_timeout_ms: u64,
    pub total_timeout_ms: u64,
}

impl Default for ProviderLatencyTimeouts {
    fn default() -> Self {
        Self {
            idle_output_timeout_ms: 20_000,
            first_output_timeout_ms: 40_000,
            total_timeout_ms: 240_000,
        }
    }
}

fn default_latency_level() -> String {
    "fast".to_string()
}

fn default_latency_levels() -> HashMap<String, ProviderLatencyTimeouts> {
    HashMap::from([
        (
            "fast".to_string(),
            ProviderLatencyTimeouts {
                idle_output_timeout_ms: 20_000,
                first_output_timeout_ms: 40_000,
                total_timeout_ms: 240_000,
            },
        ),
        (
            "high".to_string(),
            ProviderLatencyTimeouts {
                idle_output_timeout_ms: 80_000,
                first_output_timeout_ms: 160_000,
                total_timeout_ms: 960_000,
            },
        ),
        (
            "x-high".to_string(),
            ProviderLatencyTimeouts {
                idle_output_timeout_ms: 100_000,
                first_output_timeout_ms: 180_000,
                total_timeout_ms: 1_200_000,
            },
        ),
    ])
}

/// Map a model-catalog tier (the "tier flag", i.e. the route / `tura_llm_name`)
/// to a provider-latency level. Level selection is driven entirely by the tier,
/// never by the thinking / reasoning_effort parameter.
///
/// - flagship_thinking -> x-high
/// - thinking          -> high
/// - fast / instant    -> fast (lowest)
/// - embedding_high    -> high
/// - embedding_low     -> fast
pub fn latency_level_for_tier(tier: &str) -> &'static str {
    match tier.trim().to_ascii_lowercase().as_str() {
        "flagship_thinking" => "x-high",
        "thinking" => "high",
        "embedding_high" => "high",
        "fast" | "instant" | "embedding_low" => "fast",
        _ => "fast",
    }
}

impl ProviderLatencyConfig {
    pub fn selected_timeouts(&self) -> ProviderLatencyTimeouts {
        self.timeouts_for_level(&self.active)
    }

    /// Resolve the timeouts for a specific latency level, falling back to the
    /// `fast` level and finally to defaults.
    pub fn timeouts_for_level(&self, level: &str) -> ProviderLatencyTimeouts {
        self.levels
            .get(level.trim())
            .copied()
            .or_else(|| self.levels.get("fast").copied())
            .unwrap_or_default()
    }

    /// Resolve timeouts for a model-catalog tier (the tier flag).
    pub fn timeouts_for_tier(&self, tier: &str) -> ProviderLatencyTimeouts {
        self.timeouts_for_level(latency_level_for_tier(tier))
    }
}

pub fn set_provider_latency_timeouts(timeouts: ProviderLatencyTimeouts) {
    let lock =
        PROVIDER_LATENCY_TIMEOUTS.get_or_init(|| RwLock::new(ProviderLatencyTimeouts::default()));
    if let Ok(mut guard) = lock.write() {
        *guard = timeouts;
    }
}

pub fn provider_latency_timeouts() -> ProviderLatencyTimeouts {
    let lock =
        PROVIDER_LATENCY_TIMEOUTS.get_or_init(|| RwLock::new(ProviderLatencyTimeouts::default()));
    lock.read().map(|guard| *guard).unwrap_or_default()
}

/// Store the active provider-latency config (level table + active level) so it
/// can later be resolved per-tier at runtime-construction time.
pub fn set_provider_latency_config(config: ProviderLatencyConfig) {
    let lock =
        PROVIDER_LATENCY_CONFIG.get_or_init(|| RwLock::new(ProviderLatencyConfig::default()));
    if let Ok(mut guard) = lock.write() {
        *guard = config;
    }
}

pub fn provider_latency_config() -> ProviderLatencyConfig {
    let lock =
        PROVIDER_LATENCY_CONFIG.get_or_init(|| RwLock::new(ProviderLatencyConfig::default()));
    lock.read().map(|guard| guard.clone()).unwrap_or_default()
}

/// Resolve and install the global latency timeouts for a model-catalog tier
/// (the tier flag). Level selection is driven entirely by the tier, never by
/// the thinking / reasoning_effort parameter. Returns the applied timeouts.
pub fn apply_latency_for_tier(tier: &str) -> ProviderLatencyTimeouts {
    let timeouts = provider_latency_config().timeouts_for_tier(tier);
    set_provider_latency_timeouts(timeouts);
    timeouts
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProviderAuthConfig {
    #[serde(rename = "type")]
    pub auth_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub login: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_env: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub login_env: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refresh_env: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_env: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account_env: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RawRouteConfig {
    #[serde(default = "default_temperature")]
    pub default_temperature: f64,
    #[serde(default)]
    pub providers: Vec<RawProviderConfig>,
}

fn default_temperature() -> f64 {
    0.2
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawProviderConfig {
    pub provider: String,
    pub model: String,
    pub temperature: Option<f64>,
}

impl Settings {
    pub async fn default() -> Result<Arc<Self>, TuraError> {
        let explicit_config = std::env::var_os("TURA_PROVIDER_CONFIG").is_some()
            || std::env::var_os("TURALLM_CONFIG").is_some();
        if !explicit_config {
            if let Some(settings) = SETTINGS.get() {
                return Ok(settings.clone());
            }
        }
        let loaded = Arc::new(crate::tura_llm_conf::load_settings().await?);
        if !explicit_config {
            let _ = SETTINGS.set(loaded.clone());
        }
        Ok(loaded)
    }

    pub fn normalize_model_name(provider: &str, model: &str) -> String {
        let model = model.trim();
        let prefix = format!("{provider}/");
        if model.starts_with(&prefix) {
            return model[prefix.len()..].to_string();
        }
        if provider == "openai" && model.starts_with("openai/") {
            return model["openai/".len()..].to_string();
        }
        if provider == "codex" && model.starts_with("codex/") {
            return model["codex/".len()..].to_string();
        }
        model.to_string()
    }

    pub fn route_by_name(&self, name: &str) -> Option<&RouteConfig> {
        self.routes.get(name)
    }

    pub fn routes(&self) -> impl Iterator<Item = &RouteConfig> {
        self.routes.values()
    }

    pub fn provider_base_url(&self, provider: &str) -> Option<String> {
        self.provider_base_url.get(provider).cloned().or_else(|| {
            self.routes()
                .flat_map(|route| route.providers.iter())
                .find(|item| item.provider == provider)
                .map(|item| item.base_url.clone())
        })
    }

    pub fn configured_model_catalog(&self) -> HashMap<String, Vec<String>> {
        let mut catalog = HashMap::<String, Vec<String>>::new();
        for (provider, config) in &self.model_catalog.providers {
            let models = catalog.entry(provider.clone()).or_default();
            for model in config.models.values().flatten() {
                let normalized = Self::normalize_model_name(provider, model.id());
                if !models.iter().any(|existing| existing == &normalized) {
                    models.push(normalized);
                }
            }
        }
        for route in self.routes() {
            for provider in &route.providers {
                let model = Self::normalize_model_name(&provider.provider, &provider.model);
                let models = catalog.entry(provider.provider.clone()).or_default();
                if !models.iter().any(|existing| existing == &model) {
                    models.push(model);
                }
            }
        }
        for models in catalog.values_mut() {
            models.sort();
        }
        catalog
    }

    pub fn make_provider(
        provider_base_url: &HashMap<String, String>,
        provider: &str,
        model: &str,
        temperature: Option<f64>,
        route_default_temp: f64,
    ) -> Result<ProviderConfig, TuraError> {
        let base_url =
            provider_base_url
                .get(provider)
                .cloned()
                .ok_or_else(|| TuraError::UnknownProvider {
                    provider: provider.to_string(),
                })?;

        let config = ProviderConfig {
            provider: provider.to_string(),
            base_url,
            model: Self::normalize_model_name(provider, model),
            temperature: temperature.unwrap_or(route_default_temp),
        };
        config.validate()?;
        Ok(config)
    }

    pub fn make_route(
        provider_base_url: &HashMap<String, String>,
        items: &[RawProviderConfig],
        default_temperature: f64,
    ) -> Result<RouteConfig, TuraError> {
        let mut providers = Vec::with_capacity(items.len());
        for item in items {
            providers.push(Self::make_provider(
                provider_base_url,
                &item.provider,
                &item.model,
                item.temperature,
                default_temperature,
            )?);
        }
        let route = RouteConfig {
            default_temperature,
            providers,
        };
        route.validate()?;
        Ok(route)
    }
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
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

#[cfg(test)]
mod tests {
    use super::{
        apply_codex_auth_env, load_codex_auth_tokens, normalize_response_content,
        openai_login_is_oauth, provider_latency_timeouts, set_provider_latency_timeouts,
        ProviderLatencyConfig, ProviderLatencyTimeouts,
    };
    use serde_json::json;
    use std::path::PathBuf;
    use std::sync::{Mutex, OnceLock};
    use uuid::Uuid;

    fn env_lock() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
            .lock()
            .expect("env lock poisoned")
    }

    struct EnvRestore {
        keys: Vec<(&'static str, Option<String>)>,
    }

    impl EnvRestore {
        fn capture(keys: &[&'static str]) -> Self {
            Self {
                keys: keys
                    .iter()
                    .map(|key| (*key, std::env::var(key).ok()))
                    .collect(),
            }
        }
    }

    impl Drop for EnvRestore {
        fn drop(&mut self) {
            for (key, value) in &self.keys {
                if let Some(value) = value {
                    std::env::set_var(key, value);
                } else {
                    std::env::remove_var(key);
                }
            }
        }
    }

    fn unique_temp_dir(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!("tura-provider-{name}-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&path).expect("temp dir");
        path
    }

    #[test]
    fn normalizes_openai_style_tool_calls() {
        let raw = json!({
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": null,
                    "tool_calls": [{
                        "id": "call_1",
                        "type": "function",
                        "function": {
                            "name": "glob",
                            "arguments": "{\"requests\":[{\"directory\":\".\"}]}"
                        }
                    }]
                }
            }]
        });

        let content = normalize_response_content(&raw);

        assert_eq!(content["tool_calls"][0]["function"]["name"], "glob");
    }

    #[test]
    fn keeps_codex_responses_style_output_unchanged_for_codex_normalizer() {
        let raw = json!({
            "output": [{
                "type": "function_call",
                "name": "read_line",
                "arguments": "{\"requests\":[]}"
            }]
        });

        let content = normalize_response_content(&raw);

        assert_eq!(content[0]["type"], "function_call");
    }

    #[test]
    fn provider_latency_defaults_match_fast_profile() {
        let selected = ProviderLatencyConfig::default().selected_timeouts();

        assert_eq!(selected.idle_output_timeout_ms, 20_000);
        assert_eq!(selected.first_output_timeout_ms, 40_000);
        assert_eq!(selected.total_timeout_ms, 240_000);
    }

    #[test]
    fn provider_latency_level_tracks_tier_flag() {
        assert_eq!(super::latency_level_for_tier("flagship_thinking"), "x-high");
        assert_eq!(super::latency_level_for_tier("thinking"), "high");
        assert_eq!(super::latency_level_for_tier("fast"), "fast");
        assert_eq!(super::latency_level_for_tier("instant"), "fast");
        assert_eq!(super::latency_level_for_tier("embedding_high"), "high");
        assert_eq!(super::latency_level_for_tier("embedding_low"), "fast");
        // Unknown tiers fall back to the lowest level.
        assert_eq!(super::latency_level_for_tier("something_else"), "fast");
    }

    #[test]
    fn provider_latency_timeouts_for_tier_resolve_levels() {
        let config = ProviderLatencyConfig::default();

        let flagship = config.timeouts_for_tier("flagship_thinking");
        assert_eq!(flagship.total_timeout_ms, 1_200_000);

        let thinking = config.timeouts_for_tier("thinking");
        assert_eq!(thinking.total_timeout_ms, 960_000);

        let fast = config.timeouts_for_tier("instant");
        assert_eq!(fast.total_timeout_ms, 240_000);
    }

    #[test]
    fn provider_latency_global_timeout_state_is_configurable() {
        set_provider_latency_timeouts(ProviderLatencyTimeouts {
            idle_output_timeout_ms: 50_000,
            first_output_timeout_ms: 90_000,
            total_timeout_ms: 600_000,
        });

        let selected = provider_latency_timeouts();
        assert_eq!(selected.idle_output_timeout_ms, 50_000);
        assert_eq!(selected.first_output_timeout_ms, 90_000);
        assert_eq!(selected.total_timeout_ms, 600_000);
    }

    #[test]
    fn loads_codex_oauth_tokens_from_codex_home() {
        let _lock = env_lock();
        let _env = EnvRestore::capture(&[
            "CODEX_HOME",
            "OPENAI_LOGIN",
            "OPENAI_API_KEY",
            "OPENAI_REFRESH_TOKEN",
            "OPENAI_ACCOUNT_ID",
            "TURA_PROVIDER_CONFIG",
            "TURALLM_CONFIG",
        ]);
        std::env::remove_var("OPENAI_LOGIN");
        std::env::remove_var("OPENAI_API_KEY");
        std::env::remove_var("OPENAI_REFRESH_TOKEN");
        std::env::remove_var("OPENAI_ACCOUNT_ID");
        std::env::remove_var("TURA_PROVIDER_CONFIG");
        std::env::remove_var("TURALLM_CONFIG");

        let codex_home = unique_temp_dir("codex-home");
        std::fs::write(
            codex_home.join("auth.json"),
            r#"{
                "auth_mode": "chatgpt",
                "OPENAI_API_KEY": null,
                "tokens": {
                    "access_token": "local-access-token",
                    "refresh_token": "local-refresh-token",
                    "account_id": "acct-local"
                }
            }"#,
        )
        .expect("auth json");
        std::env::set_var("CODEX_HOME", &codex_home);

        let tokens = load_codex_auth_tokens().expect("codex auth tokens");
        apply_codex_auth_env(&tokens);

        assert_eq!(tokens.access_token, "local-access-token");
        assert_eq!(tokens.refresh_token, "local-refresh-token");
        assert_eq!(tokens.account_id.as_deref(), Some("acct-local"));
        assert_eq!(std::env::var("OPENAI_LOGIN").as_deref(), Ok("oauth"));
        assert_eq!(
            std::env::var("OPENAI_API_KEY").as_deref(),
            Ok("local-access-token")
        );
        assert_eq!(
            std::env::var("OPENAI_REFRESH_TOKEN").as_deref(),
            Ok("local-refresh-token")
        );
        assert_eq!(
            std::env::var("OPENAI_ACCOUNT_ID").as_deref(),
            Ok("acct-local")
        );
    }

    #[test]
    fn openai_oauth_login_uses_provider_auth_config() {
        let _lock = env_lock();
        let _env = EnvRestore::capture(&[
            "CODEX_HOME",
            "OPENAI_LOGIN",
            "OPENAI_API_KEY",
            "TURA_PROVIDER_CONFIG",
            "TURALLM_CONFIG",
        ]);
        std::env::remove_var("CODEX_HOME");
        std::env::remove_var("OPENAI_LOGIN");
        std::env::remove_var("OPENAI_API_KEY");
        std::env::remove_var("TURA_PROVIDER_CONFIG");

        let dir = unique_temp_dir("provider-config");
        let config = dir.join("provider_config.json");
        std::fs::write(&config, r#"{"provider_auth":{"openai":{"login":"oauth"}}}"#)
            .expect("provider config");
        std::env::set_var("TURALLM_CONFIG", &config);

        assert!(openai_login_is_oauth(
            &crate::tura_conf::TuraConfig::default()
        ));
    }

    #[test]
    fn auth_expired_error_detects_401_and_403_only() {
        let unauthorized: Result<(), super::TuraError> = Err(super::TuraError::HttpStatus {
            status: 401,
            body: "expired".to_string(),
        });
        let forbidden: Result<(), super::TuraError> = Err(super::TuraError::HttpStatus {
            status: 403,
            body: "forbidden".to_string(),
        });
        let rate_limited: Result<(), super::TuraError> = Err(super::TuraError::HttpStatus {
            status: 429,
            body: "slow down".to_string(),
        });
        let network: Result<(), super::TuraError> = Err(super::TuraError::Network {
            message: "boom".to_string(),
        });
        let ok: Result<(), super::TuraError> = Ok(());

        assert!(super::is_auth_expired_error(&unauthorized));
        assert!(super::is_auth_expired_error(&forbidden));
        assert!(!super::is_auth_expired_error(&rate_limited));
        assert!(!super::is_auth_expired_error(&network));
        assert!(!super::is_auth_expired_error(&ok));
    }

    #[test]
    fn persist_env_var_upserts_and_appends_preserving_other_keys() {
        let dir = unique_temp_dir("persist-env");
        let path = dir.join(".env");
        std::fs::write(&path, "ALPHA=1\nTOKEN=old\nBETA=2\n").expect("seed env");

        // Update existing key in place.
        super::persist_env_var(&path, "TOKEN", "new");
        let after = std::fs::read_to_string(&path).expect("read env");
        assert!(after.contains("TOKEN=new"));
        assert!(!after.contains("TOKEN=old"));
        assert!(after.contains("ALPHA=1"));
        assert!(after.contains("BETA=2"));

        // Append a brand-new key.
        super::persist_env_var(&path, "EXPIRES", "12345");
        let after = std::fs::read_to_string(&path).expect("read env");
        assert!(after.contains("EXPIRES=12345"));
        assert!(after.contains("TOKEN=new"));
    }
}
