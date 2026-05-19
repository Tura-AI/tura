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
    match parse_patch(patch_text).and_then(|changes| {
        for change in &changes {
            apply_change(change, session_dir)?;
        }
        Ok(changes)
    }) {
        Ok(changes) => CommandResponse {
            success: true,
            exit_code: 0,
            stdout: "Success. Updated files.".to_string(),
            stderr: String::new(),
            output: json!({}),
            changes: changes.iter().map(patch_change_value).collect(),
        },
        Err(err) => CommandResponse {
            success: false,
            exit_code: 1,
            stdout: String::new(),
            stderr: err.clone(),
            output: Value::String(err),
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

    for line in patch_text.lines() {
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
            break;
        } else if let Some(change) = current.as_mut() {
            if change.kind == "add" && line.starts_with('+') {
                change.lines.push(line[1..].to_string());
            } else if let Some(hunk_lines) = hunk.as_mut() {
                if matches!(line.as_bytes().first(), Some(b' ' | b'+' | b'-')) {
                    hunk_lines.push(line.to_string());
                }
            }
        }
    }
    finish_change(&mut changes, &mut current, &mut hunk);
    if changes.is_empty() {
        return Err("no file changes found in patch".to_string());
    }
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

fn apply_change(change: &PatchChange, session_dir: &Path) -> Result<(), String> {
    let path = safe_path(session_dir, &change.path)?;
    match change.kind.as_str() {
        "delete" => {
            if path.exists() {
                std::fs::remove_file(path).map_err(|err| err.to_string())?;
            }
        }
        "add" => {
            let mut updated = change.lines.join("\n");
            if !updated.is_empty() && !updated.ends_with('\n') {
                updated.push('\n');
            }
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).map_err(|err| err.to_string())?;
            }
            std::fs::write(path, updated).map_err(|err| err.to_string())?;
        }
        "update" => {
            let original = std::fs::read_to_string(&path).unwrap_or_default();
            let updated = apply_hunks(&original, &change.hunks)?;
            let destination = match change.move_path.as_deref() {
                Some(move_path) => safe_path(session_dir, move_path)?,
                None => path.clone(),
            };
            if let Some(parent) = destination.parent() {
                std::fs::create_dir_all(parent).map_err(|err| err.to_string())?;
            }
            std::fs::write(&destination, updated).map_err(|err| err.to_string())?;
            if destination != path && path.exists() {
                std::fs::remove_file(path).map_err(|err| err.to_string())?;
            }
        }
        _ => return Err(format!("unsupported patch change kind: {}", change.kind)),
    }
    Ok(())
}

fn apply_hunks(original: &str, hunks: &[Vec<String>]) -> Result<String, String> {
    let mut text = original.to_string();
    for hunk in hunks {
        let old = hunk
            .iter()
            .filter(|line| line.starts_with(' ') || line.starts_with('-'))
            .map(|line| &line[1..])
            .collect::<Vec<_>>()
            .join("\n");
        let new = hunk
            .iter()
            .filter(|line| line.starts_with(' ') || line.starts_with('+'))
            .map(|line| &line[1..])
            .collect::<Vec<_>>()
            .join("\n");
        if !old.is_empty() && text.contains(&old) {
            text = text.replacen(&old, &new, 1);
        } else if old.is_empty() {
            if !text.is_empty() && !text.ends_with('\n') {
                text.push('\n');
            }
            text.push_str(&new);
        } else {
            return Err(format!(
                "patch context not found: {}",
                old.chars().take(120).collect::<String>()
            ));
        }
    }
    if original.ends_with('\n') && !text.ends_with('\n') {
        text.push('\n');
    }
    Ok(text)
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

#[cfg(test)]
mod tests {
    use super::execute;
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
