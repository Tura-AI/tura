use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthResponse {
    pub healthy: bool,
    pub version: String,
    /// Canonical runtime/project root this gateway is serving (TURA_PROJECT_ROOT).
    /// Lets clients tell whether a reachable gateway belongs to their own package.
    #[serde(default)]
    pub root: String,
    /// Directory of the running gateway executable.
    #[serde(default)]
    pub exe_dir: String,
    /// Provider LLM call log directory when dev logging is enabled; absent in production.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dev_log_path: Option<String>,
}

impl Default for HealthResponse {
    fn default() -> Self {
        Self {
            healthy: true,
            version: env!("CARGO_PKG_VERSION").to_string(),
            root: String::new(),
            exe_dir: String::new(),
            dev_log_path: None,
        }
    }
}
