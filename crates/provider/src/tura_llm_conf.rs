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
        std::env::set_var("TURA_PROVIDER_CONFIG", "C:/tmp/tura-test-config.json");

        assert_eq!(
            config_path(),
            std::path::PathBuf::from("C:/tmp/tura-test-config.json")
        );

        match previous_provider {
            Some(value) => std::env::set_var("TURA_PROVIDER_CONFIG", value),
            None => std::env::remove_var("TURA_PROVIDER_CONFIG"),
        }
    }

    #[tokio::test]
    async fn bundled_config_exposes_six_model_tiers() {
        let _guard = crate::test_support::env_lock_async().await;
        let previous_provider = std::env::var_os("TURA_PROVIDER_CONFIG");
        std::env::remove_var("TURA_PROVIDER_CONFIG");

        let settings = super::load_settings().await.expect("load bundled config");
        for route in [
            "flagship_thinking",
            "thinking",
            "fast",
            "instant",
            "embedding_high",
            "embedding_low",
        ] {
            assert!(
                settings.route_by_name(route).is_some(),
                "missing route {route}"
            );
        }
        assert_eq!(settings.routes.len(), 6);
        assert!(settings
            .configured_model_catalog()
            .contains_key("openrouter"));

        match previous_provider {
            Some(value) => std::env::set_var("TURA_PROVIDER_CONFIG", value),
            None => std::env::remove_var("TURA_PROVIDER_CONFIG"),
        }
    }
}
