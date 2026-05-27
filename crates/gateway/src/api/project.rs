//! Project API handlers

use crate::api::{misc::select_directory, types::*};
use crate::mock::global_store;
use axum::{
    extract::{Path, Query},
    http::{HeaderMap, StatusCode},
    Json,
};
use std::{fs, path::PathBuf};

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
        global_store().set_current_directory(dir.clone());
        global_store()
            .list_projects()
            .into_iter()
            .find(|p| same_directory(&p.worktree, &dir))
            .unwrap_or_else(|| global_store().add_project(dir, None))
    });

    Json(CurrentProjectResponse { project })
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct ProjectDirectoryParams {
    pub directory: Option<String>,
}

// ============================================================================
// Project CRUD
// ============================================================================

pub async fn get_project(Path(project_id): Path<String>) -> Json<Project> {
    global_store()
        .get_project(&project_id)
        .map(Json)
        .unwrap_or_else(|| {
            Json(Project {
                id: project_id,
                worktree: String::new(),
                vcs: None,
                name: None,
                icon: None,
                time: ProjectTime {
                    created: 0,
                    updated: 0,
                    initialized: None,
                },
            })
        })
}

pub async fn update_project(
    Path(project_id): Path<String>,
    Json(_payload): Json<ProjectUpdateRequest>,
) -> Json<Project> {
    // Mock update - in real impl would update the project
    let project = global_store()
        .get_project(&project_id)
        .unwrap_or_else(|| Project {
            id: project_id,
            worktree: String::new(),
            vcs: None,
            name: None,
            icon: None,
            time: ProjectTime {
                created: 0,
                updated: 0,
                initialized: None,
            },
        });
    Json(project)
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct ProjectUpdateRequest {
    pub name: Option<String>,
    pub icon: Option<ProjectIcon>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct WorkspaceCreateRequest {
    pub name: Option<String>,
}

pub async fn create_named_workspace(
    Json(payload): Json<WorkspaceCreateRequest>,
) -> Result<Json<Project>, (StatusCode, String)> {
    let name = sanitize_workspace_name(payload.name.as_deref().unwrap_or("New project"));
    let directory = documents_directory().join(&name);
    fs::create_dir_all(&directory).map_err(internal_error)?;
    Ok(Json(upsert_workspace_project(directory, Some(name))))
}

pub async fn use_default_workspace() -> Result<Json<Project>, (StatusCode, String)> {
    let name = "tura workspace".to_string();
    let directory = documents_directory().join(&name);
    fs::create_dir_all(&directory).map_err(internal_error)?;
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
        upsert_workspace_project(path, Some(name))
    })))
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct DirectoryWorkspaceRequest {
    pub title: Option<String>,
}

// ============================================================================
// Project Git Init
// ============================================================================

#[derive(Debug, Clone, serde::Deserialize)]
pub struct GitInitQuery {
    pub directory: Option<String>,
    pub workspace: Option<String>,
}

pub async fn git_init_project(Query(_params): Query<GitInitQuery>) -> Json<Project> {
    Json(Project {
        id: "mock-project".to_string(),
        worktree: String::new(),
        vcs: Some("git".to_string()),
        name: None,
        icon: None,
        time: ProjectTime {
            created: chrono::Utc::now().timestamp_millis(),
            updated: chrono::Utc::now().timestamp_millis(),
            initialized: Some(chrono::Utc::now().timestamp_millis()),
        },
    })
}

// ============================================================================
// Experimental Worktree
// ============================================================================

pub async fn create_worktree(Query(params): Query<GitInitQuery>) -> Json<WorktreeResponse> {
    let source = params
        .directory
        .unwrap_or_else(|| "/tmp/mock-worktree".to_string());
    let branch = format!("workspace-{}", chrono::Utc::now().timestamp_millis());
    let directory = format!(
        "{}-{}",
        source.trim_end_matches(['/', '\\']),
        branch.replace(['/', '\\', ':'], "-")
    );

    global_store().add_project(directory.clone(), Some(branch.clone()));

    Json(WorktreeResponse {
        name: branch.clone(),
        branch,
        directory,
    })
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct WorktreeCreateRequest {
    pub directory: Option<String>,
    pub name: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct WorktreeResponse {
    pub name: String,
    pub branch: String,
    pub directory: String,
}

pub async fn reset_worktree() -> Json<bool> {
    Json(true)
}

fn same_directory(left: &str, right: &str) -> bool {
    left.replace('\\', "/")
        .trim_end_matches('/')
        .eq_ignore_ascii_case(right.replace('\\', "/").trim_end_matches('/'))
}

fn upsert_workspace_project(directory: PathBuf, name: Option<String>) -> Project {
    let worktree = directory.to_string_lossy().to_string();
    global_store().set_current_directory(worktree.clone());
    global_store()
        .list_projects()
        .into_iter()
        .find(|project| same_directory(&project.worktree, &worktree))
        .unwrap_or_else(|| global_store().add_project(worktree, name))
}

fn list_projects_with_default_workspace() -> Vec<Project> {
    let default_directory = documents_directory().join("tura workspace");
    let default_worktree = default_directory.to_string_lossy().to_string();
    let mut projects = global_store().list_projects();
    if !projects
        .iter()
        .any(|project| same_directory(&project.worktree, &default_worktree))
    {
        let _ = fs::create_dir_all(&default_directory);
        projects.insert(
            0,
            global_store().add_project(default_worktree, Some("tura workspace".to_string())),
        );
    }
    projects
}

fn documents_directory() -> PathBuf {
    if let Some(path) = xdg_documents_directory() {
        return path;
    }
    let home = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME"));
    let home = home
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    home.join("Documents")
}

fn xdg_documents_directory() -> Option<PathBuf> {
    let home = std::env::var_os("HOME").map(PathBuf::from)?;
    let config = home.join(".config").join("user-dirs.dirs");
    let content = fs::read_to_string(config).ok()?;
    for line in content.lines() {
        let line = line.trim();
        let value = line.strip_prefix("XDG_DOCUMENTS_DIR=")?;
        let value = value
            .trim_matches('"')
            .replace("$HOME", &home.to_string_lossy());
        if !value.trim().is_empty() {
            return Some(PathBuf::from(value));
        }
    }
    None
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
