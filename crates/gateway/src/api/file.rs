//! File API handlers

use crate::contracts::*;
use crate::mock::global_store;
use axum::{
    extract::Query,
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use base64::Engine;
use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

// ============================================================================
// File List
// ============================================================================

pub async fn list_files(Query(params): Query<ListFilesQuery>) -> Json<Vec<FileInfo>> {
    Json(list_files_value(params))
}

pub fn list_files_value(params: ListFilesQuery) -> Vec<FileInfo> {
    let Some(root) = workspace_root(params.directory) else {
        return Vec::new();
    };
    let Some(full_path) = safe_join(&root, params.path.as_deref().unwrap_or_default()) else {
        return Vec::new();
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

    entries
}

struct GitStatusSnapshot {
    is_git_repository: bool,
    statuses: HashMap<String, String>,
}

fn git_status_snapshot(root: &Path, relative_path: &str) -> GitStatusSnapshot {
    let mut git_probe = Command::new("git");
    git_probe
        .arg("-C")
        .arg(root)
        .arg("rev-parse")
        .arg("--is-inside-work-tree")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    tura_path::process_hardening::hide_child_console_window(&mut git_probe);
    let is_git_repository = git_probe.status().is_ok_and(|status| status.success());
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
        .arg("--ignored=matching");
    if !relative_path.trim().is_empty() {
        command.arg("--").arg(relative_path);
    }
    tura_path::process_hardening::hide_child_console_window(&mut command);

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
    path.replace('\\', "/")
        .split('/')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("/")
}

// ============================================================================
// File Content
// ============================================================================

pub async fn get_file_content(
    Query(params): Query<FileContentQuery>,
) -> Result<Json<FileContentResponse>, (StatusCode, String)> {
    get_file_content_value(params).map(Json)
}

pub fn get_file_content_value(
    params: FileContentQuery,
) -> Result<FileContentResponse, (StatusCode, String)> {
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
        return Ok(FileContentResponse {
            content_type: "media".to_string(),
            content: base64::engine::general_purpose::STANDARD.encode(bytes),
            encoding: Some("base64".to_string()),
            mime_type: Some(mime_type.to_string()),
        });
    }

    match String::from_utf8(bytes) {
        Ok(content) => Ok(FileContentResponse {
            content_type: "text".to_string(),
            content,
            encoding: None,
            mime_type: None,
        }),
        Err(_) => Ok(FileContentResponse {
            content_type: "binary".to_string(),
            content: String::new(),
            encoding: None,
            mime_type: None,
        }),
    }
}

pub async fn get_file_media(
    Query(params): Query<FileContentQuery>,
) -> Result<Response, (StatusCode, String)> {
    let (_root, path) = resolve_workspace_file_path(params.directory, &params.path, "media read")?;
    let mime_type = media_mime_type(&path).ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            "File is not a supported media type".to_string(),
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

    Ok((
        [
            (header::CONTENT_TYPE, mime_type),
            (header::CACHE_CONTROL, "no-store"),
        ],
        bytes,
    )
        .into_response())
}

pub async fn save_input_file(
    Query(params): Query<FileInputSaveQuery>,
    Json(payload): Json<FileInputSaveRequest>,
) -> Result<Json<FileInputSaveResponse>, (StatusCode, String)> {
    let root = workspace_root(params.directory).ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            "No workspace directory was provided for input file save".to_string(),
        )
    })?;
    if !payload.encoding.eq_ignore_ascii_case("base64") {
        return Err((
            StatusCode::BAD_REQUEST,
            "Input file content must use base64 encoding".to_string(),
        ));
    }
    if payload.content.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            "Input file payload is empty".to_string(),
        ));
    }
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(payload.content.as_bytes())
        .map_err(|error| {
            (
                StatusCode::BAD_REQUEST,
                format!("Invalid base64 input file: {error}"),
            )
        })?;
    if bytes.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            "Input file payload is empty".to_string(),
        ));
    }

    let directory = safe_join(&root, ".tura/media/input").ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            "Input file path must stay inside the workspace".to_string(),
        )
    })?;
    std::fs::create_dir_all(&directory).map_err(|error| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to create input media directory: {error}"),
        )
    })?;
    let name = unique_input_file_name(&directory, &payload.name);
    let path = directory.join(&name);
    std::fs::write(&path, &bytes).map_err(|error| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to save input file: {error}"),
        )
    })?;

    Ok(Json(FileInputSaveResponse {
        path: relative_display_path(&root, &path),
        absolute: display_path(&path),
        name,
        mime_type: payload.mime_type,
        size_bytes: bytes.len() as u64,
    }))
}

pub async fn open_file(
    Query(params): Query<FileContentQuery>,
) -> Result<Json<FileOpenResponse>, (StatusCode, String)> {
    open_file_value(params).map(Json)
}

pub fn open_file_value(params: FileContentQuery) -> Result<FileOpenResponse, (StatusCode, String)> {
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

    Ok(FileOpenResponse {
        path: relative_display_path(&root, &path),
        opened: true,
    })
}

pub async fn open_file_location(
    Query(params): Query<FileContentQuery>,
) -> Result<Json<FileOpenResponse>, (StatusCode, String)> {
    open_file_location_value(params).map(Json)
}

pub fn open_file_location_value(
    params: FileContentQuery,
) -> Result<FileOpenResponse, (StatusCode, String)> {
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

    Ok(FileOpenResponse {
        path: relative_display_path(&root, &path),
        opened: true,
    })
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

fn unique_input_file_name(directory: &Path, requested: &str) -> String {
    let sanitized = sanitize_input_file_name(requested);
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default();
    let prefix = format!("{stamp}-{}", std::process::id());
    let candidate = format!("{prefix}-{sanitized}");
    if !directory.join(&candidate).exists() {
        return candidate;
    }
    for index in 1..1000 {
        let candidate = format!("{prefix}-{index}-{sanitized}");
        if !directory.join(&candidate).exists() {
            return candidate;
        }
    }
    format!("{prefix}-{}-{sanitized}", rand_suffix())
}

fn sanitize_input_file_name(value: &str) -> String {
    let leaf = value
        .rsplit(['/', '\\'])
        .find(|part| !part.trim().is_empty())
        .unwrap_or("attachment.bin");
    let mut output = String::new();
    let mut previous_dash = false;
    for character in leaf.trim().chars() {
        let valid = character.is_ascii_alphanumeric() || matches!(character, '.' | '_' | '-');
        let next = if valid { character } else { '-' };
        if next == '-' {
            if !previous_dash {
                output.push(next);
            }
            previous_dash = true;
        } else {
            output.push(next);
            previous_dash = false;
        }
    }
    let cleaned = output.trim_matches(['.', '-', '_']).to_string();
    if cleaned.is_empty() {
        "attachment.bin".to_string()
    } else {
        cleaned
    }
}

fn rand_suffix() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| format!("{:x}", duration.as_nanos()))
        .unwrap_or_else(|_| "fallback".to_string())
}

fn open_with_system_default(path: &Path) -> std::io::Result<()> {
    #[cfg(any(test, feature = "business-tests"))]
    if let Some(command) = test_open_command("TURA_FILE_OPEN_COMMAND") {
        return spawn_command(command.as_str(), std::iter::empty::<&str>(), Some(path));
    }

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
    #[cfg(any(test, feature = "business-tests"))]
    if let Some(command) = test_open_command("TURA_FILE_OPEN_LOCATION_COMMAND") {
        return spawn_command(command.as_str(), std::iter::empty::<&str>(), Some(path));
    }

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

#[cfg(any(test, feature = "business-tests"))]
fn test_open_command(key: &str) -> Option<String> {
    std::env::var(key)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
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
    tura_path::process_hardening::hide_child_console_window(&mut process);
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

#[cfg(test)]
mod tests {
    use super::*;
    use axum::extract::Query;

    struct CurrentDirectoryGuard(Option<String>);

    impl CurrentDirectoryGuard {
        fn clear() -> Self {
            let previous = global_store().get_current_directory();
            global_store().clear_current_directory();
            Self(previous)
        }
    }

    impl Drop for CurrentDirectoryGuard {
        fn drop(&mut self) {
            match self.0.take() {
                Some(directory) => global_store().set_current_directory(directory),
                None => global_store().clear_current_directory(),
            }
        }
    }

    #[test]
    fn safe_join_rejects_absolute_escape_parent_and_prefix_components() {
        let temp = tempfile::tempdir().expect("temp workspace");
        let root = temp.path();
        std::fs::create_dir_all(root.join("src")).expect("src dir");
        std::fs::write(root.join("src/lib.rs"), "fn main() {}\n").expect("source file");

        assert_eq!(
            safe_join(root, "src/./lib.rs").expect("safe relative path"),
            root.join("src/lib.rs")
        );
        assert!(safe_join(root, "../outside.rs").is_none());
        assert!(safe_join(root, "/absolute/outside.rs").is_none());
        assert!(safe_join(root, r"C:\outside.rs").is_none());

        let absolute_inside = root.join("src/lib.rs");
        assert_eq!(
            safe_join(root, absolute_inside.to_string_lossy().as_ref())
                .expect("absolute inside workspace"),
            absolute_inside.canonicalize().expect("canonical source")
        );
    }

    #[test]
    fn display_and_relative_paths_are_forward_slash_normalized() {
        let root = PathBuf::from(r"C:\repo");
        let path = PathBuf::from(r"C:\repo\src\main.rs");

        assert_eq!(display_path(&path), "C:/repo/src/main.rs");
        assert_eq!(relative_display_path(&root, &path), "src/main.rs");
        assert_eq!(
            relative_display_path(&root, &PathBuf::from(r"D:\other\file.txt")),
            "D:/other/file.txt"
        );
    }

    #[test]
    fn git_status_lines_parse_status_labels_renames_and_paths() {
        assert_eq!(
            parse_git_status_line(" M src/main.rs"),
            Some(("src/main.rs".to_string(), "modified".to_string()))
        );
        assert_eq!(
            parse_git_status_line("R  old.rs -> src/new.rs"),
            Some(("src/new.rs".to_string(), "renamed".to_string()))
        );
        assert_eq!(
            parse_git_status_line("?? \"src/windows\\\\path.rs\""),
            Some(("src/windows/path.rs".to_string(), "untracked".to_string()))
        );
        assert_eq!(parse_git_status_line(""), None);
        assert_eq!(parse_git_status_line("M"), None);
    }

    #[test]
    fn git_status_label_covers_porcelain_statuses() {
        for status in ["M", "MM", "AM", "RM"] {
            assert_eq!(git_status_label(status), "modified");
        }
        assert_eq!(git_status_label("A"), "added");
        assert_eq!(git_status_label("D"), "deleted");
        assert_eq!(git_status_label("R"), "renamed");
        assert_eq!(git_status_label("C"), "copied");
        assert_eq!(git_status_label("??"), "untracked");
        assert_eq!(git_status_label("!!"), "ignored");
        assert_eq!(git_status_label("UU"), "changed");
    }

    #[test]
    fn should_hide_filters_build_cache_and_dependency_directories_only() {
        for hidden in [
            ".git",
            ".turbo",
            ".next",
            ".vite",
            ".solid",
            ".cache",
            "node_modules",
            "target",
            "dist",
            "build",
        ] {
            assert!(should_hide(hidden), "{hidden} should be hidden");
        }
        for visible in ["src", "target-notes", "build.rs", ".github"] {
            assert!(!should_hide(visible), "{visible} should remain visible");
        }
    }

    #[test]
    fn media_mime_type_is_case_insensitive_for_supported_types() {
        let cases = [
            ("photo.PNG", "image/png"),
            ("photo.jpeg", "image/jpeg"),
            ("photo.JPG", "image/jpeg"),
            ("anim.GIF", "image/gif"),
            ("asset.webp", "image/webp"),
            ("icon.svg", "image/svg+xml"),
            ("doc.PDF", "application/pdf"),
            ("clip.mp4", "video/mp4"),
            ("clip.webm", "video/webm"),
            ("clip.mov", "video/quicktime"),
            ("sound.mp3", "audio/mpeg"),
            ("sound.wav", "audio/wav"),
            ("sound.ogg", "audio/ogg"),
        ];
        for (name, expected) in cases {
            assert_eq!(media_mime_type(Path::new(name)), Some(expected));
        }
        assert_eq!(media_mime_type(Path::new("archive.zip")), None);
    }

    #[tokio::test]
    async fn get_file_content_reads_text_media_binary_and_reports_errors() {
        let temp = tempfile::tempdir().expect("temp workspace");
        let root = temp.path();
        std::fs::write(root.join("hello.txt"), "hello\n").expect("text file");
        std::fs::write(root.join("image.png"), [0x89, b'P', b'N', b'G']).expect("png file");
        std::fs::write(root.join("blob.bin"), [0xff, 0x00, 0x80]).expect("binary file");

        let text = get_file_content(Query(FileContentQuery {
            directory: Some(root.display().to_string()),
            path: "hello.txt".to_string(),
        }))
        .await
        .expect("text read")
        .0;
        assert_eq!(text.content_type, "text");
        assert_eq!(text.content, "hello\n");
        assert_eq!(text.encoding, None);

        let media = get_file_content(Query(FileContentQuery {
            directory: Some(root.display().to_string()),
            path: "image.png".to_string(),
        }))
        .await
        .expect("media read")
        .0;
        assert_eq!(media.content_type, "media");
        assert_eq!(media.encoding.as_deref(), Some("base64"));
        assert_eq!(media.mime_type.as_deref(), Some("image/png"));
        assert_eq!(media.content, "iVBORw==");

        let binary = get_file_content(Query(FileContentQuery {
            directory: Some(root.display().to_string()),
            path: "blob.bin".to_string(),
        }))
        .await
        .expect("binary read")
        .0;
        assert_eq!(binary.content_type, "binary");
        assert!(binary.content.is_empty());

        let missing = get_file_content(Query(FileContentQuery {
            directory: Some(root.display().to_string()),
            path: "missing.txt".to_string(),
        }))
        .await
        .expect_err("missing file");
        assert_eq!(missing.0, StatusCode::NOT_FOUND);

        let escape = get_file_content(Query(FileContentQuery {
            directory: Some(root.display().to_string()),
            path: "../escape.txt".to_string(),
        }))
        .await
        .expect_err("path escape");
        assert_eq!(escape.0, StatusCode::BAD_REQUEST);
        assert!(escape.1.contains("inside the workspace"));
    }

    #[tokio::test]
    async fn list_files_sorts_directories_before_files_and_hides_noise() {
        let temp = tempfile::tempdir().expect("temp workspace");
        let root = temp.path();
        std::fs::create_dir_all(root.join("src")).expect("src dir");
        std::fs::create_dir_all(root.join("target")).expect("target dir");
        std::fs::write(root.join("b.txt"), "b").expect("b file");
        std::fs::write(root.join("a.txt"), "a").expect("a file");

        let entries = list_files(Query(ListFilesQuery {
            directory: Some(root.display().to_string()),
            path: None,
        }))
        .await
        .0;

        let names = entries
            .iter()
            .map(|entry| (entry.name.as_str(), entry.file_type.as_str()))
            .collect::<Vec<_>>();
        assert_eq!(
            names,
            vec![("src", "directory"), ("a.txt", "file"), ("b.txt", "file")]
        );
        assert!(entries
            .iter()
            .all(|entry| entry.git_status.as_deref() == Some("not_git")));
        assert!(entries
            .iter()
            .find(|entry| entry.name == "a.txt")
            .and_then(|entry| entry.size_bytes)
            .is_some());
    }

    #[test]
    fn resolve_workspace_file_path_handles_absolute_and_relative_inputs() {
        let _global_state = crate::test_support::current_directory_lock();
        let _current_directory = CurrentDirectoryGuard::clear();
        let temp = tempfile::tempdir().expect("temp workspace");
        let root = temp.path();
        std::fs::write(root.join("file.txt"), "text").expect("file");

        let (resolved_root, resolved_path) = resolve_workspace_file_path(
            Some(root.display().to_string()),
            "file.txt",
            "test action",
        )
        .expect("relative path");
        assert_eq!(resolved_root, root);
        assert_eq!(resolved_path, root.join("file.txt"));

        let absolute = root.join("file.txt");
        let (absolute_root, absolute_path) =
            resolve_workspace_file_path(None, absolute.to_string_lossy().as_ref(), "test action")
                .expect("absolute path");
        assert_eq!(absolute_path, absolute);
        assert_eq!(absolute_root, root);

        let error = resolve_workspace_file_path(None, "file.txt", "test action")
            .expect_err("relative path needs workspace");
        assert_eq!(error.0, StatusCode::BAD_REQUEST);
        assert!(error.1.contains("No workspace directory was provided"));
    }
}
