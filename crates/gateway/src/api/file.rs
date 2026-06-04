//! File API handlers

use crate::api::types::*;
use crate::mock::global_store;
use axum::{extract::Query, http::StatusCode, Json};
use base64::Engine;
use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};
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

    let git_statuses = git_status_snapshot(&root, params.path.as_deref().unwrap_or_default());

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
                    git_status: Some(
                        git_statuses
                            .statuses
                            .get(&normalized_relative_path)
                            .cloned()
                            .unwrap_or_else(|| {
                                if git_statuses.is_git_repository {
                                    "clean".to_string()
                                } else {
                                    "not_git".to_string()
                                }
                            }),
                    ),
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

struct GitStatusSnapshot {
    is_git_repository: bool,
    statuses: HashMap<String, String>,
}

fn git_status_snapshot(root: &Path, relative_path: &str) -> GitStatusSnapshot {
    let is_git_repository = Command::new("git")
        .arg("-C")
        .arg(root)
        .arg("rev-parse")
        .arg("--is-inside-work-tree")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success());
    if !is_git_repository {
        return GitStatusSnapshot {
            is_git_repository,
            statuses: HashMap::new(),
        };
    }

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
        return GitStatusSnapshot {
            is_git_repository,
            statuses: HashMap::new(),
        };
    };
    if !output.status.success() {
        return GitStatusSnapshot {
            is_git_repository,
            statuses: HashMap::new(),
        };
    }

    GitStatusSnapshot {
        is_git_repository,
        statuses: String::from_utf8_lossy(&output.stdout)
            .lines()
            .filter_map(parse_git_status_line)
            .collect(),
    }
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
    let (root, path) = resolve_workspace_file_path(params.directory, &params.path, "file open")?;
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

pub async fn open_file_location(
    Query(params): Query<FileContentQuery>,
) -> Result<Json<FileOpenResponse>, (StatusCode, String)> {
    let (root, path) =
        resolve_workspace_file_path(params.directory, &params.path, "file location open")?;
    if !path.exists() {
        return Err((StatusCode::NOT_FOUND, "File was not found".to_string()));
    }

    open_with_system_file_manager(&path).map_err(|error| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to open file location: {error}"),
        )
    })?;

    Ok(Json(FileOpenResponse {
        path: relative_display_path(&root, &path),
        opened: true,
    }))
}

fn resolve_workspace_file_path(
    directory: Option<String>,
    relative_path: &str,
    action: &str,
) -> Result<(PathBuf, PathBuf), (StatusCode, String)> {
    let requested = Path::new(relative_path);
    if requested.is_absolute() {
        let path = PathBuf::from(requested);
        let root = path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| path.clone());
        return Ok((root, path));
    }
    let root = workspace_root(directory).ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            format!("No workspace directory was provided for {action}"),
        )
    })?;
    let path = safe_join(&root, relative_path).ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            "File path must stay inside the workspace".to_string(),
        )
    })?;
    Ok((root, path))
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct FileOpenResponse {
    pub path: String,
    pub opened: bool,
}

fn workspace_root(directory: Option<String>) -> Option<PathBuf> {
    directory
        .filter(|value| !value.trim().is_empty())
        .or_else(|| global_store().get_current_directory())
        .map(PathBuf::from)
}

fn safe_join(root: &Path, relative: &str) -> Option<PathBuf> {
    let requested = PathBuf::from(relative);
    if requested.is_absolute() {
        let canonical_root = root.canonicalize().ok()?;
        let canonical_requested = requested.canonicalize().ok()?;
        return canonical_requested
            .starts_with(&canonical_root)
            .then_some(canonical_requested);
    }

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
        // `start` asks Windows Shell to use the registered default app.
        spawn_command("cmd", ["/C", "start", ""], Some(path))
    }

    #[cfg(target_os = "macos")]
    {
        spawn_command("open", std::iter::empty::<&str>(), Some(path))
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        spawn_first_open_command(
            [
                OpenAttempt::new("xdg-open", &[], Some(path)),
                OpenAttempt::new("gio", &["open"], Some(path)),
                OpenAttempt::new("kde-open", &[], Some(path)),
                OpenAttempt::new("exo-open", &[], Some(path)),
            ],
            "system default app",
        )
    }
}

fn open_with_system_file_manager(path: &Path) -> std::io::Result<()> {
    #[cfg(target_os = "windows")]
    {
        if path.is_file() {
            spawn_command(
                "explorer.exe",
                [format!("/select,{}", path.display())],
                None,
            )
        } else {
            spawn_command("explorer.exe", std::iter::empty::<&str>(), Some(path))
        }
    }

    #[cfg(target_os = "macos")]
    {
        if path.is_file() {
            return spawn_command("open", ["-R"], Some(path));
        } else {
            return spawn_command("open", std::iter::empty::<&str>(), Some(path));
        }
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let target = if path.is_file() {
            path.parent().unwrap_or(path)
        } else {
            path
        };
        return spawn_first_open_command(
            [
                OpenAttempt::new("xdg-open", &[], Some(target)),
                OpenAttempt::new("gio", &["open"], Some(target)),
                OpenAttempt::new("kde-open", &[], Some(target)),
                OpenAttempt::new("exo-open", &[], Some(target)),
            ],
            "system file manager",
        );
    }
}

#[cfg(all(unix, not(target_os = "macos")))]
struct OpenAttempt<'a> {
    command: &'a str,
    args: Vec<std::ffi::OsString>,
}

#[cfg(all(unix, not(target_os = "macos")))]
impl<'a> OpenAttempt<'a> {
    fn new(command: &'a str, args: &[&str], path: Option<&Path>) -> Self {
        let mut command_args = args
            .iter()
            .map(std::ffi::OsString::from)
            .collect::<Vec<_>>();
        if let Some(path) = path {
            command_args.push(path.as_os_str().to_owned());
        }
        Self {
            command,
            args: command_args,
        }
    }
}

fn spawn_command<I, S>(command: &str, args: I, path: Option<&Path>) -> std::io::Result<()>
where
    I: IntoIterator<Item = S>,
    S: AsRef<std::ffi::OsStr>,
{
    let mut process = Command::new(command);
    process
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    if let Some(path) = path {
        process.arg(path);
    }
    process.spawn()?;
    Ok(())
}

#[cfg(all(unix, not(target_os = "macos")))]
fn spawn_first_open_command<I>(attempts: I, action: &str) -> std::io::Result<()>
where
    I: IntoIterator<Item = OpenAttempt<'static>>,
{
    let mut last_error: Option<std::io::Error> = None;
    for attempt in attempts {
        match spawn_command(attempt.command, attempt.args, None) {
            Ok(_) => return Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                last_error = Some(error);
            }
            Err(error) => return Err(error),
        }
    }
    Err(last_error.unwrap_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("No command was found for {action}"),
        )
    }))
}

fn relative_display_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .map(display_path)
        .unwrap_or_else(|_| display_path(path))
}

fn display_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}
