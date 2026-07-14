use super::files::{
    downloaded_file_value, move_unique_download, stable_hash, write_unique_download,
};
use super::types::{SearchResult, WebDiscoverArgs};
use super::util::{
    command_configured_python, command_local_executable, extension_from_url, find_on_path,
    safe_filename, snapshot_files,
};
use reqwest::blocking::Client;
use serde_json::{json, Value};
use std::path::Path;
use std::process::Command;
use std::time::Duration;

pub(super) fn download_images(
    args: &WebDiscoverArgs,
    results: &[SearchResult],
    output_dir: &Path,
    session_dir: &Path,
) -> Result<(Vec<Value>, Vec<Value>), String> {
    let client = Client::builder()
        .timeout(Duration::from_secs(45))
        .user_agent("Tura web_discover/1.0")
        .build()
        .map_err(|err| format!("failed to create web_discover download client: {err}"))?;
    let mut handles = Vec::new();
    for (index, result) in results.iter().cloned().enumerate() {
        let client = client.clone();
        let args = args.clone();
        let output_dir = output_dir.to_path_buf();
        let session_dir = session_dir.to_path_buf();
        handles.push(std::thread::spawn(
            move || -> Result<Option<(usize, Value, Value)>, String> {
                let Ok(bytes) = client
                    .get(&result.url)
                    .send()
                    .and_then(|reply| reply.error_for_status())
                    .and_then(|reply| reply.bytes())
                else {
                    return Ok(None);
                };
                let size = bytes.len() as u64;
                if size < args.min_size || size > args.max_size {
                    return Ok(None);
                }
                let ext = extension_from_url(&result.url).unwrap_or("jpg");
                let base_name = format!("{:02}-{}", index + 1, safe_filename(&result.title));
                let path = write_unique_download(&output_dir, &base_name, ext, bytes.as_ref())?;
                let item = downloaded_file_value(
                    &path,
                    &session_dir,
                    &result.url,
                    result.page_url.as_deref(),
                    &args.kind,
                );
                let record = json!({
                    "title": result.title,
                    "url": result.url,
                    "page_url": result.page_url,
                    "file_type": args.kind,
                    "local_path": item["path"],
                    "size": item["size"],
                    "source": result.source,
                });
                Ok(Some((index, record, item)))
            },
        ));
    }

    let mut indexed = Vec::new();
    for handle in handles {
        let result = handle
            .join()
            .map_err(|_| "image download worker panicked".to_string())??;
        if let Some(item) = result {
            indexed.push(item);
        }
    }
    indexed.sort_by_key(|(index, _, _)| *index);
    let records = indexed
        .iter()
        .map(|(_, record, _)| record.clone())
        .collect();
    let downloaded = indexed.into_iter().map(|(_, _, item)| item).collect();
    Ok((records, downloaded))
}

pub(super) fn download_ytdlp_media(
    args: &WebDiscoverArgs,
    results: &[SearchResult],
    output_dir: &Path,
    session_dir: &Path,
) -> Result<(Vec<Value>, Vec<Value>), String> {
    let mut handles = Vec::new();
    for (index, result) in results.iter().cloned().enumerate() {
        let args = args.clone();
        let output_dir = output_dir.to_path_buf();
        let session_dir = session_dir.to_path_buf();
        handles.push(std::thread::spawn(
            move || -> Result<Option<(usize, Value, Value)>, String> {
                let format_arg = args
                    .format_selector
                    .as_deref()
                    .unwrap_or_else(|| default_ytdlp_format(&args.kind));
                let temp_dir = output_dir.join(format!(
                    ".tura-ytdlp-{}-{}-{}",
                    std::process::id(),
                    index,
                    stable_hash(&result.url)
                ));
                std::fs::create_dir_all(&temp_dir)
                    .map_err(|err| format!("failed to create yt-dlp temp dir: {err}"))?;
                let output_template = temp_dir.join("%(title).80s-%(id)s.%(ext)s");
                let command_parts = resolve_ytdlp_command();
                let mut command = Command::new(&command_parts.0);
                command
                    .args(&command_parts.1)
                    .args([
                        "-f",
                        format_arg,
                        "--no-playlist",
                        "--no-progress",
                        "--max-filesize",
                    ])
                    .arg(args.max_size.to_string())
                    .arg("-o")
                    .arg(&output_template)
                    .arg(&result.url);
                tura_path::process_hardening::hide_child_console_window(&mut command);
                let output = command.output().map_err(|err| {
                    let _ = std::fs::remove_dir_all(&temp_dir);
                    format!("failed to run yt-dlp download: {err}")
                })?;
                if !output.status.success() {
                    let _ = std::fs::remove_dir_all(&temp_dir);
                    return Ok(None);
                }
                let mut new_files = snapshot_files(&temp_dir)
                    .into_iter()
                    .filter(|path| {
                        std::fs::metadata(path)
                            .map(|m| m.len() >= args.min_size && m.len() <= args.max_size)
                            .unwrap_or(false)
                    })
                    .collect::<Vec<_>>();
                new_files.sort_by_key(|path| ytdlp_download_candidate_rank(path, &args.kind));
                let Some(path) = new_files.first() else {
                    let _ = std::fs::remove_dir_all(&temp_dir);
                    return Ok(None);
                };
                let ext = path
                    .extension()
                    .and_then(|value| value.to_str())
                    .unwrap_or("bin");
                let base_name = format!("{:02}-{}", index + 1, safe_filename(&result.title));
                let path = move_unique_download(path, &output_dir, &base_name, ext)?;
                let _ = std::fs::remove_dir_all(&temp_dir);
                let item = downloaded_file_value(
                    &path,
                    &session_dir,
                    &result.url,
                    result.page_url.as_deref(),
                    &args.kind,
                );
                let record = json!({
                    "title": result.title,
                    "url": result.url,
                    "page_url": result.page_url,
                    "file_type": args.kind,
                    "local_path": item["path"],
                    "size": item["size"],
                    "source": result.source,
                });
                Ok(Some((index, record, item)))
            },
        ));
    }

    let mut indexed = Vec::new();
    for handle in handles {
        let result = handle
            .join()
            .map_err(|_| "yt-dlp download worker panicked".to_string())??;
        if let Some(item) = result {
            indexed.push(item);
        }
    }
    indexed.sort_by_key(|(index, _, _)| *index);
    let records = indexed
        .iter()
        .map(|(_, record, _)| record.clone())
        .collect();
    let downloaded = indexed.into_iter().map(|(_, _, item)| item).collect();
    Ok((records, downloaded))
}

pub(super) fn default_ytdlp_format(kind: &str) -> &'static str {
    if kind == "audio" {
        "bestaudio/best"
    } else {
        "best[height<=540][ext=mp4]/best[height<=540]/best"
    }
}

pub(super) fn ytdlp_download_candidate_rank(
    path: &Path,
    kind: &str,
) -> (u8, std::cmp::Reverse<u64>) {
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    let rank = if kind == "audio" {
        match extension.as_str() {
            "mp3" | "m4a" | "aac" | "opus" | "ogg" | "webm" | "flac" | "wav" => 0,
            _ => 1,
        }
    } else {
        match extension.as_str() {
            "mp4" | "mkv" | "mov" => 0,
            "webm" => 1,
            "mp3" | "m4a" | "aac" | "opus" | "ogg" | "flac" | "wav" => 2,
            _ => 3,
        }
    };
    (rank, std::cmp::Reverse(size))
}

pub(super) fn resolve_ytdlp_command() -> (String, Vec<String>) {
    for env_name in ["TURA_WEB_DISCOVER_YTDLP", "TURA_YTDLP", "YTDLP_PATH"] {
        if let Ok(path) = std::env::var(env_name) {
            if !path.trim().is_empty() && Path::new(&path).exists() {
                return (path, Vec::new());
            }
        }
    }
    if let Some(path) = command_local_executable("yt-dlp") {
        (path.display().to_string(), Vec::new())
    } else if let Some(python) = command_configured_python("TURA_WEB_DISCOVER_PYTHON") {
        (
            python.display().to_string(),
            vec!["-m".to_string(), "yt_dlp".to_string()],
        )
    } else if let Some(path) = find_on_path("yt-dlp") {
        (path.display().to_string(), Vec::new())
    } else {
        let python = find_on_path("python3")
            .or_else(|| find_on_path("python"))
            .or_else(|| find_on_path("py"))
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "python".to_string());
        (python, vec!["-m".to_string(), "yt_dlp".to_string()])
    }
}
