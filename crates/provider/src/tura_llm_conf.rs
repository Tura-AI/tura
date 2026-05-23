use std::env;
use std::path::PathBuf;

use tokio::fs;

use crate::tura_llm::{RawRouteConfig, RootConfig, Settings, TuraError};

fn config_path() -> PathBuf {
    if let Ok(env_path) = env::var("TURALLM_CONFIG") {
        let trimmed = env_path.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
    }
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let config_dir_path = manifest_dir.join("config").join("tura_llm_config.json");
    if config_dir_path.exists() {
        return config_dir_path;
    }
    manifest_dir.join("src").join("tura_llm_config.json")
}

pub async fn load_settings() -> Result<Settings, TuraError> {
    let path = config_path();
    let content = fs::read_to_string(&path).await.map_err(TuraError::io)?;
    let cfg: RootConfig = serde_json::from_str(&content)?;
    crate::tura_llm::set_provider_latency_timeouts(cfg.provider_latency.selected_timeouts());

    let build = |name: &str| -> Result<crate::tura_llm::RouteConfig, TuraError> {
        let route = cfg.routes.get(name).cloned().unwrap_or(RawRouteConfig {
            default_temperature: 0.2,
            providers: vec![],
        });
        Settings::make_route(
            &cfg.provider_base_url,
            &route.providers,
            route.default_temperature,
        )
    };

    Ok(Settings {
        tura_general: build("tura_general")?,
        tura_office: build("tura_office")?,
        tura_creative: build("tura_creative")?,
        tura_translator: build("tura_translator")?,
        tura_validator: build("tura_validator")?,
        tura_validator_advanced: build("tura_validator_advanced")?,
        tura_classifier: build("tura_classifier")?,
        tura_embedding: build("tura_embedding")?,
        tura_coder: build("tura_coder")?,
        tura_coder_advanced: build("tura_coder_advanced")?,
        tura_planner: build("tura_planner")?,
        tura_planner_advanced: build("tura_planner_advanced")?,
        tura_roleplay: build("tura_roleplay")?,
        tura_professional: build("tura_professional")?,
        tura_math: build("tura_math")?,
        tura_academic: build("tura_academic")?,
    })
}
