use router_contract::ConfigurableEntry;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ToolManifest {
    pub id: String,
    pub name: String,
    pub description: String,
    pub core: bool,
    pub category: String,
    pub execution: String,
    pub state_machine: String,
    pub supports_macro_command: bool,
    pub mutating: bool,
    pub network: bool,
    pub runtime: RuntimeSection,
    pub limits: LimitsSection,
    pub paths: PathsSection,
    #[serde(default)]
    pub configurable: Vec<ConfigurableEntry>,
    #[serde(skip)]
    pub manifest_path: PathBuf,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RuntimeSection {
    #[serde(default)]
    pub binary: String,
    #[serde(default)]
    pub entry: String,
    pub language: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LimitsSection {
    pub default_timeout_ms: u64,
    pub max_timeout_ms: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PathsSection {
    pub prompt: String,
    pub schema: String,
    pub policy: String,
}
