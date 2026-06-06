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
    repo_root().and_then(|root| {
        [
            root.join("target").join("release").join(&exe_name),
            root.join("target").join("debug").join(&exe_name),
        ]
        .into_iter()
        .find(|candidate| candidate.exists())
    })
}

pub fn repo_root() -> Option<PathBuf> {
    std::env::current_dir().ok().and_then(|current| {
        current
            .ancestors()
            .find(|path| path.join("Cargo.toml").exists() && path.join("crates").is_dir())
            .map(std::path::Path::to_path_buf)
    })
}
