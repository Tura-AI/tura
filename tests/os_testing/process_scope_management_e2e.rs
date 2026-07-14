//! Required process-scope business coverage for router-managed workers.
//!
//! This test keeps the real process tree local: a simulated worker launches a
//! long-running child, then the router process-scope helper tears down the
//! whole scope. It proves the worker boundary is stronger than direct-child
//! kill on the current OS, and records the expected cleanup strategy for other
//! supported OS families without requiring those hosts.

use anyhow::{anyhow, bail, Context, Result};
use code_tools::{
    command_run,
    commands::shell_command::{current_shell_process_scope_strategy, ShellProcessScopeStrategy},
};
use serde_json::json;
use std::{
    io::Write,
    path::Path,
    process::Stdio,
    time::{Duration, Instant},
};
use sysinfo::{Pid, System};
use tokio::process::Command;
use tura_router::process_scope::{
    attach_child_scope, configure_scoped_spawn, current_process_scope_strategy,
    ProcessScopeStrategy,
};

const EXIT_TIMEOUT: Duration = Duration::from_secs(10);
const PID_DISCOVERY_TIMEOUT: Duration = Duration::from_secs(10);

#[tokio::test]
async fn process_scope_management_kills_worker_and_spawned_child_tree() -> Result<()> {
    let temp = tempfile::tempdir().context("temp process-scope workspace")?;
    let child_script = temp.path().join("child_sleep.py");
    let worker_script = temp.path().join("worker_tree.py");
    let child_pid_file = temp.path().join("child.pid");
    write_child_script(&child_script)?;
    write_worker_script(&worker_script, &child_script, &child_pid_file)?;

    let (python, python_args) = python_command()?;
    let mut command = Command::new(&python);
    command
        .args(&python_args)
        .arg(&worker_script)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    configure_scoped_spawn(&mut command);

    let mut worker = command.spawn().context("spawn scoped worker tree")?;
    let worker_pid = worker
        .id()
        .ok_or_else(|| anyhow!("scoped worker pid should be available"))?;
    let scope = attach_child_scope(&worker)?
        .ok_or_else(|| anyhow!("current OS should attach a worker process scope"))?;
    let child_pid = wait_for_pid_file(&child_pid_file, PID_DISCOVERY_TIMEOUT)?;
    assert_process_alive(worker_pid, "worker before scope termination")?;
    assert_process_alive(child_pid, "child before scope termination")?;

    scope.terminate();
    let _ = worker.start_kill();
    let _ = wait_for_child_exit(&mut worker, EXIT_TIMEOUT).await;
    wait_for_process_dead(worker_pid, EXIT_TIMEOUT)
        .with_context(|| format!("worker pid {worker_pid} should exit after scope terminate"))?;
    wait_for_process_dead(child_pid, EXIT_TIMEOUT)
        .with_context(|| format!("child pid {child_pid} should exit after scope terminate"))?;

    Ok(())
}

#[test]
fn process_scope_management_strategy_contract_covers_all_os_families() {
    let current = current_process_scope_strategy();
    if cfg!(windows) {
        assert_eq!(current, ProcessScopeStrategy::WindowsJobObject);
    } else if cfg!(unix) {
        assert_eq!(current, ProcessScopeStrategy::UnixProcessGroup);
    } else {
        assert_eq!(current, ProcessScopeStrategy::DirectChildOnly);
    }

    let contracts = [
        StrategyContract {
            strategy: ProcessScopeStrategy::WindowsJobObject,
            parent_crash_cleanup: true,
            spawned_child_cleanup: true,
            direct_child_only: false,
        },
        StrategyContract {
            strategy: ProcessScopeStrategy::UnixProcessGroup,
            parent_crash_cleanup: true,
            spawned_child_cleanup: true,
            direct_child_only: false,
        },
        StrategyContract {
            strategy: ProcessScopeStrategy::DirectChildOnly,
            parent_crash_cleanup: false,
            spawned_child_cleanup: false,
            direct_child_only: true,
        },
    ];

    for contract in contracts {
        match contract.strategy {
            ProcessScopeStrategy::WindowsJobObject | ProcessScopeStrategy::UnixProcessGroup => {
                assert!(contract.parent_crash_cleanup);
                assert!(contract.spawned_child_cleanup);
                assert!(!contract.direct_child_only);
            }
            ProcessScopeStrategy::DirectChildOnly => {
                assert!(!contract.parent_crash_cleanup);
                assert!(!contract.spawned_child_cleanup);
                assert!(contract.direct_child_only);
            }
        }
    }
}

#[test]
fn process_scope_management_command_run_strategy_covers_all_os_families() {
    let current = current_shell_process_scope_strategy();
    if cfg!(windows) {
        assert_eq!(current, ShellProcessScopeStrategy::WindowsJobObject);
    } else if cfg!(unix) {
        assert_eq!(current, ShellProcessScopeStrategy::UnixProcessGroup);
    } else {
        assert_eq!(current, ShellProcessScopeStrategy::DirectChildOnly);
    }

    let strategies = [
        ShellProcessScopeStrategy::WindowsJobObject,
        ShellProcessScopeStrategy::UnixProcessGroup,
        ShellProcessScopeStrategy::DirectChildOnly,
    ];
    assert!(strategies.contains(&ShellProcessScopeStrategy::WindowsJobObject));
    assert!(strategies.contains(&ShellProcessScopeStrategy::UnixProcessGroup));
    assert!(strategies.contains(&ShellProcessScopeStrategy::DirectChildOnly));
}

#[test]
fn process_scope_management_command_run_timeout_kills_spawned_child_tree() -> Result<()> {
    let temp = tempfile::tempdir().context("temp command_run process-scope workspace")?;
    let pid_file = temp.path().join("command-run-child.pid");
    let child_tree_script = temp.path().join("command_run_child_tree.py");
    write_command_run_child_tree_script(&child_tree_script, &pid_file)?;
    let command_line = command_run_child_tree_command(&child_tree_script)?;
    let started = Instant::now();

    let output = command_run::execute(
        &json!({
        "commands": [{
                "command": "shell_command",
                "command_line": json!({
                    "command": command_line,
                "timeout_ms": 3000
                }).to_string()
            }]
        }),
        temp.path(),
    );

    assert!(
        started.elapsed() < Duration::from_secs(8),
        "command_run timeout should not wait on pipe-holding descendants; elapsed={:?}",
        started.elapsed()
    );
    assert_eq!(output["results"][0]["success"], false, "{output}");
    assert!(
        command_run_output_text(&output["results"][0]["output"]).contains("Timed out after"),
        "{output}"
    );
    let child_pid = wait_for_pid_file(&pid_file, PID_DISCOVERY_TIMEOUT)?;
    wait_for_process_dead(child_pid, EXIT_TIMEOUT)
        .with_context(|| format!("command_run child pid {child_pid} should be killed"))?;

    Ok(())
}

#[test]
fn process_scope_management_command_run_success_preserves_background_child_until_router_shutdown(
) -> Result<()> {
    let temp = tempfile::tempdir().context("temp command_run background workspace")?;
    let pid_file = temp.path().join("command-run-background-child.pid");
    let script = temp.path().join("command_run_background_child.py");
    write_command_run_background_child_script(&script, &pid_file)?;
    let command_line = command_run_child_tree_command(&script)?;

    let output = command_run::execute(
        &json!({
            "commands": [{
                "command": "shell_command",
                "command_line": json!({
                    "command": command_line,
                    "timeout_ms": 5000
                }).to_string()
            }]
        }),
        temp.path(),
    );

    assert_eq!(output["results"][0]["success"], true, "{output}");
    let child_pid = wait_for_pid_file(&pid_file, PID_DISCOVERY_TIMEOUT)?;
    assert_process_alive(
        child_pid,
        "command_run background child after successful parent exit",
    )?;
    let terminated = code_tools::shell_executor::terminate_retained_shell_process_scopes();
    assert!(
        terminated > 0,
        "router-level shutdown should terminate at least one retained command_run process scope"
    );
    wait_for_process_dead(child_pid, EXIT_TIMEOUT).with_context(|| {
        format!("background child pid {child_pid} should exit after router shutdown cleanup")
    })?;

    Ok(())
}

#[derive(Clone, Copy)]
struct StrategyContract {
    strategy: ProcessScopeStrategy,
    parent_crash_cleanup: bool,
    spawned_child_cleanup: bool,
    direct_child_only: bool,
}

fn command_run_output_text(output: &serde_json::Value) -> String {
    if let Some(text) = output.as_str() {
        return text.to_string();
    }
    ["stdout", "stderr"]
        .into_iter()
        .filter_map(|key| output.get(key).and_then(serde_json::Value::as_str))
        .collect::<Vec<_>>()
        .join("\n")
}

fn command_run_child_tree_command(script: &Path) -> Result<String> {
    let (python, args) = python_command()?;
    let mut parts = Vec::new();
    if cfg!(windows) {
        parts.push("&".to_string());
        parts.push(shell_single_quoted(&python));
    } else {
        parts.push(shell_single_quoted(&python));
    }
    parts.extend(args.iter().map(|arg| shell_single_quoted(arg)));
    parts.push(shell_single_quoted(&script.display().to_string()));
    Ok(parts.join(" "))
}

fn shell_single_quoted(value: &str) -> String {
    if cfg!(windows) {
        format!("'{}'", value.replace('\'', "''"))
    } else {
        format!("'{}'", value.replace('\'', "'\\''"))
    }
}

fn write_command_run_child_tree_script(script: &Path, child_pid_file: &Path) -> Result<()> {
    let child_pid_file = serde_json::to_string(&child_pid_file.display().to_string())?;
    let source = format!(
        r#"
import subprocess
import sys
import time

child = subprocess.Popen(
    [sys.executable, "-c", "import time; time.sleep(30)"],
    stdin=subprocess.DEVNULL,
    stdout=subprocess.DEVNULL,
    stderr=subprocess.DEVNULL,
)
with open({child_pid_file}, "w", encoding="utf-8") as handle:
    handle.write(str(child.pid))
    handle.flush()

try:
    while True:
        time.sleep(0.1)
finally:
    if child.poll() is None:
        child.terminate()
"#
    );
    std::fs::write(script, source)
        .with_context(|| format!("write command_run child script {}", script.display()))
}

fn write_command_run_background_child_script(script: &Path, child_pid_file: &Path) -> Result<()> {
    let child_pid_file = serde_json::to_string(&child_pid_file.display().to_string())?;
    let source = format!(
        r#"
import subprocess
import sys
import time

child = subprocess.Popen(
    [sys.executable, "-c", "import time; time.sleep(30)"],
    stdin=subprocess.DEVNULL,
    stdout=subprocess.DEVNULL,
    stderr=subprocess.DEVNULL,
)
with open({child_pid_file}, "w", encoding="utf-8") as handle:
    handle.write(str(child.pid))
    handle.flush()
"#
    );
    std::fs::write(script, source)
        .with_context(|| format!("write command_run background script {}", script.display()))
}

fn write_child_script(path: &Path) -> Result<()> {
    let mut file = std::fs::File::create(path)
        .with_context(|| format!("create child script {}", path.display()))?;
    file.write_all(
        br#"
import signal
import time

running = True

def stop(_signum, _frame):
    global running
    running = False

signal.signal(signal.SIGTERM, stop)
while running:
    time.sleep(0.1)
"#,
    )?;
    Ok(())
}

fn write_worker_script(worker: &Path, child: &Path, child_pid_file: &Path) -> Result<()> {
    let source = format!(
        r#"
import os
import subprocess
import sys
import time

child = subprocess.Popen(
    [sys.executable, r"{child}"],
    stdin=subprocess.DEVNULL,
    stdout=subprocess.DEVNULL,
    stderr=subprocess.DEVNULL,
)
with open(r"{child_pid_file}", "w", encoding="utf-8") as handle:
    handle.write(str(child.pid))
    handle.flush()

try:
    while True:
        time.sleep(0.1)
finally:
    if child.poll() is None:
        child.terminate()
"#,
        child = child.display(),
        child_pid_file = child_pid_file.display(),
    );
    std::fs::write(worker, source)
        .with_context(|| format!("write worker script {}", worker.display()))
}

fn python_command() -> Result<(String, Vec<String>)> {
    for (program, args) in [
        ("python".to_string(), Vec::new()),
        ("python3".to_string(), Vec::new()),
        ("py".to_string(), vec!["-3".to_string()]),
    ] {
        let status = std::process::Command::new(&program)
            .args(&args)
            .arg("--version")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        if status.is_ok_and(|status| status.success()) {
            return Ok((program, args));
        }
    }
    bail!("python, python3, or py -3 is required for process-scope business test")
}

fn wait_for_pid_file(path: &Path, timeout: Duration) -> Result<u32> {
    let started = Instant::now();
    let mut last_error = None;
    while started.elapsed() < timeout {
        match std::fs::read_to_string(path) {
            Ok(raw) => match raw.trim().parse::<u32>() {
                Ok(pid) if pid > 0 => return Ok(pid),
                Ok(_) => last_error = Some(anyhow!("pid file contained zero")),
                Err(error) => last_error = Some(error.into()),
            },
            Err(error) => last_error = Some(error.into()),
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    Err(last_error.unwrap_or_else(|| anyhow!("pid file {} did not appear", path.display())))
}

async fn wait_for_child_exit(child: &mut tokio::process::Child, timeout: Duration) -> Result<()> {
    tokio::time::timeout(timeout, child.wait())
        .await
        .map_err(|_| anyhow!("child wait timed out after {}ms", timeout.as_millis()))??;
    Ok(())
}

fn assert_process_alive(pid: u32, label: &str) -> Result<()> {
    if process_alive(pid) {
        return Ok(());
    }
    bail!("{label} pid {pid} was not alive")
}

fn wait_for_process_dead(pid: u32, timeout: Duration) -> Result<()> {
    let started = Instant::now();
    while started.elapsed() < timeout {
        if !process_alive(pid) {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    bail!("pid {pid} was still alive after {}ms", timeout.as_millis())
}

fn process_alive(pid: u32) -> bool {
    let mut system = System::new_all();
    system.refresh_processes();
    system.process(Pid::from_u32(pid)).is_some()
}
