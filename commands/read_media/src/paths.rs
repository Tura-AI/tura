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
                (
                    std::cmp::Reverse(
                        std::fs::metadata(path)
                            .and_then(|metadata| metadata.modified())
                            .ok(),
                    ),
                    display_input_path(path, session_dir).replace('\\', "/"),
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

    if let Some(path) = command_venv_python() {
        return Some(path);
    }
    None
}

pub(super) fn command_venv_python() -> Option<PathBuf> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let venv_python = if cfg!(windows) {
        manifest_dir
            .join(".venv")
            .join("Scripts")
            .join("python.exe")
    } else {
        manifest_dir.join(".venv").join("bin").join("python")
    };
    venv_python.exists().then_some(venv_python)
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

#[cfg(test)]
mod tests {
    use super::{
        expand_media_paths, extension_lower, media_type_for_path, resolve_media_path,
        temp_work_dir, truncate_chars, workspace_relative_path,
    };
    use crate::types::ReadMediaArgs;
    use std::path::PathBuf;

    fn args(paths: Vec<String>) -> ReadMediaArgs {
        ReadMediaArgs {
            paths,
            include_text: true,
            max_text_chars: 40_000,
            max_visuals: 2,
            max_side: 256,
            max_files: 2,
            pdf_max_pages: 2,
            document_attachment_bytes: 1_000_000,
            audio_preview_bytes: 1_000_000,
        }
    }

    #[test]
    fn media_type_and_extension_are_case_insensitive() {
        assert_eq!(
            extension_lower(&PathBuf::from("PHOTO.JPEG")).as_deref(),
            Some("jpeg")
        );
        assert_eq!(media_type_for_path(&PathBuf::from("PHOTO.JPEG")), "image");
        assert_eq!(media_type_for_path(&PathBuf::from("clip.MP4")), "video");
        assert_eq!(media_type_for_path(&PathBuf::from("sound.OPUS")), "audio");
        assert_eq!(media_type_for_path(&PathBuf::from("notes.txt")), "document");
    }

    #[test]
    fn relative_and_absolute_paths_resolve_predictably() {
        let dir = tempfile::tempdir().expect("tempdir");
        let relative = resolve_media_path("a/b.txt", dir.path());
        assert_eq!(relative, dir.path().join("a/b.txt"));

        let absolute = dir.path().join("outside.txt");
        assert_eq!(
            resolve_media_path(&absolute.display().to_string(), dir.path()),
            absolute
        );
        assert_eq!(
            workspace_relative_path("a/b.txt", dir.path()).as_deref(),
            Some(std::path::Path::new("a/b.txt"))
        );
        assert_eq!(
            workspace_relative_path(&absolute.display().to_string(), dir.path()),
            Some(PathBuf::from("outside.txt"))
        );
    }

    #[test]
    fn directory_expansion_respects_max_files_and_keeps_display_paths_relative() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(dir.path().join("a.txt"), "a").expect("write a");
        std::fs::write(dir.path().join("b.txt"), "b").expect("write b");
        std::fs::write(dir.path().join("c.txt"), "c").expect("write c");

        let expanded = expand_media_paths(&args(vec![".".to_string()]), dir.path())
            .expect("directory expansion should succeed");

        assert_eq!(expanded.len(), 2);
        assert!(expanded.iter().all(|(display, path)| {
            !display.contains(dir.path().to_string_lossy().as_ref()) && path.is_absolute()
        }));
    }

    #[test]
    fn truncate_chars_preserves_start_and_end_on_unicode_boundaries() {
        let text = "alpha😀beta😀gamma😀delta";
        let truncated = truncate_chars(text, 8);

        assert!(truncated.contains("[read_media text truncated]"));
        assert!(truncated.starts_with("alph"));
        assert!(truncated.ends_with("elta"));
    }

    #[test]
    fn temp_work_dir_names_are_unique_for_same_prefix() {
        let one = temp_work_dir("read-media-test");
        let two = temp_work_dir("read-media-test");

        assert_ne!(one, two);
        assert!(one
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with("read-media-test-")));
    }
}
