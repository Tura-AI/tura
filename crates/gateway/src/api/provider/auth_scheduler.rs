use std::sync::Once;
use tokio::time::{sleep, Duration};

static START: Once = Once::new();

/// How often the background scheduler checks registered OAuth providers for
/// tokens that are expiring soon. Checked once per hour; expiry is detected
/// proactively with a buffer wider than this interval (see
/// [`super::auth_validator::provider_auth_expires_soon`]) so a token never
/// lapses between checks.
const PROVIDER_AUTH_REFRESH_INTERVAL: Duration = Duration::from_secs(3600);

pub(super) fn start_provider_auth_scheduler() {
    START.call_once(|| {
        tokio::spawn(async {
            loop {
                refresh_registered_oauth_providers().await;
                sleep(PROVIDER_AUTH_REFRESH_INTERVAL).await;
            }
        });
    });
}

async fn refresh_registered_oauth_providers() {
    for entry in super::auth_registry::entries() {
        if entry.capabilities.supports_oauth_refresh {
            let _ = super::refresh_provider_auth_if_needed(entry.provider_id, false).await;
        }
    }
}
