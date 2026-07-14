use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
pub struct PublicConfig {
    pub deployment_mode: String,
    pub signup_enabled: bool,
    pub google_oauth_enabled: bool,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductUser {
    pub id: String,
    pub email: String,
    pub name: String,
    pub avatar_url: Option<String>,
    pub language: String,
    pub timezone: String,
    pub onboarded_at: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UserPatch {
    pub name: Option<String>,
    pub language: Option<String>,
    pub timezone: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workspace {
    pub id: String,
    pub name: String,
    pub slug: String,
    pub description: Option<String>,
    pub context: Option<String>,
    pub issue_prefix: String,
    pub avatar: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum IssueStatus {
    Backlog,
    Todo,
    InProgress,
    Review,
    Done,
    Closed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum IssuePriority {
    Low,
    Medium,
    High,
    Urgent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Issue {
    pub id: String,
    pub workspace_id: String,
    pub number: u32,
    pub title: String,
    pub description: String,
    pub status: IssueStatus,
    pub priority: IssuePriority,
    pub position: i64,
    pub assignee_type: Option<String>,
    pub assignee_id: Option<String>,
    pub project_id: Option<String>,
    pub labels: Vec<String>,
    pub session_id: Option<String>,
    pub active_task: Option<TaskRun>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct IssueInput {
    pub title: Option<String>,
    pub description: Option<String>,
    pub status: Option<IssueStatus>,
    pub priority: Option<IssuePriority>,
    pub assignee_type: Option<String>,
    pub assignee_id: Option<String>,
    pub project_id: Option<String>,
    pub labels: Option<Vec<String>>,
    pub session_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct IssueQuery {
    pub workspace_id: Option<String>,
    pub workspace_slug: Option<String>,
    pub status: Option<IssueStatus>,
    pub search: Option<String>,
    pub project_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductProject {
    pub id: String,
    pub workspace_id: String,
    pub title: String,
    pub description: String,
    pub status: String,
    pub priority: String,
    pub lead_type: Option<String>,
    pub lead_id: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductAgent {
    pub id: String,
    pub workspace_id: String,
    pub name: String,
    pub description: String,
    pub provider: String,
    pub model: String,
    pub runtime_id: Option<String>,
    pub status: String,
    pub visibility: String,
    pub thinking_level: Option<String>,
    pub run_count_7d: u32,
    pub run_count_30d: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskRun {
    pub id: String,
    pub issue_id: Option<String>,
    pub agent_id: String,
    pub runtime_id: Option<String>,
    pub status: String,
    pub session_id: Option<String>,
    pub title: String,
    pub created_at: i64,
    pub updated_at: i64,
}
