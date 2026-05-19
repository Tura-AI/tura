use std::fs;
use std::path::{Path, PathBuf};

use crate::tool_router::execute_tool::ToolExecutionResult;

use super::constants::{APPLY_DIFF_TOOL, DELETE_FILE_TOOL, WRITE_FILE_TOOL};

#[derive(Debug, Clone)]
pub(super) struct PendingChange {
    tool_name: String,
    path: PathBuf,
    before_exists: bool,
    before_content: Option<String>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct ChangeRecord {
    session_id: String,
    runtime_id: String,
    tool_name: String,
    path: String,
    before_exists: bool,
    before_content: Option<String>,
    after_exists: bool,
    after_content: Option<String>,
    created_at_ms: i64,
    reverted: bool,
}

pub(super) fn capture_pending_changes(
    tool_name: &str,
    arguments: &serde_json::Value,
) -> Vec<PendingChange> {
    if !matches!(
        tool_name,
        WRITE_FILE_TOOL | APPLY_DIFF_TOOL | DELETE_FILE_TOOL
    ) {
        return Vec::new();
    }

    tool_paths(arguments)
        .into_iter()
        .map(|path| {
            let before_content = fs::read_to_string(&path).ok();
            PendingChange {
                tool_name: tool_name.to_string(),
                path,
                before_exists: before_content.is_some(),
                before_content,
            }
        })
        .collect()
}

pub(super) fn append_successful_changes(
    session_directory: &Path,
    session_id: &str,
    runtime_id: &str,
    pending: Vec<PendingChange>,
    result: &ToolExecutionResult,
) {
    if pending.is_empty() || !tool_result_changed_files(result) {
        return;
    }

    let tracker_path = tracker_path(session_directory, session_id);
    let mut records = read_records(&tracker_path);
    for change in pending {
        let after_content = fs::read_to_string(&change.path).ok();
        records.push(ChangeRecord {
            session_id: session_id.to_string(),
            runtime_id: runtime_id.to_string(),
            tool_name: change.tool_name,
            path: change.path.display().to_string(),
            before_exists: change.before_exists,
            before_content: change.before_content,
            after_exists: after_content.is_some(),
            after_content,
            created_at_ms: chrono::Utc::now().timestamp_millis(),
            reverted: false,
        });
    }
    if let Some(parent) = tracker_path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(content) = serde_json::to_string_pretty(&records) {
        let _ = fs::write(tracker_path, content);
    }
}

fn tool_paths(arguments: &serde_json::Value) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    match arguments {
        serde_json::Value::Array(items) => {
            for item in items {
                collect_path(item, &mut paths);
            }
        }
        serde_json::Value::Object(object) => {
            if let Some(requests) = object.get("requests").and_then(serde_json::Value::as_array) {
                for item in requests {
                    collect_path(item, &mut paths);
                }
            } else {
                collect_path(arguments, &mut paths);
            }
        }
        _ => {}
    }
    paths
}

fn collect_path(value: &serde_json::Value, paths: &mut Vec<PathBuf>) {
    let Some(path) = value.get("path").and_then(serde_json::Value::as_str) else {
        return;
    };
    if !path.trim().is_empty() {
        paths.push(PathBuf::from(path));
    }
}

fn tool_result_changed_files(result: &ToolExecutionResult) -> bool {
    if !result.success {
        return false;
    }
    if result.result.get("ok").and_then(serde_json::Value::as_bool) == Some(false) {
        return false;
    }
    result
        .result
        .get("results")
        .and_then(serde_json::Value::as_array)
        .is_some_and(|items| {
            items.iter().any(|item| {
                item.get("success").and_then(serde_json::Value::as_bool) == Some(true)
                    || item.get("applied").and_then(serde_json::Value::as_bool) == Some(true)
                    || item.get("deleted").and_then(serde_json::Value::as_bool) == Some(true)
            })
        })
}

fn tracker_path(session_directory: &Path, session_id: &str) -> PathBuf {
    session_directory
        .join(".tura")
        .join("session_changes")
        .join(format!("{session_id}.json"))
}

fn read_records(path: &Path) -> Vec<ChangeRecord> {
    fs::read_to_string(path)
        .ok()
        .and_then(|content| serde_json::from_str(&content).ok())
        .unwrap_or_default()
}
