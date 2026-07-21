#![allow(dead_code)]

pub mod aliases;
pub mod api;
pub mod config;
pub mod discover;
pub mod manifest;
pub mod resolve;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub use manifest::ToolManifest;
use router_contract::{ToolConfigResponse, ToolPatch, ToolState, ToolView};

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
                ToolState::Enabled
            } else {
                ToolState::Disabled
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::tools::manifest::{
        LimitsSection, PathsSection, RuntimeSection, ToolManifest,
    };
    use router_contract::ConfigurableEntry;
    use serde_json::json;

    #[test]
    fn registry_resolves_aliases_and_unknown_values_without_mutating_input() {
        let registry = registry_with(vec![manifest("read_media", false)]);

        assert_eq!(registry.resolve_alias("view_media"), "read_media");
        assert_eq!(registry.resolve_alias("inspect_media"), "read_media");
        assert_eq!(registry.resolve_alias("unknown"), "unknown");
        assert!(registry.get("view_media").is_some());
        assert!(registry.get("unknown").is_none());
    }

    #[test]
    fn patch_tool_allows_safe_view_overrides_only_for_returned_view() {
        let registry = registry_with(vec![manifest("web_discover", false)]);

        let disabled = registry
            .patch_tool(
                "web_search",
                ToolPatch {
                    enabled: Some(false),
                    aliases: Some(vec!["search".to_string()]),
                    ..ToolPatch::default()
                },
            )
            .expect("safe patch");

        assert_eq!(disabled.id, "web_discover");
        assert!(!disabled.enabled);
        assert_eq!(disabled.state, ToolState::Disabled);
        assert_eq!(disabled.aliases, vec!["search"]);

        let original = registry.get("web_discover").expect("original view");
        assert_ne!(original.aliases, disabled.aliases);
        assert_eq!(original.state, ToolState::Unavailable);
    }

    #[test]
    fn patch_tool_rejects_unsafe_fields_and_unknown_tools() {
        let registry = registry_with(vec![manifest("read_media", false)]);

        for patch in [
            ToolPatch {
                core: Some(true),
                ..ToolPatch::default()
            },
            ToolPatch {
                execution: Some("in_process".to_string()),
                ..ToolPatch::default()
            },
            ToolPatch {
                binary: Some("replacement".to_string()),
                ..ToolPatch::default()
            },
            ToolPatch {
                mutating: Some(true),
                ..ToolPatch::default()
            },
            ToolPatch {
                network: Some(true),
                ..ToolPatch::default()
            },
            ToolPatch {
                policy: Some("allow-all".to_string()),
                ..ToolPatch::default()
            },
        ] {
            let error = registry
                .patch_tool("read_media", patch)
                .expect_err("unsafe patch should fail");
            assert_eq!(
                error,
                "unsafe manifest fields cannot be changed through gateway"
            );
        }

        assert_eq!(
            registry
                .patch_tool(
                    "missing",
                    ToolPatch {
                        enabled: Some(true),
                        ..ToolPatch::default()
                    },
                )
                .expect_err("unknown tool"),
            "unknown tool: missing"
        );
    }

    #[test]
    fn patch_config_validates_and_merges_values_over_defaults() {
        let mut tool = manifest("read_media", false);
        tool.configurable = vec![
            configurable("pdf_default_pages", "enum", json!("5"), &["5", "10"]),
            configurable("ocr_enabled", "boolean", json!(false), &[]),
        ];
        let registry = registry_with(vec![tool]);

        let patched = registry
            .patch_config(
                "view_media",
                BTreeMap::from([
                    ("pdf_default_pages".to_string(), json!("10")),
                    ("ocr_enabled".to_string(), json!(true)),
                ]),
            )
            .expect("valid config patch");

        assert_eq!(patched.id, "read_media");
        assert_eq!(patched.values["pdf_default_pages"], json!("10"));
        assert_eq!(patched.values["ocr_enabled"], json!(true));

        let invalid = registry
            .patch_config(
                "read_media",
                BTreeMap::from([("pdf_default_pages".to_string(), json!("100"))]),
            )
            .expect_err("invalid enum");
        assert_eq!(invalid, "invalid enum value for pdf_default_pages: 100");
    }

    #[test]
    fn normalize_repo_root_walks_up_from_child_when_repo_markers_exist() {
        let temp = tempfile::tempdir().expect("temp repo");
        let repo = temp.path().join("repo");
        let child = repo.join("nested").join("crate");
        std::fs::create_dir_all(repo.join("crates").join("tools")).expect("tools dir");
        std::fs::create_dir_all(repo.join("commands")).expect("commands dir");
        std::fs::create_dir_all(&child).expect("child dir");

        assert_eq!(normalize_repo_root(child), repo);
        assert_eq!(
            normalize_repo_root(temp.path().join("standalone")),
            temp.path().join("standalone")
        );
    }

    fn registry_with(manifests: Vec<ToolManifest>) -> ToolRegistry {
        ToolRegistry {
            repo_root: PathBuf::from("C:/repo"),
            tools: manifests
                .into_iter()
                .map(|manifest| (manifest.id.clone(), manifest))
                .collect(),
        }
    }

    fn manifest(id: &str, core: bool) -> ToolManifest {
        ToolManifest {
            id: id.to_string(),
            name: format!("{id} tool"),
            description: "test tool".to_string(),
            core,
            category: "test".to_string(),
            execution: if core {
                "in_process".to_string()
            } else {
                "one_shot".to_string()
            },
            state_machine: "default".to_string(),
            supports_macro_command: true,
            mutating: false,
            network: false,
            runtime: RuntimeSection {
                binary: if core {
                    String::new()
                } else {
                    format!("tura-command-{id}")
                },
                entry: String::new(),
                language: "rust".to_string(),
            },
            limits: LimitsSection {
                default_timeout_ms: 1_000,
                max_timeout_ms: 2_000,
            },
            paths: PathsSection {
                prompt: "prompt.md".to_string(),
                schema: "schema.json".to_string(),
                policy: "policy.toml".to_string(),
            },
            configurable: Vec::new(),
            manifest_path: PathBuf::from("tool.json"),
        }
    }

    fn configurable(
        key: &str,
        value_type: &str,
        default: serde_json::Value,
        enum_values: &[&str],
    ) -> ConfigurableEntry {
        ConfigurableEntry {
            key: key.to_string(),
            label: key.to_string(),
            description: format!("{key} setting"),
            value_type: value_type.to_string(),
            default,
            enum_values: enum_values.iter().map(|value| value.to_string()).collect(),
            required: false,
            scope: "workspace".to_string(),
        }
    }
}
