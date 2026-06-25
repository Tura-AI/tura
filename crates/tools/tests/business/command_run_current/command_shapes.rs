use super::helpers::*;

#[test]
fn pass_shell_command_output_matches_current_code_mode_string() {
    let _guard = env_lock_blocking();
    std::env::set_var("TURA_COMMAND_RUN_SHELL", "shell_command");
    let root = temp_workspace("shell-output");
    let output = command_run::execute(
        &json!({
            "commands": [
                { "command": "shell_command", "command_line": json!({ "command": "Write-Output current-backfill-ok" }).to_string() }
            ]
        }),
        &root,
    );

    let text = output["results"][0]["output"]
        .as_str()
        .expect("shell command_run output should be current-style text");
    assert!(text.starts_with("Exit code: 0\nWall time: "));
    assert!(text.contains("\nOutput:\n"));
    assert!(text.contains("current-backfill-ok"));
    assert!(!text.contains("\"metadata\""));
    assert!(!text.contains("\"stdout\""));
    assert!(!text.contains("\"stderr\""));
}

#[test]
fn pass_model_backfill_matches_current_shape_except_command_type_key() {
    let _guard = env_lock_blocking();
    std::env::set_var("TURA_COMMAND_RUN_SHELL", "shell_command");
    let root = temp_workspace("model-backfill");
    let output = command_run::execute(
        &json!({
            "commands": [
                { "command": "shell_command", "command_line": json!({ "command": "Write-Output command-type-diff-only" }).to_string() }
            ]
        }),
        &root,
    );
    let result = output["results"][0].as_object().expect("result object");
    let mut keys = result.keys().cloned().collect::<Vec<_>>();
    keys.sort();
    assert_eq!(keys, vec!["command_type", "output", "step", "success"]);

    let mut current_equivalent = output.clone();
    let result = current_equivalent["results"][0]
        .as_object_mut()
        .expect("result object");
    let command_type = result.remove("command_type").expect("command_type");
    result.insert("command".to_string(), command_type);

    let expected = json!({
        "results": [
            {
                "step": 1,
                "command": commands::active_shell_command_name(),
                "success": true,
                "output": current_equivalent["results"][0]["output"].clone()
            }
        ]
    });
    assert_eq!(current_equivalent, expected);
}

#[test]
fn pass_command_only_shell_text_is_mapped_to_active_shell_command() {
    let _guard = env_lock_blocking();
    std::env::set_var("TURA_COMMAND_RUN_SHELL", "shell_command");
    let root = temp_workspace("command-only-shell");
    let output = command_run::execute(
        &json!({
            "commands": [
                { "command": "Write-Output ok", "step": 1 }
            ]
        }),
        &root,
    );

    assert_eq!(output["results"][0]["success"], true);
    assert_eq!(
        output["results"][0]["command_type"],
        commands::active_shell_command_name()
    );
}

#[test]
fn pass_top_level_workdir_is_accepted_for_current_style_shell_items() {
    let _guard = env_lock_blocking();
    std::env::set_var("TURA_COMMAND_RUN_SHELL", "shell_command");
    let root = temp_workspace("top-level-workdir");
    let output = command_run::execute(
        &json!({
            "workdir": ".",
            "commands": [
                { "command": "Write-Output ok", "step": 1 }
            ]
        }),
        &root,
    );

    assert_eq!(output["results"][0]["success"], true);
}

#[test]
fn fail_cli_sandbox_rejects_shell_workdir_outside_workspace() {
    let _guard = env_lock_blocking();
    let previous_sandbox = std::env::var_os("TURA_COMMAND_RUN_SANDBOX");
    std::env::set_var("TURA_COMMAND_RUN_SHELL", "shell_command");
    std::env::set_var("TURA_COMMAND_RUN_SANDBOX", "1");
    let root = temp_workspace("sandbox-shell-workdir");
    let outside = root
        .parent()
        .expect("temp workspace should have a parent")
        .join("sandbox-shell-outside");
    fs::create_dir_all(&outside).expect("outside workdir");

    let output = command_run::execute(
        &json!({
            "commands": [
                {
                    "command": "shell_command",
                    "command_line": json!({ "command": "Get-Location", "workdir": outside, "timeout_ms": 5000 }).to_string()
                }
            ]
        }),
        &root,
    );

    assert_eq!(output["results"][0]["success"], false, "{output}");
    assert!(output["results"][0]["output"]["stderr"]
        .as_str()
        .unwrap_or_default()
        .contains("outside workspace"));
    restore_env_var("TURA_COMMAND_RUN_SANDBOX", previous_sandbox);
}

#[test]
fn pass_unknown_command_with_shell_payload_is_mapped_to_active_shell_command() {
    let _guard = env_lock_blocking();
    std::env::set_var("TURA_COMMAND_RUN_SHELL", "shell_command");
    let root = temp_workspace("unknown-command-payload");
    let output = command_run::execute(
        &json!({
            "commands": [
                {
                    "command": "Get-Content src/app.py",
                    "command_line": json!({ "command": "Write-Output mapped-ok" }).to_string(),
                    "step": 1
                }
            ]
        }),
        &root,
    );

    assert_eq!(output["results"][0]["success"], true);
    assert_eq!(
        output["results"][0]["command_type"],
        commands::active_shell_command_name()
    );
}

#[test]
fn pass_unknown_command_without_payload_runs_command_text_as_shell() {
    let _guard = env_lock_blocking();
    std::env::set_var("TURA_COMMAND_RUN_SHELL", "shell_command");
    let root = temp_workspace("unknown-command-no-payload");
    let output = command_run::execute(
        &json!({
            "commands": [
                {
                    "command": "Write-Output raw-command-ok",
                    "command_line": "",
                    "step": 1
                }
            ]
        }),
        &root,
    );

    assert_eq!(output["results"][0]["success"], true);
    assert_eq!(
        output["results"][0]["command_type"],
        commands::active_shell_command_name()
    );
}

#[test]
fn pass_command_line_without_command_defaults_to_active_shell_command() {
    let _guard = env_lock_blocking();
    std::env::set_var("TURA_COMMAND_RUN_SHELL", "shell_command");
    let root = temp_workspace("command-line-only");
    let output = command_run::execute(
        &json!({
            "commands": [
                {
                    "command_line": json!({ "command": "Write-Output command-line-only-ok" }).to_string(),
                    "step": 1
                }
            ]
        }),
        &root,
    );

    assert_eq!(output["results"][0]["success"], true);
    assert_eq!(
        output["results"][0]["command_type"],
        commands::active_shell_command_name()
    );
}

#[test]
fn pass_command_line_without_command_type_accepts_workdir_and_timeout() {
    let _guard = env_lock_blocking();
    std::env::set_var("TURA_COMMAND_RUN_SHELL", "shell_command");
    let root = temp_workspace("default-shell-workdir");
    let subdir = root.join("subdir");
    fs::create_dir_all(&subdir).expect("temp subdir");

    let output = command_run::execute(
        &json!({
            "commands": [
                {
                    "command_line": json!({ "command": "Get-Location", "timeout_ms": 5000 }).to_string(),
                    "workdir": "subdir",
                    "timeout_ms": 5000,
                    "step": 1
                }
            ]
        }),
        &root,
    );

    assert_eq!(output["results"][0]["success"], true);
    assert_eq!(
        output["results"][0]["command_type"],
        commands::active_shell_command_name()
    );
    assert!(output["results"][0]["output"]
        .as_str()
        .is_some_and(|text| text.replace('\\', "/").contains("/subdir")));
}

#[test]
fn pass_legacy_steps_shape_is_accepted() {
    let _guard = env_lock_blocking();
    std::env::set_var("TURA_COMMAND_RUN_SHELL", "shell_command");
    let root = temp_workspace("legacy-steps");
    let output = command_run::execute(
        &json!({
            "steps": [
                {
                    "tool_name": "shell_command",
                    "command_code": json!({ "command": "Write-Output legacy-steps-ok" }).to_string(),
                    "step": 1
                }
            ]
        }),
        &root,
    );

    assert_eq!(output["results"][0]["success"], true);
    assert_eq!(
        output["results"][0]["command_type"],
        commands::active_shell_command_name()
    );
}

#[test]
fn pass_command_run_arguments_accept_requests_wrapper_and_json_fence() {
    let _guard = env_lock_blocking();
    std::env::set_var("TURA_COMMAND_RUN_SHELL", "shell_command");
    let root = temp_workspace("json-fence");
    let output = command_run::execute(
        &Value::String(
            "```json\n{\"requests\":{\"commands\":[{\"command\":\"shell_command\",\"command_line\":\"{\\\"command\\\":\\\"Write-Output fenced-ok\\\",\\\"timeout_ms\\\":5000}\",\"step\":1}]}}\n```"
                .to_string(),
        ),
        &root,
    );

    assert_eq!(output["results"][0]["success"], true);
}
