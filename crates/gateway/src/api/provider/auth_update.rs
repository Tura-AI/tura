use std::collections::HashMap;

use crate::api::types::ProviderAuth;

pub(super) fn oauth_auth(
    access: String,
    refresh: Option<String>,
    expires: Option<i64>,
    account_id: Option<String>,
    login: &str,
    url: Option<String>,
) -> ProviderAuth {
    let mut metadata = HashMap::from([(
        "login".to_string(),
        serde_json::Value::String(login.to_string()),
    )]);
    if let Some(url) = url {
        metadata.insert("url".to_string(), serde_json::Value::String(url));
    }
    ProviderAuth {
        auth_type: "oauth".to_string(),
        key: Some(access.clone()),
        access: Some(access),
        refresh,
        expires,
        account_id,
        metadata: Some(metadata),
    }
}
