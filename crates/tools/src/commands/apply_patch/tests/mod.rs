use super::{
    access, comparable_path_string, execute, normalize_apply_patch_text, patch_text_from_payload,
};
use crate::runtime::tool::ToolPayload;
use serde_json::json;
use std::fs;

fn temp_workspace(name: &str) -> std::path::PathBuf {
    let path = std::env::temp_dir().join(format!("tura-apply-patch-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).expect("create temp workspace");
    path
}

fn assert_no_outer_message_or_guidance(output: &serde_json::Value) {
    assert!(output.get("message").is_none(), "{output}");
    assert!(output.get("guidance").is_none(), "{output}");
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
fn failed_middle_file_reports_successes_and_failures_separately() {
    let root = temp_workspace("partial");
    fs::write(root.join("first.txt"), "old\n").expect("first");
    fs::write(root.join("second.txt"), "actual\n").expect("second");
    fs::write(root.join("third.txt"), "old\n").expect("third");

    let result = execute(
            "*** Begin Patch\n*** Update File: first.txt\n@@\n-old\n+new\n*** Update File: second.txt\n@@\n-missing\n+value\n*** Update File: third.txt\n@@\n-old\n+new\n*** End Patch\n",
            &root,
        );

    assert!(!result.success);
    assert_eq!(result.output["error_type"], json!("ContextMismatch"));
    assert_no_outer_message_or_guidance(&result.output);
    assert!(result.output.get("failed_change").is_none());
    assert!(result.output.get("partial_changes").is_none());
    assert_eq!(result.changes[0]["path"], json!("first.txt"));
    assert_eq!(result.changes[1]["path"], json!("third.txt"));
    assert_eq!(
        result.output["failed_changes"][0]["failed_change"]["path"],
        json!("second.txt")
    );
    assert!(result.output["failed_changes"][0]["message"]
        .as_str()
        .is_some_and(|text| text.contains("patch context not found")));
    assert!(result.output["failed_changes"][0]["guidance"]
        .as_str()
        .is_some_and(|text| text.contains("after earlier changes were applied")));
    assert_eq!(
        fs::read_to_string(root.join("first.txt")).expect("first"),
        "new\n"
    );
    assert_eq!(
        fs::read_to_string(root.join("second.txt")).expect("second"),
        "actual\n"
    );
    assert_eq!(
        fs::read_to_string(root.join("third.txt")).expect("third"),
        "new\n"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn add_file_accepts_absolute_path_outside_session_dir() {
    let root = temp_workspace("outside-absolute");
    let outside = root
        .parent()
        .expect("temp workspace should have a parent")
        .join("outside-apply-patch-test.txt");
    let _ = fs::remove_file(&outside);

    let result = execute(
        &format!(
            "*** Begin Patch\n*** Add File: {}\n+ok\n*** End Patch\n",
            outside.display()
        ),
        &root,
    );

    assert!(result.success, "{}", result.stderr);
    assert_eq!(fs::read_to_string(&outside).expect("outside file"), "ok\n");
    assert_eq!(
        result.changes[0]["path"],
        json!(outside.display().to_string())
    );
    let _ = fs::remove_file(outside);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn access_locks_absolute_path_outside_session_dir() {
    let root = temp_workspace("outside-access");
    let outside = root
        .parent()
        .expect("temp workspace should have a parent")
        .join("outside-apply-patch-access.txt");
    let patch = format!(
        "*** Begin Patch\n*** Add File: {}\n+ok\n*** End Patch\n",
        outside.display()
    );

    let access = access(&patch, &root);

    assert_eq!(
        access.write_paths,
        vec![format!("absolute:{}", comparable_path_string(&outside))]
    );
    assert!(!access.workspace_write);
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
fn normalizes_fenced_or_wrapped_patch_text() {
    let patch = "*** Begin Patch\n*** Add File: app.txt\n+ok\n*** End Patch";
    assert_eq!(
        normalize_apply_patch_text(&format!("```patch\n{patch}\n```")),
        patch
    );
    assert_eq!(
        normalize_apply_patch_text(&format!("I will apply this:\n{patch}\nDone.")),
        patch
    );
    assert_eq!(
        normalize_apply_patch_text(&json!({ "patch": patch }).to_string()),
        patch
    );
    assert_eq!(
        normalize_apply_patch_text(
            &json!({ "command_line": format!("prefix\n{patch}\nsuffix") }).to_string()
        ),
        patch
    );
}

#[test]
fn normalizes_patch_body_missing_begin_marker() {
    let patch = "*** Begin Patch\n*** Add File: app.txt\n+ok\n*** End Patch";

    assert_eq!(
        normalize_apply_patch_text("apply_patch\n*** Add File: app.txt\n+ok\n*** End Patch"),
        patch
    );
    assert_eq!(
        normalize_apply_patch_text("*** Add File: app.txt\n+ok"),
        patch
    );
}

#[test]
fn function_payload_recursively_extracts_patch_body() {
    let patch = "*** Begin Patch\n*** Add File: app.txt\n+ok\n*** End Patch";
    let payload = ToolPayload::Function {
        arguments: json!({
            "request": {
                "body": format!("Here is the patch:\n{patch}\n")
            }
        }),
    };

    assert_eq!(patch_text_from_payload(&payload), patch);
}

#[test]
fn update_file_without_hunks_is_noop_success() {
    let root = temp_workspace("empty-update");
    fs::write(root.join("app.txt"), "unchanged\n").expect("fixture");

    let result = execute(
        "*** Begin Patch\n*** Update File: app.txt\n*** End Patch\n",
        &root,
    );

    assert!(result.success, "{}", result.stderr);
    assert_eq!(
        fs::read_to_string(root.join("app.txt")).expect("read fixture"),
        "unchanged\n"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn update_file_accepts_end_of_file_marker_between_changes() {
    let root = temp_workspace("eof-marker");
    fs::write(root.join("app.txt"), "old\n").expect("fixture");

    let result = execute(
        "*** Begin Patch\n*** Update File: app.txt\n@@\n-old\n+new\n*** End of File\n*** Add File: other.txt\n+ok\n*** End Patch\n",
        &root,
    );

    assert!(result.success, "{}", result.stderr);
    assert_eq!(
        fs::read_to_string(root.join("app.txt")).expect("read fixture"),
        "new\n"
    );
    assert_eq!(
        fs::read_to_string(root.join("other.txt")).expect("read fixture"),
        "ok\n"
    );
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
    assert_no_outer_message_or_guidance(&result.output);
    assert!(result.output.get("failed_change").is_none());
    assert_eq!(
        result.output["failed_changes"][0]["failed_change"]["path"],
        json!("missing.txt")
    );
    assert_eq!(
        result.output["failed_changes"][0]["message"],
        json!("UpdateFileNotFound: missing.txt")
    );
    assert_eq!(
        result.output["failed_changes"][0]["guidance"],
        json!("apply_patch failed; inspect error_type and message before retrying.")
    );
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
    assert_no_outer_message_or_guidance(&result.output);
    assert!(result.output.get("failed_change").is_none());
    assert_eq!(
        result.output["failed_changes"][0]["failed_change"]["path"],
        json!("missing.txt")
    );
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
