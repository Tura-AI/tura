//! Cross-process ownership locks for long-lived gateway services.

use anyhow::{anyhow, Context, Result};
use fs2::FileExt;
use std::fs::{File, OpenOptions};
use std::io::{Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

pub struct ProcessLock {
    file: File,
    path: PathBuf,
}

impl ProcessLock {
    pub fn acquire(root: &Path, kind: &str, mode: &str, port: Option<u16>) -> Result<Self> {
        let dir = root.join(".tura").join("locks");
        std::fs::create_dir_all(&dir)
            .with_context(|| format!("failed to create lock dir {}", dir.display()))?;
        let path = dir.join(lock_file_name(kind, mode));
        let mut file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(&path)
            .with_context(|| format!("failed to open lock {}", path.display()))?;
        file.try_lock_exclusive().map_err(|error| {
            anyhow!(
                "another {kind} owner is already running for root={}, mode={}, port={}: {error}",
                root.display(),
                mode,
                port.map(|p| p.to_string())
                    .unwrap_or_else(|| "none".to_string())
            )
        })?;
        file.set_len(0)?;
        file.seek(SeekFrom::Start(0))?;
        writeln!(file, "pid={}", std::process::id())?;
        if let Some(start_time) = current_process_start_time(std::process::id()) {
            writeln!(file, "process_start_time={start_time}")?;
        }
        writeln!(file, "kind={kind}")?;
        writeln!(file, "mode={mode}")?;
        if let Some(port) = port {
            writeln!(file, "port={port}")?;
        }

        fn current_process_start_time(pid: u32) -> Option<u64> {
            let mut system = sysinfo::System::new_all();
            system.refresh_processes();
            system
                .process(sysinfo::Pid::from_u32(pid))
                .map(sysinfo::Process::start_time)
        }
        writeln!(file, "root={}", root.display())?;
        Ok(Self { file, path })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for ProcessLock {
    fn drop(&mut self) {
        let _ = self.file.unlock();
        let _ = std::fs::remove_file(&self.path);
    }
}

fn lock_file_name(kind: &str, mode: &str) -> String {
    format!("{}-{}.lock", sanitize(kind), sanitize(mode))
}

fn sanitize(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '-'
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::ProcessLock;
    use tura_path::{DEBUG_GATEWAY_PORT, RELEASE_GATEWAY_PORT};

    #[test]
    fn rejects_second_owner_for_same_root_mode_even_on_different_port() {
        let temp = tempfile::tempdir().expect("temp dir");
        let first = ProcessLock::acquire(temp.path(), "gateway", "dev", Some(DEBUG_GATEWAY_PORT))
            .expect("first lock");
        let second = ProcessLock::acquire(temp.path(), "gateway", "dev", Some(4999));
        assert!(second.is_err());
        assert!(first.path().exists());
    }

    #[test]
    fn allows_different_modes() {
        let temp = tempfile::tempdir().expect("temp dir");
        let _dev = ProcessLock::acquire(temp.path(), "gateway", "dev", Some(DEBUG_GATEWAY_PORT))
            .expect("dev lock");
        let _release = ProcessLock::acquire(
            temp.path(),
            "gateway",
            "release",
            Some(RELEASE_GATEWAY_PORT),
        )
        .expect("release lock");
    }
}
