//! Project API handlers

use crate::api::directory_picker::select_directory;
use crate::contracts::*;
use crate::mock::global_store;
use axum::{
    extract::Query,
    http::{HeaderMap, StatusCode},
    Json,
};
use std::{
    env, fs,
    path::{Path, PathBuf},
};

const DEFAULT_WORKSPACE_NAME: &str = "tura_workspace";
const DOCUMENTS_DIRECTORY_NAMES: &[&str] = &["Documents", "文档"];

// ============================================================================
// Project List & Current
// ============================================================================

pub async fn list_projects() -> Json<Vec<Project>> {
    Json(list_projects_with_default_workspace())
}

pub async fn get_current_project(
    headers: HeaderMap,
    Query(params): Query<ProjectDirectoryParams>,
) -> Json<CurrentProjectResponse> {
    let directory = params
        .directory
        .or_else(|| encoded_header(&headers, "x-opencode-directory"))
        .or_else(|| global_store().get_current_directory());

    let project = directory.map(|dir| {
        let path = PathBuf::from(&dir);
        upsert_workspace_project(path, None)
    });

    Json(CurrentProjectResponse { project })
}

pub async fn create_named_workspace(
    Json(payload): Json<WorkspaceCreateRequest>,
) -> Result<Json<Project>, (StatusCode, String)> {
    let name = sanitize_workspace_name(payload.name.as_deref().unwrap_or("New project"));
    let directory = documents_directory().join(&name);
    prepare_workspace_directory(&directory)?;
    Ok(Json(upsert_workspace_project(directory, Some(name))))
}

pub async fn use_default_workspace() -> Result<Json<Project>, (StatusCode, String)> {
    let name = DEFAULT_WORKSPACE_NAME.to_string();
    let directory = documents_directory().join(&name);
    prepare_workspace_directory(&directory)?;
    Ok(Json(upsert_workspace_project(directory, Some(name))))
}

pub async fn select_local_workspace(
    Json(payload): Json<DirectoryWorkspaceRequest>,
) -> Result<Json<Option<Project>>, (StatusCode, String)> {
    let title = payload.title.clone();
    let selected = tokio::task::spawn_blocking(move || select_directory(title.as_deref()))
        .await
        .map_err(|error| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Directory picker task failed: {error}"),
            )
        })?
        .map_err(|error| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to open directory picker: {error}"),
            )
        })?;
    Ok(Json(selected.map(|directory| {
        let path = PathBuf::from(directory);
        let name = workspace_name_from_path(&path);
        if let Err(error) = prepare_workspace_directory(&path) {
            tracing::warn!(
                directory = %path.display(),
                error = ?error,
                "failed to prepare selected workspace directory"
            );
        }
        upsert_workspace_project(path, Some(name))
    })))
}

fn same_directory(left: &str, right: &str) -> bool {
    left.replace('\\', "/")
        .trim_end_matches('/')
        .eq_ignore_ascii_case(right.replace('\\', "/").trim_end_matches('/'))
}

fn upsert_workspace_project(directory: PathBuf, name: Option<String>) -> Project {
    if let Err(error) = prepare_workspace_directory(&directory) {
        tracing::warn!(
            directory = %directory.display(),
            error = ?error,
            "failed to ensure project workspace directory"
        );
    }
    let worktree = directory.to_string_lossy().to_string();
    global_store().set_current_directory(worktree.clone());
    global_store()
        .list_projects()
        .into_iter()
        .find(|project| same_directory(&project.worktree, &worktree))
        .unwrap_or_else(|| global_store().add_project(worktree, name))
}

fn list_projects_with_default_workspace() -> Vec<Project> {
    let default_directory = documents_directory().join(DEFAULT_WORKSPACE_NAME);
    let default_worktree = default_directory.to_string_lossy().to_string();
    let mut projects = global_store().list_projects();
    if !projects
        .iter()
        .any(|project| same_directory(&project.worktree, &default_worktree))
    {
        let _ = prepare_workspace_directory(&default_directory);
        projects.insert(
            0,
            global_store().add_project(default_worktree, Some(DEFAULT_WORKSPACE_NAME.to_string())),
        );
    }
    projects
}

fn prepare_workspace_directory(directory: &Path) -> Result<(), (StatusCode, String)> {
    fs::create_dir_all(directory).map_err(internal_error)?;
    runtime::workspace_git::ensure_workspace_git_repo(directory).map_err(|error| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to prepare workspace git repository: {error}"),
        )
    })
}

fn documents_directory() -> PathBuf {
    if let Some(path) = xdg_documents_directory() {
        return path;
    }

    let homes = home_directory_candidates();
    documents_directory_from_homes(&homes).unwrap_or_else(|| {
        env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(DOCUMENTS_DIRECTORY_NAMES[0])
    })
}

fn xdg_documents_directory() -> Option<PathBuf> {
    let home = env::var_os("HOME").map(PathBuf::from)?;
    let config = home.join(".config").join("user-dirs.dirs");
    let content = fs::read_to_string(config).ok()?;
    for line in content.lines() {
        let line = line.trim();
        let Some(value) = line.strip_prefix("XDG_DOCUMENTS_DIR=") else {
            continue;
        };
        let value = value
            .trim_matches('"')
            .replace("${HOME}", &home.to_string_lossy())
            .replace("$HOME", &home.to_string_lossy());
        if !value.trim().is_empty() {
            return Some(PathBuf::from(value));
        }
    }
    None
}

fn home_directory_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    for key in ["USERPROFILE", "HOME"] {
        if let Some(value) = env::var_os(key) {
            push_unique_path(&mut candidates, PathBuf::from(value));
        }
    }

    if let (Some(drive), Some(path)) = (env::var_os("HOMEDRIVE"), env::var_os("HOMEPATH")) {
        let mut combined = PathBuf::from(drive);
        combined.push(path);
        push_unique_path(&mut candidates, combined);
    }

    candidates
}

fn documents_directory_from_homes(homes: &[PathBuf]) -> Option<PathBuf> {
    for home in homes {
        if is_documents_directory(home) {
            return Some(home.clone());
        }
    }

    for home in homes {
        for name in DOCUMENTS_DIRECTORY_NAMES {
            let candidate = home.join(name);
            if candidate.exists() {
                return Some(candidate);
            }
        }
    }

    homes
        .first()
        .map(|home| home.join(DOCUMENTS_DIRECTORY_NAMES[0]))
}

fn is_documents_directory(path: &Path) -> bool {
    path.file_name()
        .and_then(|value| value.to_str())
        .is_some_and(|name| {
            DOCUMENTS_DIRECTORY_NAMES
                .iter()
                .any(|candidate| name.eq_ignore_ascii_case(candidate))
        })
}

fn push_unique_path(paths: &mut Vec<PathBuf>, candidate: PathBuf) {
    if candidate.as_os_str().is_empty() {
        return;
    }
    if !paths
        .iter()
        .any(|existing| same_directory_path(existing, &candidate))
    {
        paths.push(candidate);
    }
}

fn same_directory_path(left: &Path, right: &Path) -> bool {
    same_directory(&left.to_string_lossy(), &right.to_string_lossy())
}

fn sanitize_workspace_name(value: &str) -> String {
    let sanitized = value
        .trim()
        .chars()
        .map(|character| match character {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '-',
            character if character.is_control() => '-',
            character => character,
        })
        .collect::<String>()
        .trim_matches(['.', ' '])
        .to_string();
    if sanitized.is_empty() {
        "New project".to_string()
    } else {
        sanitized
    }
}

fn workspace_name_from_path(path: &std::path::Path) -> String {
    path.file_name()
        .and_then(|value| value.to_str())
        .map(sanitize_workspace_name)
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| path.to_string_lossy().to_string())
}

fn internal_error(error: std::io::Error) -> (StatusCode, String) {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        format!("Failed to prepare workspace directory: {error}"),
    )
}

fn encoded_header(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(percent_decode)
}

fn percent_decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut output = Vec::with_capacity(bytes.len());
    let mut index = 0;

    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            if let (Some(high), Some(low)) = (hex(bytes[index + 1]), hex(bytes[index + 2])) {
                output.push((high << 4) | low);
                index += 3;
                continue;
            }
        }

        output.push(bytes[index]);
        index += 1;
    }

    String::from_utf8_lossy(&output).into_owned()
}

fn hex(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        b'A'..=b'F' => Some(value - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn documents_directory_uses_existing_documents_candidate() {
        let temp = env::temp_dir().join(format!(
            "tura-project-documents-test-{}",
            uuid::Uuid::new_v4()
        ));
        let documents = temp.join("文档");
        fs::create_dir_all(&documents).expect("create localized documents directory");

        let selected =
            documents_directory_from_homes(std::slice::from_ref(&temp)).expect("documents path");

        assert_eq!(selected, documents);
        let _ = fs::remove_dir_all(temp);
    }

    #[test]
    fn documents_directory_does_not_duplicate_documents_leaf() {
        let home = PathBuf::from(r"C:\Users\alice\Documents");

        let selected =
            documents_directory_from_homes(std::slice::from_ref(&home)).expect("documents path");

        assert_eq!(selected, home);
    }

    #[test]
    fn same_directory_matches_mixed_separators_and_case() {
        assert!(same_directory(
            r"C:\Users\Alice\Documents\tura_workspace\",
            "c:/users/alice/documents/tura_workspace"
        ));
    }
}
