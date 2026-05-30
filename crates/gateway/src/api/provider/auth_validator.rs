use chrono::Utc;

use super::{config::config_value, ProviderAuthStatusResponse};

/// Refresh-ahead window. The background scheduler runs hourly, so a token is
/// considered "expiring soon" if it would lapse within this window (slightly
/// wider than the 1h interval) — guaranteeing it is refreshed on the check
/// before it actually expires.
const EXPIRES_SOON_BUFFER_MS: i64 = 70 * 60 * 1000;

pub(super) fn provider_auth_expires_soon(status: &ProviderAuthStatusResponse) -> bool {
    if status.expired == Some(true) {
        return true;
    }
    let Some(expires_at) = status
        .expires_env
        .as_deref()
        .and_then(config_value)
        .and_then(|value| value.parse::<i64>().ok())
    else {
        return false;
    };
    expires_at <= Utc::now().timestamp_millis() + EXPIRES_SOON_BUFFER_MS
}

pub(super) fn provider_auth_can_refresh(provider_id: &str) -> bool {
    tura_llm_rust::provider_auth_registry_entry(provider_id)
        .is_some_and(|entry| entry.capabilities.supports_oauth_refresh)
}
