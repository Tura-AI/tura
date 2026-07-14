use std::path::{Path, PathBuf};

use sysinfo::{Pid, System};
use tracing::warn;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeWorkerProcessSnapshot {
    pub pid: u32,
    pub parent_pid: Option<u32>,
    pub exe: Option<PathBuf>,
    pub name: String,
    pub environ: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeWorkerOrphanDecision {
    Kill,
    Keep,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct RuntimeOrphanCleanupReport {
    pub scanned: usize,
    pub killed: Vec<u32>,
    pub skipped: usize,
}

pub fn cleanup_orphan_runtime_workers() -> RuntimeOrphanCleanupReport {
    let home = tura_path::instance_home();
    let current_pid = std::process::id();
    let mut system = System::new_all();
    system.refresh_processes();
    cleanup_orphan_runtime_workers_in_system(&system, &home, current_pid)
}

fn cleanup_orphan_runtime_workers_in_system(
    system: &System,
    current_home: &Path,
    current_pid: u32,
) -> RuntimeOrphanCleanupReport {
    let mut report = RuntimeOrphanCleanupReport {
        scanned: 0,
        killed: Vec::new(),
        skipped: 0,
    };

    for (pid, process) in system.processes() {
        let snapshot = RuntimeWorkerProcessSnapshot {
            pid: pid.as_u32(),
            parent_pid: process.parent().map(Pid::as_u32),
            exe: process.exe().map(Path::to_path_buf),
            name: process.name().to_string(),
            environ: process.environ().to_vec(),
        };
        report.scanned += 1;
        if runtime_worker_orphan_decision(&snapshot, current_home, current_pid, system)
            != RuntimeWorkerOrphanDecision::Kill
        {
            report.skipped += 1;
            continue;
        }
        if process.kill() {
            report.killed.push(snapshot.pid);
        } else {
            warn!(
                pid = snapshot.pid,
                name = snapshot.name,
                "failed to kill orphan runtime worker"
            );
            report.skipped += 1;
        }
    }

    report
}

pub fn runtime_worker_orphan_decision(
    process: &RuntimeWorkerProcessSnapshot,
    current_home: &Path,
    current_pid: u32,
    system: &System,
) -> RuntimeWorkerOrphanDecision {
    if process.pid == current_pid || !is_runtime_worker(process) {
        return RuntimeWorkerOrphanDecision::Keep;
    }
    if !runtime_home_matches(process, current_home) {
        return RuntimeWorkerOrphanDecision::Keep;
    }
    if let Some(parent_pid) = env_u32(&process.environ, "TURA_ROUTER_PARENT_PID") {
        if parent_pid == current_pid {
            return RuntimeWorkerOrphanDecision::Keep;
        }
        let Some(parent) = system.process(Pid::from_u32(parent_pid)) else {
            return RuntimeWorkerOrphanDecision::Kill;
        };
        if let Some(expected_start) = env_u64(&process.environ, "TURA_ROUTER_PARENT_START_TIME") {
            if parent.start_time() != expected_start {
                return RuntimeWorkerOrphanDecision::Kill;
            }
        }
        return RuntimeWorkerOrphanDecision::Keep;
    }

    match process.parent_pid {
        Some(parent_pid) if parent_pid == current_pid => RuntimeWorkerOrphanDecision::Keep,
        Some(parent_pid) if system.process(Pid::from_u32(parent_pid)).is_some() => {
            RuntimeWorkerOrphanDecision::Keep
        }
        _ => RuntimeWorkerOrphanDecision::Kill,
    }
}

fn is_runtime_worker(process: &RuntimeWorkerProcessSnapshot) -> bool {
    env_value(&process.environ, "TURA_RUNTIME_WORKER").is_some_and(env_flag)
        || env_value(&process.environ, "TURA_ROLE") == Some("runtime_worker")
        || process
            .exe
            .as_ref()
            .and_then(|path| path.file_stem())
            .and_then(|value| value.to_str())
            .is_some_and(|stem| stem.eq_ignore_ascii_case("tura_runtime"))
}

fn runtime_home_matches(process: &RuntimeWorkerProcessSnapshot, current_home: &Path) -> bool {
    let Some(home) = env_value(&process.environ, "TURA_HOME") else {
        return false;
    };
    tura_path::normalize_path(Path::new(home)) == tura_path::normalize_path(current_home)
}

fn env_value<'a>(env: &'a [String], key: &str) -> Option<&'a str> {
    env.iter().find_map(|entry| {
        let (candidate, value) = entry.split_once('=')?;
        candidate.eq_ignore_ascii_case(key).then_some(value)
    })
}

fn env_u32(env: &[String], key: &str) -> Option<u32> {
    env_value(env, key)?.trim().parse().ok()
}

fn env_u64(env: &[String], key: &str) -> Option<u64> {
    env_value(env, key)?.trim().parse().ok()
}

fn env_flag(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot(env: Vec<(&str, &str)>) -> RuntimeWorkerProcessSnapshot {
        RuntimeWorkerProcessSnapshot {
            pid: 200,
            parent_pid: None,
            exe: Some(PathBuf::from("tura_runtime.exe")),
            name: "tura_runtime".to_string(),
            environ: env
                .into_iter()
                .map(|(key, value)| format!("{key}={value}"))
                .collect(),
        }
    }

    fn home() -> PathBuf {
        std::env::temp_dir().join("tura-runtime-orphan-decision-home")
    }

    #[test]
    fn orphan_decision_kills_same_home_runtime_with_dead_recorded_parent() {
        let home = home();
        let system = System::new_all();
        let process = snapshot(vec![
            ("TURA_RUNTIME_WORKER", "1"),
            ("TURA_ROLE", "runtime_worker"),
            ("TURA_HOME", &home.to_string_lossy()),
            ("TURA_ROUTER_PARENT_PID", "999999"),
        ]);

        assert_eq!(
            runtime_worker_orphan_decision(&process, &home, 1, &system),
            RuntimeWorkerOrphanDecision::Kill
        );
    }

    #[test]
    fn orphan_decision_skips_foreign_home_runtime() {
        let home = home();
        let foreign = home.with_file_name("foreign-tura-home");
        let system = System::new_all();
        let process = snapshot(vec![
            ("TURA_RUNTIME_WORKER", "1"),
            ("TURA_HOME", &foreign.to_string_lossy()),
            ("TURA_ROUTER_PARENT_PID", "999999"),
        ]);

        assert_eq!(
            runtime_worker_orphan_decision(&process, &home, 1, &system),
            RuntimeWorkerOrphanDecision::Keep
        );
    }

    #[test]
    fn orphan_decision_skips_current_router_child() {
        let home = home();
        let system = System::new_all();
        let mut process = snapshot(vec![
            ("TURA_RUNTIME_WORKER", "1"),
            ("TURA_HOME", &home.to_string_lossy()),
            ("TURA_ROUTER_PARENT_PID", "42"),
        ]);
        process.parent_pid = Some(42);

        assert_eq!(
            runtime_worker_orphan_decision(&process, &home, 42, &system),
            RuntimeWorkerOrphanDecision::Keep
        );
    }

    #[test]
    fn orphan_decision_skips_non_runtime_processes() {
        let home = home();
        let system = System::new_all();
        let mut process = snapshot(vec![
            ("TURA_HOME", &home.to_string_lossy()),
            ("TURA_ROUTER_PARENT_PID", "999999"),
        ]);
        process.exe = Some(PathBuf::from("not-runtime.exe"));
        process.name = "not-runtime".to_string();

        assert_eq!(
            runtime_worker_orphan_decision(&process, &home, 1, &system),
            RuntimeWorkerOrphanDecision::Keep
        );
    }
}
