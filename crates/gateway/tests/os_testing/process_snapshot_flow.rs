use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{anyhow, Context, Result};
use gateway::session::process_snapshot::{
    collect_runtime_shell_process_snapshot, collect_session_process_snapshot,
    stop_runtime_shell_process, stop_session_process, SessionProcessInfo,
};

#[test]
fn gateway_process_snapshot_business_flow_isolates_and_stops_session_processes() -> Result<()> {
    let workspace = tempfile::tempdir().context("create session workspace")?;
    let other_workspace = tempfile::tempdir().context("create unrelated workspace")?;
    let mut child = spawn_long_running_child(workspace.path(), false)?;

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

#[test]
fn gateway_runtime_shell_process_snapshot_filters_and_stops_only_marked_processes() -> Result<()> {
    let workspace = tempfile::tempdir().context("create session workspace")?;
    let mut native_child = spawn_long_running_child(workspace.path(), false)?;
    let mut shell_child = spawn_long_running_child(workspace.path(), true)?;

    let native_process = wait_for_process_snapshot(workspace.path(), native_child.id())
        .context("native child appears in broad snapshot")?;
    let shell_process = wait_for_process_snapshot(workspace.path(), shell_child.id())
        .context("runtime shell child appears in broad snapshot")?;
    assert_eq!(native_process.kind, "workspace");
    assert_eq!(shell_process.kind, "runtime_shell");

    wait_until(Duration::from_secs(8), || {
        let snapshot = collect_runtime_shell_process_snapshot(workspace.path());
        let has_shell = snapshot
            .processes
            .iter()
            .any(|process| process.pid == shell_child.id());
        let has_native = snapshot
            .processes
            .iter()
            .any(|process| process.pid == native_child.id());
        if has_shell && !has_native {
            Ok(())
        } else {
            Err(anyhow!(
                "runtime shell snapshot mismatch: has_shell={has_shell}, has_native={has_native}, snapshot={:?}",
                snapshot.processes
            ))
        }
    })?;

    let native_error = stop_runtime_shell_process(workspace.path(), native_child.id())
        .expect_err("runtime shell stopper must reject unmarked native process");
    assert!(
        native_error.contains("not a runtime shell background process"),
        "unexpected native rejection: {native_error}"
    );
    assert!(
        native_child.try_wait()?.is_none(),
        "runtime shell stopper must not kill unmarked native process"
    );

    stop_runtime_shell_process(workspace.path(), shell_child.id())
        .map_err(|error| anyhow!("stop runtime shell child: {error}"))?;
    wait_for_child_exit(&mut shell_child).context("runtime shell child exits after stop")?;

    let _ = native_child.kill();
    let _ = native_child.wait();
    Ok(())
}

fn spawn_long_running_child(workspace: &Path, runtime_shell_process: bool) -> Result<Child> {
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
    if runtime_shell_process {
        command.env("TURA_BACKGROUND_PROCESS_KIND", "runtime_shell");
    }
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
