use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{anyhow, Context, Result};
use gateway::session::process_snapshot::{
    collect_session_process_snapshot, stop_session_process, SessionProcessInfo,
};

#[test]
fn gateway_process_snapshot_business_flow_isolates_and_stops_session_processes() -> Result<()> {
    let workspace = tempfile::tempdir().context("create session workspace")?;
    let other_workspace = tempfile::tempdir().context("create unrelated workspace")?;
    let mut child = spawn_long_running_child(workspace.path())?;

    let process = wait_for_process_snapshot(workspace.path(), child.id())
        .context("child appears in snapshot")?;
    assert_eq!(process.pid, child.id());
    assert_eq!(process.kind, "workspace");
    assert!(
        process
            .cwd
            .as_deref()
            .or(process.exe.as_deref())
            .or(Some(process.command_line.as_str()))
            .is_some_and(|value| path_text_mentions(value, workspace.path())),
        "snapshot should preserve enough location context for the session process: {process:?}"
    );

    let wrong_directory_error = stop_session_process(other_workspace.path(), child.id())
        .expect_err("stopping from a different session directory must be rejected");
    assert!(
        wrong_directory_error.contains("is not under this session directory"),
        "unexpected wrong-directory error: {wrong_directory_error}"
    );
    assert!(
        child.try_wait()?.is_none(),
        "wrong-directory stop must not kill the child process"
    );

    let missing_pid_error = stop_session_process(workspace.path(), u32::MAX)
        .expect_err("missing pid should be reported without killing other processes");
    assert!(
        missing_pid_error.contains("was not found"),
        "unexpected missing-pid error: {missing_pid_error}"
    );
    assert!(
        child.try_wait()?.is_none(),
        "missing pid stop must not affect the tracked child process"
    );

    stop_session_process(workspace.path(), child.id())
        .map_err(|error| anyhow!("stop session child: {error}"))?;
    wait_for_child_exit(&mut child).context("child exits after session stop")?;

    let after_stop = collect_session_process_snapshot(workspace.path());
    assert!(
        !after_stop
            .processes
            .iter()
            .any(|process| process.pid == child.id()),
        "stopped process should disappear from the session snapshot"
    );

    Ok(())
}

fn spawn_long_running_child(workspace: &Path) -> Result<Child> {
    let mut command = if cfg!(windows) {
        let mut command = Command::new("powershell");
        command.args([
            "-NoProfile",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            "Set-Content -Path child-ready.txt -Value $PID; Start-Sleep -Seconds 60",
        ]);
        command
    } else {
        let mut command = Command::new("sh");
        command.args(["-c", "echo $$ > child-ready.txt; sleep 60"]);
        command
    };
    command
        .current_dir(workspace)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .context("spawn long-running session child")
}

fn wait_for_process_snapshot(workspace: &Path, pid: u32) -> Result<SessionProcessInfo> {
    wait_until(Duration::from_secs(8), || {
        let snapshot = collect_session_process_snapshot(workspace);
        snapshot
            .processes
            .into_iter()
            .find(|process| process.pid == pid)
            .ok_or_else(|| anyhow!("process {pid} not visible yet"))
    })
}

fn wait_for_child_exit(child: &mut Child) -> Result<()> {
    wait_until(Duration::from_secs(8), || match child.try_wait()? {
        Some(_status) => Ok(()),
        None => Err(anyhow!("child still running")),
    })
}

fn wait_until<T>(timeout: Duration, mut attempt: impl FnMut() -> Result<T>) -> Result<T> {
    let started = Instant::now();
    let mut last_error = None;
    while started.elapsed() < timeout {
        match attempt() {
            Ok(value) => return Ok(value),
            Err(error) => last_error = Some(error),
        }
        thread::sleep(Duration::from_millis(100));
    }
    Err(last_error.unwrap_or_else(|| anyhow!("condition was not satisfied before timeout")))
}

fn path_text_mentions(text: &str, path: &Path) -> bool {
    let normalized_text = text.replace('\\', "/").to_ascii_lowercase();
    let normalized_path = path
        .display()
        .to_string()
        .replace('\\', "/")
        .to_ascii_lowercase();
    normalized_text.contains(&normalized_path)
}
