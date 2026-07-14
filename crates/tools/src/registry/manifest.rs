use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CommandExecution {
    InProcess,
    OneShot,
    Persistent,
}

#[derive(Clone, Debug)]
pub struct CommandManifest {
    pub id: String,
    pub core: bool,
    pub execution: CommandExecution,
    pub binary: Option<String>,
    pub supports_macro_command: bool,
    pub mutating: bool,
    pub default_timeout_ms: u64,
    pub max_timeout_ms: u64,
    pub manifest_path: PathBuf,
}

impl CommandManifest {
    pub fn core(id: &str) -> Self {
        Self {
            id: id.to_string(),
            core: true,
            execution: CommandExecution::InProcess,
            binary: None,
            supports_macro_command: false,
            mutating: false,
            default_timeout_ms: 15_000,
            max_timeout_ms: 300_000,
            manifest_path: PathBuf::new(),
        }
    }

    pub fn external(id: &str, binary: &str) -> Self {
        Self {
            id: id.to_string(),
            core: false,
            execution: CommandExecution::OneShot,
            binary: Some(binary.to_string()),
            supports_macro_command: false,
            mutating: false,
            default_timeout_ms: 15_000,
            max_timeout_ms: 300_000,
            manifest_path: PathBuf::new(),
        }
    }

    pub fn is_external_cli(&self) -> bool {
        !self.core
            && matches!(
                self.execution,
                CommandExecution::OneShot | CommandExecution::Persistent
            )
            && self
                .binary
                .as_deref()
                .is_some_and(|binary| !binary.trim().is_empty())
    }
}

#[derive(Clone, Debug, Deserialize)]
struct RawCommandManifest {
    id: String,
    #[serde(default)]
    core: bool,
    execution: String,
    #[serde(default)]
    supports_macro_command: bool,
    #[serde(default)]
    mutating: bool,
    runtime: RawRuntimeSection,
    limits: RawLimitsSection,
}

#[derive(Clone, Debug, Default, Deserialize)]
struct RawRuntimeSection {
    #[serde(default)]
    binary: String,
}

#[derive(Clone, Debug, Deserialize)]
struct RawLimitsSection {
    default_timeout_ms: u64,
    max_timeout_ms: u64,
}

pub fn discover_manifests(repo_root: &Path) -> Vec<CommandManifest> {
    let mut manifests = BTreeMap::<String, CommandManifest>::new();
    for directory in command_registry_directories(repo_root) {
        let Ok(entries) = std::fs::read_dir(&directory) else {
            continue;
        };
        for entry in entries.flatten() {
            if let Some(manifest) = read_manifest(&entry.path().join("command.toml")) {
                manifests.entry(manifest.id.clone()).or_insert(manifest);
            }
        }
    }
    manifests.into_values().collect()
}

pub fn manifest_for(repo_root: &Path, command_id: &str) -> Option<CommandManifest> {
    let command_id = command_id.trim();
    if command_id.is_empty() {
        return None;
    }
    command_registry_directories(repo_root)
        .into_iter()
        .map(|directory| directory.join(command_id).join("command.toml"))
        .find_map(|path| read_manifest(&path))
}

pub fn command_registry_directories(repo_root: &Path) -> Vec<PathBuf> {
    vec![
        repo_root.join("commands"),
        repo_root
            .join("crates")
            .join("tools")
            .join("src")
            .join("commands"),
    ]
}

fn read_manifest(path: &Path) -> Option<CommandManifest> {
    let content = std::fs::read_to_string(path).ok()?;
    let raw: RawCommandManifest = toml::from_str(&content).ok()?;
    let execution = match raw.execution.as_str() {
        "in_process" => CommandExecution::InProcess,
        "one_shot" => CommandExecution::OneShot,
        "persistent" => CommandExecution::Persistent,
        _ => return None,
    };
    Some(CommandManifest {
        id: raw.id,
        core: raw.core,
        execution,
        binary: (!raw.runtime.binary.trim().is_empty()).then_some(raw.runtime.binary),
        supports_macro_command: raw.supports_macro_command,
        mutating: raw.mutating,
        default_timeout_ms: raw.limits.default_timeout_ms,
        max_timeout_ms: raw.limits.max_timeout_ms,
        manifest_path: path.to_path_buf(),
    })
}

#[cfg(test)]
mod tests {
    use super::{command_registry_directories, discover_manifests, manifest_for, CommandExecution};
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_root(name: &str) -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("tura-tools-command-registry-{name}-{suffix}"))
    }

    fn write_manifest(root: &Path, relative_dir: &str, id: &str, binary: &str, core: bool) {
        let directory = root.join(relative_dir).join(id);
        fs::create_dir_all(&directory).expect("create manifest directory");
        fs::write(
            directory.join("command.toml"),
            format!(
                r#"id = "{id}"
name = "{id}"
description = "{id}"
core = {core}
category = "test"
execution = "{execution}"
state_machine = "default_command"
supports_macro_command = true
mutating = false
network = false

[runtime]
binary = "{binary}"
entry = ""
language = "rust"

[limits]
default_timeout_ms = 1234
max_timeout_ms = 5678

[paths]
prompt = "prompt.md"
schema = "schema.json"
policy = "policy.toml"
"#,
                execution = if core { "in_process" } else { "one_shot" }
            ),
        )
        .expect("write manifest");
    }

    #[test]
    fn registry_directories_prioritize_workspace_commands_before_core_commands() {
        let root = temp_root("directories");
        let directories = command_registry_directories(&root);

        assert_eq!(directories[0], root.join("commands"));
        assert_eq!(
            directories[1],
            root.join("crates")
                .join("tools")
                .join("src")
                .join("commands")
        );
    }

    #[test]
    fn manifest_for_reads_external_cli_registration_from_workspace_commands() {
        let root = temp_root("external");
        write_manifest(
            &root,
            "commands",
            "image_tool",
            "tura-command-image-tool",
            false,
        );

        let manifest = manifest_for(&root, "image_tool").expect("manifest");

        assert_eq!(manifest.id, "image_tool");
        assert!(!manifest.core);
        assert_eq!(manifest.execution, CommandExecution::OneShot);
        assert_eq!(manifest.binary.as_deref(), Some("tura-command-image-tool"));
        assert!(manifest.supports_macro_command);
        assert!(!manifest.mutating);
        assert_eq!(manifest.default_timeout_ms, 1234);
        assert_eq!(manifest.max_timeout_ms, 5678);
        assert!(manifest.manifest_path.ends_with("command.toml"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn discover_manifests_deduplicates_with_workspace_registration_first() {
        let root = temp_root("discover");
        write_manifest(&root, "commands", "dupe", "workspace-binary", false);
        write_manifest(
            &root,
            "crates/tools/src/commands",
            "dupe",
            "core-binary",
            false,
        );

        let manifests = discover_manifests(&root);
        let manifest = manifests
            .iter()
            .find(|manifest| manifest.id == "dupe")
            .expect("dupe manifest");

        assert_eq!(manifest.binary.as_deref(), Some("workspace-binary"));

        let _ = fs::remove_dir_all(root);
    }
}
