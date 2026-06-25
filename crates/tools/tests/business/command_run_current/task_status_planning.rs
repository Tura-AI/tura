use super::helpers::*;

#[test]
fn pass_missing_steps_default_to_original_order() {
    let _guard = env_lock_blocking();
    std::env::set_var("TURA_COMMAND_RUN_SHELL", "shell_command");
    let root = temp_workspace("steps");
    let output = command_run::execute(
        &json!({
            "commands": [
                { "command": "shell_command", "command_line": "{\"command\":\"Write-Output one\"}" },
                { "command": "shell_command", "command_line": "{\"command\":\"Write-Output two\"}" }
            ]
        }),
        &root,
    );

    assert_eq!(output["results"][0]["step"], 1);
    assert_eq!(output["results"][1]["step"], 2);
}

#[test]
fn pass_top_level_task_status_argument_is_not_model_visible() {
    let _guard = env_lock_blocking();
    std::env::set_var("TURA_COMMAND_RUN_SHELL", "shell_command");
    let root = temp_workspace("top-level-task-status");

    let output = command_run::execute(
        &json!({
            "task_status": { "status": "done" },
            "commands": [
                { "command": "shell_command", "command_line": json!({ "command": "Write-Output ok" }).to_string() }
            ]
        }),
        &root,
    );

    assert!(output.get("task_status").is_none());
}

#[test]
fn pass_planning_command_routes_through_command_run() {
    let _guard = env_lock_blocking();
    std::env::set_var("TURA_FORCE_EXECUTE_TOOLS_PLANNING", "1");
    let root = temp_workspace("planning");

    let output = command_run::execute(
        &json!({
            "commands": [
                {
                    "command": "planning",
                    "command_line": "[{\"step\":1,\"task_summary\":\"Inspect files\"},{\"step\":1,\"task_summary\":\"Apply changes\"}]"
                }
            ]
        }),
        &root,
    );

    std::env::remove_var("TURA_FORCE_EXECUTE_TOOLS_PLANNING");

    assert_eq!(output["results"][0]["success"], true);
    assert_eq!(output["results"][0]["command_type"], "planning");
    assert_eq!(
        output["results"][0]["output"]["steps"][0]["task_summary"],
        "Inspect files"
    );
    assert_eq!(output["results"][0]["output"]["steps"][0]["step"], 1);
    assert!(output["results"][0]["output"]["steps"][0]
        .get("deliverable")
        .is_none());
    assert!(output["results"][0]["output"]["steps"][0]
        .get("task_id")
        .is_none());
    assert_eq!(output["results"][0]["output"]["steps"][1]["step"], 2);
}

#[test]
fn pass_task_status_command_inside_command_run_is_not_shell_executed() {
    let root = temp_workspace("task-status");
    let output = command_run::execute(
        &json!({
            "commands": [
                {
                    "step": 1,
                    "command_type": "task_status",
                    "command_line": "{\"status\":\"done\",\"task_group\":\"商城前端\"}"
                }
            ]
        }),
        &root,
    );

    assert_eq!(output["results"][0]["command_type"], "task_status");
    assert_eq!(output["results"][0]["success"], true);
    assert_eq!(
        output["results"][0]["output"],
        json!({ "task_status": { "status": "done", "task_group": "商城前端" } })
    );
}

#[test]
fn pass_task_status_payload_in_command_field_is_recovered() {
    let root = temp_workspace("task-status-command-field");
    let output = command_run::execute(
        &json!({
            "commands": [
                {
                    "step": 1,
                    "command_type": "task_status",
                    "command": "{\"status\":\"done\",\"task_group\":\"订单清结算微服务\"}"
                }
            ]
        }),
        &root,
    );

    assert_eq!(output["results"][0]["command_type"], "task_status");
    assert_eq!(output["results"][0]["success"], true);
    assert_eq!(
        output["results"][0]["output"],
        json!({ "task_status": { "status": "done", "task_group": "订单清结算微服务" } })
    );
}

#[test]
fn pass_task_status_accepts_no_required_arguments() {
    let root = temp_workspace("task-status-empty");
    let output = command_run::execute(
        &json!({
            "commands": [
                {
                    "step": 1,
                    "command_type": "task_status",
                    "command_line": "{}"
                }
            ]
        }),
        &root,
    );

    assert_eq!(output["results"][0]["command_type"], "task_status");
    assert_eq!(output["results"][0]["success"], true);
    assert_eq!(output["results"][0]["output"], json!({ "task_status": {} }));
}

#[test]
fn fail_task_status_rejects_status_outside_doing_question_or_done() {
    let root = temp_workspace("task-status-invalid");
    let output = command_run::execute(
        &json!({
            "commands": [
                {
                    "step": 1,
                    "command_type": "task_status",
                    "command_line": "{\"status\":\"blocked\"}"
                }
            ]
        }),
        &root,
    );

    assert_eq!(output["results"][0]["command_type"], "task_status");
    assert_eq!(output["results"][0]["success"], false);
    assert_eq!(
        output["results"][0]["error"],
        "task_status status must be doing, question, or done"
    );
}

#[test]
fn fail_planning_command_is_unavailable_by_default() {
    let _guard = env_lock_blocking();
    std::env::remove_var("TURA_FORCE_PLANNING");
    std::env::remove_var("TURA_FORCE_EXECUTE_TOOLS_PLANNING");
    let root = temp_workspace("planning-disabled");

    let output = command_run::execute(
        &json!({
            "commands": [
                {
                    "command": "planning",
                    "command_line": "[{\"task_summary\":\"Inspect files\"},{\"task_summary\":\"Apply changes\"}]"
                }
            ]
        }),
        &root,
    );

    assert_eq!(output["results"][0]["success"], false);
    assert_eq!(
        output["results"][0]["error"],
        "unsupported command_run command"
    );
}

#[tokio::test]
async fn fail_command_run_rejects_commands_outside_agent_capabilities() {
    let root = temp_workspace("allowed-commands");
    let allowed = BTreeSet::from(["shell_command".to_string()]);
    let output = command_run::execute_async_value_with_allowed(
        json!({
            "commands": [
                {
                    "command_type": "read_media",
                    "command_line": "read_media sample.png"
                }
            ]
        }),
        root,
        Some(allowed),
    )
    .await;

    assert_eq!(output["results"][0]["command_type"], "read_media");
    assert_eq!(output["results"][0]["success"], false);
    assert_eq!(
        output["results"][0]["error"],
        "unsupported command_run command"
    );
}

#[test]
fn pass_task_status_compact_context_routes_and_outputs_summary() {
    let _guard = env_lock_blocking();
    std::env::set_var("TURA_COMMAND_RUN_SHELL", "shell_command");
    let root = temp_workspace("compact-context");
    let output = command_run::execute(
        &json!({
            "commands": [
                {
                    "step": 1,
                    "command_type": "shell_command",
                    "command_line": json!({ "command": "Write-Output before-compact" }).to_string()
                },
                {
                    "step": 2,
                    "command_type": "task_status",
                    "command_line": "{\"compact_context\":\"Goal done partly. Next read src/lib.rs.\"}"
                }
            ]
        }),
        &root,
    );

    assert_eq!(output["results"][1]["command_type"], "task_status");
    assert_eq!(output["results"][1]["success"], true);
    assert_eq!(
        output["results"][1]["output"]["task_status"]["compact_context"],
        "Goal done partly. Next read src/lib.rs."
    );
}

#[test]
fn fail_task_status_compact_context_must_be_final_highest_step() {
    let _guard = env_lock_blocking();
    std::env::set_var("TURA_COMMAND_RUN_SHELL", "shell_command");
    let root = temp_workspace("compact-context-position");
    let output = command_run::execute(
        &json!({
            "commands": [
                {
                    "step": 2,
                    "command_type": "task_status",
                    "command_line": "{\"compact_context\":\"summary\"}"
                },
                {
                    "step": 3,
                    "command_type": "shell_command",
                    "command_line": json!({ "command": "Write-Output after" }).to_string()
                }
            ]
        }),
        &root,
    );

    assert_eq!(output["results"][0]["success"], false);
    assert_eq!(
        output["results"][0]["error"],
        "task_status compact_context must be the final command in the highest step of command_run"
    );
}
