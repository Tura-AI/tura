use super::{
    apply_codex_auth_env, load_codex_auth_tokens, normalize_response_content,
    openai_login_is_oauth, provider_latency_timeouts, set_provider_latency_timeouts,
    ProviderLatencyConfig, ProviderLatencyTimeouts,
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
    let _lock = crate::test_support::env_lock();
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
    let _lock = crate::test_support::env_lock();
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
