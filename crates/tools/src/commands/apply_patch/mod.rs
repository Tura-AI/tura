pub const COMMAND_NAME: &str = "apply_patch";
pub const PROMPT: &str = include_str!("prompt.md");
pub const POLICY: &str = include_str!("policy.toml");
pub const SCHEMA: &str = include_str!("schema.json");

use super::{shell_command, CommandResponse};
use crate::runtime::file_locks::Access;
use crate::runtime::tool::{
    FunctionToolOutput, ToolCall, ToolContext, ToolError, ToolHandler, ToolPayload,
};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
struct PatchChange {
    kind: String,
    path: String,
    move_path: Option<String>,
    hunks: Vec<Vec<String>>,
    lines: Vec<String>,
}

#[derive(Clone, Debug)]
struct PatchError {
    kind: &'static str,
    message: String,
    failed_change: Option<Value>,
}

pub struct ApplyPatchHandler;

#[async_trait::async_trait]
impl ToolHandler for ApplyPatchHandler {
    fn tool_name(&self) -> &'static str {
        "apply_patch"
    }

    async fn is_mutating(&self, _call: &ToolCall, _ctx: &ToolContext) -> bool {
        true
    }

    async fn access(&self, call: &ToolCall, ctx: &ToolContext) -> Access {
        let patch_text = patch_text_from_payload(&call.payload);
        access(&patch_text, &ctx.session_dir)
    }

    async fn handle(
        &self,
        call: ToolCall,
        ctx: ToolContext,
    ) -> Result<FunctionToolOutput, ToolError> {
        let patch_text = patch_text_from_payload(&call.payload);
        let response = execute(&patch_text, &ctx.session_dir);
        let success = response.success;
        Ok(FunctionToolOutput::from_value(
            shell_command::json_like_output(
                response.exit_code,
                response.stdout,
                response.stderr,
                response.output,
                response.changes,
            ),
            Some(success),
        ))
    }
}

fn patch_text_from_payload(payload: &ToolPayload) -> String {
    match payload {
        ToolPayload::Freeform { input } => input.clone(),
        ToolPayload::Function { arguments } => arguments
            .get("patch")
            .or_else(|| arguments.get("command"))
            .and_then(Value::as_str)
            .map(ToString::to_string)
            .unwrap_or_else(|| arguments.as_str().unwrap_or_default().to_string()),
    }
}

pub fn execute(patch_text: &str, session_dir: &Path) -> CommandResponse {
    match parse_patch(patch_text) {
        Ok(changes) => {
            let mut applied_changes = Vec::new();
            for change in &changes {
                match apply_change(change, session_dir) {
                    Ok(applied_change) => applied_changes.push(applied_change),
                    Err(err) => {
                        let partial = !applied_changes.is_empty();
                        let mut output = json!({
                            "error_type": err.kind,
                            "message": err.message,
                            "guidance": apply_patch_failure_guidance(err.kind, partial),
                        });
                        if let Some(failed_change) = err.failed_change {
                            output["failed_change"] = failed_change;
                        }
                        if partial {
                            output["partial_changes"] = Value::Array(applied_changes.clone());
                        }
                        return CommandResponse {
                            success: false,
                            exit_code: 1,
                            stdout: String::new(),
                            stderr: output["message"].as_str().unwrap_or_default().to_string(),
                            output,
                            changes: applied_changes,
                        };
                    }
                }
            }
            CommandResponse {
                success: true,
                exit_code: 0,
                stdout: "Success. Updated files.".to_string(),
                stderr: String::new(),
                output: json!({}),
                changes: applied_changes,
            }
        }
        Err(err) => CommandResponse {
            success: false,
            exit_code: 1,
            stdout: String::new(),
            stderr: err.clone(),
            output: json!({
                "error_type": "ParseError",
                "message": err,
            }),
            changes: Vec::new(),
        },
    }
}

pub fn access(patch_text: &str, session_dir: &Path) -> Access {
    match parse_patch(patch_text) {
        Ok(changes) => Access {
            write_paths: changes
                .iter()
                .flat_map(|change| {
                    let mut keys = Vec::new();
                    if let Some(key) = lock_key(session_dir, &change.path) {
                        keys.push(key);
                    }
                    if let Some(move_path) = change.move_path.as_deref() {
                        if let Some(key) = lock_key(session_dir, move_path) {
                            keys.push(key);
                        }
                    }
                    keys
                })
                .collect(),
            ..Access::default()
        },
        Err(_) => Access {
            workspace_write: true,
            ..Access::default()
        },
    }
}

fn parse_patch(patch_text: &str) -> Result<Vec<PatchChange>, String> {
    let mut changes = Vec::new();
    let mut current: Option<PatchChange> = None;
    let mut hunk: Option<Vec<String>> = None;
    let mut started = false;
    let mut ended = false;

    for (line_index, line) in patch_text.lines().enumerate() {
        let line_number = line_index + 1;
        if !started {
            if line.trim().is_empty() {
                continue;
            }
            if line == "*** Begin Patch" {
                started = true;
                continue;
            }
            return Err(format!(
                "invalid patch: expected *** Begin Patch at line {line_number}"
            ));
        }
        if let Some(path) = line.strip_prefix("*** Update File: ") {
            finish_change(&mut changes, &mut current, &mut hunk);
            current = Some(PatchChange {
                kind: "update".to_string(),
                path: path.to_string(),
                move_path: None,
                hunks: Vec::new(),
                lines: Vec::new(),
            });
        } else if let Some(path) = line.strip_prefix("*** Add File: ") {
            finish_change(&mut changes, &mut current, &mut hunk);
            current = Some(PatchChange {
                kind: "add".to_string(),
                path: path.to_string(),
                move_path: None,
                hunks: Vec::new(),
                lines: Vec::new(),
            });
        } else if let Some(path) = line.strip_prefix("*** Delete File: ") {
            finish_change(&mut changes, &mut current, &mut hunk);
            current = Some(PatchChange {
                kind: "delete".to_string(),
                path: path.to_string(),
                move_path: None,
                hunks: Vec::new(),
                lines: Vec::new(),
            });
        } else if let Some(path) = line.strip_prefix("*** Move to: ") {
            let Some(change) = current.as_mut() else {
                return Err("move target without file".to_string());
            };
            if change.kind != "update" {
                return Err("move target is only valid for update file changes".to_string());
            }
            change.move_path = Some(path.to_string());
        } else if line.starts_with("@@") {
            let Some(change) = current.as_ref() else {
                return Err("hunk without file".to_string());
            };
            if change.kind != "update" {
                return Err("hunk is only valid for update file changes".to_string());
            }
            if let Some(hunk_lines) = hunk.take() {
                current
                    .as_mut()
                    .expect("change exists")
                    .hunks
                    .push(hunk_lines);
            }
            hunk = Some(Vec::new());
        } else if line.starts_with("*** End Patch") {
            finish_change(&mut changes, &mut current, &mut hunk);
            ended = true;
            break;
        } else if let Some(change) = current.as_mut() {
            if change.kind == "add" && line.starts_with('+') {
                change.lines.push(line[1..].to_string());
            } else if let Some(hunk_lines) = hunk.as_mut() {
                if matches!(line.as_bytes().first(), Some(b' ' | b'+' | b'-')) {
                    hunk_lines.push(line.to_string());
                } else if line.trim().is_empty() {
                    hunk_lines.push(format!(" {line}"));
                } else {
                    return Err(format!(
                        "invalid patch line {line_number}: hunk lines must start with space, +, or -"
                    ));
                }
            } else if line.trim().is_empty() {
                continue;
            } else {
                return Err(format!(
                    "invalid patch line {line_number}: content must be inside a hunk"
                ));
            }
        } else if line.trim().is_empty() {
            continue;
        } else {
            return Err(format!(
                "invalid patch line {line_number}: expected file operation"
            ));
        }
    }
    if !started {
        return Err("invalid patch: missing *** Begin Patch".to_string());
    }
    if !ended {
        return Err("invalid patch: missing *** End Patch".to_string());
    }
    if changes.is_empty() {
        return Err("no file changes found in patch".to_string());
    }
    validate_changes(&changes)?;
    Ok(changes)
}

fn finish_change(
    changes: &mut Vec<PatchChange>,
    current: &mut Option<PatchChange>,
    hunk: &mut Option<Vec<String>>,
) {
    if let Some(hunk_lines) = hunk.take() {
        if let Some(change) = current.as_mut() {
            change.hunks.push(hunk_lines);
        }
    }
    if let Some(change) = current.take() {
        changes.push(change);
    }
}

fn validate_changes(changes: &[PatchChange]) -> Result<(), String> {
    for change in changes {
        if change.path.trim().is_empty() {
            return Err("invalid patch: file path must not be empty".to_string());
        }
        match change.kind.as_str() {
            "add" => {
                if change.move_path.is_some() {
                    return Err("invalid patch: add file cannot have move target".to_string());
                }
                if !change.hunks.is_empty() {
                    return Err("invalid patch: add file cannot contain hunks".to_string());
                }
            }
            "delete" => {
                if change.move_path.is_some() {
                    return Err("invalid patch: delete file cannot have move target".to_string());
                }
                if !change.hunks.is_empty() || !change.lines.is_empty() {
                    return Err("invalid patch: delete file cannot contain content".to_string());
                }
            }
            "update" => {
                if change.hunks.is_empty() {
                    return Err(format!(
                        "invalid patch: update file {} must contain at least one hunk",
                        change.path
                    ));
                }
                if change.hunks.iter().any(Vec::is_empty) {
                    return Err(format!(
                        "invalid patch: update file {} contains an empty hunk",
                        change.path
                    ));
                }
            }
            other => return Err(format!("unsupported patch change kind: {other}")),
        }
    }
    Ok(())
}

fn apply_change(change: &PatchChange, session_dir: &Path) -> Result<Value, PatchError> {
    let path = safe_path(session_dir, &change.path).map_err(PatchError::path)?;
    match change.kind.as_str() {
        "delete" => {
            if !path.exists() {
                return Err(PatchError::missing_file("DeleteFileNotFound", change));
            }
            std::fs::remove_file(path).map_err(PatchError::io)?;
        }
        "add" => {
            if path.exists() {
                return Err(PatchError::file_exists(change));
            }
            let mut updated = change.lines.join("\n");
            if !updated.is_empty() && !updated.ends_with('\n') {
                updated.push('\n');
            }
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).map_err(PatchError::io)?;
            }
            std::fs::write(path, updated).map_err(PatchError::io)?;
        }
        "update" => {
            if !path.exists() {
                return Err(PatchError::missing_file("UpdateFileNotFound", change));
            }
            let original = std::fs::read_to_string(&path).map_err(PatchError::io)?;
            let updated = apply_hunks(&original, &change.hunks)
                .map_err(|message| PatchError::context_mismatch(message, change))?;
            let destination = match change.move_path.as_deref() {
                Some(move_path) => safe_path(session_dir, move_path).map_err(PatchError::path)?,
                None => path.clone(),
            };
            if let Some(parent) = destination.parent() {
                std::fs::create_dir_all(parent).map_err(PatchError::io)?;
            }
            std::fs::write(&destination, updated).map_err(PatchError::io)?;
            if destination != path && path.exists() {
                std::fs::remove_file(path).map_err(PatchError::io)?;
            }
        }
        _ => {
            return Err(PatchError {
                kind: "ParseError",
                message: format!("unsupported patch change kind: {}", change.kind),
                failed_change: Some(patch_change_value(change)),
            })
        }
    }
    Ok(patch_change_value(change))
}

fn apply_hunks(original: &str, hunks: &[Vec<String>]) -> Result<String, String> {
    let mut lines = original
        .lines()
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    let line_ending = dominant_line_ending(original);
    let original_had_final_newline = original.ends_with('\n') || original.ends_with('\r');
    let mut replacements = Vec::new();
    let mut search_start = 0;

    for hunk in hunks {
        let old = hunk
            .iter()
            .filter(|line| line.starts_with(' ') || line.starts_with('-'))
            .map(|line| &line[1..])
            .map(ToString::to_string)
            .collect::<Vec<_>>();
        let new = hunk
            .iter()
            .filter(|line| line.starts_with(' ') || line.starts_with('+'))
            .map(|line| &line[1..])
            .map(ToString::to_string)
            .collect::<Vec<_>>();
        if old.is_empty() {
            replacements.push((lines.len(), lines.len(), new));
            search_start = lines.len();
        } else if let Some(start) = seek_sequence(&lines, &old, search_start) {
            let end = start + old.len();
            let new = replacement_lines_for_hunk(hunk, &lines[start..end]);
            replacements.push((start, end, new));
            search_start = end;
        } else {
            return Err(format!(
                "patch context not found: {}",
                old.join("\n").chars().take(120).collect::<String>()
            ));
        }
    }

    for (start, end, new) in replacements.into_iter().rev() {
        lines.splice(start..end, new);
    }
    Ok(join_lines(&lines, line_ending, original_had_final_newline))
}

fn replacement_lines_for_hunk(hunk: &[String], actual_old_lines: &[String]) -> Vec<String> {
    let mut actual_index = 0;
    let mut replacement = Vec::new();
    for line in hunk {
        if let Some(text) = line.strip_prefix(' ') {
            replacement.push(
                actual_old_lines
                    .get(actual_index)
                    .cloned()
                    .unwrap_or_else(|| text.to_string()),
            );
            actual_index += 1;
        } else if let Some(text) = line.strip_prefix('-') {
            let _ = text;
            actual_index += 1;
        } else if let Some(text) = line.strip_prefix('+') {
            replacement.push(text.to_string());
        }
    }
    replacement
}

fn safe_path(root: &Path, raw: &str) -> Result<PathBuf, String> {
    let root = root.canonicalize().map_err(|err| err.to_string())?;
    let raw_path = patch_path(raw);
    let path = if raw_path.is_absolute() {
        raw_path
    } else {
        root.join(raw_path)
    };
    let path = path.canonicalize().unwrap_or(path);
    if !path_is_inside(&path, &root) {
        return Err(format!("path outside session directory: {raw}"));
    }
    Ok(path)
}

fn lock_key(root: &Path, raw: &str) -> Option<String> {
    let path = safe_path(root, raw).ok()?;
    let root = root.canonicalize().ok()?;
    path.strip_prefix(&root)
        .ok()
        .map(|path| path.to_string_lossy().replace('\\', "/"))
        .or_else(|| {
            let path = comparable_path_string(&path);
            let root = comparable_path_string(&root);
            path.strip_prefix(&root)
                .map(|suffix| suffix.trim_start_matches('/').to_string())
        })
}

fn patch_path(raw: &str) -> PathBuf {
    let trimmed = raw.trim();
    #[cfg(windows)]
    {
        let normalized = trimmed.replace('\\', "/");
        let bytes = normalized.as_bytes();
        if normalized.len() > 3
            && bytes[0] == b'/'
            && bytes[1].is_ascii_alphabetic()
            && bytes[2] == b'/'
        {
            let drive = (bytes[1] as char).to_ascii_uppercase();
            return PathBuf::from(format!("{drive}:\\{}", normalized[3..].replace('/', "\\")));
        }
    }
    PathBuf::from(trimmed)
}

fn path_is_inside(path: &Path, root: &Path) -> bool {
    path == root || path.starts_with(root) || {
        let path = comparable_path_string(path);
        let root = comparable_path_string(root);
        path == root || path.starts_with(&(root + "/"))
    }
}

fn comparable_path_string(path: &Path) -> String {
    let mut text = path.to_string_lossy().replace('\\', "/");
    if let Some(stripped) = text.strip_prefix("//?/") {
        text = stripped.to_string();
    }
    #[cfg(windows)]
    {
        text = text.to_ascii_lowercase();
    }
    text.trim_end_matches('/').to_string()
}

fn patch_change_value(change: &PatchChange) -> Value {
    match change.kind.as_str() {
        "add" => json!({"kind": change.kind, "path": change.path, "lines": change.lines}),
        _ => json!({
            "kind": change.kind,
            "path": change.path,
            "move_path": change.move_path,
            "hunks": change.hunks
        }),
    }
}

impl PatchError {
    fn context_mismatch(message: String, change: &PatchChange) -> Self {
        Self {
            kind: "ContextMismatch",
            message,
            failed_change: Some(patch_change_value(change)),
        }
    }

    fn io(err: std::io::Error) -> Self {
        let kind = if err.kind() == std::io::ErrorKind::PermissionDenied {
            "PermissionDenied"
        } else {
            "IoError"
        };
        Self {
            kind,
            message: err.to_string(),
            failed_change: None,
        }
    }

    fn missing_file(kind: &'static str, change: &PatchChange) -> Self {
        Self {
            kind,
            message: format!("{}: {}", kind, change.path),
            failed_change: Some(patch_change_value(change)),
        }
    }

    fn file_exists(change: &PatchChange) -> Self {
        Self {
            kind: "AddFileExists",
            message: format!("AddFileExists: {}", change.path),
            failed_change: Some(patch_change_value(change)),
        }
    }

    fn path(message: String) -> Self {
        let kind = if message.contains("outside session directory") {
            "PermissionDenied"
        } else {
            "IoError"
        };
        Self {
            kind,
            message,
            failed_change: None,
        }
    }
}

fn apply_patch_failure_guidance(kind: &str, partial: bool) -> &'static str {
    match (kind, partial) {
        ("ContextMismatch", true) => {
            "apply_patch failed because expected context was not found after earlier changes were applied; read the current file and retry smaller hunks. Subsequent commands run against a partially changed tree."
        }
        ("ContextMismatch", false) => {
            "apply_patch failed because expected context was not found; read the current file and retry with a smaller hunk."
        }
        (_, true) => {
            "apply_patch failed after earlier changes were applied; inspect partial_changes before retrying."
        }
        _ => "apply_patch failed; inspect error_type and message before retrying.",
    }
}

fn dominant_line_ending(text: &str) -> &'static str {
    if text.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    }
}

fn join_lines(lines: &[String], line_ending: &str, final_newline: bool) -> String {
    let mut text = lines.join(line_ending);
    if final_newline && !text.is_empty() {
        text.push_str(line_ending);
    }
    text
}

fn seek_sequence(lines: &[String], pattern: &[String], start: usize) -> Option<usize> {
    if pattern.is_empty() {
        return Some(start.min(lines.len()));
    }
    if pattern.len() > lines.len() || start > lines.len().saturating_sub(pattern.len()) {
        return None;
    }
    let max_start = lines.len() - pattern.len();
    for index in start..=max_start {
        let candidate = &lines[index..index + pattern.len()];
        if sequence_matches(candidate, pattern, MatchMode::Exact)
            || sequence_matches(candidate, pattern, MatchMode::TrimEnd)
            || sequence_matches(candidate, pattern, MatchMode::Trim)
            || sequence_matches(candidate, pattern, MatchMode::Normalized)
        {
            return Some(index);
        }
    }
    None
}

#[derive(Clone, Copy)]
enum MatchMode {
    Exact,
    TrimEnd,
    Trim,
    Normalized,
}

fn sequence_matches(candidate: &[String], pattern: &[String], mode: MatchMode) -> bool {
    candidate
        .iter()
        .zip(pattern)
        .all(|(left, right)| match mode {
            MatchMode::Exact => left == right,
            MatchMode::TrimEnd => left.trim_end() == right.trim_end(),
            MatchMode::Trim => left.trim() == right.trim(),
            MatchMode::Normalized => normalize_match_text(left) == normalize_match_text(right),
        })
}

fn normalize_match_text(text: &str) -> String {
    text.trim()
        .chars()
        .map(|ch| match ch {
            '\u{00a0}' | '\u{2007}' | '\u{202f}' => ' ',
            '\u{2010}' | '\u{2011}' | '\u{2012}' | '\u{2013}' | '\u{2014}' | '\u{2212}' => '-',
            '\u{2018}' | '\u{2019}' | '\u{201a}' | '\u{201b}' => '\'',
            '\u{201c}' | '\u{201d}' | '\u{201e}' | '\u{201f}' => '"',
            other => other,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::execute;
    use serde_json::json;
    use std::fs;

    fn temp_workspace(name: &str) -> std::path::PathBuf {
        let path =
            std::env::temp_dir().join(format!("tura-apply-patch-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("create temp workspace");
        path
    }

    #[test]
    fn add_file_accepts_relative_path_under_session_dir() {
        let root = temp_workspace("relative");
        let result = execute(
            "*** Begin Patch\n*** Add File: checked.txt\n+ok\n*** End Patch\n",
            &root,
        );

        assert!(result.success, "{}", result.stderr);
        assert_eq!(
            fs::read_to_string(root.join("checked.txt")).expect("created file"),
            "ok\n"
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn update_file_matches_lf_patch_against_crlf_file_and_preserves_crlf() {
        let root = temp_workspace("crlf");
        fs::write(root.join("app.txt"), "alpha\r\nold\r\nomega\r\n").expect("fixture");

        let result = execute(
            "*** Begin Patch\n*** Update File: app.txt\n@@\n alpha\n-old\n+new\n omega\n*** End Patch\n",
            &root,
        );

        assert!(result.success, "{}", result.stderr);
        assert_eq!(
            fs::read_to_string(root.join("app.txt")).expect("read fixture"),
            "alpha\r\nnew\r\nomega\r\n"
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn update_file_tolerates_trailing_whitespace_context_mismatch() {
        let root = temp_workspace("trailing-space");
        fs::write(root.join("app.txt"), "alpha  \nold\t\nomega\n").expect("fixture");

        let result = execute(
            "*** Begin Patch\n*** Update File: app.txt\n@@\n alpha\n-old\n+new\n omega\n*** End Patch\n",
            &root,
        );

        assert!(result.success, "{}", result.stderr);
        assert_eq!(
            fs::read_to_string(root.join("app.txt")).expect("read fixture"),
            "alpha  \nnew\nomega\n"
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn update_file_tolerates_normalized_unicode_punctuation_context() {
        let root = temp_workspace("unicode-normalize");
        fs::write(root.join("app.txt"), "say “hello”\nold – value\n").expect("fixture");

        let result = execute(
            "*** Begin Patch\n*** Update File: app.txt\n@@\n say \"hello\"\n-old - value\n+new - value\n*** End Patch\n",
            &root,
        );

        assert!(result.success, "{}", result.stderr);
        assert_eq!(
            fs::read_to_string(root.join("app.txt")).expect("read fixture"),
            "say “hello”\nnew - value\n"
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn update_file_applies_multiple_hunks_without_position_shift() {
        let root = temp_workspace("multi-hunk");
        fs::write(root.join("app.txt"), "one\nold-a\nmiddle\nold-b\nend\n").expect("fixture");

        let result = execute(
            "*** Begin Patch\n*** Update File: app.txt\n@@\n-old-a\n+new-a\n@@\n-old-b\n+new-b\n*** End Patch\n",
            &root,
        );

        assert!(result.success, "{}", result.stderr);
        assert_eq!(
            fs::read_to_string(root.join("app.txt")).expect("read fixture"),
            "one\nnew-a\nmiddle\nnew-b\nend\n"
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn failed_later_file_reports_partial_changes_and_failed_change() {
        let root = temp_workspace("partial");
        fs::write(root.join("first.txt"), "old\n").expect("first");
        fs::write(root.join("second.txt"), "actual\n").expect("second");

        let result = execute(
            "*** Begin Patch\n*** Update File: first.txt\n@@\n-old\n+new\n*** Update File: second.txt\n@@\n-missing\n+value\n*** End Patch\n",
            &root,
        );

        assert!(!result.success);
        assert_eq!(result.output["error_type"], json!("ContextMismatch"));
        assert_eq!(
            result.output["partial_changes"][0]["path"],
            json!("first.txt")
        );
        assert_eq!(result.output["failed_change"]["path"], json!("second.txt"));
        assert_eq!(
            fs::read_to_string(root.join("first.txt")).expect("first"),
            "new\n"
        );
        assert_eq!(
            fs::read_to_string(root.join("second.txt")).expect("second"),
            "actual\n"
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn path_escape_failure_is_classified_as_permission_denied() {
        let root = temp_workspace("path-escape");
        let outside = root.parent().unwrap().join("outside-apply-patch-test.txt");
        let _ = fs::remove_file(&outside);

        let result = execute(
            &format!(
                "*** Begin Patch\n*** Add File: {}\n+bad\n*** End Patch\n",
                outside.display()
            ),
            &root,
        );

        assert!(!result.success);
        assert_eq!(result.output["error_type"], json!("PermissionDenied"));
        assert!(!outside.exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn parser_rejects_patch_without_begin_marker() {
        let root = temp_workspace("missing-begin");

        let result = execute("*** Add File: app.txt\n+ok\n*** End Patch\n", &root);

        assert!(!result.success);
        assert_eq!(result.output["error_type"], json!("ParseError"));
        assert!(result.output["message"]
            .as_str()
            .is_some_and(|text| text.contains("Begin Patch")));
        assert!(!root.join("app.txt").exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn parser_rejects_patch_without_end_marker() {
        let root = temp_workspace("missing-end");

        let result = execute("*** Begin Patch\n*** Add File: app.txt\n+ok\n", &root);

        assert!(!result.success);
        assert_eq!(result.output["error_type"], json!("ParseError"));
        assert!(result.output["message"]
            .as_str()
            .is_some_and(|text| text.contains("End Patch")));
        assert!(!root.join("app.txt").exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn parser_rejects_update_content_outside_hunk() {
        let root = temp_workspace("outside-hunk");
        fs::write(root.join("app.txt"), "old\n").expect("fixture");

        let result = execute(
            "*** Begin Patch\n*** Update File: app.txt\n-old\n+new\n*** End Patch\n",
            &root,
        );

        assert!(!result.success);
        assert_eq!(result.output["error_type"], json!("ParseError"));
        assert_eq!(
            fs::read_to_string(root.join("app.txt")).expect("fixture"),
            "old\n"
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn update_file_missing_is_structured_error() {
        let root = temp_workspace("update-missing");

        let result = execute(
            "*** Begin Patch\n*** Update File: missing.txt\n@@\n-old\n+new\n*** End Patch\n",
            &root,
        );

        assert!(!result.success);
        assert_eq!(result.output["error_type"], json!("UpdateFileNotFound"));
        assert_eq!(result.output["failed_change"]["path"], json!("missing.txt"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn delete_file_missing_is_structured_error() {
        let root = temp_workspace("delete-missing");

        let result = execute(
            "*** Begin Patch\n*** Delete File: missing.txt\n*** End Patch\n",
            &root,
        );

        assert!(!result.success);
        assert_eq!(result.output["error_type"], json!("DeleteFileNotFound"));
        assert_eq!(result.output["failed_change"]["path"], json!("missing.txt"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn add_file_existing_is_structured_error() {
        let root = temp_workspace("add-existing");
        fs::write(root.join("app.txt"), "old\n").expect("fixture");

        let result = execute(
            "*** Begin Patch\n*** Add File: app.txt\n+new\n*** End Patch\n",
            &root,
        );

        assert!(!result.success);
        assert_eq!(result.output["error_type"], json!("AddFileExists"));
        assert_eq!(
            fs::read_to_string(root.join("app.txt")).expect("fixture"),
            "old\n"
        );
        let _ = fs::remove_dir_all(root);
    }

    #[cfg(windows)]
    #[test]
    fn add_file_accepts_git_bash_absolute_path_inside_session_dir() {
        let root = temp_workspace("git-bash-path");
        let path = root.join("checked.txt");
        let raw = path.to_string_lossy().replace('\\', "/");
        let drive = raw
            .chars()
            .next()
            .expect("drive letter")
            .to_ascii_lowercase();
        let git_bash_path = format!("/{drive}/{}", &raw[3..]);
        let result = execute(
            &format!("*** Begin Patch\n*** Add File: {git_bash_path}\n+ok\n*** End Patch\n"),
            &root,
        );

        assert!(result.success, "{}", result.stderr);
        assert_eq!(fs::read_to_string(path).expect("created file"), "ok\n");
        let _ = fs::remove_dir_all(root);
    }
}
