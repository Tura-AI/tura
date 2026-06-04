use super::args::{parse_args_text, parse_args_value};
use super::files::{web_discover_write_scope, workspace_relative_path};
use crate::runtime::file_locks::Access;
use serde_json::Value;
use std::path::Path;

pub(super) fn access(command_line: &str, session_dir: &Path) -> Access {
    let Ok(args) = parse_args_text(command_line) else {
        return Access::default();
    };
    if args.download_dir.is_none() {
        return Access::default();
    }
    let mut access = Access::default();
    if let Some(dir) = args.download_dir.as_deref() {
        if let Some(relative) = workspace_relative_path(dir, session_dir) {
            access
                .write_paths
                .push(web_discover_write_scope(&args, &relative));
        }
    }
    access
}

pub(super) fn access_for_value(value: Value, session_dir: &Path) -> Access {
    let Ok(args) = parse_args_value(value) else {
        return Access::default();
    };
    if args.download_dir.is_none() {
        return Access::default();
    }
    let mut access = Access::default();
    if let Some(dir) = args.download_dir.as_deref() {
        if let Some(relative) = workspace_relative_path(dir, session_dir) {
            access
                .write_paths
                .push(web_discover_write_scope(&args, &relative));
        }
    }
    access
}
