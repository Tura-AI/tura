use super::{
    apply_codex_auth_env, load_codex_auth_tokens, normalize_response_content,
    openai_login_is_oauth, provider_latency_timeouts, refresh_openai_access_token_if_needed,
    set_provider_latency_timeouts, ProviderConfig, ProviderLatencyConfig, ProviderLatencyTimeouts,
};
use serde_json::json;
use std::path::PathBuf;
use uuid::Uuid;

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
fn provider_alias_preserves_auth_identity_and_resolves_runtime_protocol() {
    let gemini = ProviderConfig {
        provider: "gemini-api".to_string(),
        base_url: "https://generativelanguage.googleapis.com/v1beta".to_string(),
        model: "gemini-3.5-flash".to_string(),
        temperature: 0.2,
    };
    let mistral = ProviderConfig {
        provider: "mistral".to_string(),
        base_url: "https://api.mistral.ai/v1".to_string(),
        model: "mistral-medium-3.5".to_string(),
        temperature: 0.2,
    };

    assert_eq!(gemini.provider, "gemini-api");
    assert_eq!(gemini.runtime_provider(), "google");
    assert_eq!(mistral.runtime_provider(), "mistral");
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
fn normalizes_codex_responses_events_when_output_array_is_empty() {
    let raw = json!({
        "object": "response",
        "output": [],
        "output_text": "I will inspect before editing.",
        "events": [
            {
                "type": "response.output_item.done",
                "item": {
                    "type": "message",
                    "id": "msg_1",
                    "content": [{
                        "type": "output_text",
                        "text": "I will inspect before editing."
                    }]
                }
            },
            {
                "type": "response.output_item.added",
                "item": {
                    "type": "function_call",
                    "id": "fc_1",
                    "call_id": "call_1",
                    "name": "command_run",
                    "arguments": ""
                }
            },
            {
                "type": "response.function_call_arguments.done",
                "item_id": "fc_1",
                "arguments": "{\"commands\":[{\"command_type\":\"shell_command\",\"command_line\":\"pwd\"}]}"
            },
            {
                "type": "response.output_item.done",
                "item": {
                    "type": "function_call",
                    "id": "fc_1",
                    "call_id": "call_1",
                    "name": "command_run",
                    "status": "completed",
                    "arguments": "{\"commands\":[{\"command_type\":\"shell_command\",\"command_line\":\"pwd\"}]}"
                }
            }
        ]
    });

    let content = normalize_response_content(&raw);

    assert_eq!(content["text"], "I will inspect before editing.");
    assert_eq!(content["tool_calls"][0]["id"], "call_1");
    assert_eq!(
        content["tool_calls"][0]["function"]["arguments"]["commands"][0]["command_line"],
        "pwd"
    );
}

#[test]
fn provider_latency_defaults_match_high_profile() {
    let selected = ProviderLatencyConfig::default().selected_timeouts();

    assert_eq!(selected.idle_output_timeout_ms, 80_000);
    assert_eq!(selected.first_output_timeout_ms, 160_000);
    assert_eq!(selected.total_timeout_ms, 960_000);
}

#[test]
fn provider_latency_level_tracks_tier_flag() {
    assert_eq!(super::latency_level_for_tier("thinking"), "x-high");
    assert_eq!(super::latency_level_for_tier("fast"), "high");
    assert_eq!(super::latency_level_for_tier("embedding_high"), "x-high");
    assert_eq!(super::latency_level_for_tier("embedding_low"), "high");
    // Unknown tiers fall back to the high level.
    assert_eq!(super::latency_level_for_tier("something_else"), "high");
}

#[test]
fn provider_latency_timeouts_for_tier_resolve_levels() {
    let config = ProviderLatencyConfig::default();

    let thinking = config.timeouts_for_tier("thinking");
    assert_eq!(thinking.total_timeout_ms, 1_200_000);

    let fast = config.timeouts_for_tier("fast");
    assert_eq!(fast.total_timeout_ms, 960_000);
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
    let _lock = crate::test_support::env_lock();
    let _env = EnvRestore::capture(&[
        "CODEX_HOME",
        "OPENAI_LOGIN",
        "OPENAI_API_KEY",
        "OPENAI_REFRESH_TOKEN",
        "OPENAI_ACCOUNT_ID",
        "TURA_PROVIDER_CONFIG",
    ]);
    std::env::remove_var("OPENAI_LOGIN");
    std::env::remove_var("OPENAI_API_KEY");
    std::env::remove_var("OPENAI_REFRESH_TOKEN");
    std::env::remove_var("OPENAI_ACCOUNT_ID");
    std::env::remove_var("TURA_PROVIDER_CONFIG");

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

#[tokio::test]
async fn prefers_codex_auth_when_env_token_is_still_marked_valid() {
    let _lock = crate::test_support::env_lock_async().await;
    let _restore = EnvRestore::capture(&[
        "CODEX_HOME",
        "TURA_ENV_PATH",
        "OPENAI_LOGIN",
        "OPENAI_API_KEY",
        "OPENAI_REFRESH_TOKEN",
        "OPENAI_TOKEN_EXPIRES",
        "OPENAI_ACCOUNT_ID",
    ]);
    let root = unique_temp_dir("rotated-codex-auth");
    let codex_home = root.join("codex-home");
    std::fs::create_dir_all(&codex_home).expect("create codex home");
    std::fs::write(
        codex_home.join("auth.json"),
        r#"{
            "tokens": {
                "access_token": "fresh-codex-access",
                "refresh_token": "fresh-codex-refresh",
                "account_id": "fresh-account"
            }
        }"#,
    )
    .expect("write codex auth");
    let env_path = root.join("tura.env");
    std::fs::write(
        &env_path,
        concat!(
            "OPENAI_LOGIN=oauth\n",
            "OPENAI_API_KEY=stale-env-access\n",
            "OPENAI_REFRESH_TOKEN=stale-env-refresh\n",
            "OPENAI_TOKEN_EXPIRES=4102444800000\n",
        ),
    )
    .expect("write tura env");

    std::env::set_var("CODEX_HOME", &codex_home);
    std::env::set_var("TURA_ENV_PATH", &env_path);
    for key in [
        "OPENAI_LOGIN",
        "OPENAI_API_KEY",
        "OPENAI_REFRESH_TOKEN",
        "OPENAI_TOKEN_EXPIRES",
        "OPENAI_ACCOUNT_ID",
    ] {
        std::env::remove_var(key);
    }
    let conf = crate::TuraConfig::new(".env.missing");

    // Regression: a future expiry on stale .env data must not mask a newer Codex login.
    let access = refresh_openai_access_token_if_needed(&conf)
        .await
        .expect("select codex access token");

    assert_eq!(access, "fresh-codex-access");
    assert_eq!(
        std::env::var("OPENAI_API_KEY").as_deref(),
        Ok("fresh-codex-access")
    );
    assert_eq!(
        std::env::var("OPENAI_REFRESH_TOKEN").as_deref(),
        Ok("fresh-codex-refresh")
    );
    assert_eq!(
        std::env::var("OPENAI_ACCOUNT_ID").as_deref(),
        Ok("fresh-account")
    );
}

#[test]
fn openai_oauth_login_uses_provider_auth_config() {
    let _lock = crate::test_support::env_lock();
    let _env = EnvRestore::capture(&[
        "CODEX_HOME",
        "OPENAI_LOGIN",
        "OPENAI_API_KEY",
        "TURA_PROVIDER_CONFIG",
    ]);
    std::env::remove_var("CODEX_HOME");
    std::env::remove_var("OPENAI_LOGIN");
    std::env::remove_var("OPENAI_API_KEY");
    std::env::remove_var("TURA_PROVIDER_CONFIG");

    let dir = unique_temp_dir("provider-config");
    let config = dir.join("provider_config.json");
    std::fs::write(&config, r#"{"provider_auth":{"openai":{"login":"oauth"}}}"#)
        .expect("provider config");
    std::env::set_var("TURA_PROVIDER_CONFIG", &config);

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
fn provider_failure_retry_classifier_rejects_auth_billing_and_missing_model_errors() {
    for status in [401, 402, 403, 404] {
        assert!(super::TuraError::HttpStatus {
            status,
            body: "provider rejected request".to_string(),
        }
        .is_non_retryable_provider_failure());
    }

    assert!(super::TuraError::Config {
        message: "API Key not found for provider 'openai'".to_string(),
    }
    .is_non_retryable_provider_failure());
    assert!(super::TuraError::AllProvidersFailed {
        message: "openai:gpt => http status 404: model not found".to_string(),
    }
    .is_non_retryable_provider_failure());
    assert!(super::TuraError::ProviderRequest {
        provider: "openai".to_string(),
        message: "billing quota is not enabled".to_string(),
    }
    .is_non_retryable_provider_failure());

    assert!(!super::TuraError::HttpStatus {
        status: 429,
        body: "slow down".to_string(),
    }
    .is_non_retryable_provider_failure());
    assert!(!super::TuraError::Network {
        message: "connection reset".to_string(),
    }
    .is_non_retryable_provider_failure());
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
