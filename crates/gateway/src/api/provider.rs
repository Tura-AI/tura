//! Provider / Auth API handlers

use crate::contracts::*;
use crate::mock::global_store;
use axum::extract::{Json, Path, Query};
use chrono::Utc;
use fs2::FileExt;
use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io;
use std::path::{Path as FsPath, PathBuf};
use std::sync::{Mutex, MutexGuard};
use tura_llm_rust::{AuthMethodKind, OAuthAuthorizeKind};

mod auth_refresh;
mod auth_validation;
mod oauth_flow;
use auth_validation::{validate_provider_auth_config, validation_detail};
pub use oauth_flow::{
    oauth_authorize, oauth_callback, oauth_callback_info, oauth_redirect_callback,
};
mod catalog;
pub use catalog::{list_providers, validate_model};
use catalog::{provider_display_name, provider_runtime_id};
mod auth_registry;
mod auth_scheduler;
mod auth_update;
mod auth_validator;
pub(crate) mod config;
mod metadata;
mod oauth_support;

use config::{config_value, upsert_env_value};
use metadata::{
    legacy_auth_method_type, oauth_authorize_endpoint, oauth_token_endpoint, provider_api_key_url,
    provider_auth_docs_url,
};
use oauth_support::{browser_login_url, github_copilot_oauth_client_id, google_oauth_client_id};

static PROVIDER_AUTH_WRITE_LOCK: Mutex<()> = Mutex::new(());

// ============================================================================
// Provider List
// ============================================================================

// ============================================================================

pub async fn set_auth(
    Path(provider_id): Path<String>,
    Json(payload): Json<ProviderAuth>,
) -> Json<bool> {
    if payload
        .access
        .as_deref()
        .or(payload.key.as_deref())
        .map(str::trim)
        .unwrap_or_default()
        .is_empty()
    {
        return Json(false);
    }
    let saved = persist_provider_auth(&provider_id, &payload).is_ok();
    let _ = global_store().set_auth(&provider_id, payload);
    Json(saved)
}

pub async fn remove_auth(Path(provider_id): Path<String>) -> Json<bool> {
    Json(logout_provider_auth(&provider_id).is_ok() && global_store().remove_auth(&provider_id))
}

pub(super) async fn refresh_provider_auth_if_needed(
    provider_id: &str,
    force: bool,
) -> Result<bool, String> {
    auth_refresh::refresh_provider_auth_if_needed(provider_id, force).await
}

pub(crate) fn start_provider_auth_scheduler() {
    auth_scheduler::start_provider_auth_scheduler();
}

pub async fn provider_auth_status(
    Path(provider_id): Path<String>,
) -> Json<ProviderAuthStatusResponse> {
    let _ = refresh_provider_auth_if_needed(&provider_id, false).await;
    Json(build_provider_auth_status(&provider_id))
}

pub async fn provider_auth_validate(
    Path(provider_id): Path<String>,
    Json(payload): Json<ProviderAuthValidationRequest>,
) -> Json<ProviderAuthActionResponse> {
    let status = build_provider_auth_status(&provider_id);
    let receipt = validate_provider_auth_config(&provider_id, &status, Some(&payload)).await;
    Json(ProviderAuthActionResponse {
        ok: receipt.ok,
        provider_id,
        code: receipt.code,
        message: receipt.message,
        level: Some(receipt.level),
        details: receipt.details,
        status: Some(status),
    })
}

pub async fn provider_auth_refresh(
    Path(provider_id): Path<String>,
) -> Json<ProviderAuthActionResponse> {
    let can_refresh = auth_validator::provider_auth_can_refresh(&provider_id);
    let refresh_result = refresh_provider_auth_if_needed(&provider_id, true).await;
    let status = build_provider_auth_status(&provider_id);
    let ok = can_refresh && status.authenticated && refresh_result.is_ok();
    let (code, message, details) = if !can_refresh {
        (
            "provider.auth.refresh.unsupported",
            "provider auth method does not support refresh".to_string(),
            vec![validation_detail("provider.auth.refresh.unsupported", None)],
        )
    } else if let Err(error) = refresh_result {
        (
            "provider.auth.refresh.failed",
            format!("provider auth refresh failed: {error}"),
            vec![validation_detail(
                "provider.auth.refresh.failed",
                Some(error),
            )],
        )
    } else if ok {
        (
            "provider.auth.refresh.succeeded",
            "provider auth refreshed".to_string(),
            vec![validation_detail("provider.auth.refresh.succeeded", None)],
        )
    } else {
        (
            "provider.auth.not_configured",
            "provider auth is not configured".to_string(),
            vec![validation_detail("provider.auth.not_configured", None)],
        )
    };
    Json(ProviderAuthActionResponse {
        ok,
        provider_id,
        code: code.to_string(),
        message,
        level: Some(if ok { "valid" } else { "invalid" }.to_string()),
        details,
        status: Some(status),
    })
}

pub async fn provider_auth_logout(
    Path(provider_id): Path<String>,
) -> Json<ProviderAuthActionResponse> {
    let result = logout_provider_auth(&provider_id);
    let status = build_provider_auth_status(&provider_id);
    let ok = result.is_ok();
    let (code, message, details) = result
        .map(|_| {
            (
                "provider.auth.logout.succeeded",
                "provider auth logged out".to_string(),
                vec![validation_detail("provider.auth.logout.succeeded", None)],
            )
        })
        .unwrap_or_else(|error| {
            (
                "provider.auth.logout.failed",
                format!("provider auth logout failed: {error}"),
                vec![validation_detail(
                    "provider.auth.logout.failed",
                    Some(error.to_string()),
                )],
            )
        });
    Json(ProviderAuthActionResponse {
        ok,
        provider_id,
        code: code.to_string(),
        message,
        level: Some(if ok { "valid" } else { "invalid" }.to_string()),
        details,
        status: Some(status),
    })
}

// ============================================================================
// Provider Auth
// ============================================================================

pub async fn provider_auth(
    Query(_params): Query<ProviderAuthQuery>,
) -> Json<HashMap<String, Vec<ProviderAuthMethod>>> {
    let mut response = HashMap::new();
    for entry in auth_registry::entries() {
        let methods = provider_auth_methods(entry.provider_id);
        if !methods.is_empty() {
            response.insert(entry.provider_id.to_string(), methods);
        }
    }
    if let Ok(settings) = tura_llm_rust::Settings::default().await {
        for (provider_id, provider) in &settings.model_catalog.providers {
            if response.contains_key(provider_id) {
                continue;
            }
            let methods = provider_auth_methods_from_config(provider_id, provider);
            if !methods.is_empty() {
                response.insert(provider_id.to_string(), methods);
            }
        }
    }
    Json(response)
}

fn provider_auth_methods(provider_id: &str) -> Vec<ProviderAuthMethod> {
    let Some(entry) = auth_registry::entry(provider_id) else {
        return Vec::new();
    };
    entry
        .auth_methods
        .iter()
        .map(|method| {
            let unavailable_reason = auth_method_unavailable_reason(
                provider_id,
                method.kind,
                entry.oauth_authorize_kind,
            );
            ProviderAuthMethod {
                method_type: legacy_auth_method_type(method.kind).to_string(),
                kind: method.kind,
                login: method.login.to_string(),
                label: method.label.to_string(),
                prompts: None,
                token_env: entry.token_env.map(ToString::to_string),
                login_env: entry.login_env.map(ToString::to_string),
                authorize_url: oauth_authorize_endpoint(entry.oauth_authorize_kind),
                token_url: oauth_token_endpoint(entry.oauth_authorize_kind),
                api_key_url: provider_api_key_url(provider_id),
                docs_url: provider_auth_docs_url(provider_id),
                configured_value: entry
                    .token_env
                    .and_then(|token_env| provider_auth_method_value(provider_id, token_env)),
                available: unavailable_reason.is_none(),
                unavailable_reason,
                supports_refresh: entry.capabilities.supports_oauth_refresh,
            }
        })
        .collect()
}

fn provider_auth_methods_from_config(
    provider_id: &str,
    provider: &tura_llm_rust::ProviderCatalogConfig,
) -> Vec<ProviderAuthMethod> {
    let envs = provider_auth_envs_from_config(provider);
    envs.into_iter()
        .map(|env| ProviderAuthMethod {
            method_type: "api".to_string(),
            kind: AuthMethodKind::ApiKey,
            login: "api".to_string(),
            label: env.clone(),
            prompts: None,
            token_env: Some(env.clone()),
            login_env: None,
            authorize_url: None,
            token_url: None,
            api_key_url: provider_api_key_url(provider_id),
            docs_url: provider_auth_docs_url(provider_id).or_else(|| provider.api_docs.clone()),
            configured_value: provider_auth_method_value(provider_id, &env),
            available: true,
            unavailable_reason: None,
            supports_refresh: false,
        })
        .collect()
}

fn provider_auth_envs_from_config(provider: &tura_llm_rust::ProviderCatalogConfig) -> Vec<String> {
    let mut envs = Vec::new();
    if let Some(token_env) = provider
        .token_env
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        envs.push(token_env.to_string());
    }
    for env in &provider.env {
        if !env.trim().is_empty() && !envs.iter().any(|item| item == env) {
            envs.push(env.clone());
        }
    }
    envs
}

fn configured_provider_value(provider_id: &str, env: &str) -> Option<String> {
    if auth_registry::entry(provider_id).is_some() {
        provider_key_value_for_env(provider_id, env)
    } else {
        config_value(env).map(|value| value.trim().to_string())
    }
}

fn provider_auth_method_value(provider_id: &str, env: &str) -> Option<String> {
    configured_provider_value(provider_id, env)
        .or_else(|| config_value(env).map(|value| value.trim().to_string()))
        .filter(|value| !value.is_empty())
}

fn auth_method_unavailable_reason(
    provider_id: &str,
    kind: AuthMethodKind,
    authorize_kind: Option<OAuthAuthorizeKind>,
) -> Option<String> {
    if !matches!(kind, AuthMethodKind::OAuthPkce | AuthMethodKind::DeviceCode) {
        return None;
    }
    match authorize_kind.unwrap_or(OAuthAuthorizeKind::Unsupported) {
        OAuthAuthorizeKind::OpenAiPkce => None,
        OAuthAuthorizeKind::AnthropicPkce => None,
        OAuthAuthorizeKind::GooglePkce => {
            google_oauth_client_id(provider_id).is_none().then(|| {
                "GOOGLE_OAUTH_CLIENT_ID is required before Google OAuth can start".to_string()
            })
        }
        OAuthAuthorizeKind::GithubDevice => github_copilot_oauth_client_id().is_none().then(|| {
            "GITHUB_COPILOT_CLIENT_ID is required before GitHub Copilot OAuth can start".to_string()
        }),
        OAuthAuthorizeKind::BrowserTokenPaste | OAuthAuthorizeKind::Unsupported => Some(format!(
            "{} OAuth is listed but is not implemented yet",
            provider_display_name(provider_id)
        )),
    }
}

fn persist_provider_auth(provider_id: &str, auth: &ProviderAuth) -> io::Result<()> {
    let _write_guard = provider_auth_write_guard();
    let key = auth.access.as_deref().or(auth.key.as_deref());
    let Some(key) = key.filter(|value| !value.trim().is_empty()) else {
        return Ok(());
    };

    let env_path = std::env::var("TURA_ENV_PATH")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            tura_llm_rust::TuraConfig::default()
                .env_path()
                .to_path_buf()
        });
    let _file_guard = provider_auth_file_lock()?;
    let api_key = auth
        .metadata
        .as_ref()
        .and_then(|metadata| metadata.get("token_env"))
        .and_then(|value| value.as_str())
        .filter(|value| !value.trim().is_empty())
        .map(|value| value.trim().to_string())
        .unwrap_or_else(|| provider_env_key(provider_id));
    upsert_env_value(&env_path, &api_key, key)?;
    std::env::set_var(&api_key, key);

    let requested_login = auth
        .metadata
        .as_ref()
        .and_then(|metadata| metadata.get("login"))
        .and_then(|value| value.as_str())
        .map(ToString::to_string)
        .unwrap_or_else(|| login_value_for_auth(provider_id, auth));

    if auth.auth_type == "api" || requested_login == "api" {
        let login_key = provider_login_key(provider_id);
        upsert_env_value(&env_path, &login_key, "api")?;
        std::env::set_var(&login_key, "api");

        upsert_provider_auth_config(provider_id, "api", Some(&login_key), auth, None)?;
    }

    if auth.auth_type == "oauth" || matches!(requested_login.as_str(), "oauth" | "browser") {
        let login = requested_login.as_str();
        let login_key = provider_login_key(provider_id);
        upsert_env_value(&env_path, &login_key, login)?;
        std::env::set_var(&login_key, login);

        if let Some(registry_entry) = tura_llm_rust::provider_auth_registry_entry(provider_id) {
            if let (Some(refresh_key), Some(refresh)) = (
                registry_entry.refresh_env,
                auth.refresh
                    .as_deref()
                    .filter(|value| !value.trim().is_empty()),
            ) {
                upsert_env_value(&env_path, refresh_key, refresh)?;
                std::env::set_var(refresh_key, refresh);
            }
            if let (Some(expires_key), Some(expires)) = (registry_entry.expires_env, auth.expires) {
                let expires_value = expires.to_string();
                upsert_env_value(&env_path, expires_key, &expires_value)?;
                std::env::set_var(expires_key, expires_value);
            }
            if let (Some(account_key), Some(account_id)) = (
                registry_entry.account_env,
                auth.account_id
                    .as_deref()
                    .filter(|value| !value.trim().is_empty()),
            ) {
                upsert_env_value(&env_path, account_key, account_id)?;
                std::env::set_var(account_key, account_id);
            }
        }

        upsert_provider_auth_config(
            provider_id,
            login,
            Some(&login_key),
            auth,
            auth.metadata
                .as_ref()
                .and_then(|metadata| metadata.get("url"))
                .and_then(|value| value.as_str())
                .map(ToString::to_string)
                .or_else(|| Some(browser_login_url(provider_id))),
        )?;
    }

    Ok(())
}

fn provider_auth_write_guard() -> MutexGuard<'static, ()> {
    match PROVIDER_AUTH_WRITE_LOCK.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

struct ProviderAuthFileLock {
    file: File,
}

impl Drop for ProviderAuthFileLock {
    fn drop(&mut self) {
        let _ = self.file.unlock();
    }
}

fn provider_auth_file_lock() -> io::Result<ProviderAuthFileLock> {
    let config_path = config::provider_config_path();
    let lock_path = provider_auth_lock_path(&config_path);
    if let Some(parent) = lock_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(&lock_path)?;
    file.lock_exclusive()?;
    Ok(ProviderAuthFileLock { file })
}

fn provider_auth_lock_path(config_path: &FsPath) -> PathBuf {
    let mut lock_path = config_path.to_path_buf();
    lock_path.set_extension("auth.lock");
    lock_path
}

fn login_value_for_auth(provider_id: &str, auth: &ProviderAuth) -> String {
    if let Some(login) = auth
        .metadata
        .as_ref()
        .and_then(|metadata| metadata.get("login"))
        .and_then(|value| value.as_str())
        .filter(|value| !value.trim().is_empty())
    {
        return login.to_string();
    }
    if auth.auth_type == "api" {
        return "api".to_string();
    }
    tura_llm_rust::provider_default_auth_method(provider_id)
        .map(|method| method.login.to_string())
        .unwrap_or_else(|| {
            if auth.auth_type == "oauth" {
                "oauth".to_string()
            } else {
                auth.auth_type.clone()
            }
        })
}

fn build_provider_auth_status(provider_id: &str) -> ProviderAuthStatusResponse {
    let entry = tura_llm_rust::provider_auth_registry_entry(provider_id);
    let config_entry = read_provider_auth_config(provider_id);
    let login = config_entry
        .as_ref()
        .and_then(|entry| entry.login.clone())
        .or_else(|| {
            entry
                .and_then(|entry| entry.login_env)
                .and_then(|key| std::env::var(key).ok())
        })
        .or_else(|| {
            entry
                .and_then(|entry| entry.login_env)
                .and_then(|key| tura_llm_rust::TuraConfig::default().get(key))
        });
    let token_env = config_entry
        .as_ref()
        .and_then(|entry| entry.token_env.clone())
        .or_else(|| {
            entry
                .and_then(|entry| entry.token_env)
                .map(ToString::to_string)
        });
    let refresh_env = config_entry
        .as_ref()
        .and_then(|entry| entry.refresh_env.clone())
        .or_else(|| {
            entry
                .and_then(|entry| entry.refresh_env)
                .map(ToString::to_string)
        });
    let expires_env = config_entry
        .as_ref()
        .and_then(|entry| entry.expires_env.clone())
        .or_else(|| {
            entry
                .and_then(|entry| entry.expires_env)
                .map(ToString::to_string)
        });
    let account_env = config_entry
        .as_ref()
        .and_then(|entry| entry.account_env.clone())
        .or_else(|| {
            entry
                .and_then(|entry| entry.account_env)
                .map(ToString::to_string)
        });

    let local_codex_auth = openai_codex_auth(provider_id);
    let configured = token_env.as_deref().and_then(config_value).is_some()
        || provider_key_exists(provider_id)
        || local_codex_auth.is_some()
        || bedrock_credentials_exist(provider_id);
    let expires_at = expires_env
        .as_deref()
        .and_then(config_value)
        .and_then(|value| value.parse::<i64>().ok());
    let expired = expires_at.map(|expires_at| expires_at <= Utc::now().timestamp_millis());
    let authenticated = (configured && expired != Some(true))
        || local_codex_auth
            .as_ref()
            .is_some_and(|auth| !auth.access_token.trim().is_empty());
    let auth_state = if authenticated {
        if matches!(login.as_deref(), Some("api")) {
            tura_llm_rust::AuthState::ApiKeyConfigured
        } else {
            tura_llm_rust::AuthState::Authenticated
        }
    } else if expired == Some(true) {
        tura_llm_rust::AuthState::Expired
    } else {
        tura_llm_rust::AuthState::NotConfigured
    };
    let runtime_state = if entry.and_then(|entry| entry.disabled_reason).is_some() {
        tura_llm_rust::ProviderRuntimeState::Disabled
    } else if authenticated {
        tura_llm_rust::ProviderRuntimeState::Ready
    } else if configured {
        tura_llm_rust::ProviderRuntimeState::Configured
    } else {
        tura_llm_rust::ProviderRuntimeState::MissingAuth
    };

    ProviderAuthStatusResponse {
        provider_id: provider_id.to_string(),
        display_name: provider_display_name(provider_id),
        login,
        configured,
        authenticated,
        expired,
        account_id: config_entry
            .as_ref()
            .and_then(|entry| entry.account_id.clone())
            .or_else(|| account_env.as_deref().and_then(config_value))
            .or_else(|| local_codex_auth.and_then(|auth| auth.account_id)),
        token_env,
        login_env: config_entry
            .as_ref()
            .and_then(|entry| entry.login_env.clone())
            .or_else(|| {
                entry
                    .and_then(|entry| entry.login_env)
                    .map(ToString::to_string)
            }),
        refresh_env,
        expires_env,
        updated_at: config_entry.and_then(|entry| entry.updated_at),
        auth_state,
        runtime_state,
        last_error_category: entry
            .and_then(|entry| entry.disabled_reason)
            .map(ToString::to_string),
    }
}

#[derive(Debug, Clone)]
struct OpenAiCodexAuth {
    access_token: String,
    account_id: Option<String>,
}

fn openai_codex_auth(provider_id: &str) -> Option<OpenAiCodexAuth> {
    if provider_id != "openai" {
        return None;
    }
    let home = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME"))?;
    let path = PathBuf::from(home).join(".codex").join("auth.json");
    let value: serde_json::Value = serde_json::from_str(&fs::read_to_string(path).ok()?).ok()?;
    let tokens = value.get("tokens")?;
    let access_token = tokens
        .get("access_token")
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.trim().is_empty())?
        .to_string();
    let account_id = tokens
        .get("account_id")
        .or_else(|| value.get("account_id"))
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(ToString::to_string);
    Some(OpenAiCodexAuth {
        access_token,
        account_id,
    })
}

fn read_provider_auth_config(provider_id: &str) -> Option<tura_llm_rust::ProviderAuthConfig> {
    let path = config::provider_config_path();
    let content = fs::read_to_string(path).ok()?;
    let root: tura_llm_rust::RootConfig = serde_json::from_str(&content).ok()?;
    root.provider_auth.get(provider_id).cloned()
}

fn bedrock_credentials_exist(provider_id: &str) -> bool {
    provider_id == "bedrock"
        && [
            "AWS_ACCESS_KEY_ID",
            "AWS_PROFILE",
            "AWS_REGION",
            "AWS_DEFAULT_REGION",
        ]
        .iter()
        .any(|key| config_value(key).is_some())
}

fn logout_provider_auth(provider_id: &str) -> io::Result<()> {
    let _write_guard = provider_auth_write_guard();
    let env_path = std::env::var("TURA_ENV_PATH")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            tura_llm_rust::TuraConfig::default()
                .env_path()
                .to_path_buf()
        });
    let _file_guard = provider_auth_file_lock()?;
    let status = build_provider_auth_status(provider_id);

    let current_login = status.login.as_deref();
    let should_clear_shared_token = match provider_id {
        "openai" => current_login == Some("oauth"),
        "openai-api" => current_login != Some("oauth"),
        "claude-code" => current_login == Some("oauth"),
        "anthropic" => !matches!(current_login, Some("oauth" | "browser")),
        "antigravity" => current_login == Some("oauth"),
        "github-copilot" => matches!(current_login, Some("device" | "oauth" | "api")),
        "anthropic-api" | "antigravity-api" => current_login != Some("browser"),
        _ => true,
    };

    if should_clear_shared_token {
        if let Some(token_env) = status.token_env.as_deref() {
            upsert_env_value(&env_path, token_env, "")?;
            std::env::remove_var(token_env);
        }
    }
    if let Some(login_env) = status.login_env.as_deref() {
        upsert_env_value(&env_path, login_env, "")?;
        std::env::remove_var(login_env);
    }
    for key in [
        status.refresh_env.as_deref(),
        status.expires_env.as_deref(),
        tura_llm_rust::provider_auth_registry_entry(provider_id)
            .and_then(|entry| entry.account_env),
    ]
    .into_iter()
    .flatten()
    {
        upsert_env_value(&env_path, key, "")?;
        std::env::remove_var(key);
    }

    update_provider_auth_config_status(provider_id, "revoked")
}

fn update_provider_auth_config_status(provider_id: &str, status: &str) -> io::Result<()> {
    let path = config::provider_config_path();
    let content = fs::read_to_string(&path)?;
    let mut root: serde_json::Value = serde_json::from_str(&content)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
    if let Some(entry) = root
        .get_mut("provider_auth")
        .and_then(|value| value.as_object_mut())
        .and_then(|provider_auth| provider_auth.get_mut(provider_id))
        .and_then(|entry| entry.as_object_mut())
    {
        entry.insert(
            "status".to_string(),
            serde_json::Value::String(status.to_string()),
        );
        entry.insert(
            "updated_at".to_string(),
            serde_json::Value::String(Utc::now().to_rfc3339()),
        );
    }
    let formatted = serde_json::to_string_pretty(&root)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
    fs::write(path, format!("{formatted}\n"))
}

fn upsert_provider_auth_config(
    provider_id: &str,
    login: &str,
    login_env: Option<&str>,
    auth: &ProviderAuth,
    auth_url: Option<String>,
) -> io::Result<()> {
    let path = config::provider_config_path();
    let content = fs::read_to_string(&path)?;
    let mut root: serde_json::Value = serde_json::from_str(&content)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;

    if !root.is_object() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "tura llm config root must be a JSON object",
        ));
    }

    let Some(root_object) = root.as_object_mut() else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "tura llm config root must be a JSON object",
        ));
    };
    let provider_auth = root_object
        .entry("provider_auth")
        .or_insert_with(|| serde_json::json!({}));
    if !provider_auth.is_object() {
        *provider_auth = serde_json::json!({});
    }

    let auth_type = match login {
        "api" => "api_key",
        "browser" => "browser_token",
        "local" => "local_cli_token",
        "device" => "device_code",
        "aws" => "aws_credentials",
        _ => "oauth",
    };
    let registry_entry = tura_llm_rust::provider_auth_registry_entry(provider_id);

    let token_env = auth
        .metadata
        .as_ref()
        .and_then(|metadata| metadata.get("token_env"))
        .and_then(|value| value.as_str())
        .filter(|value| !value.trim().is_empty())
        .map(|value| value.trim().to_string())
        .unwrap_or_else(|| provider_env_key(provider_id));

    let mut entry = serde_json::json!({
        "type": auth_type,
        "login": login,
        "status": "connected",
        "provider": provider_runtime_id(provider_id),
        "auth_url": auth_url,
        "token_env": token_env,
        "login_env": login_env,
        "updated_at": Utc::now().to_rfc3339(),
    });
    if let Some(registry_entry) = registry_entry {
        if let Some(refresh_env) = registry_entry.refresh_env {
            entry["refresh_env"] = serde_json::Value::String(refresh_env.to_string());
        }
        if let Some(expires_env) = registry_entry.expires_env {
            entry["expires_env"] = serde_json::Value::String(expires_env.to_string());
        }
        if let Some(account_env) = registry_entry.account_env {
            entry["account_env"] = serde_json::Value::String(account_env.to_string());
        }
        if !registry_entry.default_base_url.is_empty() {
            entry["endpoint"] =
                serde_json::Value::String(registry_entry.default_base_url.to_string());
        }
        if let Some(reason) = registry_entry.disabled_reason {
            entry["unsupported_reason"] = serde_json::Value::String(reason.to_string());
        }
    }
    if provider_id == "codex" && login == "oauth" {
        entry["endpoint"] = serde_json::Value::String(
            "https://chatgpt.com/backend-api/codex/responses".to_string(),
        );
        entry["refresh_env"] = serde_json::Value::String("OPENAI_REFRESH_TOKEN".to_string());
        entry["expires_env"] = serde_json::Value::String("OPENAI_TOKEN_EXPIRES".to_string());
        entry["account_env"] = serde_json::Value::String("OPENAI_ACCOUNT_ID".to_string());
        if let Some(account_id) = auth.account_id.as_deref() {
            entry["account_id"] = serde_json::Value::String(account_id.to_string());
        }
    }
    let Some(provider_auth_object) = provider_auth.as_object_mut() else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "provider_auth must be a JSON object",
        ));
    };
    provider_auth_object.insert(provider_id.to_string(), entry);

    let formatted = serde_json::to_string_pretty(&root)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
    fs::write(path, format!("{formatted}\n"))
}

fn provider_env_key(provider_id: &str) -> String {
    tura_llm_rust::provider_token_env(provider_id)
        .map(ToString::to_string)
        .unwrap_or_else(|| format!("{}_API_KEY", provider_id.to_ascii_uppercase()))
}

fn provider_login_key(provider_id: &str) -> String {
    tura_llm_rust::provider_login_env(provider_id)
        .map(ToString::to_string)
        .unwrap_or_else(|| format!("{}_LOGIN", provider_id.to_ascii_uppercase()))
}

fn provider_key_exists(provider_id: &str) -> bool {
    if provider_id == "github-copilot" {
        return ["COPILOT_GITHUB_TOKEN", "GH_TOKEN", "GITHUB_TOKEN"]
            .into_iter()
            .any(|key| provider_key_value_for_env(provider_id, key).is_some());
    }
    let key = provider_env_key(provider_id);
    provider_key_value_for_env(provider_id, &key).is_some()
}

fn provider_key_value_for_env(provider_id: &str, key: &str) -> Option<String> {
    let value = std::env::var(key)
        .ok()
        .or_else(|| tura_llm_rust::TuraConfig::default().get(key))
        .filter(|value| !value.trim().is_empty())
        .map(|value| value.trim().to_string())?;
    if matches!(
        provider_id,
        "codex"
            | "openai"
            | "openai-api"
            | "claude-code"
            | "anthropic"
            | "anthropic-api"
            | "antigravity"
            | "antigravity-api"
            | "github-copilot"
    ) {
        let login_key = provider_login_key(provider_id);
        let login = std::env::var(&login_key)
            .ok()
            .or_else(|| tura_llm_rust::TuraConfig::default().get(&login_key));
        let login_matches = match provider_id {
            "codex" => login.as_deref() == Some("oauth"),
            "openai" => !matches!(login.as_deref(), Some("oauth" | "browser")),
            "claude-code" => login.as_deref() == Some("oauth"),
            "anthropic" => !matches!(login.as_deref(), Some("oauth" | "browser")),
            "antigravity" => login.as_deref() == Some("oauth"),
            "github-copilot" => matches!(login.as_deref(), Some("device" | "oauth" | "api")),
            "openai-api" | "anthropic-api" | "antigravity-api" => {
                !matches!(login.as_deref(), Some("oauth" | "browser"))
            }
            _ => true,
        };
        if !login_matches {
            return None;
        }
    }
    Some(value)
}

#[cfg(test)]
#[path = "provider/tests.rs"]
mod tests;
