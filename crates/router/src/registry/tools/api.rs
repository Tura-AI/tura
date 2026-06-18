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
        "image_generate" => vec![
            "image_gen".to_string(),
            "generate_image".to_string(),
            "text_to_image".to_string(),
            "t2i".to_string(),
        ],
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::tools::manifest::{LimitsSection, PathsSection, RuntimeSection};

    #[test]
    fn view_for_core_manifest_is_enabled_without_external_binary() {
        let manifest = manifest("shell_command", true, "");

        let view = view_for_manifest(&manifest, Path::new("C:/repo"));

        assert_eq!(view.id, "shell_command");
        assert_eq!(view.name, "shell_command tool");
        assert!(view.core);
        assert!(view.enabled);
        assert_eq!(view.state, ToolState::Enabled);
        assert_eq!(view.binary, None);
        assert_eq!(view.binary_path, None);
        assert_eq!(
            view.aliases,
            vec!["shell".to_string(), "shll".to_string(), "shall".to_string()]
        );
    }

    #[test]
    fn view_for_external_manifest_is_unavailable_when_binary_is_missing() {
        let manifest = manifest("read_media", false, "definitely-missing-tura-tool");

        let view = view_for_manifest(&manifest, Path::new("C:/repo"));

        assert_eq!(view.id, "read_media");
        assert!(!view.core);
        assert!(!view.enabled);
        assert_eq!(view.state, ToolState::Unavailable);
        assert_eq!(view.binary.as_deref(), Some("definitely-missing-tura-tool"));
        assert_eq!(view.binary_path, None);
        assert_eq!(
            view.aliases,
            vec!["view_media".to_string(), "inspect_media".to_string()]
        );
    }

    #[test]
    fn view_for_external_manifest_resolves_binary_from_repo_candidates() {
        let temp = tempfile::tempdir().expect("temp repo");
        let bin = temp.path().join("bin");
        std::fs::create_dir_all(&bin).expect("create bin");
        let file_name = if cfg!(windows) {
            "tura-command-test.exe"
        } else {
            "tura-command-test"
        };
        let binary_path = bin.join(file_name);
        std::fs::write(&binary_path, b"binary").expect("write binary");
        let manifest = manifest("test_tool", false, "tura-command-test");

        let view = view_for_manifest(&manifest, temp.path());

        assert!(view.enabled);
        assert_eq!(view.state, ToolState::Enabled);
        assert_eq!(view.binary.as_deref(), Some("tura-command-test"));
        assert_eq!(
            view.binary_path.as_deref(),
            Some(binary_path.display().to_string().as_str())
        );
    }

    #[test]
    fn config_for_manifest_projects_defaults_by_key() {
        let mut manifest = manifest("web_discover", false, "tura-command-web-discover");
        manifest.configurable = vec![
            configurable(
                "max_results",
                "enum",
                serde_json::json!("5"),
                &["1", "5", "10"],
            ),
            configurable("safe_search", "boolean", serde_json::json!(true), &[]),
        ];

        let response = config_for_manifest(&manifest);

        assert_eq!(response.id, "web_discover");
        assert_eq!(response.configurable.len(), 2);
        assert_eq!(response.values["max_results"], serde_json::json!("5"));
        assert_eq!(response.values["safe_search"], serde_json::json!(true));
    }

    #[test]
    fn default_aliases_cover_known_tools_and_leave_unknown_empty() {
        assert_eq!(
            default_aliases("web_discover"),
            vec![
                "web_search".to_string(),
                "web_fetch".to_string(),
                "discover_web".to_string(),
                "search_web".to_string()
            ]
        );
        assert_eq!(default_aliases("unknown"), Vec::<String>::new());
    }

    fn manifest(id: &str, core: bool, binary: &str) -> ToolManifest {
        ToolManifest {
            id: id.to_string(),
            name: format!("{id} tool"),
            description: format!("{id} description"),
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
                binary: binary.to_string(),
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
            manifest_path: Path::new("tool.json").to_path_buf(),
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
