use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub language: Option<String>,
    pub theme: Option<String>,
    pub model: Option<String>,
    pub agent: Option<String>,
    pub skill_folders: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            language: None,
            theme: None,
            model: Some(crate::session::config::DEFAULT_SESSION_MODEL.to_string()),
            agent: Some(crate::session::config::DEFAULT_SESSION_AGENT.to_string()),
            skill_folders: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigPatch {
    pub language: Option<String>,
    pub theme: Option<String>,
    pub model: Option<String>,
    pub agent: Option<String>,
    pub skill_folders: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TuraConfigResponse {
    pub path: String,
    pub tiers: Vec<TuraConfigTier>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TuraConfigUpdate {
    pub tier: String,
    pub provider: String,
    pub model: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct TuraConfigTier {
    pub tier: String,
    pub current: Option<TuraConfigSelection>,
    pub options: Vec<TuraConfigOption>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TuraConfigSelection {
    pub provider: String,
    pub model: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct TuraConfigOption {
    pub provider: String,
    pub provider_name: String,
    pub model: String,
    pub model_name: String,
}
