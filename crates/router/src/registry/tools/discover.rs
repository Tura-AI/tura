use super::manifest::ToolManifest;
use std::fs;
use std::path::{Path, PathBuf};

pub fn discover_manifests(repo_root: &Path) -> Vec<ToolManifest> {
    let mut manifests = Vec::new();
    manifests.extend(discover_under(
        &repo_root
            .join("crates")
            .join("tools")
            .join("src")
            .join("commands"),
    ));
    manifests.extend(discover_under(&repo_root.join("commands")));
    manifests.sort_by(|left, right| left.id.cmp(&right.id));
    manifests
}

fn discover_under(root: &Path) -> Vec<ToolManifest> {
    let Ok(entries) = fs::read_dir(root) else {
        return Vec::new();
    };
    entries
        .flatten()
        .filter_map(|entry| read_manifest(entry.path().join("command.toml")))
        .collect()
}

fn read_manifest(path: PathBuf) -> Option<ToolManifest> {
    let content = fs::read_to_string(&path).ok()?;
    let mut manifest: ToolManifest = toml::from_str(&content).ok()?;
    validate_manifest(&manifest).ok()?;
    manifest.manifest_path = path;
    Some(manifest)
}

fn validate_manifest(manifest: &ToolManifest) -> Result<(), String> {
    if manifest.id.trim().is_empty() {
        return Err("tool manifest id is required".to_string());
    }
    if !matches!(
        manifest.execution.as_str(),
        "in_process" | "one_shot" | "persistent"
    ) {
        return Err(format!("invalid execution for {}", manifest.id));
    }
    for entry in &manifest.configurable {
        if !matches!(entry.value_type.as_str(), "enum" | "string" | "boolean") {
            return Err(format!("invalid configurable type for {}", entry.key));
        }
        if entry.value_type == "enum" && entry.enum_values.is_empty() {
            return Err(format!(
                "enum configurable {} must include values",
                entry.key
            ));
        }
    }
    Ok(())
}
