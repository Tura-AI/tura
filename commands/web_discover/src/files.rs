use super::types::WebDiscoverArgs;
use super::util::content_type_for_path;
use serde_json::{json, Value};
use std::io::Write;
use std::path::{Path, PathBuf};

pub(super) const DEFAULT_DOWNLOAD_DIR: &str = ".tura/media";

pub(super) fn downloaded_file_value(
    path: &Path,
    session_dir: &Path,
    source_url: &str,
    source_page_url: Option<&str>,
    kind: &str,
) -> Value {
    let metadata = std::fs::metadata(path).ok();
    json!({
        "path": relative_or_display(path, session_dir),
        "absolute_path": path.display().to_string(),
        "name": path.file_name().and_then(|v| v.to_str()).unwrap_or_default(),
        "url": source_url,
        "source_page_url": source_page_url,
        "file_type": kind,
        "content_type": content_type_for_path(path, kind),
        "size": metadata.map(|m| m.len()).unwrap_or(0),
    })
}

pub(super) fn resolve_download_dir(
    args: &WebDiscoverArgs,
    session_dir: &Path,
) -> Result<PathBuf, String> {
    let raw = args.download_dir.as_deref().unwrap_or(DEFAULT_DOWNLOAD_DIR);
    let path = PathBuf::from(raw);
    let resolved = if path.is_absolute() {
        path
    } else {
        session_dir.join(path)
    };
    Ok(resolved)
}

pub(super) fn download_dir_arg_or_default(args: &WebDiscoverArgs) -> &str {
    args.download_dir.as_deref().unwrap_or(DEFAULT_DOWNLOAD_DIR)
}

pub(super) fn relative_or_display(path: &Path, session_dir: &Path) -> String {
    path.strip_prefix(session_dir)
        .map(|path| path.display().to_string())
        .unwrap_or_else(|_| path.display().to_string())
}

pub(super) fn workspace_relative_path(path: &str, session_dir: &Path) -> Option<PathBuf> {
    let path = PathBuf::from(path);
    let resolved = if path.is_absolute() {
        path
    } else {
        session_dir.join(path)
    };
    resolved
        .strip_prefix(session_dir)
        .ok()
        .map(Path::to_path_buf)
}

pub(super) fn web_discover_write_scope(args: &WebDiscoverArgs, relative_dir: &Path) -> String {
    format!(
        "{}/.web_discover-{}-{}",
        relative_dir.display(),
        args.kind,
        stable_hash(&args.query)
    )
}

pub(super) fn stable_hash(value: &str) -> u64 {
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in value.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

pub(super) fn write_unique_download(
    output_dir: &Path,
    base_name: &str,
    extension: &str,
    bytes: &[u8],
) -> Result<PathBuf, String> {
    for copy in 0..1000 {
        let suffix = if copy == 0 {
            String::new()
        } else {
            format!("-{copy}")
        };
        let path = output_dir.join(format!("{base_name}{suffix}.{extension}"));
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(mut file) => {
                file.write_all(bytes)
                    .map_err(|err| format!("failed to write download: {err}"))?;
                return Ok(path);
            }
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(err) => return Err(format!("failed to write download: {err}")),
        }
    }
    Err(format!(
        "failed to choose unique download name for {base_name}.{extension}"
    ))
}

pub(super) fn move_unique_download(
    source: &Path,
    output_dir: &Path,
    base_name: &str,
    extension: &str,
) -> Result<PathBuf, String> {
    for copy in 0..1000 {
        let suffix = if copy == 0 {
            String::new()
        } else {
            format!("-{copy}")
        };
        let path = output_dir.join(format!("{base_name}{suffix}.{extension}"));
        if path.exists() {
            continue;
        }
        match std::fs::rename(source, &path) {
            Ok(()) => return Ok(path),
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(err) => return Err(format!("failed to move downloaded media: {err}")),
        }
    }
    Err(format!(
        "failed to choose unique download name for {base_name}.{extension}"
    ))
}
