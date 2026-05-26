//! File API handlers

use crate::api::types::*;
use crate::mock::global_store;
use axum::{extract::Query, http::StatusCode, Json};
use base64::Engine;
use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::time::UNIX_EPOCH;

// ============================================================================
// File List
// ============================================================================

#[derive(Debug, Clone, serde::Deserialize)]
pub struct ListFilesQuery {
    pub directory: Option<String>,
    pub path: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct FileInfo {
    pub name: String,
    pub path: String,
    #[serde(rename = "type")]
    pub file_type: String,
    pub absolute: String,
    pub ignored: bool,
    pub git_status: Option<String>,
    pub size_bytes: Option<u64>,
    pub modified_at: Option<u64>,
}

pub async fn list_files(Query(params): Query<ListFilesQuery>) -> Json<Vec<FileInfo>> {
    let Some(root) = workspace_root(params.directory) else {
        return Json(Vec::new());
    };
    let Some(full_path) = safe_join(&root, params.path.as_deref().unwrap_or_default()) else {
        return Json(Vec::new());
    };

    let git_statuses = git_status_map(&root, params.path.as_deref().unwrap_or_default());

    let mut entries = std::fs::read_dir(&full_path)
        .map(|dir| {
            dir.filter_map(|entry| {
                let entry = entry.ok()?;
                let path = entry.path();
                let metadata = entry.metadata().ok()?;
                let name = path.file_name()?.to_string_lossy().to_string();
                if should_hide(&name) {
                    return None;
                }

                let relative_path = relative_display_path(&root, &path);
                let normalized_relative_path = normalize_git_path(&relative_path);
                Some(FileInfo {
                    name,
                    path: relative_path,
                    file_type: if metadata.is_dir() {
                        "directory".to_string()
                    } else {
                        "file".to_string()
                    },
                    absolute: display_path(&path),
                    ignored: false,
                    git_status: git_statuses.get(&normalized_relative_path).cloned(),
                    size_bytes: if metadata.is_file() {
                        Some(metadata.len())
                    } else {
                        None
                    },
                    modified_at: metadata
                        .modified()
                        .ok()
                        .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
                        .map(|duration| duration.as_millis() as u64),
                })
            })
            .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    entries.sort_by(
        |left, right| match (left.file_type.as_str(), right.file_type.as_str()) {
            ("directory", "file") => std::cmp::Ordering::Less,
            ("file", "directory") => std::cmp::Ordering::Greater,
            _ => left.name.to_lowercase().cmp(&right.name.to_lowercase()),
        },
    );

    Json(entries)
}

fn git_status_map(root: &Path, relative_path: &str) -> HashMap<String, String> {
    let mut command = Command::new("git");
    command
        .arg("-C")
        .arg(root)
        .arg("status")
        .arg("--porcelain=v1")
        .arg("--ignored=matching")
        .arg("--")
        .arg(relative_path);

    let Ok(output) = command.output() else {
        return HashMap::new();
    };
    if !output.status.success() {
        return HashMap::new();
    }

    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(parse_git_status_line)
        .collect()
}

fn parse_git_status_line(line: &str) -> Option<(String, String)> {
    if line.len() < 4 {
        return None;
    }
    let status = line.get(0..2)?.trim();
    let raw_path = line
        .get(3..)?
        .rsplit(" -> ")
        .next()?
        .trim()
        .trim_matches('"');
    Some((
        normalize_git_path(raw_path),
        git_status_label(status).to_string(),
    ))
}

fn git_status_label(status: &str) -> &'static str {
    match status {
        "M" | "MM" | "AM" | "RM" => "modified",
        "A" => "added",
        "D" => "deleted",
        "R" => "renamed",
        "C" => "copied",
        "??" => "untracked",
        "!!" => "ignored",
        _ => "changed",
    }
}

fn normalize_git_path(path: &str) -> String {
    path.replace('\\', "/").trim_matches('/').to_string()
}

// ============================================================================
// File Content
// ============================================================================

#[derive(Debug, Clone, serde::Deserialize)]
pub struct FileContentQuery {
    pub path: String,
    pub directory: Option<String>,
}

pub async fn get_file_content(
    Query(params): Query<FileContentQuery>,
) -> Result<Json<FileContentResponse>, (StatusCode, String)> {
    let root = workspace_root(params.directory).ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            "No workspace directory was provided for file read".to_string(),
        )
    })?;
    let path = safe_join(&root, &params.path).ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            "File path must stay inside the workspace".to_string(),
        )
    })?;
    let bytes = std::fs::read(&path).map_err(|error| {
        (
            if error.kind() == std::io::ErrorKind::NotFound {
                StatusCode::NOT_FOUND
            } else {
                StatusCode::INTERNAL_SERVER_ERROR
            },
            error.to_string(),
        )
    })?;

    if let Some(mime_type) = media_mime_type(&path) {
        return Ok(Json(FileContentResponse {
            content_type: "media".to_string(),
            content: base64::engine::general_purpose::STANDARD.encode(bytes),
            encoding: Some("base64".to_string()),
            mime_type: Some(mime_type.to_string()),
        }));
    }

    match String::from_utf8(bytes) {
        Ok(content) => Ok(Json(FileContentResponse {
            content_type: "text".to_string(),
            content,
            encoding: None,
            mime_type: None,
        })),
        Err(_) => Ok(Json(FileContentResponse {
            content_type: "binary".to_string(),
            content: String::new(),
            encoding: None,
            mime_type: None,
        })),
    }
}

pub async fn open_file(
    Query(params): Query<FileContentQuery>,
) -> Result<Json<FileOpenResponse>, (StatusCode, String)> {
    let root = workspace_root(params.directory).ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            "No workspace directory was provided for file open".to_string(),
        )
    })?;
    let path = safe_join(&root, &params.path).ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            "File path must stay inside the workspace".to_string(),
        )
    })?;
    if !path.exists() {
        return Err((StatusCode::NOT_FOUND, "File was not found".to_string()));
    }

    open_with_system_default(&path).map_err(|error| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to open file: {error}"),
        )
    })?;

    Ok(Json(FileOpenResponse {
        path: relative_display_path(&root, &path),
        opened: true,
    }))
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct FileOpenResponse {
    pub path: String,
    pub opened: bool,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct FileWriteRequest {
    pub content: String,
    pub path: Option<String>,
}

pub async fn write_file(
    Query(params): Query<FileContentQuery>,
    Json(_payload): Json<FileWriteRequest>,
) -> Json<FileWriteResponse> {
    Json(FileWriteResponse {
        path: params.path,
        written: true,
    })
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct FileWriteResponse {
    pub path: String,
    pub written: bool,
}

// ============================================================================
// File Status
// ============================================================================

pub async fn get_file_status() -> Json<FileStatusResponse> {
    Json(FileStatusResponse { files: vec![] })
}

// ============================================================================
// Find
// ============================================================================

#[derive(Debug, Clone, serde::Deserialize)]
pub struct FileSearchParams {
    pub query: String,
    pub directory: Option<String>,
    #[serde(rename = "type")]
    pub file_type: Option<String>,
    pub dirs: Option<String>,
    pub limit: Option<usize>,
}

pub async fn find_files(Query(params): Query<FileSearchParams>) -> Json<Vec<String>> {
    let Some(base_dir) = workspace_root(params.directory) else {
        return Json(Vec::new());
    };
    let query = params.query.to_lowercase();
    let limit = params.limit.unwrap_or(50);

    fn search_recursive(
        root: &Path,
        dir: &Path,
        query: &str,
        limit: usize,
        include_dirs: bool,
        is_dir_only: bool,
    ) -> Vec<String> {
        let mut results = Vec::new();

        if results.len() >= limit {
            return results;
        }

        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.filter_map(|e| e.ok()) {
                let path = entry.path();
                let name_raw = path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();
                if should_hide(&name_raw) {
                    continue;
                }
                let name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_lowercase())
                    .unwrap_or_default();

                if name.contains(query)
                    && ((!is_dir_only && (!path.is_dir() || include_dirs))
                        || (is_dir_only && path.is_dir()))
                {
                    results.push(relative_display_path(root, &path));
                }

                if path.is_dir() && results.len() < limit {
                    let sub_results = search_recursive(
                        root,
                        &path,
                        query,
                        limit - results.len(),
                        include_dirs,
                        is_dir_only,
                    );
                    results.extend(sub_results);
                }

                if results.len() >= limit {
                    break;
                }
            }
        }

        results
    }

    let is_dir_only = params.file_type.as_deref() == Some("directory");
    let include_dirs = params.dirs.as_deref() == Some("true");
    let results = search_recursive(
        &base_dir,
        &base_dir,
        &query,
        limit,
        include_dirs,
        is_dir_only,
    );

    Json(results)
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct SymbolSearchParams {
    pub query: String,
    pub directory: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SymbolResult {
    pub name: String,
    pub kind: String,
    pub path: String,
    pub line: u32,
}

pub async fn find_symbols(Query(params): Query<SymbolSearchParams>) -> Json<Vec<SymbolResult>> {
    let Some(root) = workspace_root(params.directory) else {
        return Json(Vec::new());
    };
    let query = params.query.to_ascii_lowercase();
    let mut results = Vec::new();
    collect_symbols(&root, &root, &query, &mut results);
    results.truncate(200);
    Json(results)
}

fn collect_symbols(root: &Path, directory: &Path, query: &str, results: &mut Vec<SymbolResult>) {
    if results.len() >= 200 {
        return;
    }
    let Ok(entries) = std::fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if should_skip_path(&path) {
            continue;
        }
        if path.is_dir() {
            collect_symbols(root, &path, query, results);
            continue;
        }
        if !is_symbol_source_file(&path) {
            continue;
        }
        let Ok(content) = std::fs::read_to_string(&path) else {
            continue;
        };
        for (index, line) in content.lines().enumerate() {
            if let Some((name, kind)) = parse_symbol_line(line) {
                if query.is_empty() || name.to_ascii_lowercase().contains(query) {
                    results.push(SymbolResult {
                        name,
                        kind,
                        path: path
                            .strip_prefix(root)
                            .unwrap_or(&path)
                            .to_string_lossy()
                            .replace('\\', "/"),
                        line: (index + 1) as u32,
                    });
                    if results.len() >= 200 {
                        return;
                    }
                }
            }
        }
    }
}

fn should_skip_path(path: &Path) -> bool {
    path.file_name()
        .and_then(|value| value.to_str())
        .is_some_and(|name| {
            matches!(
                name,
                ".git" | "target" | "node_modules" | "dist" | "build" | ".tura"
            )
        })
}

fn is_symbol_source_file(path: &Path) -> bool {
    path.extension()
        .and_then(|value| value.to_str())
        .is_some_and(|extension| {
            matches!(
                extension,
                "rs" | "ts"
                    | "tsx"
                    | "js"
                    | "jsx"
                    | "py"
                    | "go"
                    | "java"
                    | "kt"
                    | "c"
                    | "cc"
                    | "cpp"
                    | "h"
                    | "hpp"
            )
        })
}

fn parse_symbol_line(line: &str) -> Option<(String, String)> {
    let trimmed = line.trim_start();
    let patterns = [
        ("pub fn ", "function"),
        ("fn ", "function"),
        ("async fn ", "function"),
        ("pub struct ", "struct"),
        ("struct ", "struct"),
        ("pub enum ", "enum"),
        ("enum ", "enum"),
        ("class ", "class"),
        ("function ", "function"),
        ("export function ", "function"),
        ("def ", "function"),
    ];
    for (prefix, kind) in patterns {
        if let Some(rest) = trimmed.strip_prefix(prefix) {
            let name = rest
                .split(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_' || ch == '-'))
                .next()
                .unwrap_or_default()
                .trim()
                .to_string();
            if !name.is_empty() {
                return Some((name, kind.to_string()));
            }
        }
    }
    None
}

fn workspace_root(directory: Option<String>) -> Option<PathBuf> {
    directory
        .filter(|value| !value.trim().is_empty())
        .or_else(|| global_store().get_current_directory())
        .map(PathBuf::from)
}

fn safe_join(root: &Path, relative: &str) -> Option<PathBuf> {
    let mut path = PathBuf::from(root);
    for component in Path::new(relative).components() {
        match component {
            Component::Normal(part) => path.push(part),
            Component::CurDir => {}
            Component::Prefix(_) | Component::RootDir | Component::ParentDir => return None,
        }
    }
    Some(path)
}

fn should_hide(name: &str) -> bool {
    matches!(
        name,
        ".git"
            | ".turbo"
            | ".next"
            | ".vite"
            | ".solid"
            | ".cache"
            | "node_modules"
            | "target"
            | "dist"
            | "build"
    )
}

fn media_mime_type(path: &Path) -> Option<&'static str> {
    match path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| extension.to_ascii_lowercase())
        .as_deref()
    {
        Some("png") => Some("image/png"),
        Some("jpg") | Some("jpeg") => Some("image/jpeg"),
        Some("gif") => Some("image/gif"),
        Some("webp") => Some("image/webp"),
        Some("svg") => Some("image/svg+xml"),
        Some("pdf") => Some("application/pdf"),
        Some("mp4") => Some("video/mp4"),
        Some("webm") => Some("video/webm"),
        Some("mov") => Some("video/quicktime"),
        Some("mp3") => Some("audio/mpeg"),
        Some("wav") => Some("audio/wav"),
        Some("ogg") => Some("audio/ogg"),
        _ => None,
    }
}

fn open_with_system_default(path: &Path) -> std::io::Result<()> {
    #[cfg(target_os = "windows")]
    {
        Command::new("cmd")
            .args(["/C", "start", ""])
            .arg(path)
            .spawn()?;
        Ok(())
    }

    #[cfg(target_os = "macos")]
    {
        Command::new("open").arg(path).spawn()?;
        return Ok(());
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        Command::new("xdg-open").arg(path).spawn()?;
        return Ok(());
    }
}

fn relative_display_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .map(display_path)
        .unwrap_or_else(|_| display_path(path))
}

fn display_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}
