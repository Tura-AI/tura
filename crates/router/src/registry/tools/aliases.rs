use super::manifest::ToolManifest;

pub fn resolve_alias<'a>(value: &str, manifests: impl Iterator<Item = &'a ToolManifest>) -> String {
    let normalized = value.trim().to_ascii_lowercase().replace('-', "_");
    match normalized.as_str() {
        "shell" | "shll" | "shall" => "shell_command".to_string(),
        "image_gen" | "generate_image" | "text_to_image" | "t2i" => "generate_media".to_string(),
        "view_media" | "inspect_media" => "read_media".to_string(),
        "web_search" | "web_fetch" | "discover_web" | "search_web" => "web_discover".to_string(),
        other => manifests
            .filter(|manifest| manifest.id == other)
            .map(|manifest| manifest.id.clone())
            .next()
            .unwrap_or_else(|| other.to_string()),
    }
}
