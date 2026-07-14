use std::path::{Path, PathBuf};

use serde::Serialize;
use sysinfo::{Pid, Process, System};

const MAX_PROCESSES: usize = 24;
const MAX_CMD_CHARS: usize = 500;
const BACKGROUND_PROCESS_KIND_ENV: &str = "TURA_BACKGROUND_PROCESS_KIND";
const RUNTIME_SHELL_BACKGROUND_PROCESS_KIND: &str = "runtime_shell";

#[derive(Debug, Clone, Serialize)]
pub struct SessionProcessSnapshot {
    pub session_directory: String,
    pub processes: Vec<SessionProcessInfo>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SessionProcessInfo {
    pub pid: u32,
    pub name: String,
    pub exe: Option<String>,
    pub cwd: Option<String>,
    pub command_line: String,
    pub running_file: Option<String>,
    pub kind: String,
}

pub fn collect_session_process_snapshot(session_directory: &Path) -> SessionProcessSnapshot {
    let target = normalize_path(session_directory);
    let mut system = System::new_all();
    system.refresh_processes();

    let mut processes = system
        .processes()
        .iter()
        .filter_map(|(pid, process)| process_info_for_session(*pid, process, &target))
        .collect::<Vec<_>>();
    processes.sort_by_key(|process| (process.name.clone(), process.pid));
    processes.truncate(MAX_PROCESSES);

    SessionProcessSnapshot {
        session_directory: session_directory.display().to_string(),
        processes,
    }
}

pub fn collect_runtime_shell_process_snapshot(session_directory: &Path) -> SessionProcessSnapshot {
    let mut snapshot = collect_session_process_snapshot(session_directory);
    snapshot
        .processes
        .retain(|process| process.kind == RUNTIME_SHELL_BACKGROUND_PROCESS_KIND);
    snapshot
}

pub fn stop_session_process(session_directory: &Path, target_pid: u32) -> Result<(), String> {
    let target = normalize_path(session_directory);
    let mut system = System::new_all();
    system.refresh_processes();

    let Some((_, process)) = system
        .processes()
        .iter()
        .find(|(pid, _)| pid.as_u32() == target_pid)
    else {
        return Err(format!("process {target_pid} was not found"));
    };

    if process_info_for_session(Pid::from_u32(target_pid), process, &target).is_none() {
        return Err(format!(
            "process {target_pid} is not under this session directory"
        ));
    }

    if process.kill() {
        Ok(())
    } else {
        Err(format!("failed to stop process {target_pid}"))
    }
}

pub fn stop_runtime_shell_process(session_directory: &Path, target_pid: u32) -> Result<(), String> {
    let target = normalize_path(session_directory);
    let mut system = System::new_all();
    system.refresh_processes();

    let Some((_, process)) = system
        .processes()
        .iter()
        .find(|(pid, _)| pid.as_u32() == target_pid)
    else {
        return Err(format!("process {target_pid} was not found"));
    };

    let Some(info) = process_info_for_session(Pid::from_u32(target_pid), process, &target) else {
        return Err(format!(
            "process {target_pid} is not under this session directory"
        ));
    };
    if info.kind != RUNTIME_SHELL_BACKGROUND_PROCESS_KIND {
        return Err(format!(
            "process {target_pid} is not a runtime shell background process"
        ));
    }

    if process.kill() {
        Ok(())
    } else {
        Err(format!("failed to stop process {target_pid}"))
    }
}

fn process_info_for_session(
    pid: Pid,
    process: &Process,
    target: &Path,
) -> Option<SessionProcessInfo> {
    let cmd = process.cmd().join(" ");
    let cwd_matches = process
        .cwd()
        .map(|cwd| path_is_under(cwd, target))
        .unwrap_or(false);
    let exe_matches = process
        .exe()
        .map(|exe| path_is_under(exe, target))
        .unwrap_or(false);
    let cmd_matches = command_mentions_path(&cmd, target);

    if !cwd_matches && !exe_matches && !cmd_matches {
        return None;
    }

    let kind = if runtime_shell_background_process(process) {
        RUNTIME_SHELL_BACKGROUND_PROCESS_KIND
    } else if cmd.contains("command_run") || process.name().contains("command_run") {
        "command_run"
    } else {
        "workspace"
    };

    Some(SessionProcessInfo {
        pid: pid.as_u32(),
        name: process.name().to_string(),
        exe: process.exe().map(|path| path.display().to_string()),
        cwd: process.cwd().map(|path| path.display().to_string()),
        command_line: truncate(&cmd, MAX_CMD_CHARS),
        running_file: process
            .exe()
            .map(|path| path.display().to_string())
            .or_else(|| cmd.split_whitespace().next().map(ToString::to_string)),
        kind: kind.to_string(),
    })
}

fn runtime_shell_background_process(process: &Process) -> bool {
    let marker = format!("{BACKGROUND_PROCESS_KIND_ENV}={RUNTIME_SHELL_BACKGROUND_PROCESS_KIND}");
    process.environ().iter().any(|value| value == &marker)
}

fn command_mentions_path(cmd: &str, target: &Path) -> bool {
    let normalized_cmd = normalize_text_path(cmd);
    let normalized_target = normalize_text_path(&target.display().to_string());
    !normalized_target.is_empty() && normalized_cmd.contains(&normalized_target)
}

fn path_is_under(path: &Path, target: &Path) -> bool {
    normalize_path(path).starts_with(target)
}

fn normalize_path(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

fn normalize_text_path(value: &str) -> String {
    value.replace('\\', "/").to_ascii_lowercase()
}

fn truncate(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        return value.to_string();
    }
    let mut out = value.chars().take(max_chars).collect::<String>();
    out.push_str("...");
    out
}
