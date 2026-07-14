use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use crate::state_machine::session_management::{PlanStatus, SessionManagement};
use tracing::warn;

const TURA_GIT_USER_NAME: &str = "Tura";
const TURA_GIT_USER_EMAIL: &str = "tura@local.invalid";
const TURA_EXCLUDE_LINES: &[&str] = &[".tura/", "sessions/"];

pub fn ensure_workspace_git_repo(workspace: impl AsRef<Path>) -> Result<(), String> {
    let workspace = workspace.as_ref();
    if workspace.as_os_str().is_empty() {
        return Err("workspace path is empty".to_string());
    }
    fs::create_dir_all(workspace).map_err(|error| {
        format!(
            "failed to create workspace directory {}: {error}",
            workspace.display()
        )
    })?;

    let git_marker = workspace.join(".git");
    if !git_marker.exists() {
        if let Err(error) = run_git(workspace, &["init"]) {
            warn!(
                directory = %workspace.display(),
                error = %error,
                "failed to initialize workspace git repository; continuing without git checkpoints"
            );
            return Ok(());
        }
    }
    if let Err(error) = ensure_tura_git_exclude(workspace) {
        warn!(
            directory = %workspace.display(),
            error = %error,
            "failed to update workspace git exclude; continuing without git exclude setup"
        );
    }
    Ok(())
}

pub fn commit_session_checkpoint(
    session: &SessionManagement,
    event: impl AsRef<str>,
) -> Result<Option<String>, String> {
    let workspace = &session.session_directory;
    ensure_workspace_git_repo(workspace)?;

    run_git(workspace, &["add", "-A", "--", "."])?;

    let event = normalized_line(event.as_ref(), "session_exit");
    let task_group = session_task_group(session);
    let subject = format!(
        "tura {event} {}: {}",
        session.session_id,
        truncate_for_subject(&task_group, 72)
    );
    let body = format!(
        "Session-Id: {}\nTask-Group: {}\nEvent: {}",
        session.session_id, task_group, event
    );

    let user_name_config = format!("user.name={TURA_GIT_USER_NAME}");
    let user_email_config = format!("user.email={TURA_GIT_USER_EMAIL}");
    run_git(
        workspace,
        &[
            "-c",
            &user_name_config,
            "-c",
            &user_email_config,
            "commit",
            "--allow-empty",
            "-m",
            &subject,
            "-m",
            &body,
        ],
    )?;

    let output = run_git(workspace, &["rev-parse", "HEAD"])?;
    let hash = String::from_utf8_lossy(&output.stdout).trim().to_string();
    Ok((!hash.is_empty()).then_some(hash))
}

fn ensure_tura_git_exclude(workspace: &Path) -> Result<(), String> {
    let output = run_git(workspace, &["rev-parse", "--git-path", "info/exclude"])?;
    let raw_path = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if raw_path.is_empty() {
        return Ok(());
    }
    let exclude_path = if Path::new(&raw_path).is_absolute() {
        PathBuf::from(raw_path)
    } else {
        workspace.join(raw_path)
    };
    if let Some(parent) = exclude_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "failed to create git exclude directory {}: {error}",
                parent.display()
            )
        })?;
    }
    let existing = fs::read_to_string(&exclude_path).unwrap_or_default();
    let existing_lines = existing.lines().map(str::trim).collect::<Vec<_>>();
    let missing = TURA_EXCLUDE_LINES
        .iter()
        .copied()
        .filter(|line| !existing_lines.iter().any(|existing| existing == line))
        .collect::<Vec<_>>();
    if missing.is_empty() {
        return Ok(());
    }
    let mut updated = existing;
    if !updated.is_empty() && !updated.ends_with('\n') {
        updated.push('\n');
    }
    for line in missing {
        updated.push_str(line);
        updated.push('\n');
    }
    fs::write(&exclude_path, updated).map_err(|error| {
        format!(
            "failed to update git exclude {}: {error}",
            exclude_path.display()
        )
    })
}

fn run_git(workspace: &Path, args: &[&str]) -> Result<Output, String> {
    let mut command = Command::new("git");
    command.arg("-C").arg(workspace).args(args);
    tura_path::process_hardening::hide_child_console_window(&mut command);
    let output = command
        .output()
        .map_err(|error| format!("failed to run git in {}: {error}", workspace.display()))?;
    if output.status.success() {
        return Ok(output);
    }
    Err(format!(
        "git -C {} {} failed with status {}\nstdout:\n{}\nstderr:\n{}",
        workspace.display(),
        args.join(" "),
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    ))
}

fn session_task_group(session: &SessionManagement) -> String {
    let plan_summary = session.task_plan.plan_summary.trim();
    let task_group = if !plan_summary.is_empty() {
        plan_summary
    } else {
        session
            .task_plan
            .detailed_tasks
            .iter()
            .find(|task| matches!(task.status, PlanStatus::Doing | PlanStatus::Todo))
            .map(|task| task.task_summary.as_str())
            .or_else(|| {
                session
                    .task_plan
                    .detailed_tasks
                    .iter()
                    .find(|task| !task.task_summary.trim().is_empty())
                    .map(|task| task.task_summary.as_str())
            })
            .unwrap_or(session.session_name.as_str())
    };
    normalized_line(task_group, "untitled task group")
}

fn normalized_line(value: &str, fallback: &str) -> String {
    let normalized = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.is_empty() {
        fallback.to_string()
    } else {
        normalized
    }
}

fn truncate_for_subject(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        return value.to_string();
    }
    value.chars().take(max_chars).collect::<String>()
}

#[cfg(test)]
mod tests {
    use super::{commit_session_checkpoint, ensure_workspace_git_repo};
    use crate::state_machine::session_management::{
        PlanStatus, SessionInput, SessionManagement, TaskStep,
    };
    use chrono::Utc;
    use std::process::Command;

    #[test]
    fn ensure_workspace_git_repo_initializes_local_git_and_excludes_tura_state() {
        let temp = tempfile::tempdir().expect("temp workspace");

        ensure_workspace_git_repo(temp.path()).expect("workspace git init");

        assert!(temp.path().join(".git").exists());
        let exclude = temp.path().join(".git").join("info").join("exclude");
        let content = std::fs::read_to_string(exclude).expect("git exclude");
        assert!(content.lines().any(|line| line.trim() == ".tura/"));
        assert!(content.lines().any(|line| line.trim() == "sessions/"));
    }

    #[test]
    fn commit_session_checkpoint_creates_commit_with_session_id_and_task_group() {
        let temp = tempfile::tempdir().expect("temp workspace");
        std::fs::write(temp.path().join("src.txt"), "first").expect("fixture file");
        let mut session = SessionManagement::new(
            "session-git-test".to_string(),
            "Git test".to_string(),
            temp.path().to_path_buf(),
            false,
            "coding".to_string(),
            SessionInput {
                user_input: "commit workspace".to_string(),
                file_input: Vec::new(),
                agent: None,
                runtime_context: None,
                planning_mode_override: None,
            },
            "commit workspace".to_string(),
            Utc::now(),
        );
        session.task_plan.plan_summary = "Runtime git checkpoint".to_string();
        session.task_plan.detailed_tasks.push(TaskStep {
            task_id: "task-1".to_string(),
            task_summary: "Runtime git checkpoint".to_string(),
            status: PlanStatus::Doing,
            ..TaskStep::default()
        });

        let hash = commit_session_checkpoint(&session, "completed")
            .expect("session checkpoint commit")
            .expect("commit hash");
        assert!(!hash.is_empty());

        let output = Command::new("git")
            .arg("-C")
            .arg(temp.path())
            .args(["log", "-1", "--pretty=%B"])
            .output()
            .expect("git log");
        assert!(output.status.success());
        let message = String::from_utf8_lossy(&output.stdout);
        assert!(message.contains("session-git-test"));
        assert!(message.contains("Session-Id: session-git-test"));
        assert!(message.contains("Task-Group: Runtime git checkpoint"));
        assert!(message.contains("Runtime git checkpoint"));
        assert!(message.contains("completed"));
    }

    #[test]
    fn commit_session_checkpoint_allows_empty_terminal_checkpoint() {
        let temp = tempfile::tempdir().expect("temp workspace");
        let session = test_session(temp.path(), "session-empty-commit", "Idle checkpoint");

        let first = commit_session_checkpoint(&session, "completed")
            .expect("first checkpoint commit")
            .expect("first commit hash");
        let second = commit_session_checkpoint(&session, "completed")
            .expect("second checkpoint commit")
            .expect("second commit hash");

        assert_ne!(
            first, second,
            "end-of-turn checkpoint commits must be durable even when the workspace has no file diff"
        );
        let output = Command::new("git")
            .arg("-C")
            .arg(temp.path())
            .args(["rev-list", "--count", "HEAD"])
            .output()
            .expect("git rev-list");
        assert!(output.status.success());
        assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "2");
    }

    #[test]
    fn commit_session_checkpoint_normalizes_multiline_event_and_task_group() {
        let temp = tempfile::tempdir().expect("temp workspace");
        let mut session = test_session(temp.path(), "session-normalized-commit", "Fallback");
        session.task_plan.plan_summary = "  Runtime\nterminal\tcheckpoint  ".to_string();

        commit_session_checkpoint(&session, "  completed\nwith\tspacing  ")
            .expect("normalized checkpoint commit")
            .expect("commit hash");

        let output = Command::new("git")
            .arg("-C")
            .arg(temp.path())
            .args(["log", "-1", "--pretty=%B"])
            .output()
            .expect("git log");
        assert!(output.status.success());
        let message = String::from_utf8_lossy(&output.stdout);
        assert!(message.contains("Task-Group: Runtime terminal checkpoint"));
        assert!(message.contains("Event: completed with spacing"));
        assert!(!message.contains("completed\nwith"));
    }

    #[test]
    fn commit_session_checkpoint_ignores_nested_runtime_sessions_git_repo() {
        let temp = tempfile::tempdir().expect("temp workspace");
        std::fs::write(temp.path().join("src.txt"), "workspace content").expect("fixture file");
        let sessions_dir = temp.path().join("sessions");
        std::fs::create_dir_all(&sessions_dir).expect("nested sessions dir fixture");
        let nested_init = Command::new("git")
            .arg("-C")
            .arg(&sessions_dir)
            .arg("init")
            .output()
            .expect("nested sessions git init");
        assert!(nested_init.status.success());

        let session = test_session(temp.path(), "session-nested-state", "Nested runtime state");

        commit_session_checkpoint(&session, "completed")
            .expect("checkpoint must ignore nested runtime session state")
            .expect("commit hash");

        let output = Command::new("git")
            .arg("-C")
            .arg(temp.path())
            .args(["status", "--short", "--", "sessions"])
            .output()
            .expect("git status sessions");
        assert!(output.status.success());
        assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "");
    }

    #[test]
    fn ensure_workspace_git_repo_rejects_empty_workspace_path() {
        let error = ensure_workspace_git_repo("").expect_err("empty path should be rejected");

        assert_eq!(error, "workspace path is empty");
    }

    #[test]
    fn ensure_workspace_git_repo_allows_unusable_git_metadata() {
        let temp = tempfile::tempdir().expect("temp workspace");
        std::fs::write(temp.path().join(".git"), "not a git directory")
            .expect("invalid git marker fixture");

        ensure_workspace_git_repo(temp.path())
            .expect("git metadata failures should not block runtime startup");
    }

    fn test_session(
        workspace: &std::path::Path,
        session_id: &str,
        session_name: &str,
    ) -> SessionManagement {
        SessionManagement::new(
            session_id.to_string(),
            session_name.to_string(),
            workspace.to_path_buf(),
            false,
            "coding".to_string(),
            SessionInput {
                user_input: "commit workspace".to_string(),
                file_input: Vec::new(),
                agent: None,
                runtime_context: None,
                planning_mode_override: None,
            },
            "commit workspace".to_string(),
            Utc::now(),
        )
    }
}
