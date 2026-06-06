use crate::registry::resolve_binary_target;
use std::path::{Path, PathBuf};

pub fn resolve_tool_binary(repo_root: &Path, binary: &str) -> Option<PathBuf> {
    if binary.trim().is_empty() {
        return None;
    }
    resolve_binary_target(repo_root, binary)
}
