use std::path::PathBuf;

use crate::registry::{self, CommandExecution, CommandManifest};

#[derive(Clone, Debug)]
pub struct ExternalCommandMetadata {
    pub command_id: String,
    pub binary_name: String,
    pub binary_path: Option<PathBuf>,
    pub manifest: CommandManifest,
}

pub fn metadata_for(command_id: &str) -> Option<ExternalCommandMetadata> {
    let command_id = crate::commands::canonical_command(command_id);
    let root = repo_root()?;
    metadata_for_in_root(&root, &command_id)
}

pub fn metadata_for_in_root(
    repo_root: &std::path::Path,
    command_id: &str,
) -> Option<ExternalCommandMetadata> {
    let command_id = crate::commands::canonical_command(command_id);
    let manifest = registry::manifest_for(repo_root, &command_id)?;
    if manifest.core || !matches!(manifest.execution, CommandExecution::OneShot) {
        return None;
    }
    let binary_name = manifest.binary.as_deref()?.trim();
    if binary_name.is_empty() {
        return None;
    }
    Some(ExternalCommandMetadata {
        command_id,
        binary_name: binary_name.to_string(),
        binary_path: resolve_binary_in_root(repo_root, binary_name),
        manifest,
    })
}

pub fn resolve_binary(binary_name: &str) -> Option<PathBuf> {
    let root = repo_root()?;
    resolve_binary_in_root(&root, binary_name)
}

pub fn resolve_binary_in_root(repo_root: &std::path::Path, binary_name: &str) -> Option<PathBuf> {
    let override_var = format!(
        "TURA_{}_BIN",
        binary_name
            .trim_start_matches("tura-command-")
            .replace('-', "_")
            .to_ascii_uppercase()
    );
    if let Ok(path) = std::env::var(override_var) {
        let path = PathBuf::from(path);
        if path.exists() {
            return Some(path);
        }
    }
    let exe_name = if cfg!(windows) {
        format!("{binary_name}.exe")
    } else {
        binary_name.to_string()
    };
    binary_candidates(repo_root, &exe_name)
        .into_iter()
        .find(|candidate| candidate.exists())
}

/// Candidate locations for a packaged command binary, in priority order.
///
/// Packaged builds place the command binaries next to the gateway executable
/// (e.g. `target/release/` or `bin/`) and set `TURA_PROJECT_ROOT`, so those are
/// checked before the source-tree `target/{release,debug}` layout used in dev.
pub fn binary_candidates(repo_root: &std::path::Path, exe_name: &str) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Ok(current_exe) = std::env::current_exe() {
        if let Some(dir) = current_exe.parent() {
            candidates.push(dir.join(exe_name));
        }
    }
    if let Some(root) = std::env::var_os("TURA_PROJECT_ROOT").map(PathBuf::from) {
        candidates.push(root.join("bin").join(exe_name));
        candidates.push(root.join(exe_name));
        candidates.push(root.join("target").join("release").join(exe_name));
        candidates.push(root.join("target").join("debug").join(exe_name));
    }
    candidates.push(repo_root.join("bin").join(exe_name));
    candidates.push(repo_root.join(exe_name));
    candidates.push(repo_root.join("target").join("release").join(exe_name));
    candidates.push(repo_root.join("target").join("debug").join(exe_name));
    candidates
}

pub fn repo_root() -> Option<PathBuf> {
    if let Some(root) = std::env::var_os("TURA_PROJECT_ROOT").map(PathBuf::from) {
        if root.exists() {
            return Some(root);
        }
    }
    std::env::current_dir().ok().and_then(|current| {
        current
            .ancestors()
            .find(|path| path.join("Cargo.toml").exists() && path.join("crates").is_dir())
            .map(std::path::Path::to_path_buf)
    })
}

#[cfg(test)]
mod tests {
    use super::{binary_candidates, metadata_for, metadata_for_in_root, repo_root};
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_root(name: &str) -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("tura-external-client-{name}-{suffix}"))
    }

    fn write_manifest(root: &Path, id: &str, binary: &str) {
        let directory = root.join("commands").join(id);
        fs::create_dir_all(&directory).expect("create manifest directory");
        fs::write(
            directory.join("command.toml"),
            format!(
                r#"id = "{id}"
name = "{id}"
description = "{id}"
core = false
category = "test"
execution = "one_shot"
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
"#
            ),
        )
        .expect("write manifest");
    }

    #[test]
    fn metadata_for_known_external_commands_uses_registered_binary_names() {
        let read_media = metadata_for("read_media").expect("read_media metadata");
        assert_eq!(read_media.command_id, "read_media");
        assert_eq!(read_media.binary_name, "tura-command-read-media");
        assert_eq!(read_media.manifest.default_timeout_ms, 60_000);

        let generate_media = metadata_for("generate_media").expect("generate_media metadata");
        assert_eq!(generate_media.command_id, "generate_media");
        assert_eq!(generate_media.binary_name, "tura-command-generate-media");

        let web_discover = metadata_for("web_discover").expect("web_discover metadata");
        assert_eq!(web_discover.command_id, "web_discover");
        assert_eq!(web_discover.binary_name, "tura-command-web-discover");

        assert!(metadata_for("shell_command").is_none());
        assert!(metadata_for("").is_none());
    }

    #[test]
    fn binary_candidates_include_packaged_and_cargo_target_locations() {
        let exe_name = if cfg!(windows) {
            "tura-command-read-media.exe"
        } else {
            "tura-command-read-media"
        };
        let root = repo_root().expect("repo root");
        let candidates = binary_candidates(&root, exe_name);

        assert!(!candidates.is_empty());
        assert!(candidates.iter().any(|path| path.ends_with(exe_name)));
        assert!(candidates
            .iter()
            .any(|path| path.to_string_lossy().contains("target")));
    }

    #[test]
    fn binary_candidates_keep_release_before_debug_for_project_root_entries() {
        let root = repo_root().expect("repo root");
        let candidates = binary_candidates(&root, "tura-command-web-discover");
        let release_index = candidates
            .iter()
            .position(|path| path.ends_with("target/release/tura-command-web-discover"));
        let debug_index = candidates
            .iter()
            .position(|path| path.ends_with("target/debug/tura-command-web-discover"));

        if let (Some(release_index), Some(debug_index)) = (release_index, debug_index) {
            assert!(
                release_index < debug_index,
                "release binary must be preferred before debug: {candidates:?}"
            );
        }
    }

    #[test]
    fn binary_metadata_does_not_require_binary_to_exist() {
        let metadata = metadata_for("web_discover").expect("web_discover metadata");
        assert_eq!(metadata.command_id, "web_discover");
        assert_eq!(metadata.binary_name, "tura-command-web-discover");
        if let Some(path) = metadata.binary_path {
            assert!(
                path.exists(),
                "resolved binary must exist: {}",
                path.display()
            );
        }
    }

    #[test]
    fn metadata_for_in_root_reads_workspace_command_registry_binary() {
        let root = temp_root("manifest");
        write_manifest(&root, "image_tool", "tura-command-image-tool");

        let metadata = metadata_for_in_root(&root, "image_tool").expect("metadata");

        assert_eq!(metadata.command_id, "image_tool");
        assert_eq!(metadata.binary_name, "tura-command-image-tool");
        assert!(metadata.binary_path.is_none());
        assert!(metadata.manifest.supports_macro_command);
        assert_eq!(metadata.manifest.default_timeout_ms, 1234);

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn metadata_for_in_root_resolves_registered_binary_from_target_release_before_debug() {
        let root = temp_root("binary");
        write_manifest(&root, "image_tool", "tura-command-image-tool");
        let exe_name = if cfg!(windows) {
            "tura-command-image-tool.exe"
        } else {
            "tura-command-image-tool"
        };
        let release_binary = root.join("target").join("release").join(exe_name);
        fs::create_dir_all(release_binary.parent().expect("release parent"))
            .expect("create release directory");
        fs::write(&release_binary, b"binary").expect("write release binary");

        let metadata = metadata_for_in_root(&root, "image_tool").expect("metadata");

        assert_eq!(
            metadata.binary_path.as_deref(),
            Some(release_binary.as_path())
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn repo_root_finds_workspace_from_current_directory() {
        let root = repo_root().expect("repo root should be discoverable from tests");
        assert!(root.join("Cargo.toml").exists(), "{}", root.display());
        assert!(root.join("crates").is_dir(), "{}", root.display());
    }
}
