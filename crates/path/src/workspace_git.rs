use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

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

    if !workspace.join(".git").exists() && run_git(workspace, &["init"]).is_err() {
        return Ok(());
    }
    let _ = ensure_tura_git_exclude(workspace);
    Ok(())
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
    crate::process_hardening::hide_child_console_window(&mut command);
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

#[cfg(test)]
mod tests {
    use super::ensure_workspace_git_repo;

    #[test]
    fn initializes_repository_and_excludes_runtime_state() {
        let temp = tempfile::tempdir().expect("temp workspace");

        ensure_workspace_git_repo(temp.path()).expect("workspace should initialize");

        assert!(temp.path().join(".git").exists());
        let exclude = std::fs::read_to_string(temp.path().join(".git/info/exclude"))
            .expect("git exclude should exist");
        assert!(exclude.lines().any(|line| line.trim() == ".tura/"));
        assert!(exclude.lines().any(|line| line.trim() == "sessions/"));
    }

    #[test]
    fn rejects_empty_workspace_path() {
        assert_eq!(
            ensure_workspace_git_repo("").expect_err("empty path should fail"),
            "workspace path is empty"
        );
    }
}
