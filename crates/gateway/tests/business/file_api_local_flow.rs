use axum::extract::{Json, Query};
use axum::http::StatusCode;
use gateway::api::file::{get_file_content, list_files, open_file, open_file_location};
use gateway::contracts::{FileContentQuery, ListFilesQuery};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[tokio::test]
async fn gateway_file_api_business_flow_lists_reads_and_rejects_workspace_escape() {
    let workspace = tempfile::tempdir().expect("workspace");
    let root = workspace.path();
    fs::create_dir_all(root.join("src")).expect("src directory");
    fs::create_dir_all(root.join("target")).expect("hidden target directory");
    fs::write(root.join("README.md"), "hello from gateway\n").expect("readme");
    fs::write(root.join("src").join("main.rs"), "fn main() {}\n").expect("main");
    fs::write(root.join("image.png"), [0x89, b'P', b'N', b'G']).expect("png");
    fs::write(root.join("blob.bin"), [0xff, 0x00, 0x80]).expect("binary");

    let Json(root_entries) = list_files(Query(ListFilesQuery {
        directory: Some(root.to_string_lossy().to_string()),
        path: None,
    }))
    .await;
    let names = root_entries
        .iter()
        .map(|entry| (entry.name.as_str(), entry.file_type.as_str()))
        .collect::<Vec<_>>();
    assert_eq!(
        names,
        vec![
            ("src", "directory"),
            ("blob.bin", "file"),
            ("image.png", "file"),
            ("README.md", "file")
        ]
    );
    assert!(
        !root_entries.iter().any(|entry| entry.name == "target"),
        "build/cache directories should stay hidden from the file list"
    );
    assert!(
        root_entries
            .iter()
            .all(|entry| entry.git_status.as_deref() == Some("not_git")),
        "non-git workspaces should have stable not_git status"
    );

    let Json(src_entries) = list_files(Query(ListFilesQuery {
        directory: Some(root.to_string_lossy().to_string()),
        path: Some("src".to_string()),
    }))
    .await;
    assert_eq!(src_entries.len(), 1);
    assert_eq!(src_entries[0].path, "src/main.rs");
    assert_eq!(src_entries[0].file_type, "file");

    let Json(text) = get_file_content(Query(FileContentQuery {
        directory: Some(root.to_string_lossy().to_string()),
        path: "README.md".to_string(),
    }))
    .await
    .expect("read text");
    assert_eq!(text.content_type, "text");
    assert!(text.content.contains("hello from gateway"));
    assert_eq!(text.encoding, None);

    let Json(media) = get_file_content(Query(FileContentQuery {
        directory: Some(root.to_string_lossy().to_string()),
        path: "image.png".to_string(),
    }))
    .await
    .expect("read media");
    assert_eq!(media.content_type, "media");
    assert_eq!(media.encoding.as_deref(), Some("base64"));
    assert_eq!(media.mime_type.as_deref(), Some("image/png"));
    assert!(!media.content.is_empty());

    let Json(binary) = get_file_content(Query(FileContentQuery {
        directory: Some(root.to_string_lossy().to_string()),
        path: "blob.bin".to_string(),
    }))
    .await
    .expect("read binary");
    assert_eq!(binary.content_type, "binary");
    assert_eq!(binary.content, "");

    let escape = get_file_content(Query(FileContentQuery {
        directory: Some(root.to_string_lossy().to_string()),
        path: "../outside.txt".to_string(),
    }))
    .await
    .expect_err("path escape should be rejected");
    assert_eq!(escape.0, StatusCode::BAD_REQUEST);
    assert!(escape.1.contains("inside the workspace"));

    let missing = get_file_content(Query(FileContentQuery {
        directory: Some(root.to_string_lossy().to_string()),
        path: "missing.txt".to_string(),
    }))
    .await
    .expect_err("missing file should be not found");
    assert_eq!(missing.0, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn gateway_file_api_business_flow_handles_absolute_paths_without_workspace_leakage() {
    let workspace = tempfile::tempdir().expect("workspace");
    let outside = tempfile::tempdir().expect("outside");
    let root = workspace.path();
    let inside_file = root.join("src").join("absolute.txt");
    fs::create_dir_all(inside_file.parent().expect("inside parent")).expect("inside dir");
    fs::write(&inside_file, "absolute path inside workspace\n").expect("inside text");
    let outside_file = outside.path().join("secret.txt");
    fs::write(&outside_file, "must not leak\n").expect("outside text");

    let Json(inside) = get_file_content(Query(FileContentQuery {
        directory: Some(root.to_string_lossy().to_string()),
        path: inside_file.to_string_lossy().to_string(),
    }))
    .await
    .expect("absolute path inside workspace should be readable");
    assert_eq!(inside.content_type, "text");
    assert!(inside.content.contains("absolute path inside workspace"));

    let escape = get_file_content(Query(FileContentQuery {
        directory: Some(root.to_string_lossy().to_string()),
        path: outside_file.to_string_lossy().to_string(),
    }))
    .await
    .expect_err("absolute path outside workspace should be rejected");
    assert_eq!(escape.0, StatusCode::BAD_REQUEST);
    assert!(escape.1.contains("inside the workspace"));

    let Json(escaped_list) = list_files(Query(ListFilesQuery {
        directory: Some(root.to_string_lossy().to_string()),
        path: Some("../".to_string()),
    }))
    .await;
    assert!(
        escaped_list.is_empty(),
        "escaped list path should not reveal parent directory entries"
    );

    let Json(missing_list) = list_files(Query(ListFilesQuery {
        directory: Some(root.to_string_lossy().to_string()),
        path: Some("missing-directory".to_string()),
    }))
    .await;
    assert!(
        missing_list.is_empty(),
        "missing list path should be an empty file tree response"
    );
}

#[tokio::test]
async fn gateway_file_api_business_flow_open_actions_validate_paths_before_process_launch() {
    let workspace = tempfile::tempdir().expect("workspace");
    let root = workspace.path();
    fs::write(root.join("README.md"), "hello from gateway\n").expect("readme");

    let missing_workspace = open_file(Query(FileContentQuery {
        directory: None,
        path: "README.md".to_string(),
    }))
    .await
    .expect_err("relative open without workspace should fail");
    assert_eq!(missing_workspace.0, StatusCode::BAD_REQUEST);
    assert!(missing_workspace
        .1
        .contains("No workspace directory was provided for file open"));

    let escape = open_file(Query(FileContentQuery {
        directory: Some(root.to_string_lossy().to_string()),
        path: "../outside.txt".to_string(),
    }))
    .await
    .expect_err("open should reject path escape");
    assert_eq!(escape.0, StatusCode::BAD_REQUEST);
    assert!(escape.1.contains("inside the workspace"));

    let missing = open_file(Query(FileContentQuery {
        directory: Some(root.to_string_lossy().to_string()),
        path: "missing.txt".to_string(),
    }))
    .await
    .expect_err("missing file should be not found before open");
    assert_eq!(missing.0, StatusCode::NOT_FOUND);
    assert!(missing.1.contains("File was not found"));

    let missing_location = open_file_location(Query(FileContentQuery {
        directory: Some(root.to_string_lossy().to_string()),
        path: "missing.txt".to_string(),
    }))
    .await
    .expect_err("missing file location should be not found before open");
    assert_eq!(missing_location.0, StatusCode::NOT_FOUND);
    assert!(missing_location.1.contains("File was not found"));
}

#[tokio::test]
async fn gateway_file_api_business_flow_reports_git_statuses_for_workspace_tree() {
    let workspace = tempfile::tempdir().expect("workspace");
    let root = workspace.path();
    run_git(root, ["init"]);
    run_git(
        root,
        ["config", "user.email", "gateway-file-api@example.test"],
    );
    run_git(root, ["config", "user.name", "Gateway File API Test"]);

    fs::create_dir_all(root.join("src")).expect("src directory");
    fs::write(root.join(".gitignore"), "ignored.log\n").expect("gitignore");
    fs::write(root.join("tracked.txt"), "before\n").expect("tracked");
    fs::write(root.join("rename-old.txt"), "rename me\n").expect("renamed source");
    fs::write(root.join("src").join("clean.rs"), "fn clean() {}\n").expect("clean src");
    run_git(
        root,
        [
            "add",
            ".gitignore",
            "tracked.txt",
            "rename-old.txt",
            "src/clean.rs",
        ],
    );
    run_git(root, ["commit", "-m", "seed file api status fixture"]);

    fs::write(root.join("tracked.txt"), "after\n").expect("modified tracked");
    fs::write(root.join("untracked.txt"), "new\n").expect("untracked");
    fs::write(root.join("ignored.log"), "ignored\n").expect("ignored");
    fs::write(root.join("src").join("new.rs"), "fn new_file() {}\n").expect("new src");
    run_git(root, ["mv", "rename-old.txt", "rename-new.txt"]);

    let Json(root_entries) = list_files(Query(ListFilesQuery {
        directory: Some(root.to_string_lossy().to_string()),
        path: None,
    }))
    .await;
    let status_by_name = root_entries
        .iter()
        .map(|entry| (entry.name.as_str(), entry.git_status.as_deref()))
        .collect::<HashMap<_, _>>();

    assert_eq!(
        status_by_name.get("tracked.txt").copied(),
        Some(Some("modified"))
    );
    assert_eq!(
        status_by_name.get("untracked.txt").copied(),
        Some(Some("untracked"))
    );
    assert_eq!(
        status_by_name.get("ignored.log").copied(),
        Some(Some("ignored"))
    );
    assert_eq!(
        status_by_name.get("rename-new.txt").copied(),
        Some(Some("renamed"))
    );
    assert!(
        !status_by_name.contains_key(".git"),
        "internal git metadata must stay hidden from the file tree"
    );

    let Json(src_entries) = list_files(Query(ListFilesQuery {
        directory: Some(root.to_string_lossy().to_string()),
        path: Some("src".to_string()),
    }))
    .await;
    let status_by_path = src_entries
        .iter()
        .map(|entry| (entry.path.as_str(), entry.git_status.as_deref()))
        .collect::<HashMap<_, _>>();
    assert_eq!(
        status_by_path.get("src/clean.rs").copied(),
        Some(Some("clean"))
    );
    assert_eq!(
        status_by_path.get("src/new.rs").copied(),
        Some(Some("untracked"))
    );
}

#[tokio::test]
async fn gateway_file_api_business_flow_open_actions_launch_configured_command_and_report_failures()
{
    let workspace = tempfile::tempdir().expect("workspace");
    let root = workspace.path();
    let file = root.join("docs").join("README.md");
    fs::create_dir_all(file.parent().expect("file parent")).expect("docs dir");
    fs::write(&file, "hello from gateway open action\n").expect("readme");

    let launcher = write_open_launcher(root);
    let default_log = root.join("default-open.log");
    let location_log = root.join("location-open.log");
    let _default_command = EnvGuard::set("TURA_FILE_OPEN_COMMAND", launcher_command(&launcher));
    let _default_log = EnvGuard::set("TURA_FILE_OPEN_LOG", default_log.as_os_str());
    let _location_command = EnvGuard::set(
        "TURA_FILE_OPEN_LOCATION_COMMAND",
        launcher_command(&launcher),
    );
    let _location_log = EnvGuard::set("TURA_FILE_OPEN_LOCATION_LOG", location_log.as_os_str());

    let Json(opened) = open_file(Query(FileContentQuery {
        directory: Some(root.to_string_lossy().to_string()),
        path: "docs/README.md".to_string(),
    }))
    .await
    .expect("configured open command should succeed");
    assert!(opened.opened);
    assert_eq!(opened.path, "docs/README.md");

    let Json(location_opened) = open_file_location(Query(FileContentQuery {
        directory: Some(root.to_string_lossy().to_string()),
        path: "docs/README.md".to_string(),
    }))
    .await
    .expect("configured file-manager command should succeed");
    assert!(location_opened.opened);
    assert_eq!(location_opened.path, "docs/README.md");

    assert_eq!(
        normalize_logged_path(&wait_for_logged_path(&default_log)),
        normalize_logged_path(&file.to_string_lossy())
    );
    assert_eq!(
        normalize_logged_path(&wait_for_logged_path(&location_log)),
        normalize_logged_path(&file.to_string_lossy())
    );

    let _failing_command = EnvGuard::set(
        "TURA_FILE_OPEN_COMMAND",
        root.join("missing-launcher").as_os_str(),
    );
    let failure = open_file(Query(FileContentQuery {
        directory: Some(root.to_string_lossy().to_string()),
        path: "docs/README.md".to_string(),
    }))
    .await
    .expect_err("missing configured launcher should be surfaced");
    assert_eq!(failure.0, StatusCode::INTERNAL_SERVER_ERROR);
    assert!(failure.1.contains("Failed to open file"));
}

struct EnvGuard {
    key: &'static str,
    previous: Option<std::ffi::OsString>,
}

impl EnvGuard {
    fn set(key: &'static str, value: impl AsRef<std::ffi::OsStr>) -> Self {
        let previous = std::env::var_os(key);
        std::env::set_var(key, value);
        Self { key, previous }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        match self.previous.take() {
            Some(value) => std::env::set_var(self.key, value),
            None => std::env::remove_var(self.key),
        }
    }
}

fn write_open_launcher(root: &Path) -> PathBuf {
    #[cfg(windows)]
    {
        let script = root.join("record-open.cmd");
        fs::write(
            &script,
            "@echo off\r\nif not \"%TURA_FILE_OPEN_LOG%\"==\"\" > \"%TURA_FILE_OPEN_LOG%\" echo %~1\r\nif not \"%TURA_FILE_OPEN_LOCATION_LOG%\"==\"\" > \"%TURA_FILE_OPEN_LOCATION_LOG%\" echo %~1\r\nexit /b 0\r\n",
        )
        .expect("write windows launcher");
        script
    }
    #[cfg(not(windows))]
    {
        let script = root.join("record-open.sh");
        fs::write(
            &script,
            "#!/usr/bin/env sh\nif [ -n \"$TURA_FILE_OPEN_LOG\" ]; then printf '%s' \"$1\" > \"$TURA_FILE_OPEN_LOG\"; fi\nif [ -n \"$TURA_FILE_OPEN_LOCATION_LOG\" ]; then printf '%s' \"$1\" > \"$TURA_FILE_OPEN_LOCATION_LOG\"; fi\n",
        )
        .expect("write unix launcher");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut permissions = fs::metadata(&script)
                .expect("launcher metadata")
                .permissions();
            permissions.set_mode(0o755);
            fs::set_permissions(&script, permissions).expect("launcher executable");
        }
        script
    }
}

fn launcher_command(path: &Path) -> String {
    path.to_string_lossy().to_string()
}

fn wait_for_logged_path(path: &Path) -> String {
    let start = std::time::Instant::now();
    while start.elapsed() < std::time::Duration::from_secs(3) {
        if let Ok(value) = fs::read_to_string(path) {
            if !value.trim().is_empty() {
                return value;
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
    panic!("open launcher did not write {}", path.display());
}

fn normalize_logged_path(value: &str) -> String {
    value.trim().replace('\\', "/")
}

fn run_git<const N: usize>(root: &Path, args: [&str; N]) {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .expect("git must be available for file API git-status business flow");
    assert!(
        output.status.success(),
        "git command failed: git -C {} {}\nstdout:\n{}\nstderr:\n{}",
        root.display(),
        args.join(" "),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
