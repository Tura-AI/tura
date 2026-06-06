#![allow(dead_code)]

pub mod aliases;
pub mod api;
pub mod config;
pub mod discover;
pub mod manifest;
pub mod resolve;
pub mod state;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub use api::{ToolConfigResponse, ToolPatch, ToolView};
pub use manifest::ToolManifest;

#[derive(Clone, Debug, Default)]
pub struct ToolRegistry {
    repo_root: PathBuf,
    tools: BTreeMap<String, ToolManifest>,
}

impl ToolRegistry {
    pub fn discover(repo_root: impl Into<PathBuf>) -> Self {
        let repo_root = normalize_repo_root(repo_root.into());
        let tools = discover::discover_manifests(&repo_root)
            .into_iter()
            .map(|manifest| (manifest.id.clone(), manifest))
            .collect();
        Self { repo_root, tools }
    }

    pub fn list(&self) -> Vec<ToolView> {
        self.tools
            .values()
            .map(|manifest| api::view_for_manifest(manifest, &self.repo_root))
            .collect()
    }

    pub fn get(&self, tool_id: &str) -> Option<ToolView> {
        let id = self.resolve_alias(tool_id);
        self.tools
            .get(&id)
            .map(|manifest| api::view_for_manifest(manifest, &self.repo_root))
    }

    pub fn config(&self, tool_id: &str) -> Option<ToolConfigResponse> {
        let id = self.resolve_alias(tool_id);
        self.tools.get(&id).map(api::config_for_manifest)
    }

    pub fn patch_tool(&self, tool_id: &str, patch: ToolPatch) -> Result<ToolView, String> {
        if patch.core.is_some()
            || patch.execution.is_some()
            || patch.binary.is_some()
            || patch.mutating.is_some()
            || patch.network.is_some()
            || patch.policy.is_some()
        {
            return Err("unsafe manifest fields cannot be changed through gateway".to_string());
        }
        let id = self.resolve_alias(tool_id);
        let manifest = self
            .tools
            .get(&id)
            .ok_or_else(|| format!("unknown tool: {tool_id}"))?;
        let mut view = api::view_for_manifest(manifest, &self.repo_root);
        if let Some(enabled) = patch.enabled {
            view.enabled = enabled;
            view.state = if enabled {
                state::ToolState::Enabled
            } else {
                state::ToolState::Disabled
            };
        }
        if let Some(aliases) = patch.aliases {
            view.aliases = aliases;
        }
        Ok(view)
    }

    pub fn patch_config(
        &self,
        tool_id: &str,
        values: BTreeMap<String, serde_json::Value>,
    ) -> Result<ToolConfigResponse, String> {
        let id = self.resolve_alias(tool_id);
        let manifest = self
            .tools
            .get(&id)
            .ok_or_else(|| format!("unknown tool: {tool_id}"))?;
        config::validate_configurable_values(manifest, &values)?;
        let mut response = api::config_for_manifest(manifest);
        response.values.extend(values);
        Ok(response)
    }

    pub fn resolve_alias(&self, value: &str) -> String {
        aliases::resolve_alias(value, self.tools.values())
    }

    #[allow(dead_code)]
    pub fn repo_root(&self) -> &Path {
        &self.repo_root
    }
}

fn normalize_repo_root(path: PathBuf) -> PathBuf {
    path.ancestors()
        .find(|candidate| {
            candidate.join("crates").join("tools").is_dir() && candidate.join("commands").is_dir()
        })
        .map(Path::to_path_buf)
        .unwrap_or(path)
}
