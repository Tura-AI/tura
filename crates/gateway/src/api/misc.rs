//! Permission, Question, Agent, Command, VCS, LSP, Skill, Path, Formatter, Log handlers

use crate::api::types::*;
use crate::mock::global_store;
use axum::{
    extract::{Path, Query},
    http::HeaderMap,
    Json,
};
use std::collections::HashMap;
use std::fs;
use std::path::{Path as StdPath, PathBuf};
use std::process::Command as ProcessCommand;
use std::time::Duration;

// ============================================================================
// Permission
// ============================================================================

pub async fn list_permissions() -> Json<Vec<PermissionRequest>> {
    // Return all permissions across sessions
    let store = global_store();
    let mut all_permissions = Vec::new();
    for session in store.list_sessions() {
        all_permissions.extend(store.list_permissions(&session.id));
    }
    Json(all_permissions)
}

// ============================================================================
// Question
// ============================================================================

pub async fn list_questions() -> Json<Vec<QuestionRequest>> {
    Json(vec![])
}

pub async fn reject_question(Path(request_id): Path<String>) -> Json<bool> {
    Json(global_store().reject_question(&request_id))
}

pub async fn reply_question(
    Path(request_id): Path<String>,
    Json(payload): Json<QuestionReplyRequest>,
) -> Json<QuestionReplyResponse> {
    Json(QuestionReplyResponse {
        success: global_store().reply_question(&request_id, &payload.response),
    })
}

// ============================================================================
// Agent
// ============================================================================

pub async fn list_agents() -> Json<Vec<Agent>> {
    let model = crate::api::provider::active_agent_model().await;
    Json(vec![
        Agent {
            name: "general".to_string(),
            description: "General session agent".to_string(),
            mode: "primary".to_string(),
            native: true,
            hidden: false,
            model: None,
            options: HashMap::new(),
            permission: PermissionRuleset {
                allow: vec!["*".to_string()],
                deny: vec![],
            },
        },
        Agent {
            name: "coding_agent".to_string(),
            description: "Coding session agent".to_string(),
            mode: "primary".to_string(),
            native: true,
            hidden: false,
            model: model.clone(),
            options: HashMap::new(),
            permission: PermissionRuleset {
                allow: vec!["*".to_string()],
                deny: vec![],
            },
        },
        Agent {
            name: "coding_agent_fast".to_string(),
            description: "Coding session agent using the gpt-5.3-codex-spark prompt".to_string(),
            mode: "primary".to_string(),
            native: true,
            hidden: false,
            model,
            options: HashMap::new(),
            permission: PermissionRuleset {
                allow: vec!["*".to_string()],
                deny: vec![],
            },
        },
    ])
}

pub async fn console_switch() -> Json<bool> {
    Json(false)
}

// ============================================================================
// Command
// ============================================================================

pub async fn list_commands() -> Json<Vec<Command>> {
    Json(discover_commands())
}

pub async fn execute_command(
    Json(payload): Json<ExecuteCommandRequest>,
) -> Json<ExecuteCommandResponse> {
    let command_name = payload.command.trim().trim_start_matches('/').to_string();
    let command = discover_commands()
        .into_iter()
        .find(|command| command.name == command_name);
    let output = match command.and_then(|command| command.template) {
        Some(template) => render_command_template(&template, payload.args.as_deref().unwrap_or_default()),
        None => format!(
            "Command `{}` is not configured. Add a markdown or JSON command under .tura/commands, .opencode/command, .opencode/commands, command, or commands.",
            command_name
        ),
    };
    Json(ExecuteCommandResponse { output })
}

fn discover_commands() -> Vec<Command> {
    let mut commands = Vec::new();
    for directory in command_directories() {
        let Ok(entries) = fs::read_dir(&directory) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                if let Some(command) = command_from_file(&path) {
                    commands.push(command);
                }
            }
        }
    }
    commands.sort_by(|left, right| left.name.cmp(&right.name));
    commands.dedup_by(|left, right| left.name == right.name);
    commands
}

fn command_directories() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Some(current_directory) = global_store().get_current_directory() {
        roots.push(PathBuf::from(current_directory));
    }
    if let Ok(current_directory) = std::env::current_dir() {
        roots.push(current_directory);
    }

    let suffixes = [
        [".tura", "commands"].as_slice(),
        [".opencode", "command"].as_slice(),
        [".opencode", "commands"].as_slice(),
        ["command"].as_slice(),
        ["commands"].as_slice(),
    ];

    let mut directories = Vec::new();
    for root in roots {
        for suffix in suffixes {
            let mut directory = root.clone();
            for part in suffix {
                directory.push(part);
            }
            directories.push(directory);
        }
    }
    directories
}

fn command_from_file(path: &StdPath) -> Option<Command> {
    let extension = path.extension()?.to_string_lossy().to_ascii_lowercase();
    if !matches!(extension.as_str(), "md" | "txt" | "json") {
        return None;
    }
    let content = fs::read_to_string(path).ok()?;
    if extension == "json" {
        return command_from_json(path, &content);
    }
    let name = path.file_stem()?.to_string_lossy().to_string();
    let description = first_command_description(&content).unwrap_or_else(|| name.clone());
    Some(Command {
        name,
        description,
        agent: None,
        model: None,
        source: "command".to_string(),
        template: Some(content),
        subtask: false,
        hints: vec![],
    })
}

fn command_from_json(path: &StdPath, content: &str) -> Option<Command> {
    let value: serde_json::Value = serde_json::from_str(content).ok()?;
    let fallback_name = path.file_stem()?.to_string_lossy().to_string();
    let name = value
        .get("name")
        .and_then(serde_json::Value::as_str)
        .unwrap_or(&fallback_name)
        .to_string();
    let description = value
        .get("description")
        .and_then(serde_json::Value::as_str)
        .or_else(|| value.get("summary").and_then(serde_json::Value::as_str))
        .unwrap_or(&name)
        .to_string();
    Some(Command {
        name,
        description,
        agent: value
            .get("agent")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string),
        model: value
            .get("model")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string),
        source: "command".to_string(),
        template: value
            .get("template")
            .and_then(serde_json::Value::as_str)
            .or_else(|| value.get("prompt").and_then(serde_json::Value::as_str))
            .map(str::to_string),
        subtask: value
            .get("subtask")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false),
        hints: value
            .get("hints")
            .and_then(serde_json::Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(serde_json::Value::as_str)
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default(),
    })
}

fn first_command_description(content: &str) -> Option<String> {
    content
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(|line| line.trim_start_matches('#').trim().to_string())
        .filter(|line| !line.is_empty())
}

fn render_command_template(template: &str, args: &[String]) -> String {
    let joined_args = args.join(" ");
    let mut output = template.replace("$ARGUMENTS", &joined_args);
    for (index, arg) in args.iter().enumerate() {
        output = output.replace(&format!("${}", index + 1), arg);
    }
    output
}

pub async fn open_directory_picker(
    Json(payload): Json<DirectoryPickerRequest>,
) -> Result<Json<serde_json::Value>, (axum::http::StatusCode, String)> {
    let title = payload.title.clone();
    let selected = tokio::task::spawn_blocking(move || select_directory(title.as_deref()))
        .await
        .map_err(|error| {
            (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                format!("Directory picker task failed: {error}"),
            )
        })?
        .map_err(|error| {
            (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to open directory picker: {error}"),
            )
        })?;
    let result = match selected {
        Some(path) if payload.multiple.unwrap_or(false) => serde_json::json!([path]),
        Some(path) => serde_json::json!(path),
        None => serde_json::Value::Null,
    };
    Ok(Json(result))
}

pub(crate) fn select_directory(title: Option<&str>) -> anyhow::Result<Option<String>> {
    #[cfg(target_os = "windows")]
    {
        let escaped_title = title.unwrap_or("Select directory").replace('\'', "''");
        let script = format!(
            "Add-Type -AssemblyName System.Windows.Forms; \
             $f = New-Object System.Windows.Forms.Form; \
             $f.TopMost = $true; \
             $f.StartPosition = 'CenterScreen'; \
             $f.ShowInTaskbar = $false; \
             $d = New-Object System.Windows.Forms.FolderBrowserDialog; \
             $d.Description = '{}'; \
             $d.ShowNewFolderButton = $true; \
             if ($d.ShowDialog($f) -eq [System.Windows.Forms.DialogResult]::OK) {{ $d.SelectedPath }}; \
             $f.Dispose()",
            escaped_title,
        );
        let output = ProcessCommand::new("powershell")
            .args([
                "-NoProfile",
                "-STA",
                "-ExecutionPolicy",
                "Bypass",
                "-Command",
                &script,
            ])
            .output()?;
        if !output.status.success() {
            return Ok(None);
        }
        let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
        Ok((!path.is_empty()).then_some(path))
    }

    #[cfg(target_os = "macos")]
    {
        let prompt = applescript_string(title.unwrap_or("Select directory"));
        let script = format!("POSIX path of (choose folder with prompt {prompt})");
        let output = ProcessCommand::new("osascript")
            .args(["-e", &script])
            .output()?;
        if !output.status.success() {
            return Ok(None);
        }
        selected_path_from_stdout(&output.stdout)
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let title = title.unwrap_or("Select directory");
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."));
        let attempts: [(&str, Vec<String>); 3] = [
            (
                "zenity",
                vec![
                    "--file-selection".to_string(),
                    "--directory".to_string(),
                    "--title".to_string(),
                    title.to_string(),
                ],
            ),
            (
                "kdialog",
                vec![
                    "--title".to_string(),
                    title.to_string(),
                    "--getexistingdirectory".to_string(),
                    home.to_string_lossy().to_string(),
                ],
            ),
            (
                "yad",
                vec![
                    "--file-selection".to_string(),
                    "--directory".to_string(),
                    "--title".to_string(),
                    title.to_string(),
                ],
            ),
        ];

        let mut saw_picker = false;
        for (command, args) in attempts {
            match ProcessCommand::new(command).args(args).output() {
                Ok(output) => {
                    saw_picker = true;
                    if output.status.success() {
                        return selected_path_from_stdout(&output.stdout);
                    }
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error.into()),
            }
        }

        if saw_picker {
            Ok(None)
        } else {
            Err(anyhow::anyhow!(
                "No Linux directory picker was found. Install zenity, kdialog, or yad."
            ))
        }
    }
}

#[cfg(not(target_os = "windows"))]
fn selected_path_from_stdout(stdout: &[u8]) -> anyhow::Result<Option<String>> {
    let path = String::from_utf8_lossy(stdout).trim().to_string();
    Ok((!path.is_empty()).then_some(path))
}

#[cfg(target_os = "macos")]
fn applescript_string(value: &str) -> String {
    let escaped = value.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{escaped}\"")
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct DirectoryPickerRequest {
    pub title: Option<String>,
    pub multiple: Option<bool>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct ExecuteCommandRequest {
    pub command: String,
    pub args: Option<Vec<String>>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ExecuteCommandResponse {
    pub output: String,
}

// ============================================================================
// VCS
// ============================================================================

pub async fn get_vcs_info() -> Json<VcsInfo> {
    Json(vcs_info_for_directory(current_vcs_directory().as_deref()))
}

pub async fn get_vcs_diff() -> Json<VcsDiffResponse> {
    Json(VcsDiffResponse {
        files: git_diff_for_directory(current_vcs_directory().as_deref()),
    })
}

pub(crate) fn current_vcs_directory() -> Option<String> {
    global_store().get_current_directory().or_else(|| {
        std::env::current_dir()
            .ok()
            .map(|path| path.display().to_string())
    })
}

pub(crate) fn vcs_info_for_directory(directory: Option<&str>) -> VcsInfo {
    let branch = git_output(directory, &["branch", "--show-current"])
        .filter(|value| !value.is_empty())
        .or_else(|| git_output(directory, &["rev-parse", "--short", "HEAD"]))
        .unwrap_or_else(|| "unknown".to_string());
    let default_branch = git_output(directory, &["symbolic-ref", "refs/remotes/origin/HEAD"])
        .and_then(|value| value.rsplit('/').next().map(str::to_string))
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| {
            for candidate in ["main", "master"] {
                if git_output(
                    directory,
                    &["show-ref", "--verify", &format!("refs/heads/{candidate}")],
                )
                .is_some()
                {
                    return candidate.to_string();
                }
            }
            "unknown".to_string()
        });

    VcsInfo {
        branch,
        default_branch,
    }
}

pub(crate) fn git_diff_for_directory(directory: Option<&str>) -> Vec<FileDiff> {
    let Some(diff) = git_output(directory, &["diff", "--no-ext-diff", "--unified=3", "--"]) else {
        return Vec::new();
    };
    parse_unified_diff(&diff)
}

fn git_output(directory: Option<&str>, args: &[&str]) -> Option<String> {
    let mut command = ProcessCommand::new("git");
    command.args(args);
    if let Some(directory) = directory.filter(|value| !value.trim().is_empty()) {
        command.current_dir(StdPath::new(directory));
    }
    let output = command.output().ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn parse_unified_diff(diff: &str) -> Vec<FileDiff> {
    let mut files = Vec::new();
    let mut current: Option<FileDiff> = None;

    for line in diff.lines() {
        if line.starts_with("diff --git ") {
            if let Some(file) = current.take() {
                files.push(file);
            }
            current = Some(FileDiff {
                old_file_name: String::new(),
                new_file_name: String::new(),
                hunks: Vec::new(),
            });
            continue;
        }

        let Some(file) = current.as_mut() else {
            continue;
        };

        if let Some(old_name) = line.strip_prefix("--- ") {
            file.old_file_name = strip_diff_prefix(old_name);
            continue;
        }
        if let Some(new_name) = line.strip_prefix("+++ ") {
            file.new_file_name = strip_diff_prefix(new_name);
            continue;
        }
        if let Some(header) = line.strip_prefix("@@ ") {
            if let Some(hunk) = parse_hunk_header(header) {
                file.hunks.push(hunk);
            }
            continue;
        }
        if let Some(hunk) = file.hunks.last_mut() {
            hunk.lines.push(line.to_string());
        }
    }

    if let Some(file) = current {
        files.push(file);
    }

    files
        .into_iter()
        .filter(|file| !file.old_file_name.is_empty() || !file.new_file_name.is_empty())
        .collect()
}

fn strip_diff_prefix(value: &str) -> String {
    value
        .trim()
        .strip_prefix("a/")
        .or_else(|| value.trim().strip_prefix("b/"))
        .unwrap_or_else(|| value.trim())
        .to_string()
}

fn parse_hunk_header(header: &str) -> Option<DiffHunk> {
    let mut parts = header.split_whitespace();
    let old_part = parts.next()?.strip_prefix('-')?;
    let new_part = parts.next()?.strip_prefix('+')?;
    let (old_start, old_lines) = parse_hunk_range(old_part)?;
    let (new_start, new_lines) = parse_hunk_range(new_part)?;
    Some(DiffHunk {
        old_start,
        old_lines,
        new_start,
        new_lines,
        lines: Vec::new(),
    })
}

fn parse_hunk_range(value: &str) -> Option<(u32, u32)> {
    let (start, lines) = value.split_once(',').unwrap_or((value, "1"));
    Some((start.parse().ok()?, lines.parse().ok()?))
}

// ============================================================================
// LSP
// ============================================================================

pub async fn get_lsp_status() -> Json<Vec<LSPStatus>> {
    let statuses = router_services_status()
        .await
        .ok()
        .map(lsp_statuses_from_router)
        .unwrap_or_default();

    Json(statuses)
}

pub async fn get_service_status() -> Json<ServiceStatusResponse> {
    let router_url =
        std::env::var("TURA_ROUTER_URL").unwrap_or_else(|_| "http://127.0.0.1:8080".to_string());
    let session_directory = crate::mock::global_store()
        .get_current_directory()
        .or_else(|| {
            std::env::current_dir()
                .ok()
                .map(|path| path.display().to_string())
        });
    let session_processes = session_directory.as_deref().map(|directory| {
        crate::session::process_snapshot::collect_session_process_snapshot(std::path::Path::new(
            directory,
        ))
    });
    let mut response = ServiceStatusResponse {
        mano: ServiceHealth {
            status: "connected".to_string(),
            url: None,
            error: None,
        },
        router: ServiceHealth {
            status: "checking".to_string(),
            url: Some(router_url.clone()),
            error: None,
        },
        lsp: Vec::new(),
        session_processes,
        docker: crate::session::docker_snapshot::collect_docker_snapshot(),
    };

    match router_services_status().await {
        Ok(status) => {
            response.router.status = "connected".to_string();
            response.lsp = lsp_statuses_from_router(status);
        }
        Err(error) => {
            response.router.status = "error".to_string();
            response.router.error = Some(error);
        }
    }

    Json(response)
}

pub async fn stop_service_process(Path(pid): Path<u32>) -> Json<StopProcessResponse> {
    let Some(session_directory) =
        crate::mock::global_store()
            .get_current_directory()
            .or_else(|| {
                std::env::current_dir()
                    .ok()
                    .map(|path| path.display().to_string())
            })
    else {
        return Json(StopProcessResponse {
            success: false,
            message: "no current session directory is available".to_string(),
        });
    };

    let result = tokio::task::spawn_blocking(move || {
        crate::session::process_snapshot::stop_session_process(
            std::path::Path::new(&session_directory),
            pid,
        )
    })
    .await
    .map_err(|error| format!("process stop task failed: {error}"))
    .and_then(|result| result);

    Json(match result {
        Ok(()) => StopProcessResponse {
            success: true,
            message: format!("stopped process {pid}"),
        },
        Err(error) => StopProcessResponse {
            success: false,
            message: error,
        },
    })
}

async fn router_services_status() -> Result<RouterServicesStatus, String> {
    let router_url =
        std::env::var("TURA_ROUTER_URL").unwrap_or_else(|_| "http://127.0.0.1:8080".to_string());
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(2))
        .build()
        .map_err(|error| format!("failed to build router status client: {error}"))?;
    let url = format!("{}/services/status", router_url.trim_end_matches('/'));
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|error| format!("router status request failed: {error}"))?;

    if !response.status().is_success() {
        return Err(format!("router status returned HTTP {}", response.status()));
    }

    response
        .json::<RouterServicesStatus>()
        .await
        .map_err(|error| format!("failed to parse router status: {error}"))
}

fn lsp_statuses_from_router(status: RouterServicesStatus) -> Vec<LSPStatus> {
    status
        .workers
        .into_iter()
        .filter(|worker| worker.service_name == "lsp")
        .map(|worker| LSPStatus {
            id: worker.worker_id,
            name: worker.service_name,
            root: worker.url,
            pid: worker.pid,
            executable_path: Some(worker.executable_path),
            status: if worker.alive {
                "connected".to_string()
            } else {
                "error".to_string()
            },
        })
        .collect()
}

#[derive(Debug, serde::Serialize)]
pub struct ServiceStatusResponse {
    pub mano: ServiceHealth,
    pub router: ServiceHealth,
    pub lsp: Vec<LSPStatus>,
    pub session_processes: Option<crate::session::process_snapshot::SessionProcessSnapshot>,
    pub docker: crate::session::docker_snapshot::DockerSnapshot,
}

#[derive(Debug, serde::Serialize)]
pub struct ServiceHealth {
    pub status: String,
    pub url: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
struct RouterServicesStatus {
    #[serde(default)]
    workers: Vec<RouterWorkerStatus>,
}

#[derive(Debug, serde::Deserialize)]
struct RouterWorkerStatus {
    worker_id: String,
    service_name: String,
    url: String,
    alive: bool,
    #[serde(default)]
    pid: Option<u32>,
    #[serde(default)]
    executable_path: String,
}

#[derive(Debug, serde::Serialize)]
pub struct StopProcessResponse {
    pub success: bool,
    pub message: String,
}

// ============================================================================
// Skill
// ============================================================================

pub async fn list_skills() -> Json<Vec<Skill>> {
    Json(discover_skills())
}

pub async fn list_plugins() -> Json<Vec<PluginInfo>> {
    Json(discover_plugins())
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct PluginInfo {
    pub id: String,
    pub name: String,
    pub description: String,
    pub path: String,
    pub enabled: bool,
    pub skills: Vec<Skill>,
}

fn discover_skills() -> Vec<Skill> {
    let mut skills = Vec::new();
    for directory in skill_directories() {
        if directory.is_dir() {
            skills.extend(discover_skills_in_directory(&directory));
        }
    }
    skills.sort_by(|left, right| left.name.cmp(&right.name).then(left.path.cmp(&right.path)));
    skills.dedup_by(|left, right| left.name == right.name && left.path == right.path);
    skills
}

fn skill_directories() -> Vec<PathBuf> {
    let mut directories: Vec<PathBuf> = global_store()
        .get_config()
        .skill_folders
        .into_iter()
        .filter(|value| !value.trim().is_empty())
        .map(PathBuf::from)
        .collect();

    let mut roots = Vec::new();
    if let Some(current_directory) = global_store().get_current_directory() {
        roots.push(PathBuf::from(current_directory));
    }
    if let Ok(current_directory) = std::env::current_dir() {
        roots.push(current_directory);
    }
    if let Some(home) = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .ok()
        .filter(|value| !value.trim().is_empty())
    {
        roots.push(PathBuf::from(home));
    }

    for root in roots {
        directories.push(root.join(".tura").join("skills"));
        directories.push(root.join(".codex").join("skills"));
        directories.push(root.join("skills"));
        for plugin_root in plugin_roots_for_root(&root) {
            directories.push(plugin_root.join("skills"));
        }
    }
    directories
}

fn discover_plugins() -> Vec<PluginInfo> {
    let mut plugins = Vec::new();
    for root in discovery_roots() {
        for plugin_root in plugin_roots_for_root(&root) {
            if let Some(plugin) = plugin_from_directory(&plugin_root) {
                plugins.push(plugin);
            }
        }
    }
    plugins.sort_by(|left, right| left.id.cmp(&right.id).then(left.path.cmp(&right.path)));
    plugins.dedup_by(|left, right| left.id == right.id && left.path == right.path);
    plugins
}

fn discovery_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Some(current_directory) = global_store().get_current_directory() {
        roots.push(PathBuf::from(current_directory));
    }
    if let Ok(current_directory) = std::env::current_dir() {
        roots.push(current_directory);
    }
    if let Some(home) = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .ok()
        .filter(|value| !value.trim().is_empty())
    {
        roots.push(PathBuf::from(home));
    }
    roots
}

fn plugin_roots_for_root(root: &StdPath) -> Vec<PathBuf> {
    let mut plugin_roots = Vec::new();
    let candidates = [
        root.join(".codex").join("plugins"),
        root.join(".codex").join("plugins").join("cache"),
        root.join(".agents").join("plugins"),
        root.join(".tura").join("plugins"),
    ];
    for candidate in candidates {
        collect_plugin_roots(&candidate, 0, &mut plugin_roots);
    }
    plugin_roots
}

fn collect_plugin_roots(directory: &StdPath, depth: usize, output: &mut Vec<PathBuf>) {
    if depth > 3 || !directory.is_dir() {
        return;
    }
    if directory.join(".codex-plugin").join("plugin.json").exists()
        || directory.join("plugin.json").exists()
    {
        output.push(directory.to_path_buf());
        return;
    }
    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_plugin_roots(&path, depth + 1, output);
        }
    }
}

fn plugin_from_directory(directory: &StdPath) -> Option<PluginInfo> {
    let manifest_path = if directory.join(".codex-plugin").join("plugin.json").exists() {
        directory.join(".codex-plugin").join("plugin.json")
    } else {
        directory.join("plugin.json")
    };
    let content = fs::read_to_string(&manifest_path).ok()?;
    let value: serde_json::Value = serde_json::from_str(&content).ok()?;
    let fallback_id = directory.file_name()?.to_string_lossy().to_string();
    let id = value
        .get("id")
        .or_else(|| value.get("name"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or(&fallback_id)
        .to_string();
    let name = value
        .get("name")
        .and_then(serde_json::Value::as_str)
        .unwrap_or(&id)
        .to_string();
    let description = value
        .get("description")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_string();
    Some(PluginInfo {
        id,
        name,
        description,
        path: directory.display().to_string(),
        enabled: value
            .get("enabled")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(true),
        skills: discover_skills_in_directory(&directory.join("skills")),
    })
}

fn discover_skills_in_directory(directory: &StdPath) -> Vec<Skill> {
    let mut skills = Vec::new();
    if let Some(skill) = skill_from_directory(directory) {
        skills.push(skill);
        return skills;
    }
    let Ok(entries) = fs::read_dir(directory) else {
        return skills;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if let Some(skill) = skill_from_directory(&path) {
                skills.push(skill);
            }
        }
    }
    skills
}

fn skill_from_directory(directory: &StdPath) -> Option<Skill> {
    let json_path = directory.join("skill.json");
    if json_path.exists() {
        if let Some(skill) = skill_from_json(directory, &json_path) {
            return Some(skill);
        }
    }

    let markdown_path = directory.join("SKILL.md");
    if markdown_path.exists() {
        return skill_from_markdown(directory, &markdown_path);
    }
    None
}

fn skill_from_json(directory: &StdPath, path: &StdPath) -> Option<Skill> {
    let content = fs::read_to_string(path).ok()?;
    let value: serde_json::Value = serde_json::from_str(&content).ok()?;
    let fallback_name = directory.file_name()?.to_string_lossy().to_string();
    let name = value
        .get("name")
        .and_then(serde_json::Value::as_str)
        .unwrap_or(&fallback_name)
        .to_string();
    let description = value
        .get("description")
        .and_then(serde_json::Value::as_str)
        .or_else(|| value.get("summary").and_then(serde_json::Value::as_str))
        .unwrap_or(&name)
        .to_string();
    Some(Skill {
        name,
        description,
        path: directory.display().to_string(),
    })
}

fn skill_from_markdown(directory: &StdPath, path: &StdPath) -> Option<Skill> {
    let content = fs::read_to_string(path).ok()?;
    let fallback_name = directory.file_name()?.to_string_lossy().to_string();
    let name = markdown_title(&content).unwrap_or_else(|| fallback_name.clone());
    let description = markdown_description(&content).unwrap_or_else(|| name.clone());
    Some(Skill {
        name,
        description,
        path: directory.display().to_string(),
    })
}

fn markdown_title(content: &str) -> Option<String> {
    content
        .lines()
        .map(str::trim)
        .find_map(|line| line.strip_prefix("# ").map(str::trim))
        .map(str::to_string)
        .filter(|value| !value.is_empty())
}

fn markdown_description(content: &str) -> Option<String> {
    content
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#') && !line.starts_with("---"))
        .find(|line| !line.contains(':') || line.starts_with("Use "))
        .map(str::to_string)
}

// ============================================================================
// Path
// ============================================================================

pub async fn get_paths(headers: HeaderMap, Query(params): Query<PathParams>) -> Json<PathResponse> {
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .unwrap_or_else(|_| "C:\\Users\\default".to_string());
    let appdata =
        std::env::var("APPDATA").unwrap_or_else(|_| format!("{}\\AppData\\Roaming", home));
    let state =
        std::env::var("LOCALAPPDATA").unwrap_or_else(|_| format!("{}\\AppData\\Local", home));
    let cwd = std::env::current_dir()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let worktree = params
        .directory
        .or_else(|| encoded_header(&headers, "x-opencode-directory"))
        .unwrap_or(cwd);
    Json(PathResponse {
        home,
        state,
        config: appdata,
        worktree: worktree.clone(),
        directory: worktree,
    })
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct PathParams {
    pub directory: Option<String>,
}

fn encoded_header(headers: &HeaderMap, name: &str) -> Option<String> {
    let value = headers.get(name)?.to_str().ok()?;
    Some(percent_decode(value))
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

// ============================================================================
// Formatter
// ============================================================================

pub async fn format_code(Json(payload): Json<FormatRequest>) -> Json<FormatResponse> {
    Json(FormatResponse {
        formatted: payload.code,
    })
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct FormatRequest {
    pub code: String,
    pub language: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct FormatResponse {
    pub formatted: String,
}

// ============================================================================
// Log
// ============================================================================

pub async fn write_log(Json(payload): Json<LogRequest>) -> Json<bool> {
    println!(
        "[{}] {}: {}",
        payload.service,
        payload.level.to_uppercase(),
        payload.message
    );
    Json(true)
}

// ============================================================================
// Config Providers
// ============================================================================

pub async fn get_config_providers() -> Json<Vec<ProviderConfig>> {
    Json(vec![])
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ProviderConfig {
    pub id: String,
    pub name: String,
    pub requires_auth: bool,
}
