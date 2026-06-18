use super::handler_parse::{
    command_values, parse_arguments_value, parse_command_item, string_field, u64_field,
};
use super::{
    normalize_command_steps, normalize_json_or_cli_command_arguments,
    normalize_shell_command_arguments, parse_args,
};
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
fn normalize_preserves_duplicate_dependency_groups_and_extends_backwards_steps() {
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
    assert_eq!(steps, vec![1, 2, 2, 3]);
}

#[test]
fn normalize_scrambled_steps_never_move_backwards_or_merge_repaired_groups() {
    let mut args = parse_args(&json!({
        "commands": [
            { "command": "shell_command", "command_line": "echo three", "step": 3 },
            { "command": "shell_command", "command_line": "echo two", "step": 2 },
            { "command": "shell_command", "command_line": "echo four", "step": 4 },
            { "command": "shell_command", "command_line": "echo one", "step": 1 }
        ]
    }))
    .expect("parse args");

    normalize_command_steps(&mut args.commands);

    let steps = args
        .commands
        .iter()
        .map(|command| command.effective_step())
        .collect::<Vec<_>>();
    assert_eq!(steps, vec![3, 4, 5, 6]);
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
fn normalize_external_commands_default_to_15_second_timeout() {
    let args = parse_args(&json!({
        "commands": [
            {
                "command_type": "read_media",
                "path": "note.txt",
                "step": 1
            }
        ]
    }))
    .expect("parse command_run args");

    let arguments = normalize_json_or_cli_command_arguments(&args.commands[0], "read_media")
        .expect("normalize read_media arguments");

    assert_eq!(arguments["path"], json!("note.txt"));
    assert_eq!(arguments["timeout_ms"], json!(15_000));
}

#[test]
fn normalize_image_generate_defaults_to_100_second_timeout() {
    let args = parse_args(&json!({
        "commands": [
            {
                "command_type": "image_generate",
                "command_line": "--prompt logo",
                "step": 1
            }
        ]
    }))
    .expect("parse command_run args");

    let arguments = normalize_json_or_cli_command_arguments(&args.commands[0], "image_generate")
        .expect("normalize image_generate arguments");

    assert_eq!(arguments["cli"], json!("--prompt logo"));
    assert_eq!(arguments["timeout_ms"], json!(100_000));
}

#[test]
fn normalize_external_commands_keep_explicit_timeout_fields() {
    let args = parse_args(&json!({
        "commands": [
            {
                "command_type": "web_discover",
                "command_line": "{\"query\":\"docs\",\"timeout_secs\":2}",
                "timeout_ms": 5000,
                "step": 1
            }
        ]
    }))
    .expect("parse command_run args");

    let arguments = normalize_json_or_cli_command_arguments(&args.commands[0], "web_discover")
        .expect("normalize web_discover arguments");

    assert_eq!(arguments["query"], json!("docs"));
    assert_eq!(arguments["timeout_secs"], json!(2));
    assert!(arguments.get("timeout_ms").is_none());
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
fn normalize_command_value_for_execution_adds_actual_shell_command_type() {
    let normalized = super::normalize_command_value_for_execution(
        json!({
            "command_line": "Write-Output normalized-ok",
            "step": 3,
            "timeout_ms": 5000
        }),
        0,
    )
    .expect("normalize command value");

    assert_eq!(
        normalized["command_type"],
        crate::commands::active_shell_command_name()
    );
    assert_eq!(normalized["command_line"], "Write-Output normalized-ok");
    assert_eq!(normalized["step"], 3);
    assert_eq!(normalized["timeout_ms"], 5000);
}

#[test]
fn normalize_command_value_for_execution_does_not_type_plain_summary_text() {
    let normalized = super::normalize_command_value_for_execution(
        json!({
            "command": "large file scan",
            "step": 1
        }),
        0,
    )
    .expect("plain summary should still parse as a non-executable command record");

    assert!(normalized.get("command_type").is_none());
    assert_eq!(normalized["command"], "large file scan");
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
fn parse_single_stringified_shell_object_without_commands_is_wrapped() {
    let args = parse_args(&json!({
        "command": json!({ "command": "echo ok", "timeout_ms": 5000 }).to_string(),
        "timeoutMs": 120000
    }))
    .expect("parse args");

    assert_eq!(args.commands.len(), 1);
    assert_eq!(
        args.commands[0].command,
        crate::commands::active_shell_command_name()
    );
    assert_eq!(
        args.commands[0].command_line,
        json!({ "command": "echo ok", "timeout_ms": 5000 }).to_string()
    );
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

#[test]
fn parse_arguments_value_accepts_requests_wrapper_and_plain_values() {
    let wrapped = parse_arguments_value(&json!({
        "requests": {
            "commands": [
                {"command_type": "shell_command", "command_line": "echo wrapped"}
            ]
        }
    }))
    .expect("requests wrapper");
    let fenced = parse_arguments_value(&Value::String(
        "```json\n{\"requests\":{\"commands\":[{\"command\":\"shell_command\"}]}}\n```".to_string(),
    ))
    .expect("fenced requests wrapper");
    let plain = parse_arguments_value(&json!({"commands": [{"command": "echo plain"}]}))
        .expect("plain arguments");

    assert_eq!(wrapped["commands"][0]["command_line"], "echo wrapped");
    assert_eq!(fenced["commands"][0]["command"], "shell_command");
    assert_eq!(plain["commands"][0]["command"], "echo plain");
}

#[test]
fn parse_arguments_value_reports_jsonish_errors_with_context() {
    let error = parse_arguments_value(&Value::String("```json\n{\"commands\":[}\n```".to_string()))
        .expect_err("invalid fenced json should fail");

    assert!(error.contains("failed to parse command_run arguments"));
}

#[test]
fn command_values_wraps_single_objects_and_strings_but_drops_scalars() {
    assert_eq!(command_values(&json!([{"command": "one"}])).len(), 1);
    assert_eq!(command_values(&json!({"command": "one"})).len(), 1);
    assert_eq!(command_values(&json!("echo one")), vec![json!("echo one")]);
    assert!(command_values(&json!(false)).is_empty());
    assert!(command_values(&json!(42)).is_empty());
}

#[test]
fn parse_command_item_recovers_inline_arguments_and_residual_fields() {
    let item = parse_command_item(&json!({
        "command_type": "task_status",
        "command": "{\"status\":\"done\"}",
        "parameters": {"status": "done"},
        "workdir": "workspace",
        "step": "3",
        "timeoutMs": "4000"
    }))
    .expect("parse command item");

    assert_eq!(item.command, "task_status");
    assert_eq!(item.command_line, "{\"status\":\"done\"}");
    assert_eq!(item.inline_arguments, Some(json!({"status": "done"})));
    assert_eq!(item.workdir.as_deref(), Some("workspace"));
    assert_eq!(item.step, Some(3));
    assert_eq!(item.timeout_ms, Some(4000));
}

#[test]
fn parse_command_item_uses_shell_when_only_payload_field_is_present() {
    let item = parse_command_item(&json!({
        "payload": "echo payload-only",
        "extra": "kept"
    }))
    .expect("parse payload-only item");

    assert_eq!(item.command, crate::commands::active_shell_command_name());
    assert_eq!(item.command_line, "echo payload-only");
    assert_eq!(item.inline_arguments, Some(json!({"extra": "kept"})));
}

#[test]
fn parse_command_item_rejects_non_object_non_string_and_missing_command() {
    assert!(parse_command_item(&json!(null))
        .expect_err("null command item")
        .contains("expected object"));
    assert!(parse_command_item(&json!({"step": 1}))
        .expect_err("missing command")
        .contains("missing field `command_type`"));
}

#[test]
fn field_helpers_trim_only_string_presence_and_parse_unsigned_numbers() {
    let object = json!({
        "blank": " ",
        "array": ["a", "b"],
        "object": {"k": "v"},
        "number": 42,
        "numberString": "43",
        "badNumber": "-1"
    });
    let object = object.as_object().expect("object");

    assert_eq!(string_field(object, &["blank"]), None);
    assert_eq!(
        string_field(object, &["array"]),
        Some("[\"a\",\"b\"]".to_string())
    );
    assert_eq!(
        string_field(object, &["object"]),
        Some("{\"k\":\"v\"}".to_string())
    );
    assert_eq!(u64_field(object, &["number"]), Some(42));
    assert_eq!(u64_field(object, &["numberString"]), Some(43));
    assert_eq!(u64_field(object, &["badNumber"]), None);
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
