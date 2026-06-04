use std::collections::HashSet;
use std::path::{Path, PathBuf};

use sysinfo::{Pid, Process, System};

#[derive(Debug, Clone, serde::Serialize)]
pub struct DirectoryProcessCleanup {
    pub directory: String,
    pub killed: Vec<KilledProcess>,
    pub skipped: Vec<SkippedProcess>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct KilledProcess {
    pub pid: u32,
    pub name: String,
    pub cwd: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SkippedProcess {
    pub pid: u32,
    pub name: String,
    pub reason: String,
}

pub fn kill_processes_in_directory(
    directory: impl AsRef<Path>,
) -> Result<DirectoryProcessCleanup, String> {
    let target = directory
        .as_ref()
        .canonicalize()
        .map_err(|err| format!("failed to resolve cleanup directory: {err}"))?;

    let mut system = System::new_all();
    system.refresh_processes();

    let protected = protected_processes(&system);
    let current = sysinfo::get_current_pid().ok();
    let mut killed = Vec::new();
    let mut skipped = Vec::new();

    for (pid, process) in system.processes() {
        let raw_pid = pid.as_u32();
        let name = process.name().to_string();

        if Some(*pid) == current || protected.contains(pid) {
            skipped.push(SkippedProcess {
                pid: raw_pid,
                name,
                reason: "gateway control process".to_string(),
            });
            continue;
        }

        if let Some(reason) = control_plane_reason(process) {
            skipped.push(SkippedProcess {
                pid: raw_pid,
                name,
                reason,
            });
            continue;
        }
        let Some(cwd) = process.cwd() else {
            continue;
        };

        if !path_is_under(cwd, &target) {
            continue;
        }

        if process.kill() {
            killed.push(KilledProcess {
                pid: raw_pid,
                name,
                cwd: cwd.to_string_lossy().to_string(),
            });
        } else {
            skipped.push(SkippedProcess {
                pid: raw_pid,
                name,
                reason: format!("failed to kill process in {}", cwd.display()),
            });
        }
    }

    Ok(DirectoryProcessCleanup {
        directory: target.to_string_lossy().to_string(),
        killed,
        skipped,
    })
}

fn protected_processes(system: &System) -> HashSet<Pid> {
    let mut protected = HashSet::new();
    let mut cursor = sysinfo::get_current_pid().ok();

    while let Some(pid) = cursor {
        if !protected.insert(pid) {
            break;
        }
        cursor = system.process(pid).and_then(|process| process.parent());
    }

    protected
}

fn control_plane_reason(process: &Process) -> Option<String> {
    let name = process.name().to_ascii_lowercase();
    let cmd = process
        .cmd()
        .iter()
        .map(|part| part.to_string())
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase();
    let exe = process
        .exe()
        .map(|path| path.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    let haystack = format!("{name} {cmd} {exe}");

    // 受保护控制面进程：router、gateway 二进制（runtime worker 同一二进制 + TURA_ROLE）。
    let protected = [
        "tura_router",
        "cargo run -p tura_router",
        "cargo run -p gateway",
        "target\\debug\\gateway",
        "target/debug/gateway",
        "target\\release\\gateway",
        "target/release/gateway",
    ];

    if protected.iter().any(|needle| haystack.contains(needle)) {
        return Some("tura control plane process".to_string());
    }

    None
}

fn path_is_under(path: &Path, directory: &Path) -> bool {
    let path = canonical_or_self(path);
    path == directory || path.starts_with(directory)
}

fn canonical_or_self(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}
