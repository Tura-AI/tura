#![cfg(feature = "business-tests")]

use anyhow::{Context, Result};
use axum::{
    extract::Query,
    http::{HeaderMap, HeaderValue, StatusCode},
    Json,
};
use gateway::api::{
    global::{get_config, health, patch_config},
    path::get_paths,
    project::{create_named_workspace, get_current_project, list_projects, use_default_workspace},
    types::{ConfigPatch, PathParams, ProjectDirectoryParams, WorkspaceCreateRequest},
};
use std::{
    env,
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
};
use tempfile::TempDir;

static ENV_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

#[tokio::test]
async fn workspace_config_path_and_project_apis_share_a_local_workspace_view() -> Result<()> {
    let _guard = ENV_LOCK.lock().await;
    let temp = TempDir::new().context("create temp workspace root")?;
    let home = temp.path().join("home");
    let documents = home.join("Documents");
    let appdata = temp.path().join("roaming");
    let local_appdata = temp.path().join("local");
    let project_root = temp.path().join("gateway-root");
    let selected = temp.path().join("selected workspace");
    let header_selected = temp.path().join("encoded workspace");
    for directory in [
        home.as_path(),
        documents.as_path(),
        appdata.as_path(),
        local_appdata.as_path(),
        project_root.as_path(),
        selected.as_path(),
        header_selected.as_path(),
    ] {
        fs::create_dir_all(directory)
            .with_context(|| format!("create directory {}", directory.display()))?;
    }

    let _env = EnvGuard::set([
        ("USERPROFILE", home.as_os_str()),
        ("HOME", home.as_os_str()),
        ("APPDATA", appdata.as_os_str()),
        ("LOCALAPPDATA", local_appdata.as_os_str()),
        ("TURA_PROJECT_ROOT", project_root.as_os_str()),
        ("LOG_PATH", temp.path().join("logs").as_os_str()),
    ]);
    let _cwd = CurrentDirGuard::change_to(temp.path())?;

    let Json(health_body) = health().await;
    assert!(health_body.healthy);
    assert_eq!(
        normalize_path(&health_body.root),
        normalize_path(&project_root)
    );
    assert_eq!(
        health_body.dev_log_path.as_deref().map(normalize_slashes),
        Some(normalize_path(temp.path().join("logs")))
    );

    let Json(initial_config) = get_config().await;
    assert!(initial_config.model.is_some());
    let Json(updated_config) = patch_config(Json(ConfigPatch {
        language: Some("zh-CN".to_string()),
        theme: Some("contrast".to_string()),
        model: Some("openai/gpt-5.5".to_string()),
        agent: Some("coding".to_string()),
        skill_folders: Some(vec![selected.to_string_lossy().to_string()]),
    }))
    .await;
    assert_eq!(updated_config.language.as_deref(), Some("zh-CN"));
    assert_eq!(updated_config.theme.as_deref(), Some("contrast"));
    assert_eq!(updated_config.model.as_deref(), Some("openai/gpt-5.5"));
    assert_eq!(updated_config.agent.as_deref(), Some("coding"));
    assert_eq!(
        updated_config.skill_folders,
        vec![selected.to_string_lossy().to_string()]
    );

    let Json(query_paths) = get_paths(
        HeaderMap::new(),
        Query(PathParams {
            directory: Some(selected.to_string_lossy().to_string()),
        }),
    )
    .await;
    assert_eq!(normalize_slashes(query_paths.home), normalize_path(&home));
    assert_eq!(
        normalize_slashes(query_paths.config),
        normalize_path(&appdata)
    );
    assert_eq!(
        normalize_slashes(query_paths.state),
        normalize_path(&local_appdata)
    );
    assert_eq!(
        normalize_slashes(query_paths.worktree),
        normalize_path(&selected)
    );
    assert_eq!(
        normalize_slashes(query_paths.directory),
        normalize_path(&selected)
    );

    let mut headers = HeaderMap::new();
    headers.insert(
        "x-opencode-directory",
        HeaderValue::from_str(&percent_encode(&header_selected.to_string_lossy()))
            .context("encode header path")?,
    );
    let Json(header_paths) =
        get_paths(headers.clone(), Query(PathParams { directory: None })).await;
    assert_eq!(
        normalize_slashes(header_paths.worktree),
        normalize_path(&header_selected)
    );

    let Json(current_from_query) = get_current_project(
        HeaderMap::new(),
        Query(ProjectDirectoryParams {
            directory: Some(selected.to_string_lossy().to_string()),
        }),
    )
    .await;
    let current_from_query = current_from_query
        .project
        .context("query directory should create current project")?;
    assert_eq!(
        normalize_slashes(current_from_query.worktree),
        normalize_path(&selected)
    );

    let Json(current_from_header) =
        get_current_project(headers, Query(ProjectDirectoryParams { directory: None })).await;
    let current_from_header = current_from_header
        .project
        .context("header directory should create current project")?;
    assert_eq!(
        normalize_slashes(current_from_header.worktree),
        normalize_path(&header_selected)
    );

    let Json(default_project) = use_default_workspace()
        .await
        .map_err(|(_, body)| anyhow::anyhow!(body))?;
    assert_eq!(default_project.name.as_deref(), Some("tura_workspace"));
    assert!(PathBuf::from(&default_project.worktree).is_dir());
    assert_eq!(
        normalize_slashes(default_project.worktree),
        normalize_path(documents.join("tura_workspace"))
    );

    let Json(named_project) = create_named_workspace(Json(WorkspaceCreateRequest {
        name: Some(" Bad:/Name * With Spaces. ".to_string()),
    }))
    .await
    .map_err(|(_, body)| anyhow::anyhow!(body))?;
    assert_eq!(
        named_project.name.as_deref(),
        Some("Bad--Name - With Spaces")
    );
    assert!(PathBuf::from(&named_project.worktree).is_dir());
    assert_eq!(
        normalize_slashes(named_project.worktree),
        normalize_path(documents.join("Bad--Name - With Spaces"))
    );

    let Json(projects) = list_projects().await;
    assert!(projects
        .iter()
        .any(|project| normalize_slashes(&project.worktree) == normalize_path(&selected)));
    assert!(projects
        .iter()
        .any(|project| normalize_slashes(&project.worktree) == normalize_path(&header_selected)));
    assert!(projects
        .iter()
        .any(|project| normalize_slashes(&project.worktree)
            == normalize_path(documents.join("tura_workspace"))));

    Ok(())
}

struct CurrentDirGuard {
    original: PathBuf,
}

impl CurrentDirGuard {
    fn change_to(path: &Path) -> Result<Self> {
        let original = env::current_dir().context("read current directory")?;
        env::set_current_dir(path)
            .with_context(|| format!("change current directory to {}", path.display()))?;
        Ok(Self { original })
    }
}

impl Drop for CurrentDirGuard {
    fn drop(&mut self) {
        let _ = env::set_current_dir(&self.original);
    }
}

struct EnvGuard {
    entries: Vec<(&'static str, Option<OsString>)>,
}

impl EnvGuard {
    fn set<'a>(entries: impl IntoIterator<Item = (&'static str, &'a std::ffi::OsStr)>) -> Self {
        let mut previous = Vec::new();
        for (key, value) in entries {
            previous.push((key, env::var_os(key)));
            env::set_var(key, value);
        }
        Self { entries: previous }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        for (key, value) in self.entries.drain(..).rev() {
            if let Some(value) = value {
                env::set_var(key, value);
            } else {
                env::remove_var(key);
            }
        }
    }
}

fn normalize_path(path: impl AsRef<Path>) -> String {
    normalize_slashes(path.as_ref().to_string_lossy())
}

fn normalize_slashes(path: impl AsRef<str>) -> String {
    path.as_ref().replace('\\', "/")
}

fn percent_encode(value: &str) -> String {
    let mut encoded = String::new();
    for byte in value.as_bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(char::from(*byte));
            }
            byte => encoded.push_str(&format!("%{byte:02X}")),
        }
    }
    encoded
}
