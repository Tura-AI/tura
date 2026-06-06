use super::types::ReadMediaArgs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

pub(super) fn expand_media_paths(
    args: &ReadMediaArgs,
    session_dir: &Path,
) -> Result<Vec<(String, PathBuf)>, String> {
    let mut expanded = Vec::new();
    for path in &args.paths {
        let resolved = resolve_media_path(path, session_dir);
        if resolved.is_dir() {
            let mut entries = std::fs::read_dir(&resolved)
                .map_err(|err| {
                    format!(
                        "failed to read media directory {}: {err}",
                        resolved.display()
                    )
                })?
                .filter_map(Result::ok)
                .map(|entry| entry.path())
                .filter(|path| path.is_file())
                .collect::<Vec<_>>();
            entries.sort_by_key(|path| {
                std::cmp::Reverse(
                    std::fs::metadata(path)
                        .and_then(|metadata| metadata.modified())
                        .ok(),
                )
            });
            for file in entries.into_iter().take(args.max_files) {
                expanded.push((display_input_path(&file, session_dir), file));
            }
        } else {
            expanded.push((path.to_string(), resolved));
        }
    }
    Ok(expanded)
}

fn display_input_path(path: &Path, session_dir: &Path) -> String {
    path.strip_prefix(session_dir)
        .map(|path| path.display().to_string())
        .unwrap_or_else(|_| path.display().to_string())
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

fn chrono_like_millis() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0)
}

pub(super) fn temp_work_dir(prefix: &str) -> PathBuf {
    let counter = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "{prefix}-{}-{}-{counter}",
        std::process::id(),
        chrono_like_millis()
    ))
}

pub(super) fn extension_lower(path: &Path) -> Option<String> {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_ascii_lowercase())
}

pub(super) fn media_type_for_path(path: &Path) -> &'static str {
    match extension_lower(path).as_deref() {
        Some("png" | "jpg" | "jpeg" | "webp" | "bmp") => "image",
        Some("pdf") => "pdf",
        Some("mp4" | "avi" | "mov" | "mkv" | "webm") => "video",
        Some("mp3" | "wav" | "m4a" | "aac" | "flac" | "ogg" | "opus") => "audio",
        _ => "document",
    }
}

pub(super) fn resolve_media_path(path: &str, session_dir: &Path) -> PathBuf {
    let candidate = PathBuf::from(path);
    if candidate.is_absolute() {
        candidate
    } else {
        session_dir.join(candidate)
    }
}

pub(super) fn workspace_relative_path(path: &str, session_dir: &Path) -> Option<PathBuf> {
    let resolved = resolve_media_path(path, session_dir);
    resolved
        .strip_prefix(session_dir)
        .ok()
        .map(Path::to_path_buf)
}

pub(super) fn truncate_chars(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    let head = max_chars / 2;
    let tail = max_chars.saturating_sub(head);
    let start = text.chars().take(head).collect::<String>();
    let end = text
        .chars()
        .rev()
        .take(tail)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<String>();
    format!("{start}\n...[read_media text truncated]...\n{end}")
}
