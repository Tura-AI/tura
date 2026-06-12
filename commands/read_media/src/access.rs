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

#[cfg(test)]
mod tests {
    use super::access_for_value;
    use serde_json::json;

    #[test]
    fn access_for_value_returns_workspace_relative_read_paths() {
        let dir = tempfile::tempdir().expect("tempdir");
        let nested = dir.path().join("media").join("image.png");
        std::fs::create_dir_all(nested.parent().expect("parent")).expect("mkdir");
        std::fs::write(&nested, b"png").expect("write");

        let access = access_for_value(
            &json!({ "paths": ["media/image.png", nested.display().to_string()] }),
            dir.path(),
        );

        let expected = std::path::Path::new("media")
            .join("image.png")
            .display()
            .to_string();
        assert_eq!(
            access.read_paths,
            vec!["media/image.png".to_string(), expected]
        );
        assert!(access.write_paths.is_empty());
        assert!(!access.workspace_write);
    }

    #[test]
    fn access_for_invalid_value_is_empty() {
        let dir = tempfile::tempdir().expect("tempdir");
        let access = access_for_value(&json!({ "maxFiles": 1 }), dir.path());

        assert!(access.read_paths.is_empty());
        assert!(access.write_paths.is_empty());
    }
}
