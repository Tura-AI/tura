use super::manifest::{ConfigurableEntry, ToolManifest};
use super::{resolve, state::ToolState};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ToolView {
    pub id: String,
    pub name: String,
    pub description: String,
    pub core: bool,
    pub category: String,
    pub execution: String,
    pub enabled: bool,
    pub aliases: Vec<String>,
    pub supports_macro_command: bool,
    pub mutating: bool,
    pub network: bool,
    pub configurable: Vec<ConfigurableEntry>,
    pub state: ToolState,
    pub binary: Option<String>,
    pub binary_path: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct ToolPatch {
    pub enabled: Option<bool>,
    pub aliases: Option<Vec<String>>,
    pub core: Option<bool>,
    pub execution: Option<String>,
    pub binary: Option<String>,
    pub mutating: Option<bool>,
    pub network: Option<bool>,
    pub policy: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ToolConfigResponse {
    pub id: String,
    pub configurable: Vec<ConfigurableEntry>,
    pub values: BTreeMap<String, serde_json::Value>,
}

pub fn view_for_manifest(manifest: &ToolManifest, repo_root: &Path) -> ToolView {
    let binary_path = resolve::resolve_tool_binary(repo_root, &manifest.runtime.binary)
        .map(|path| path.display().to_string());
    let state = if !manifest.core && binary_path.is_none() {
        ToolState::Unavailable
    } else {
        ToolState::Enabled
    };
    ToolView {
        id: manifest.id.clone(),
        name: manifest.name.clone(),
        description: manifest.description.clone(),
        core: manifest.core,
        category: manifest.category.clone(),
        execution: manifest.execution.clone(),
        enabled: state != ToolState::Unavailable,
        aliases: default_aliases(&manifest.id),
        supports_macro_command: manifest.supports_macro_command,
        mutating: manifest.mutating,
        network: manifest.network,
        configurable: manifest.configurable.clone(),
        state,
        binary: (!manifest.runtime.binary.is_empty()).then(|| manifest.runtime.binary.clone()),
        binary_path,
    }
}

pub fn config_for_manifest(manifest: &ToolManifest) -> ToolConfigResponse {
    let values = manifest
        .configurable
        .iter()
        .map(|entry| (entry.key.clone(), entry.default.clone()))
        .collect();
    ToolConfigResponse {
        id: manifest.id.clone(),
        configurable: manifest.configurable.clone(),
        values,
    }
}

fn default_aliases(id: &str) -> Vec<String> {
    match id {
        "read_media" => vec!["view_media".to_string(), "inspect_media".to_string()],
        "web_discover" => vec![
            "web_search".to_string(),
            "web_fetch".to_string(),
            "discover_web".to_string(),
            "search_web".to_string(),
        ],
        "shell_command" => vec!["shell".to_string(), "shll".to_string(), "shall".to_string()],
        _ => Vec::new(),
    }
}
