use std::fs;
use std::io;
use std::path::{Path as FsPath, PathBuf};

pub(crate) fn provider_config_path() -> PathBuf {
    std::env::var("TURA_PROVIDER_CONFIG")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            std::env::current_dir()
                .unwrap_or_default()
                .join("crates")
                .join("provider")
                .join("config")
                .join("provider_config.json")
        })
}

pub(super) fn config_value(key: &str) -> Option<String> {
    std::env::var(key)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            tura_llm_rust::TuraConfig::default()
                .get(key)
                .filter(|value| !value.trim().is_empty())
        })
}

pub(super) fn upsert_env_value(path: &FsPath, key: &str, value: &str) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let mut lines = fs::read_to_string(path)
        .map(|content| content.lines().map(ToString::to_string).collect::<Vec<_>>())
        .unwrap_or_default();
    let prefix = format!("{key}=");
    let next = format!("{key}={}", quote_env_value(value));
    let mut replaced = false;
    for line in &mut lines {
        if line.trim_start().starts_with(&prefix) {
            *line = next.clone();
            replaced = true;
            break;
        }
    }
    if !replaced {
        lines.push(next);
    }
    let mut content = lines.join("\n");
    content.push('\n');
    fs::write(path, content)
}

fn quote_env_value(value: &str) -> String {
    let escaped = value.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{escaped}\"")
}
