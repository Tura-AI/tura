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
    let name = queue_item_name(now, std::process::id(), id);
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
                if is_discardable_queue_error(&error) {
                    let _ = fs::remove_file(&processing);
                    tracing::warn!(
                        path = %processing.display(),
                        error = %error,
                        "discarding dirty session queue item"
                    );
                } else {
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

fn queue_item_name(now: i64, pid: u32, id: u64) -> String {
    format!("{now:020}-{pid}-{id:020}.json")
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

fn queue_root() -> PathBuf {
    default_db_dir().join(QUEUE_DIR)
}

#[cfg(test)]
mod tests {
    use super::{is_async_write, queue_item_name};
    use crate::{
        CommandCheckpoint, DeleteSessionRequest, DeleteWorkspaceRequest, GetSessionRequest,
        ListSessionRecordsRequest, ListSessionsRequest, SessionLogCommand, UpsertSessionRequest,
    };
    use serde_json::json;

    fn upsert() -> UpsertSessionRequest {
        UpsertSessionRequest {
            session: json!({ "id": "session", "management": { "session_id": "session", "state": "created" } }),
            parent_id: None,
            messages: Vec::new(),
            todos: Vec::new(),
        }
    }

    #[test]
    fn queue_item_names_keep_same_tick_ids_in_numeric_order() {
        let now = 42;
        let pid = 7;
        let mut names = vec![
            queue_item_name(now, pid, 10),
            queue_item_name(now, pid, 2),
            queue_item_name(now, pid, 1),
        ];
        names.sort();

        assert_eq!(
            names,
            vec![
                queue_item_name(now, pid, 1),
                queue_item_name(now, pid, 2),
                queue_item_name(now, pid, 10),
            ]
        );
        assert!(names[0].ends_with("-00000000000000000001.json"));
    }

    fn checkpoint() -> CommandCheckpoint {
        CommandCheckpoint {
            session_id: "session".to_string(),
            turn_id: "turn".to_string(),
            runtime_worker_id: None,
            provider_call_id: None,
            command_run_id: None,
            command_id: None,
            event_seq: None,
            command_type: None,
            command_line: None,
            status: "turn_started".to_string(),
            output_summary: None,
            changes: json!({}),
            started_at: None,
            finished_at: None,
        }
    }

    #[test]
    fn async_write_classifier_accepts_only_mutating_commands() {
        let write_commands = [
            SessionLogCommand::UpsertSession(upsert()),
            SessionLogCommand::ApplyCommandCheckpoint(Box::new(checkpoint())),
            SessionLogCommand::DeleteSession(DeleteSessionRequest {
                session_id: "session".to_string(),
            }),
            SessionLogCommand::DeleteWorkspace(DeleteWorkspaceRequest {
                workspace: "workspace".to_string(),
            }),
        ];
        for command in &write_commands {
            assert!(is_async_write(command), "{command:?} should be queued");
        }

        let read_commands = [
            SessionLogCommand::GetSession(GetSessionRequest {
                session_id: "session".to_string(),
            }),
            SessionLogCommand::ListWorkspaces,
            SessionLogCommand::ListSessions(ListSessionsRequest {
                workspace: "workspace".to_string(),
                page: 0,
                page_size: 10,
            }),
            SessionLogCommand::ListSessionRecords(ListSessionRecordsRequest {
                session_id: "session".to_string(),
                page: 0,
                page_size: 10,
            }),
            SessionLogCommand::Shutdown,
        ];
        for command in &read_commands {
            assert!(!is_async_write(command), "{command:?} should not be queued");
        }
    }
}
