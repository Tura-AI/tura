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

use crate::{file_queue, ipc, SessionLogStore};
use session_log_contract::client::session_db_owner_lock_path;

/// Run the session_db process: boot the owned store, mark interrupted sessions,
/// start the durable file-queue drain, then serve until the process exits.
pub fn run_socket_service() -> Result<()> {
    let _lock = SessionDbOwnerLock::acquire()?;
    let store = SessionLogStore::open_default()?;
    let interrupted = store.mark_running_sessions_interrupted()?;
    tracing::info!(
        interrupted_running_sessions = interrupted,
        "session_db service starting"
    );
    let feed_hub = ipc::SessionFeedHub::default();
    let drain = FileQueueDrainThread::start(store.clone(), feed_hub.clone());
    let result = ipc::serve_blocking_with_feed_hub(store, feed_hub);
    drain.stop();
    result
}

struct FileQueueDrainThread {
    stop: Arc<AtomicBool>,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl FileQueueDrainThread {
    fn start(store: SessionLogStore, feed_hub: ipc::SessionFeedHub) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let thread_stop = Arc::clone(&stop);
        let handle = std::thread::spawn(move || {
            while !thread_stop.load(Ordering::SeqCst) {
                if let Err(error) =
                    file_queue::drain_queue_with_feed(&store, 1000, |entry| feed_hub.publish(entry))
                {
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
