//! Project API handlers

use crate::api::types::*;
use crate::mock::global_store;
use axum::{
    extract::{Path, Query},
    http::HeaderMap,
    Json,
};

// ============================================================================
// Project List & Current
// ============================================================================

pub async fn list_projects() -> Json<Vec<Project>> {
    Json(global_store().list_projects())
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
