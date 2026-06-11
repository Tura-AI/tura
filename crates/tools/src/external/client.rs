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
