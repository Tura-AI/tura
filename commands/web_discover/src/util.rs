use regex::Regex;
use serde_json::Value;
use std::path::{Path, PathBuf};

pub(super) fn split_cli_words(input: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut current = String::new();
    let mut quote = None;
    for ch in input.chars() {
        match (quote, ch) {
            (Some(q), c) if c == q => quote = None,
            (None, '"' | '\'') => quote = Some(ch),
            (None, c) if c.is_whitespace() => {
                if !current.is_empty() {
                    words.push(std::mem::take(&mut current));
                }
            }
            _ => current.push(ch),
        }
    }
    if !current.is_empty() {
        words.push(current);
    }
    words
}

pub(super) fn string_field(value: &Value, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(Value::as_str))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

pub(super) fn string_field_at(value: &Value, paths: &[&[&str]]) -> Option<String> {
    paths.iter().find_map(|path| {
        let mut current = value;
        for key in *path {
            current = current.get(*key)?;
        }
        current
            .as_str()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToString::to_string)
    })
}

pub(super) fn u64_field(value: &Value, keys: &[&str]) -> Option<u64> {
    keys.iter().find_map(|key| {
        value
            .get(*key)
            .and_then(|value| value.as_u64().or_else(|| value.as_str()?.parse().ok()))
    })
}

pub(super) fn clean_text(value: &str) -> String {
    let without_tags = Regex::new("(?is)<[^>]+>")
        .ok()
        .map(|re| re.replace_all(value, " ").to_string())
        .unwrap_or_else(|| value.to_string());
    html_unescape(&without_tags)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

pub(super) fn html_unescape(value: &str) -> String {
    value
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#x27;", "'")
        .replace("&#39;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&nbsp;", " ")
}

pub(super) fn json_unescape(value: &str) -> String {
    serde_json::from_str::<String>(&format!("\"{value}\""))
        .unwrap_or_else(|_| value.replace("\\/", "/"))
}

pub(super) fn percent_decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0usize;
    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            if let Ok(hex) = std::str::from_utf8(&bytes[index + 1..index + 3]) {
                if let Ok(byte) = u8::from_str_radix(hex, 16) {
                    out.push(byte);
                    index += 3;
                    continue;
                }
            }
        }
        out.push(if bytes[index] == b'+' {
            b' '
        } else {
            bytes[index]
        });
        index += 1;
    }
    String::from_utf8_lossy(&out).to_string()
}

pub(super) fn safe_filename(value: &str) -> String {
    let cleaned = value
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '-' })
        .collect::<String>();
    cleaned
        .split('-')
        .filter(|part| !part.is_empty())
        .take(8)
        .collect::<Vec<_>>()
        .join("-")
        .chars()
        .take(80)
        .collect::<String>()
        .if_empty("result")
}

pub(super) trait EmptyDefault {
    fn if_empty(self, fallback: &str) -> String;
}

impl EmptyDefault for String {
    fn if_empty(self, fallback: &str) -> String {
        if self.is_empty() {
            fallback.to_string()
        } else {
            self
        }
    }
}

pub(super) fn extension_from_url(url: &str) -> Option<&'static str> {
    let lower = url.to_ascii_lowercase();
    for (needle, extension) in [
        (".tar.gz", "tar.gz"),
        (".jpeg", "jpg"),
        (".jpg", "jpg"),
        (".png", "png"),
        (".webp", "webp"),
        (".gif", "gif"),
        (".avif", "avif"),
        (".svg", "svg"),
        (".zip", "zip"),
        (".glb", "glb"),
        (".gltf", "gltf"),
        (".obj", "obj"),
        (".fbx", "fbx"),
        (".blend", "blend"),
        (".stl", "stl"),
        (".usdz", "usdz"),
        (".dae", "dae"),
        (".hdr", "hdr"),
        (".exr", "exr"),
        (".ktx2", "ktx2"),
        (".dds", "dds"),
        (".tga", "tga"),
        (".wav", "wav"),
        (".mp3", "mp3"),
        (".ogg", "ogg"),
        (".flac", "flac"),
        (".m4a", "m4a"),
        (".aac", "aac"),
        (".opus", "opus"),
        (".glsl", "glsl"),
        (".wgsl", "wgsl"),
        (".vert", "vert"),
        (".frag", "frag"),
        (".hlsl", "hlsl"),
        (".tsx", "tsx"),
        (".ts", "ts"),
        (".js", "js"),
    ] {
        if lower.contains(needle) {
            return Some(extension);
        }
    }
    None
}

pub(super) fn content_type_for_path(path: &Path, kind: &str) -> &'static str {
    match path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "webp" => "image/webp",
        "gif" => "image/gif",
        "avif" => "image/avif",
        "svg" => "image/svg+xml",
        "mp4" => "video/mp4",
        "webm" => "video/webm",
        "mp3" => "audio/mpeg",
        "m4a" => "audio/mp4",
        "wav" => "audio/wav",
        "ogg" => "audio/ogg",
        "flac" => "audio/flac",
        "aac" => "audio/aac",
        "opus" => "audio/opus",
        "zip" => "application/zip",
        "glb" => "model/gltf-binary",
        "gltf" => "model/gltf+json",
        "obj" => "model/obj",
        "fbx" | "blend" | "stl" | "usdz" | "dae" => "model/3d",
        "hdr" | "exr" | "ktx2" | "dds" | "tga" => "application/octet-stream",
        "glsl" | "wgsl" | "vert" | "frag" | "hlsl" => "text/plain",
        "js" | "ts" | "tsx" => "text/plain",
        _ if kind == "website" => "text/markdown",
        _ => "application/octet-stream",
    }
}

pub(super) fn snapshot_files(path: &Path) -> Vec<PathBuf> {
    std::fs::read_dir(path)
        .ok()
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_file())
        .collect()
}

pub(super) fn find_on_path(exe: &str) -> Option<PathBuf> {
    let path_var = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path_var) {
        let candidate = dir.join(if cfg!(windows) {
            format!("{exe}.exe")
        } else {
            exe.to_string()
        });
        if candidate.exists() {
            return Some(candidate);
        }
    }
    None
}

pub(super) fn command_local_python(primary_env: &str) -> Option<PathBuf> {
    command_configured_python(primary_env).or_else(|| {
        find_on_path("python3")
            .or_else(|| find_on_path("python"))
            .or_else(|| find_on_path("py"))
    })
}

pub(super) fn command_configured_python(primary_env: &str) -> Option<PathBuf> {
    for env_name in [primary_env, "TURA_COMMAND_PYTHON"] {
        if let Ok(value) = std::env::var(env_name) {
            let path = PathBuf::from(value.trim());
            if path.exists() {
                return Some(path);
            }
        }
    }

    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let venv_python = if cfg!(windows) {
        manifest_dir
            .join(".venv")
            .join("Scripts")
            .join("python.exe")
    } else {
        manifest_dir.join(".venv").join("bin").join("python")
    };
    if venv_python.exists() {
        return Some(venv_python);
    }
    None
}

pub(super) fn command_local_executable(exe: &str) -> Option<PathBuf> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let bin_dir = if cfg!(windows) {
        manifest_dir.join(".venv").join("Scripts")
    } else {
        manifest_dir.join(".venv").join("bin")
    };
    let names = if cfg!(windows) {
        vec![format!("{exe}.exe"), format!("{exe}.cmd"), exe.to_string()]
    } else {
        vec![exe.to_string()]
    };
    names
        .into_iter()
        .map(|name| bin_dir.join(name))
        .find(|candidate| candidate.exists())
}

pub(super) fn env_value(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .or_else(|| {
            tura_llm_rust::TuraConfig::default()
                .get(name)
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
        })
}

pub(super) fn truncate_chars(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    text.chars().take(max_chars).collect::<String>()
}

pub(super) fn middle_truncate_chars(text: &str, max_chars: usize) -> String {
    let char_count = text.chars().count();
    if char_count <= max_chars {
        return text.to_string();
    }
    let marker = "\n...[truncated]...\n";
    let marker_len = marker.chars().count();
    if max_chars <= marker_len {
        return text.chars().take(max_chars).collect();
    }
    let keep = max_chars - marker_len;
    let head = keep / 2;
    let tail = keep - head;
    let start = text.chars().take(head).collect::<String>();
    let end = text
        .chars()
        .rev()
        .take(tail)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<String>();
    format!("{start}{marker}{end}")
}
