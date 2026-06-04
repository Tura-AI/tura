use super::args::parse_args_value;
use super::paths::workspace_relative_path;
use crate::runtime::file_locks::Access;
use serde_json::Value;
use std::path::Path;

pub(super) fn access_for_value(value: &Value, session_dir: &Path) -> Access {
    let Ok(args) = parse_args_value(value.clone()) else {
        return Access::default();
    };
    Access {
        read_paths: args
            .paths
            .iter()
            .filter_map(|path| workspace_relative_path(path, session_dir))
            .map(|path| path.display().to_string())
            .collect(),
        ..Access::default()
    }
}
