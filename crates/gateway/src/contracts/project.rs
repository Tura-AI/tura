use serde::{Deserialize, Serialize};

// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub id: String,
    pub worktree: String,
    pub vcs: Option<String>,
    pub name: Option<String>,
    pub icon: Option<ProjectIcon>,
    pub time: ProjectTime,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectIcon {
    pub url: Option<String>,
    pub override_: Option<String>,
    pub color: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectTime {
    pub created: i64,
    pub updated: i64,
    pub initialized: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CurrentProjectResponse {
    pub project: Option<Project>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProjectDirectoryParams {
    pub directory: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct WorkspaceCreateRequest {
    pub name: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DirectoryWorkspaceRequest {
    pub title: Option<String>,
}
