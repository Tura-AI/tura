pub const POLICY: &str = include_str!("../policy.toml");

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReadMediaPolicy {
    pub max_text_chars: usize,
    pub max_visuals: usize,
    pub max_side: u32,
    pub max_files: usize,
    pub pdf_default_pages: usize,
    pub document_attachment_bytes: u64,
    pub audio_preview_bytes: u64,
}

impl ReadMediaPolicy {
    pub fn from_policy_toml(policy: &str) -> Self {
        let media_compression =
            configurable_default(policy, "media_compression").unwrap_or("balanced");
        let pdf_default_pages =
            configurable_default(policy, "pdf_default_pages").unwrap_or("standard");
        let directory_default_files =
            configurable_default(policy, "directory_default_files").unwrap_or("standard");
        let document_attachment_size =
            configurable_default(policy, "document_attachment_size").unwrap_or("standard");
        let audio_preview_size =
            configurable_default(policy, "audio_preview_size").unwrap_or("standard");

        let (max_text_chars, max_visuals, max_side) = match media_compression {
            "compact" => (20_000, 4, 384),
            "detailed" => (80_000, 10, 768),
            _ => (40_000, 6, 512),
        };
        let pdf_default_pages = match pdf_default_pages {
            "first" => 1,
            "extended" => 10,
            _ => 5,
        };
        let max_files = match directory_default_files {
            "few" => 8,
            "many" => 50,
            _ => 20,
        };
        let document_attachment_bytes = match document_attachment_size {
            "small" => 500_000,
            "large" => 2_000_000,
            _ => 1_000_000,
        };
        let audio_preview_bytes = match audio_preview_size {
            "small" => 500_000,
            "large" => 2_000_000,
            _ => 1_000_000,
        };

        Self {
            max_text_chars,
            max_visuals,
            max_side,
            max_files,
            pdf_default_pages,
            document_attachment_bytes,
            audio_preview_bytes,
        }
    }
}

pub fn read_media_policy() -> ReadMediaPolicy {
    ReadMediaPolicy::from_policy_toml(POLICY)
}

fn configurable_default<'a>(policy: &'a str, key: &str) -> Option<&'a str> {
    let mut in_configurable = false;
    for line in policy.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            in_configurable = trimmed == "[configurable]";
            continue;
        }
        if !in_configurable || trimmed.starts_with('#') {
            continue;
        }
        let Some((name, value)) = trimmed.split_once('=') else {
            continue;
        };
        if name.trim() != key {
            continue;
        }
        let (_, value) = value.split_once("default")?;
        let (_, value) = value.split_once('"')?;
        let (value, _) = value.split_once('"')?;
        return Some(value);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::{configurable_default, ReadMediaPolicy, POLICY};

    #[test]
    fn policy_exposes_bounded_configurable_defaults() {
        assert_eq!(
            configurable_default(POLICY, "media_compression"),
            Some("balanced")
        );
        assert!(POLICY.contains(r#"enum = ["compact", "balanced", "detailed"]"#));
        assert_eq!(
            ReadMediaPolicy::from_policy_toml(POLICY).pdf_default_pages,
            5
        );
    }
}
