use super::{normalize_command_steps, normalize_shell_command_arguments, parse_args};
use serde_json::json;
use serde_json::Value;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn parse_missing_steps_default_to_original_order_steps() {
    let args = parse_args(&json!({
        "commands": [
            { "command": "shell_command", "command_line": "pwd" },
            { "command": "shell_command", "command_line": "pwd" }
        ]
    }))
    .expect("parse args");

    assert_eq!(args.commands[0].effective_step(), 1);
    assert_eq!(args.commands[1].effective_step(), 2);
}

#[test]
fn normalize_duplicate_steps_extends_in_input_order() {
    let mut args = parse_args(&json!({
        "commands": [
            { "command": "shell_command", "command_line": "echo a", "step": 1 },
            { "command": "shell_command", "command_line": "echo b", "step": 2 },
            { "command": "shell_command", "command_line": "echo c", "step": 2 },
            { "command": "shell_command", "command_line": "echo d", "step": 3 }
        ]
    }))
    .expect("parse args");

    normalize_command_steps(&mut args.commands);

    let steps = args
        .commands
        .iter()
        .map(|command| command.effective_step())
        .collect::<Vec<_>>();
    assert_eq!(steps, vec![1, 2, 3, 4]);
}

#[test]
fn parse_empty_command_run_is_error() {
    let error = parse_args(&json!({ "commands": [] })).expect_err("empty command run");

    assert_eq!(error, "command_run commands must not be empty");
}

#[test]
fn parse_compact_context_must_be_final_highest_step() {
    let error = parse_args(&json!({
        "commands": [
            {
                "step": 2,
                "command_type": "compact_context",
                "command_line": "summary"
            },
            {
                "step": 3,
                "command_type": "shell_command",
                "command_line": "echo after"
            }
        ]
    }))
    .expect_err("compact_context position");

    assert_eq!(
        error,
        "compact_context must be the final command in the highest step of command_run"
    );
}

#[test]
fn parse_command_only_shell_text_is_mapped_to_active_shell_command() {
    let args = parse_args(&json!({
        "commands": [
            { "command": "echo ok", "step": 1 }
        ]
    }))
    .expect("parse args");

    assert_eq!(
        args.commands[0].command,
        crate::commands::active_shell_command_name()
    );
    assert_eq!(args.commands[0].command_line, "echo ok");
}

#[test]
fn normalize_shell_commands_default_to_15_second_timeout() {
    let args = parse_args(&json!({
        "commands": [
            {
                "command": "shell_command",
                "command_line": "echo timeout-default-ok",
                "step": 1
            }
        ]
    }))
    .expect("parse command_run args");

    let arguments =
        normalize_shell_command_arguments(&args.commands[0]).expect("normalize shell arguments");

    assert_eq!(arguments["timeout_ms"], json!(15_000));
}

#[test]
fn parse_command_line_without_command_type_accepts_workdir_and_timeout() {
    let args = parse_args(&json!({
        "commands": [
            {
                "command_line": "pwd",
                "workdir": "subdir",
                "timeout_ms": 5000,
                "step": 1
            }
        ]
    }))
    .expect("parse args");

    assert_eq!(
        args.commands[0].command,
        crate::commands::active_shell_command_name()
    );
    assert_eq!(args.commands[0].command_line, "pwd");
    assert_eq!(args.commands[0].workdir.as_deref(), Some("subdir"));
    assert_eq!(args.commands[0].timeout_ms, Some(5000));
}

#[test]
fn parse_legacy_steps_shape_is_accepted() {
    let args = parse_args(&json!({
        "steps": [
            {
                "tool_name": "shell_command",
                "command_code": "echo legacy-steps-ok",
                "step": 1
            }
        ]
    }))
    .expect("parse args");

    assert_eq!(args.commands[0].command, "shell_command");
    assert_eq!(args.commands[0].command_line, "echo legacy-steps-ok");
}

#[test]
fn parse_command_run_arguments_accept_requests_wrapper_and_json_fence() {
    let args = parse_args(&Value::String(
            "```json\n{\"requests\":{\"commands\":[{\"command\":\"shell_command\",\"command_line\":\"echo fenced-ok\",\"step\":1}]}}\n```"
                .to_string(),
        ))
        .expect("parse args");

    assert_eq!(args.commands[0].command, "shell_command");
    assert_eq!(args.commands[0].command_line, "echo fenced-ok");
}

#[test]
fn parse_command_line_wrapped_apply_patch_routes_to_apply_patch() {
    let args = parse_args(&json!({
            "commands": [
                {
                    "command": "shell_command",
                    "command_line": "apply_patch <<'PATCH'\n*** Begin Patch\n*** Update File: app.txt\n@@\n-old\n+new\n*** End Patch\nPATCH",
                    "step": 1
                }
            ]
        }))
        .expect("parse args");

    assert_eq!(args.commands[0].command, "apply_patch");
    assert!(args.commands[0].command_line.starts_with("*** Begin Patch"));
}

#[test]
fn parse_apply_patch_missing_begin_marker_is_repaired() {
    let args = parse_args(&json!({
            "commands": [
                {
                    "command_type": "apply_patch",
                    "command_line": "apply_patch\n*** Update File: app.txt\n@@\n-old\n+new\n*** End Patch",
                    "step": 1
                }
            ]
        }))
        .expect("parse args");

    assert_eq!(args.commands[0].command, "apply_patch");
    assert_eq!(
        args.commands[0].command_line,
        "*** Begin Patch\n*** Update File: app.txt\n@@\n-old\n+new\n*** End Patch"
    );
}

#[test]
fn parse_aliases_cmd_and_command_line_are_accepted() {
    let args = parse_args(&json!({
        "commands": [
            { "cmd": "shell_command", "commandLine": "echo ok", "step": 1 }
        ]
    }))
    .expect("parse args");

    assert_eq!(args.commands[0].command, "shell_command");
    assert_eq!(args.commands[0].command_line, "echo ok");
}

#[test]
fn parse_single_shell_object_without_commands_is_wrapped() {
    let args = parse_args(&json!({
        "command": "echo ok",
        "timeoutMs": 120000
    }))
    .expect("parse args");

    assert_eq!(args.commands.len(), 1);
    assert_eq!(args.commands[0].command_line, "echo ok");
    assert_eq!(args.commands[0].timeout_ms, Some(120000));
}

#[test]
fn parse_command_only_here_string_patch_is_routed_to_apply_patch() {
    let args = parse_args(&json!({
            "commands": [
                {
                    "command": "@'\n*** Begin Patch\n*** Update File: app.txt\n@@\n-old\n+new\n*** End Patch\n'@",
                    "step": 1
                }
            ]
        }))
        .expect("parse args");

    assert_eq!(args.commands[0].command, "apply_patch");
    assert!(args.commands[0].command_line.starts_with("*** Begin Patch"));
}

#[tokio::test]
async fn streaming_executor_returns_safe_shell_result_before_finish() {
    let workspace = temporary_workspace("streaming-safe-shell-before-finish");
    let mut executor = super::StreamingCommandRunExecutor::new(workspace.clone());

    let result = executor
        .push_command_value(json!({
            "command": "shell_command",
            "command_line": "echo streamed-safe-shell",
            "timeout_ms": 3000,
            "step": 1
        }))
        .await;

    assert!(
        !result.is_empty(),
        "streaming shell result should be available before finish()"
    );
    assert_eq!(
        result[0].get("success").and_then(Value::as_bool),
        Some(true)
    );

    let _ = std::fs::remove_dir_all(workspace);
}

fn temporary_workspace(prefix: &str) -> std::path::PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after UNIX_EPOCH")
        .as_nanos();
    let path = std::env::temp_dir().join(format!("{prefix}-{}-{nanos}", std::process::id()));
    std::fs::create_dir_all(&path)
        .unwrap_or_else(|error| panic!("failed to create {}: {error}", path.display()));
    path
}
