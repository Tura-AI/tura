//! Minimal Multica-compatible product API projection for the GUI.
//!
//! This is intentionally thin: durable collaboration storage can replace this
//! store later without changing the GUI contract.

use crate::session::session_store;
use axum::{
    extract::{Path, Query},
    Json,
};
use chrono::Utc;
use lazy_static::lazy_static;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use uuid::Uuid;

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

#[derive(Debug)]
struct ProductStore {
    user: RwLock<ProductUser>,
    workspaces: RwLock<HashMap<String, Workspace>>,
    issues: RwLock<HashMap<String, Issue>>,
    projects: RwLock<HashMap<String, ProductProject>>,
    agents: RwLock<HashMap<String, ProductAgent>>,
    issue_counter: RwLock<u32>,
}

impl ProductStore {
    fn new() -> Self {
        let now = Utc::now().timestamp_millis();
        let workspace_id = "local".to_string();
        let runtime_id = "runtime-local".to_string();
        let agent_id = "thinking-planning".to_string();
        let task_id = "task-active".to_string();

        let mut workspaces = HashMap::new();
        workspaces.insert(
            workspace_id.clone(),
            Workspace {
                id: workspace_id.clone(),
                name: "Local Workspace".to_string(),
                slug: "local".to_string(),
                description: Some("Tura local workbench".to_string()),
                context: Some("Local coding tasks, sessions, files, and agent runs.".to_string()),
                issue_prefix: "TURA".to_string(),
                avatar: Some("T".to_string()),
                created_at: now,
                updated_at: now,
            },
        );

        let mut agents = HashMap::new();
        agents.insert(
            agent_id.clone(),
            ProductAgent {
                id: agent_id.clone(),
                workspace_id: workspace_id.clone(),
                name: "Thinking Planning".to_string(),
                description: "Default Tura thinking agent with planning".to_string(),
                provider: "openai".to_string(),
                model: "default".to_string(),
                runtime_id: Some(runtime_id.clone()),
                status: "online".to_string(),
                visibility: "workspace".to_string(),
                thinking_level: Some("medium".to_string()),
                run_count_7d: 3,
                run_count_30d: 12,
            },
        );

        let active_task = TaskRun {
            id: task_id.clone(),
            issue_id: Some("issue-2".to_string()),
            agent_id: agent_id.clone(),
            runtime_id: Some(runtime_id.clone()),
            status: "running".to_string(),
            session_id: None,
            title: "Connect GUI and gateway".to_string(),
            created_at: now - 2_400_000,
            updated_at: now - 120_000,
        };

        let seeded_issues = vec![
            Issue {
                id: "issue-1".to_string(),
                workspace_id: workspace_id.clone(),
                number: 1,
                title: "Shape the local workbench".to_string(),
                description: "Show the few signals that matter: board, agents, runtime, session."
                    .to_string(),
                status: IssueStatus::Todo,
                priority: IssuePriority::High,
                position: 1,
                assignee_type: Some("agent".to_string()),
                assignee_id: Some(agent_id.clone()),
                project_id: Some("project-core".to_string()),
                labels: vec!["gui".to_string()],
                session_id: None,
                active_task: None,
                created_at: now - 3_600_000,
                updated_at: now - 900_000,
            },
            Issue {
                id: "issue-2".to_string(),
                workspace_id: workspace_id.clone(),
                number: 2,
                title: "Wire product APIs through gateway".to_string(),
                description:
                    "Expose Multica-compatible contracts without bypassing Tura runtime boundaries."
                        .to_string(),
                status: IssueStatus::InProgress,
                priority: IssuePriority::Urgent,
                position: 2,
                assignee_type: Some("agent".to_string()),
                assignee_id: Some(agent_id.clone()),
                project_id: Some("project-core".to_string()),
                labels: vec!["gateway".to_string()],
                session_id: None,
                active_task: Some(active_task.clone()),
                created_at: now - 2_900_000,
                updated_at: now - 120_000,
            },
            Issue {
                id: "issue-3".to_string(),
                workspace_id: workspace_id.clone(),
                number: 3,
                title: "Keep the transcript one click away".to_string(),
                description:
                    "Issue work should always map back to a Tura session when execution starts."
                        .to_string(),
                status: IssueStatus::Review,
                priority: IssuePriority::Medium,
                position: 3,
                assignee_type: Some("agent".to_string()),
                assignee_id: Some(agent_id.clone()),
                project_id: Some("project-core".to_string()),
                labels: vec!["runtime".to_string()],
                session_id: None,
                active_task: None,
                created_at: now - 1_900_000,
                updated_at: now - 300_000,
            },
            Issue {
                id: "issue-4".to_string(),
                workspace_id: workspace_id.clone(),
                number: 4,
                title: "Provider auth visible, not noisy".to_string(),
                description: "Surface model and auth health as compact controls.".to_string(),
                status: IssueStatus::Done,
                priority: IssuePriority::Low,
                position: 4,
                assignee_type: None,
                assignee_id: None,
                project_id: Some("project-core".to_string()),
                labels: vec!["provider".to_string()],
                session_id: None,
                active_task: None,
                created_at: now - 5_000_000,
                updated_at: now - 1_000_000,
            },
        ];
        let issues = seeded_issues
            .into_iter()
            .map(|issue| (issue.id.clone(), issue))
            .collect();

        let mut projects = HashMap::new();
        projects.insert(
            "project-core".to_string(),
            ProductProject {
                id: "project-core".to_string(),
                workspace_id: workspace_id.clone(),
                title: "Tura GUI".to_string(),
                description: "Minimal gateway-backed workbench".to_string(),
                status: "active".to_string(),
                priority: "high".to_string(),
                lead_type: Some("agent".to_string()),
                lead_id: Some(agent_id),
                created_at: now - 7_200_000,
                updated_at: now - 120_000,
            },
        );

        Self {
            user: RwLock::new(ProductUser {
                id: "local-user".to_string(),
                email: "local@tura.dev".to_string(),
                name: "Local User".to_string(),
                avatar_url: None,
                language: "en".to_string(),
                timezone: "Europe/Paris".to_string(),
                onboarded_at: Some(now),
            }),
            workspaces: RwLock::new(workspaces),
            issues: RwLock::new(issues),
            projects: RwLock::new(projects),
            agents: RwLock::new(agents),
            issue_counter: RwLock::new(4),
        }
    }

    fn workspace_id(&self, query: &IssueQuery) -> String {
        if let Some(workspace_id) = query.workspace_id.clone() {
            return workspace_id;
        }
        if let Some(slug) = query.workspace_slug.as_deref() {
            if let Some(workspace) = self
                .workspaces
                .read()
                .values()
                .find(|workspace| workspace.slug == slug)
            {
                return workspace.id.clone();
            }
        }
        "local".to_string()
    }
}

lazy_static! {
    static ref PRODUCT_STORE: ProductStore = ProductStore::new();
}

pub async fn public_config() -> Json<PublicConfig> {
    Json(PublicConfig {
        deployment_mode: "local".to_string(),
        signup_enabled: false,
        google_oauth_enabled: false,
        version: env!("CARGO_PKG_VERSION").to_string(),
    })
}

pub async fn current_user() -> Json<ProductUser> {
    Json(PRODUCT_STORE.user.read().clone())
}

pub fn current_user_snapshot() -> ProductUser {
    PRODUCT_STORE.user.read().clone()
}

pub async fn patch_current_user(Json(input): Json<UserPatch>) -> Json<ProductUser> {
    let mut user = PRODUCT_STORE.user.write();
    if let Some(name) = input.name.filter(|value| !value.trim().is_empty()) {
        user.name = name;
    }
    if let Some(language) = input.language.filter(|value| !value.trim().is_empty()) {
        user.language = language;
    }
    if let Some(timezone) = input.timezone.filter(|value| !value.trim().is_empty()) {
        user.timezone = timezone;
    }
    Json(user.clone())
}

pub async fn list_workspaces() -> Json<Vec<Workspace>> {
    Json(sorted_values(PRODUCT_STORE.workspaces.read().clone()))
}

pub async fn list_issues(Query(query): Query<IssueQuery>) -> Json<Vec<Issue>> {
    Json(filter_issues(query))
}

pub async fn quick_create_issue(Json(input): Json<IssueInput>) -> Json<Issue> {
    Json(create_issue_record(input))
}

pub async fn patch_issue(
    Path(issue_id): Path<String>,
    Json(input): Json<IssueInput>,
) -> Json<Option<Issue>> {
    let mut issues = PRODUCT_STORE.issues.write();
    let Some(issue) = issues.get_mut(&issue_id) else {
        return Json(None);
    };
    if let Some(title) = input.title.filter(|value| !value.trim().is_empty()) {
        issue.title = title;
    }
    if let Some(description) = input.description {
        issue.description = description;
    }
    if let Some(status) = input.status {
        issue.status = status;
    }
    if let Some(priority) = input.priority {
        issue.priority = priority;
    }
    if input.assignee_type.is_some() {
        issue.assignee_type = input.assignee_type;
    }
    if input.assignee_id.is_some() {
        issue.assignee_id = input.assignee_id;
    }
    if input.project_id.is_some() {
        issue.project_id = input.project_id;
    }
    if let Some(labels) = input.labels {
        issue.labels = labels;
    }
    if input.session_id.is_some() {
        issue.session_id = input.session_id;
    }
    issue.updated_at = Utc::now().timestamp_millis();
    Json(Some(issue.clone()))
}

pub async fn issue_usage(Path(_issue_id): Path<String>) -> Json<Value> {
    Json(serde_json::json!({
        "tasks": 1,
        "tokens": 1840,
        "cost": 0.04
    }))
}

pub async fn list_product_projects() -> Json<Vec<ProductProject>> {
    Json(sorted_values(PRODUCT_STORE.projects.read().clone()))
}

pub async fn list_product_agents() -> Json<Vec<ProductAgent>> {
    let mut agents = sorted_values(PRODUCT_STORE.agents.read().clone());
    for agent in &mut agents {
        let session_count = session_store()
            .list_sessions()
            .into_iter()
            .filter(|session| session.agent.as_deref() == Some(agent.id.as_str()))
            .count() as u32;
        agent.run_count_7d = agent.run_count_7d.max(session_count);
        agent.run_count_30d = agent.run_count_30d.max(session_count);
    }
    Json(agents)
}

pub async fn agent_templates() -> Json<Vec<Value>> {
    Json(vec![
        serde_json::json!({"id":"bug-fixer","name":"Bug fixer","description":"Find, patch, verify."}),
        serde_json::json!({"id":"frontend-builder","name":"Frontend builder","description":"Build compact product UI."}),
        serde_json::json!({"id":"webapp-tester","name":"Web app tester","description":"Run Playwright checks."}),
    ])
}

fn filter_issues(query: IssueQuery) -> Vec<Issue> {
    let workspace_id = PRODUCT_STORE.workspace_id(&query);
    let search = query
        .search
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_ascii_lowercase);
    let mut issues: Vec<_> = PRODUCT_STORE
        .issues
        .read()
        .values()
        .filter(|issue| issue.workspace_id == workspace_id)
        .filter(|issue| {
            query
                .status
                .as_ref()
                .is_none_or(|status| &issue.status == status)
        })
        .filter(|issue| {
            query
                .project_id
                .as_ref()
                .is_none_or(|project_id| issue.project_id.as_ref() == Some(project_id))
        })
        .filter(|issue| {
            search.as_ref().is_none_or(|search| {
                issue.title.to_ascii_lowercase().contains(search)
                    || issue.description.to_ascii_lowercase().contains(search)
            })
        })
        .cloned()
        .collect();
    issues.sort_by(|left, right| {
        left.position
            .cmp(&right.position)
            .then_with(|| left.number.cmp(&right.number))
    });
    issues
}

fn create_issue_record(input: IssueInput) -> Issue {
    let now = Utc::now().timestamp_millis();
    let mut counter = PRODUCT_STORE.issue_counter.write();
    *counter += 1;
    let issue = Issue {
        id: Uuid::new_v4().to_string(),
        workspace_id: "local".to_string(),
        number: *counter,
        title: input.title.unwrap_or_else(|| "Untitled issue".to_string()),
        description: input.description.unwrap_or_default(),
        status: input.status.unwrap_or(IssueStatus::Todo),
        priority: input.priority.unwrap_or(IssuePriority::Medium),
        position: i64::from(*counter),
        assignee_type: input.assignee_type,
        assignee_id: input.assignee_id,
        project_id: input.project_id,
        labels: input.labels.unwrap_or_default(),
        session_id: input.session_id,
        active_task: None,
        created_at: now,
        updated_at: now,
    };
    PRODUCT_STORE
        .issues
        .write()
        .insert(issue.id.clone(), issue.clone());
    issue
}

fn sorted_values<T>(map: HashMap<String, T>) -> Vec<T>
where
    T: Clone + Serialize,
{
    let mut entries: Vec<_> = map.into_iter().collect();
    entries.sort_by(|left, right| left.0.cmp(&right.0));
    entries.into_iter().map(|(_, value)| value).collect()
}

#[cfg(test)]
mod tests {
    use super::{create_issue_record, filter_issues, IssueInput, IssueQuery, IssueStatus};

    #[test]
    fn issue_search_filters_title() {
        let created = create_issue_record(IssueInput {
            title: Some("Unique product board item".to_string()),
            description: None,
            status: Some(IssueStatus::Todo),
            priority: None,
            assignee_type: None,
            assignee_id: None,
            project_id: None,
            labels: None,
            session_id: None,
        });

        let found = filter_issues(IssueQuery {
            workspace_id: Some("local".to_string()),
            workspace_slug: None,
            status: None,
            search: Some("unique product".to_string()),
            project_id: None,
        });

        assert!(found.iter().any(|issue| issue.id == created.id));
    }

    #[test]
    fn issue_input_can_bind_session() {
        let created = create_issue_record(IssueInput {
            title: Some("Session linked issue".to_string()),
            description: None,
            status: Some(IssueStatus::Todo),
            priority: None,
            assignee_type: None,
            assignee_id: None,
            project_id: None,
            labels: None,
            session_id: Some("session-test".to_string()),
        });

        assert_eq!(created.session_id.as_deref(), Some("session-test"));
    }
}
