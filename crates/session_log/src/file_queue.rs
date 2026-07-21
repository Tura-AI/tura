//! File-backed handoff queue for session DB writes.
//!
//! Runtime processes and explicit offline handoffs enqueue JSON commands here
//! and let the session DB owner drain them.

use std::fs;
use std::path::Path;

use anyhow::{Context, Result};

use crate::{ipc, SessionLogStore};
use session_log_contract::client::{failed_queue_dir, pending_queue_dir, processing_queue_dir};
use session_log_contract::{SessionFeedEntry, SessionLogCommand};

pub fn drain_queue(store: &SessionLogStore, limit: usize) -> Result<u64> {
    drain_queue_with_feed(store, limit, |_| {})
}

pub(crate) fn drain_queue_with_feed(
    store: &SessionLogStore,
    limit: usize,
    mut publish: impl FnMut(SessionFeedEntry),
) -> Result<u64> {
    let pending = pending_queue_dir();
    fs::create_dir_all(&pending)?;
    fs::create_dir_all(processing_queue_dir())?;
    fs::create_dir_all(failed_queue_dir())?;
    recover_orphaned_processing()?;

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
        let processing = processing_queue_dir().join(&file_name);
        if fs::rename(&path, &processing).is_err() {
            continue;
        }
        match apply_file(store, &processing) {
            Ok(feed_entries) => {
                for entry in feed_entries {
                    publish(entry);
                }
                let _ = fs::remove_file(&processing);
                applied += 1;
            }
            Err(error) => {
                let failed = failed_queue_dir().join(&file_name);
                let error_path = failed.with_extension("error.txt");
                let move_result = fs::rename(&processing, &failed);
                if move_result.is_ok() {
                    let _ = fs::write(&error_path, error.to_string());
                }
                if is_discardable_queue_error(&error) {
                    tracing::warn!(
                        path = %failed.display(),
                        error = %error,
                        "quarantined dirty session queue item"
                    );
                } else {
                    tracing::warn!(
                        path = %failed.display(),
                        error = %error,
                        "failed to apply session queue item"
                    );
                }
                if let Err(move_error) = move_result {
                    tracing::warn!(
                        path = %processing.display(),
                        target = %failed.display(),
                        error = %move_error,
                        "failed to quarantine session queue item"
                    );
                }
            }
        }
    }
    Ok(applied)
}

fn recover_orphaned_processing() -> Result<()> {
    let processing = processing_queue_dir();
    let pending = pending_queue_dir();
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

fn apply_file(store: &SessionLogStore, path: &Path) -> Result<Vec<SessionFeedEntry>> {
    let raw = fs::read_to_string(path)
        .with_context(|| format!("failed to read session queue item {}", path.display()))?;
    let command: SessionLogCommand = serde_json::from_str(&raw)
        .with_context(|| format!("failed to parse session queue item {}", path.display()))?;
    apply_command(store, command)
}

fn apply_command(
    store: &SessionLogStore,
    command: SessionLogCommand,
) -> Result<Vec<SessionFeedEntry>> {
    if !matches!(
        &command,
        SessionLogCommand::CreateSession(_)
            | SessionLogCommand::ExecuteSessionCommand(_)
            | SessionLogCommand::UpdateSession(_)
            | SessionLogCommand::UpdateSessionTodos(_)
            | SessionLogCommand::PersistSessionDelta(_)
            | SessionLogCommand::ApplyCommandCheckpoint(_)
            | SessionLogCommand::MarkSessionInterrupted(_)
            | SessionLogCommand::DeleteSession(_)
            | SessionLogCommand::DeleteWorkspace(_)
    ) {
        anyhow::bail!("session queue only accepts write commands: {command:?}");
    }
    Ok(ipc::execute_command_with_feed(store, command)?.committed_feed_entries)
}

fn is_discardable_queue_error(error: &anyhow::Error) -> bool {
    if error.is::<serde_json::Error>() {
        return true;
    }
    error.chain().any(|cause| {
        let text = cause.to_string();
        text.contains("failed to parse session queue item")
            || text.contains("invalid canonical session state")
            || text.contains("session queue only accepts write commands")
    })
}
