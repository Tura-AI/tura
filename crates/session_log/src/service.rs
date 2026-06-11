//! Session DB service entry points.
//!
//! This process is the owner of session-log SQLite writes. Router may start and
//! monitor it, but gateway/runtime data calls target this service.

use anyhow::Result;
use fs2::FileExt;
use std::io::{Seek, SeekFrom, Write};
use std::path::PathBuf;

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
    start_file_queue_drain(store.clone());
    crate::ipc::serve_blocking(store)
}

fn start_file_queue_drain(store: SessionLogStore) {
    std::thread::spawn(move || loop {
        if let Err(error) = file_queue::drain_queue(&store, 1000) {
            tracing::warn!(error = %error, "failed to drain session file queue");
        }
        std::thread::sleep(std::time::Duration::from_millis(250));
    });
}

struct SessionDbOwnerLock {
    file: std::fs::File,
    path: PathBuf,
}

impl SessionDbOwnerLock {
    fn acquire() -> Result<Self> {
        let dir = tura_path::locks_dir();
        std::fs::create_dir_all(&dir)?;
        let path = dir.join(format!("session-db-{}.lock", tura_path::build_kind()));
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
