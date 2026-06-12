use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct ExternalCommandMetadata {
    pub command_id: String,
    pub binary_name: String,
    pub binary_path: Option<PathBuf>,
}

pub fn metadata_for(command_id: &str) -> Option<ExternalCommandMetadata> {
    let binary_name = match command_id {
        "read_media" => "tura-command-read-media",
        "web_discover" => "tura-command-web-discover",
        _ => return None,
    };
    Some(ExternalCommandMetadata {
        command_id: command_id.to_string(),
        binary_name: binary_name.to_string(),
        binary_path: resolve_binary(binary_name),
    })
}

pub fn resolve_binary(binary_name: &str) -> Option<PathBuf> {
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
    binary_candidates(&exe_name)
        .into_iter()
        .find(|candidate| candidate.exists())
}

/// Candidate locations for a packaged command binary, in priority order.
///
/// Packaged builds place the command binaries next to the gateway executable
/// (e.g. `target/release/` or `bin/`) and set `TURA_PROJECT_ROOT`, so those are
/// checked before the source-tree `target/{release,debug}` layout used in dev.
fn binary_candidates(exe_name: &str) -> Vec<PathBuf> {
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
    if let Some(root) = repo_root() {
        candidates.push(root.join("target").join("release").join(exe_name));
        candidates.push(root.join("target").join("debug").join(exe_name));
    }
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
    use super::{binary_candidates, metadata_for, repo_root};

    #[test]
    fn metadata_for_known_external_commands_uses_packaged_binary_names() {
        let read_media = metadata_for("read_media").expect("read_media metadata");
        assert_eq!(read_media.command_id, "read_media");
        assert_eq!(read_media.binary_name, "tura-command-read-media");

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
        let candidates = binary_candidates(exe_name);

        assert!(!candidates.is_empty());
        assert!(candidates.iter().any(|path| path.ends_with(exe_name)));
        assert!(candidates
            .iter()
            .any(|path| path.to_string_lossy().contains("target")));
    }

    #[test]
    fn binary_candidates_keep_release_before_debug_for_project_root_entries() {
        let candidates = binary_candidates("tura-command-web-discover");
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
    fn repo_root_finds_workspace_from_current_directory() {
        let root = repo_root().expect("repo root should be discoverable from tests");
        assert!(root.join("Cargo.toml").exists(), "{}", root.display());
        assert!(root.join("crates").is_dir(), "{}", root.display());
    }
}
