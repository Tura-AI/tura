//! Session DB service entry points.
//!
//! This process is the owner of session-log SQLite writes. Router may start and
//! monitor it, but gateway/runtime data calls target this service.

use anyhow::Result;
use fs2::FileExt;
use std::io::{Seek, SeekFrom, Write};
use std::path::PathBuf;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use crate::{file_queue, SessionLogStore};

/// Run the session_db process: boot the owned store, replay the durable write
/// queue, mark interrupted sessions, start the background queue drain, then
/// serve the data-path socket until the process exits.
pub fn run_socket_service() -> Result<()> {
    let _lock = SessionDbOwnerLock::acquire()?;
    let store = SessionLogStore::open_default()?;
    let replayed = store.replay_pending_write_queue()?;
    let interrupted = store.mark_running_sessions_interrupted()?;
    tracing::info!(
        replayed_queue_items = replayed,
        interrupted_running_sessions = interrupted,
        "session_db service starting"
    );
    let drain = FileQueueDrainThread::start(store.clone());
    let result = crate::ipc::serve_blocking(store);
    drain.stop();
    result
}

/// Explain the recovery action when the session_db socket is unavailable but
/// the per-home owner lock is still held by another process.
pub fn unreachable_owner_lock_message() -> Option<String> {
    if crate::ipc::service_is_running() {
        return None;
    }
    let path = session_db_owner_lock_path();
    if !path.exists() {
        return None;
    }
    let file = match std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(&path)
    {
        Ok(file) => file,
        Err(error) => {
            return Some(format_owner_lock_message(
                &path,
                read_owner_lock_record(&path).as_ref(),
                &error.to_string(),
            ))
        }
    };
    match file.try_lock_exclusive() {
        Ok(()) => {
            let _ = file.unlock();
            None
        }
        Err(error) => Some(format_owner_lock_message(
            &path,
            read_owner_lock_record(&path).as_ref(),
            &error.to_string(),
        )),
    }
}

pub fn session_db_owner_lock_path() -> PathBuf {
    tura_path::locks_dir().join(format!("session-db-{}.lock", tura_path::build_kind()))
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct OwnerLockRecord {
    pid: Option<u32>,
    kind: Option<String>,
    build_kind: Option<String>,
    home: Option<String>,
}

fn read_owner_lock_record(path: &std::path::Path) -> Option<OwnerLockRecord> {
    let raw = std::fs::read_to_string(path).ok()?;
    let mut record = OwnerLockRecord {
        pid: None,
        kind: None,
        build_kind: None,
        home: None,
    };
    for line in raw.lines() {
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let value = value.trim();
        match key.trim() {
            "pid" => record.pid = value.parse::<u32>().ok(),
            "kind" => record.kind = Some(value.to_string()),
            "build_kind" => record.build_kind = Some(value.to_string()),
            "home" => record.home = Some(value.to_string()),
            _ => {}
        }
    }
    Some(record)
}

fn format_owner_lock_message(
    path: &std::path::Path,
    record: Option<&OwnerLockRecord>,
    lock_error: &str,
) -> String {
    let owner = match record {
        Some(record) => {
            let mut parts = Vec::new();
            if let Some(pid) = record.pid {
                parts.push(format!("pid {pid}"));
            }
            if let Some(kind) = record.kind.as_deref() {
                parts.push(format!("kind {kind}"));
            }
            if let Some(build_kind) = record.build_kind.as_deref() {
                parts.push(format!("build {build_kind}"));
            }
            if let Some(home) = record.home.as_deref() {
                parts.push(format!("home {home}"));
            }
            if parts.is_empty() {
                "owner details unavailable".to_string()
            } else {
                parts.join(", ")
            }
        }
        None => "owner details unavailable".to_string(),
    };
    let kill_hint = record
        .and_then(|record| record.pid)
        .map(kill_process_hint)
        .unwrap_or_else(|| {
            "Close other Tura windows or kill the stale tura_session_db process, then retry."
                .to_string()
        });
    format!(
        "Process lock error: session_db is not reachable, but its owner lock is held at {} ({owner}; lock error: {lock_error}). {kill_hint}",
        path.display()
    )
}

fn kill_process_hint(pid: u32) -> String {
    if cfg!(windows) {
        format!("Kill the stale process and retry. PowerShell: Stop-Process -Id {pid} -Force")
    } else {
        format!("Kill the stale process and retry. Shell: kill {pid}")
    }
}

struct FileQueueDrainThread {
    stop: Arc<AtomicBool>,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl FileQueueDrainThread {
    fn start(store: SessionLogStore) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let thread_stop = Arc::clone(&stop);
        let handle = std::thread::spawn(move || {
            while !thread_stop.load(Ordering::SeqCst) {
                if let Err(error) = file_queue::drain_queue(&store, 1000) {
                    tracing::warn!(error = %error, "failed to drain session file queue");
                }
                for _ in 0..25 {
                    if thread_stop.load(Ordering::SeqCst) {
                        return;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
            }
        });
        Self {
            stop,
            handle: Some(handle),
        }
    }

    fn stop(mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for FileQueueDrainThread {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

struct SessionDbOwnerLock {
    file: std::fs::File,
    path: PathBuf,
}

impl SessionDbOwnerLock {
    fn acquire() -> Result<Self> {
        let dir = tura_path::locks_dir();
        std::fs::create_dir_all(&dir)?;
        let path = session_db_owner_lock_path();
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(&path)?;
        file.try_lock_exclusive().map_err(|error| {
            anyhow::anyhow!(
                "another session_db owner already owns {}: {error}",
                path.display()
            )
        })?;
        file.set_len(0)?;
        file.seek(SeekFrom::Start(0))?;
        writeln!(file, "pid={}", std::process::id())?;
        writeln!(file, "kind=session_db")?;
        writeln!(file, "build_kind={}", tura_path::build_kind())?;
        writeln!(file, "home={}", tura_path::instance_home().display())?;
        Ok(Self { file, path })
    }
}

impl Drop for SessionDbOwnerLock {
    fn drop(&mut self) {
        let _ = self.file.unlock();
        let _ = std::fs::remove_file(&self.path);
    }
}

#[cfg(test)]
mod tests {
    use super::{format_owner_lock_message, OwnerLockRecord};
    use std::path::Path;

    #[test]
    fn owner_lock_message_names_pid_and_kill_command() {
        let record = OwnerLockRecord {
            pid: Some(29816),
            kind: Some("session_db".to_string()),
            build_kind: Some("release".to_string()),
            home: Some("C:\\workspace\\tura".to_string()),
        };
        let message = format_owner_lock_message(
            Path::new("C:\\workspace\\tura\\.tura\\locks\\session-db-release.lock"),
            Some(&record),
            "file is locked",
        );

        assert!(message.contains("Process lock error"));
        assert!(message.contains("session_db is not reachable"));
        assert!(message.contains("pid 29816"));
        assert!(message.contains("Kill the stale process"));
        if cfg!(windows) {
            assert!(message.contains("Stop-Process -Id 29816 -Force"));
        } else {
            assert!(message.contains("kill 29816"));
        }
    }
}
