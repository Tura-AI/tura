//! File-backed handoff queue for session DB writes.
//!
//! Runtime processes must not block on session-log storage. They enqueue JSON
//! commands here and let the session DB owner drain them.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use anyhow::{Context, Result};

use crate::{path::default_db_dir, SessionLogCommand, SessionLogStore};

const QUEUE_DIR: &str = "message_queue";
const PENDING_DIR: &str = "pending";
const PROCESSING_DIR: &str = "processing";
const FAILED_DIR: &str = "failed";

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

pub fn is_async_write(command: &SessionLogCommand) -> bool {
    matches!(
        command,
        SessionLogCommand::UpsertSession(_)
            | SessionLogCommand::ApplyCommandCheckpoint(_)
            | SessionLogCommand::DeleteSession(_)
            | SessionLogCommand::DeleteWorkspace(_)
    )
}

pub fn enqueue_command(command: &SessionLogCommand) -> Result<PathBuf> {
    let pending = queue_root().join(PENDING_DIR);
    fs::create_dir_all(&pending)
        .with_context(|| format!("failed to create session queue {}", pending.display()))?;

    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let now = chrono::Utc::now()
        .timestamp_nanos_opt()
        .unwrap_or_else(|| chrono::Utc::now().timestamp_micros() * 1000);
    let name = format!("{now:020}-{}-{id}.json", std::process::id());
    let tmp_path = pending.join(format!("{name}.tmp"));
    let final_path = pending.join(name);
    let payload = serde_json::to_vec(command)?;
    fs::write(&tmp_path, payload)
        .with_context(|| format!("failed to write session queue item {}", tmp_path.display()))?;
    fs::rename(&tmp_path, &final_path).with_context(|| {
        format!(
            "failed to publish session queue item {}",
            final_path.display()
        )
    })?;
    Ok(final_path)
}

pub fn drain_queue(store: &SessionLogStore, limit: usize) -> Result<u64> {
    let root = queue_root();
    let pending = root.join(PENDING_DIR);
    fs::create_dir_all(&pending)?;
    fs::create_dir_all(root.join(PROCESSING_DIR))?;
    fs::create_dir_all(root.join(FAILED_DIR))?;
    recover_orphaned_processing(&root)?;

    let mut paths = fs::read_dir(&pending)?
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("json"))
        .collect::<Vec<_>>();
    paths.sort();

    let mut applied = 0;
    for path in paths.into_iter().take(limit) {
        let Some(file_name) = path.file_name().map(|name| name.to_owned()) else {
            continue;
        };
        let processing = root.join(PROCESSING_DIR).join(&file_name);
        if fs::rename(&path, &processing).is_err() {
            continue;
        }
        match apply_file(store, &processing) {
            Ok(()) => {
                let _ = fs::remove_file(&processing);
                applied += 1;
            }
            Err(error) => {
                let failed = root.join(FAILED_DIR).join(&file_name);
                let _ = fs::rename(&processing, &failed);
                let error_path = failed.with_extension("error.txt");
                let _ = fs::write(&error_path, error.to_string());
                tracing::warn!(
                    path = %failed.display(),
                    error = %error,
                    "failed to apply session queue item"
                );
            }
        }
    }
    Ok(applied)
}

fn recover_orphaned_processing(root: &Path) -> Result<()> {
    let processing = root.join(PROCESSING_DIR);
    let pending = root.join(PENDING_DIR);
    if !processing.exists() {
        return Ok(());
    }

    for entry in fs::read_dir(&processing)?.flatten() {
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }
        let Some(file_name) = path.file_name() else {
            continue;
        };
        let recovered = pending.join(file_name);
        if recovered.exists() {
            continue;
        }
        if let Err(error) = fs::rename(&path, &recovered) {
            tracing::warn!(
                path = %path.display(),
                target = %recovered.display(),
                error = %error,
                "failed to recover orphaned session queue item"
            );
        }
    }
    Ok(())
}

fn apply_file(store: &SessionLogStore, path: &Path) -> Result<()> {
    let raw = fs::read_to_string(path)
        .with_context(|| format!("failed to read session queue item {}", path.display()))?;
    let command: SessionLogCommand = serde_json::from_str(&raw)
        .with_context(|| format!("failed to parse session queue item {}", path.display()))?;
    apply_command(store, command)
}

fn apply_command(store: &SessionLogStore, command: SessionLogCommand) -> Result<()> {
    match command {
        SessionLogCommand::UpsertSession(payload) => store.upsert_session(payload),
        SessionLogCommand::ApplyCommandCheckpoint(payload) => {
            store.apply_command_checkpoint(*payload)
        }
        SessionLogCommand::DeleteSession(payload) => store.delete_session(payload),
        SessionLogCommand::DeleteWorkspace(payload) => store.delete_workspace(payload),
        other => anyhow::bail!("session queue only accepts write commands: {other:?}"),
    }
}

fn queue_root() -> PathBuf {
    default_db_dir().join(QUEUE_DIR)
}
