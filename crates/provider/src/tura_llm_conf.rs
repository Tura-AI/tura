use std::env;
use std::path::PathBuf;

use tokio::fs;

use crate::tura_llm::{RootConfig, Settings, TuraError};

fn config_path() -> PathBuf {
    if let Ok(env_path) = env::var("TURA_PROVIDER_CONFIG") {
        let trimmed = env_path.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
    }
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir.join("config").join("provider_config.json")
}

pub async fn load_settings() -> Result<Settings, TuraError> {
    let path = config_path();
    let content = fs::read_to_string(&path).await.map_err(TuraError::io)?;
    let cfg: RootConfig = serde_json::from_str(&content)?;
    crate::tura_llm::set_provider_latency_timeouts(cfg.provider_latency.selected_timeouts());
    crate::tura_llm::set_provider_latency_config(cfg.provider_latency.clone());

    let mut routes = std::collections::HashMap::new();
    for (name, route) in &cfg.routes {
        Settings::make_route(
            &cfg.provider_base_url,
            &route.providers,
            route.default_temperature,
        )
        .map(|route| routes.insert(name.clone(), route))?;
    }

    Ok(Settings {
        provider_base_url: cfg.provider_base_url,
        routes,
        model_catalog: cfg.model_catalog,
        provider_enums: cfg.provider_enums,
    })
}

#[cfg(test)]
mod tests {
    use super::config_path;

    #[test]
    fn config_path_prefers_explicit_provider_config() {
        let _guard = crate::test_support::env_lock();
        let previous_provider = std::env::var_os("TURA_PROVIDER_CONFIG");
        // SAFETY: the caller ensures no concurrent foreign environment access races with this mutation.
        #[allow(
            unsafe_code,
            reason = "Rust 2024 process-environment mutation audited at the caller"
        )]
        unsafe {
            std::env::set_var("TURA_PROVIDER_CONFIG", "C:/tmp/tura-test-config.json")
        };

        assert_eq!(
            config_path(),
            std::path::PathBuf::from("C:/tmp/tura-test-config.json")
        );

        match previous_provider {
            // SAFETY: the caller ensures no concurrent foreign environment access races with this mutation.
            Some(value) => {
                #[allow(
                    unsafe_code,
                    reason = "Rust 2024 process-environment mutation audited at the caller"
                )]
                unsafe {
                    std::env::set_var("TURA_PROVIDER_CONFIG", value)
                }
            }
            // SAFETY: the caller ensures no concurrent foreign environment access races with this mutation.
            None => {
                #[allow(
                    unsafe_code,
                    reason = "Rust 2024 process-environment mutation audited at the caller"
                )]
                unsafe {
                    std::env::remove_var("TURA_PROVIDER_CONFIG")
                }
            }
        }
    }

    #[tokio::test]
    async fn bundled_config_exposes_model_tiers_and_minimax_metadata() {
        let _guard = crate::test_support::env_lock_async().await;
        let previous_provider = std::env::var_os("TURA_PROVIDER_CONFIG");
        // SAFETY: the caller ensures no concurrent foreign environment access races with this mutation.
        #[allow(
            unsafe_code,
            reason = "Rust 2024 process-environment mutation audited at the caller"
        )]
        unsafe {
            std::env::remove_var("TURA_PROVIDER_CONFIG")
        };

        let settings = super::load_settings().await.expect("load bundled config");
        for route in ["thinking", "fast", "embedding_high", "embedding_low"] {
            assert!(
                settings.route_by_name(route).is_some(),
                "missing route {route}"
            );
        }
        assert_eq!(settings.routes.len(), 4);
        assert!(settings
            .configured_model_catalog()
            .contains_key("openrouter"));
        assert_eq!(
            settings.provider_base_url("mistral").as_deref(),
            Some("https://api.mistral.ai/v1")
        );
        assert_eq!(
            settings.provider_base_url("github-copilot").as_deref(),
            Some("https://api.githubcopilot.com")
        );
        assert_eq!(
            settings.provider_base_url("minimax").as_deref(),
            Some("https://api.minimax.io/v1")
        );
        assert_eq!(
            settings.provider_base_url("minimax_cn").as_deref(),
            Some("https://api.minimaxi.com/v1")
        );
        assert_eq!(
            settings.provider_base_url("minimax_anthropic").as_deref(),
            Some("https://api.minimax.io/anthropic")
        );
        assert_eq!(
            settings
                .provider_base_url("minimax_anthropic_cn")
                .as_deref(),
            Some("https://api.minimaxi.com/anthropic")
        );

        let minimax = settings
            .model_catalog
            .providers
            .get("minimax")
            .expect("minimax provider catalog");
        assert_eq!(minimax.runtime_provider, "minimax");
        assert_eq!(minimax.token_env.as_deref(), Some("MINIMAX_API_KEY"));
        assert_eq!(minimax.auth_methods, ["api_key"]);

        let minimax_models = minimax.models.get("thinking").expect("minimax models");
        let minimax_m3 = minimax_models
            .iter()
            .find(|model| model.id() == "MiniMax-M3")
            .and_then(|model| model.detail())
            .expect("MiniMax-M3 metadata");
        assert_eq!(minimax_m3.limit.context, 1_000_000);
        assert_eq!(minimax_m3.limit.input, 1_000_000);
        assert_eq!(minimax_m3.modalities.input, ["text", "image", "video"]);
        assert_eq!(
            minimax_m3.options.get("thinking"),
            Some(&serde_json::json!(["adaptive", "disabled"]))
        );
        assert_eq!(
            minimax_m3.options.get("pricing_usd_per_million_tokens"),
            Some(&serde_json::json!({
                "input": 0.6,
                "output": 2.4,
                "cache_read": 0.12,
                "cache_write": null
            }))
        );
        assert_eq!(
            minimax_m3.options.get("regional_endpoints"),
            Some(&serde_json::json!([
                {
                    "region": "global_en",
                    "openai_base_url": "https://api.minimax.io/v1",
                    "anthropic_base_url": "https://api.minimax.io/anthropic"
                },
                {
                    "region": "cn_zh",
                    "openai_base_url": "https://api.minimaxi.com/v1",
                    "anthropic_base_url": "https://api.minimaxi.com/anthropic"
                }
            ]))
        );

        let minimax_m27 = minimax_models
            .iter()
            .find(|model| model.id() == "MiniMax-M2.7")
            .and_then(|model| model.detail())
            .expect("MiniMax-M2.7 metadata");
        assert_eq!(minimax_m27.limit.context, 204_800);
        assert_eq!(minimax_m27.limit.input, 204_800);
        assert_eq!(minimax_m27.modalities.input, ["text"]);
        assert_eq!(
            minimax_m27.options.get("thinking"),
            Some(&serde_json::json!(["always_on"]))
        );
        assert_eq!(
            minimax_m27.options.get("pricing_usd_per_million_tokens"),
            Some(&serde_json::json!({
                "input": 0.3,
                "output": 1.2,
                "cache_read": 0.06,
                "cache_write": 0.375
            }))
        );

        match previous_provider {
            // SAFETY: the caller ensures no concurrent foreign environment access races with this mutation.
            Some(value) => {
                #[allow(
                    unsafe_code,
                    reason = "Rust 2024 process-environment mutation audited at the caller"
                )]
                unsafe {
                    std::env::set_var("TURA_PROVIDER_CONFIG", value)
                }
            }
            // SAFETY: the caller ensures no concurrent foreign environment access races with this mutation.
            None => {
                #[allow(
                    unsafe_code,
                    reason = "Rust 2024 process-environment mutation audited at the caller"
                )]
                unsafe {
                    std::env::remove_var("TURA_PROVIDER_CONFIG")
                }
            }
        }
    }
}
