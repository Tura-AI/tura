use axum::extract::{Json, Path, Query};
use axum::http::HeaderMap;
use gateway::api::session::{session_shell, ShellRequest};
use gateway::api::{path::get_paths, types::PathParams};
use gateway::session_store;
use serde_json::json;

#[tokio::test]
async fn gateway_path_business_flow_prefers_query_directory_then_decoded_header() {
    let mut headers = HeaderMap::new();
    headers.insert(
        "x-opencode-directory",
        "C%3A%5CUsers%5Cliuliu%5Cencoded%20workspace"
            .parse()
            .expect("header value"),
    );

    let Json(from_header) = get_paths(headers.clone(), Query(PathParams { directory: None })).await;
    assert_eq!(
        from_header.directory,
        "C:\\Users\\liuliu\\encoded workspace"
    );
    assert_eq!(from_header.worktree, from_header.directory);

    let Json(from_query) = get_paths(
        headers,
        Query(PathParams {
            directory: Some("/tmp/query workspace".to_string()),
        }),
    )
    .await;
    assert_eq!(from_query.directory, "/tmp/query workspace");
    assert_eq!(from_query.worktree, "/tmp/query workspace");
}

#[tokio::test]
async fn gateway_session_shell_business_flow_runs_in_session_directory_and_reports_failures() {
    let workspace = tempfile::tempdir().expect("workspace");
    let session = session_store().create_session(
        Some(workspace.path().to_string_lossy().to_string()),
        Some("Shell session".to_string()),
        None,
        Some("coding".to_string()),
        false,
        false,
        false,
        None,
        false,
        false,
    );

    let Json(success) = session_shell(
        Path(session.id.clone()),
        Json(ShellRequest {
            input: success_command(),
        }),
    )
    .await;
    assert!(
        success.output.contains("gateway-shell-ok"),
        "shell success output should include command stdout: {}",
        success.output
    );
    assert!(
        workspace.path().join("gateway-shell-output.txt").exists(),
        "shell command should run inside the session workspace"
    );

    let Json(failure) = session_shell(
        Path(session.id.clone()),
        Json(ShellRequest {
            input: failure_command(),
        }),
    )
    .await;
    assert!(
        failure.output.contains("gateway-shell-bad"),
        "shell failure should include stderr/stdout context: {}",
        failure.output
    );
    assert!(
        failure.output.contains("exit status"),
        "shell failure should include exit status: {}",
        failure.output
    );

    let Json(empty) = session_shell(
        Path(session.id),
        Json(ShellRequest {
            input: "   ".to_string(),
        }),
    )
    .await;
    assert_eq!(empty.output, "");

    assert_eq!(json!(true), json!(workspace.path().exists()));
}

#[tokio::test]
async fn gateway_session_shell_business_flow_combines_stdout_stderr_status_and_spawn_errors() {
    let workspace = tempfile::tempdir().expect("workspace");
    let session = session_store().create_session(
        Some(workspace.path().to_string_lossy().to_string()),
        Some("Shell stderr session".to_string()),
        None,
        Some("coding".to_string()),
        false,
        false,
        false,
        None,
        false,
        false,
    );

    let Json(mixed) = session_shell(
        Path(session.id),
        Json(ShellRequest {
            input: mixed_output_failure_command(),
        }),
    )
    .await;
    assert!(
        mixed.output.contains("gateway-shell-stdout"),
        "combined shell output should preserve stdout: {}",
        mixed.output
    );
    assert!(
        mixed.output.contains("gateway-shell-stderr"),
        "combined shell output should preserve stderr: {}",
        mixed.output
    );
    assert!(
        mixed.output.contains("exit status"),
        "combined shell output should include the failed status: {}",
        mixed.output
    );

    let missing_dir = workspace.path().join("missing-session-workspace");
    let missing_session = session_store().create_session(
        Some(missing_dir.to_string_lossy().to_string()),
        Some("Missing shell directory".to_string()),
        None,
        Some("coding".to_string()),
        false,
        false,
        false,
        None,
        false,
        false,
    );
    if missing_dir.exists() {
        std::fs::remove_dir_all(&missing_dir).expect("remove test session directory");
    }
    let Json(spawn_error) = session_shell(
        Path(missing_session.id),
        Json(ShellRequest {
            input: success_command(),
        }),
    )
    .await;
    assert!(
        spawn_error
            .output
            .contains("failed to run shell command: failed to spawn session shell command"),
        "missing workspace should surface a bounded spawn error: {}",
        spawn_error.output
    );
    assert!(
        spawn_error
            .output
            .contains(&missing_dir.to_string_lossy().to_string()),
        "spawn error should name the missing session directory: {}",
        spawn_error.output
    );
}

fn success_command() -> String {
    if cfg!(windows) {
        "Set-Content -Path gateway-shell-output.txt -Value gateway-shell-ok; Get-Content gateway-shell-output.txt".to_string()
    } else {
        "printf gateway-shell-ok > gateway-shell-output.txt; cat gateway-shell-output.txt"
            .to_string()
    }
}

fn failure_command() -> String {
    if cfg!(windows) {
        "Write-Error gateway-shell-bad; exit 5".to_string()
    } else {
        "echo gateway-shell-bad >&2; exit 5".to_string()
    }
}

fn mixed_output_failure_command() -> String {
    if cfg!(windows) {
        "Write-Output gateway-shell-stdout; Write-Error gateway-shell-stderr; exit 7".to_string()
    } else {
        "echo gateway-shell-stdout; echo gateway-shell-stderr >&2; exit 7".to_string()
    }
}
