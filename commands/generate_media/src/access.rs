use super::args::{parse_args_text, parse_args_value};
use super::files::workspace_relative_path;
use crate::runtime::file_locks::Access;
use serde_json::Value;
use std::path::Path;

pub(super) fn access(command_line: &str, session_dir: &Path) -> Access {
    let Ok(args) = parse_args_text(command_line) else {
        return Access::default();
    };
    access_for_args(&args, session_dir)
}

pub(super) fn access_for_value(value: Value, session_dir: &Path) -> Access {
    let Ok(args) = parse_args_value(value) else {
        return Access::default();
    };
    access_for_args(&args, session_dir)
}

fn access_for_args(args: &super::types::GenerateMediaArgs, session_dir: &Path) -> Access {
    let mut access = Access::default();
    for reference in &args.references {
        if reference.starts_with("http://")
            || reference.starts_with("https://")
            || reference.starts_with("data:")
        {
            continue;
        }
        if let Some(path) = workspace_relative_path(reference, session_dir) {
            access.read_paths.push(path.display().to_string());
        }
    }
    if let Some(path) = workspace_relative_path(&args.output_dir, session_dir) {
        access
            .write_paths
            .push(format!("{}/.generate_media", path.display()));
    } else {
        access.workspace_write = true;
    }
    access
}
