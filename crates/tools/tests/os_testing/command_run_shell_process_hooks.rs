#[allow(dead_code, unused_imports)]
#[path = "../business/helpers/command_run_current.rs"]
mod helpers;

use helpers::*;

#[test]
fn pass_timeout_returns_quick_failure() {
    let _guard = env_lock_blocking();
    std::env::set_var("TURA_COMMAND_RUN_SHELL", "shell_command");
    let root = temp_workspace("timeout");
    let command = if cfg!(windows) {
        "Start-Sleep -Seconds 10"
    } else {
        "sleep 10"
    };
    let started = Instant::now();
    let output = command_run::execute(
        &json!({
            "commands": [
                {
                    "command": "shell_command",
                    "command_line": json!({ "command": command, "timeout_ms": 500 }).to_string()
                }
            ]
        }),
        &root,
    );

    assert_eq!(output["results"][0]["success"], false);
    assert!(output["results"][0]["output"]
        .as_str()
        .unwrap_or_default()
        .contains("Timed out after"));
    assert!(
        output["results"][0].get("error").is_none(),
        "timeout must be returned by the shell runtime as model-visible tool output, not by dropping command_run dispatch"
    );
    assert!(started.elapsed() < Duration::from_secs(5));
}

#[test]
fn fail_timeout_kills_descendant_process_tree_quickly() {
    let _guard = env_lock_blocking();
    std::env::set_var("TURA_COMMAND_RUN_SHELL", "bash");
    let root = temp_workspace("descendant-timeout");
    let started = Instant::now();
    let output = command_run::execute(
        &json!({
            "commands": [
                {
                    "command": "bash",
                    "command_line": json!({ "command": "sleep 10", "timeout_ms": 500 }).to_string()
                }
            ]
        }),
        &root,
    );

    assert_eq!(output["results"][0]["success"], false);
    assert!(output["results"][0]["output"]
        .as_str()
        .unwrap_or_default()
        .contains("Timed out after"));
    assert!(
        output["results"][0].get("error").is_none(),
        "descendant timeout must be converted by the shell runtime instead of outer command_run timeout"
    );
    assert!(started.elapsed() < Duration::from_secs(5));
}

#[tokio::test]
async fn pass_async_command_run_entry_does_not_start_nested_runtime() {
    let _guard = env_lock().await;
    std::env::set_var("TURA_COMMAND_RUN_SHELL", "shell_command");
    let root = temp_workspace("async-entry");
    let output = command_run::execute_async_value(
        json!({
            "commands": [
                { "command": "shell_command", "command_line": json!({ "command": "Write-Output async-ok" }).to_string() }
            ]
        }),
        root,
    )
    .await;

    assert_eq!(output["results"][0]["success"], true);
    assert!(output["results"][0]["output"]
        .as_str()
        .unwrap_or_default()
        .contains("async-ok"));
}

#[test]
fn pass_bash_surface_runs_posix_script_without_exposing_shell_command() {
    let _guard = env_lock_blocking();
    std::env::set_var("TURA_COMMAND_RUN_SHELL", "bash");
    let root = temp_workspace("bash-script");
    let output = command_run::execute(
        &json!({
            "commands": [
                {
                    "command": "bash",
                    "command_line": json!({ "command": "for x in one two; do echo $x; done", "timeout_ms": 5000 }).to_string()
                }
            ]
        }),
        &root,
    );

    assert_eq!(commands::canonical_command("shell_command"), "bash");
    assert!(output["results"][0].get("command").is_none());
    assert_eq!(output["results"][0]["command_type"], "bash");
    assert_eq!(output["results"][0]["success"], true);
    assert!(output["results"][0]["output"]
        .as_str()
        .unwrap_or_default()
        .contains("one"));
}

#[test]
#[cfg(unix)]
fn pass_zsh_surface_runs_zsh_script_without_exposing_shell_command() {
    let _guard = env_lock_blocking();
    if !zsh_available() {
        eprintln!("zsh unavailable; skipping zsh execution fixture");
        return;
    }
    std::env::set_var("TURA_COMMAND_RUN_SHELL", "zsh");
    let root = temp_workspace("zsh-script");
    let output = command_run::execute(
        &json!({
            "commands": [
                {
                    "command": "zsh",
                    "command_line": json!({ "command": "arr=(alpha beta); print -r -- ${arr[1]}; [[ -n \"$ZSH_VERSION\" ]]", "timeout_ms": 5000 }).to_string()
                }
            ]
        }),
        &root,
    );

    assert_eq!(commands::canonical_command("shell_command"), "zsh");
    assert!(output["results"][0].get("command").is_none());
    assert_eq!(output["results"][0]["command_type"], "zsh");
    assert_eq!(output["results"][0]["success"], true);
    let text = output["results"][0]["output"].as_str().unwrap_or_default();
    assert!(text.contains("alpha"), "{output}");
    assert!(!text.contains("beta"), "{output}");
}

#[test]
fn fail_zsh_surface_reports_missing_configured_binary() {
    let _guard = env_lock_blocking();
    let previous_shell = std::env::var_os("TURA_COMMAND_RUN_SHELL");
    let previous_zsh_path = std::env::var_os("TURA_ZSH_PATH");
    std::env::set_var("TURA_COMMAND_RUN_SHELL", "zsh");
    std::env::set_var(
        "TURA_ZSH_PATH",
        if cfg!(windows) {
            r"C:\definitely\missing\tura-zsh.exe"
        } else {
            "/definitely/missing/tura-zsh"
        },
    );
    let root = temp_workspace("zsh-missing");
    let output = command_run::execute(
        &json!({
            "commands": [
                {
                    "command": "zsh",
                    "command_line": json!({ "command": "print -r -- zsh-ok", "timeout_ms": 1000 }).to_string()
                }
            ]
        }),
        &root,
    );

    restore_env_var("TURA_COMMAND_RUN_SHELL", previous_shell);
    restore_env_var("TURA_ZSH_PATH", previous_zsh_path);
    assert_eq!(output["results"][0]["command_type"], "zsh");
    assert_eq!(output["results"][0]["success"], false);
    assert!(output["results"][0]["output"]
        .as_str()
        .unwrap_or_default()
        .contains("zsh executable was not found"));
}

#[test]
fn pass_shell_surface_isolation_canonicalizes_to_one_active_shell() {
    let _guard = env_lock_blocking();

    std::env::set_var("TURA_COMMAND_RUN_SHELL", "bash");
    assert_eq!(commands::canonical_command("shell_command"), "bash");
    assert_eq!(commands::canonical_command("shll"), "bash");
    assert_eq!(commands::canonical_command("bash"), "bash");
    assert_eq!(commands::canonical_command("zsh"), "bash");

    std::env::set_var("TURA_COMMAND_RUN_SHELL", "zsh");
    assert_eq!(commands::canonical_command("shell_command"), "zsh");
    assert_eq!(commands::canonical_command("shll"), "zsh");
    assert_eq!(commands::canonical_command("bash"), "zsh");
    assert_eq!(commands::canonical_command("zsh"), "zsh");

    std::env::set_var("TURA_COMMAND_RUN_SHELL", "shll");
    assert_eq!(commands::canonical_command("bash"), "shell_command");
    assert_eq!(commands::canonical_command("zsh"), "shell_command");
    assert_eq!(
        commands::canonical_command("shell_command"),
        "shell_command"
    );
}

#[tokio::test]
async fn fail_pre_tool_hook_blocks_tool_before_runtime() {
    let _guard = env_lock().await;
    std::env::set_var("TURA_COMMAND_RUN_SHELL", "shell_command");
    let root = temp_workspace("pre-hook");
    let ctx = ToolContext::new(root);
    ctx.set_pre_hook(|call| {
        Err(ToolError::RespondToModel(format!(
            "blocked by hook: {}",
            call.tool_name
        )))
    });
    let router = ToolRouter::new();
    let call = ToolCall {
        tool_name: "shell_command".to_string(),
        call_id: "call_pre_hook".to_string(),
        payload: ToolPayload::Function {
            arguments: json!({ "command": "Write-Output should-not-run", "timeout_ms": 5000 }),
        },
    };

    let error = router
        .dispatch(call, ctx.clone(), false)
        .await
        .expect_err("pre hook should block dispatch");

    assert!(error.to_string().contains("blocked by hook"));
    assert!(
        ctx.events().is_empty(),
        "pre hook should run before tool-started events"
    );
}

#[tokio::test]
async fn pass_post_tool_hook_can_replace_model_visible_response() {
    let _guard = env_lock().await;
    std::env::set_var("TURA_COMMAND_RUN_SHELL", "shell_command");
    let root = temp_workspace("post-hook");
    let ctx = ToolContext::new(root);
    ctx.set_post_hook(|_call, output: &mut FunctionToolOutput| {
        output.body = json!({ "output": "replaced by post hook", "metadata": { "exit_code": 0 } });
        output.success = Some(true);
        Ok(())
    });
    let router = ToolRouter::new();
    let call = ToolCall {
        tool_name: "shell_command".to_string(),
        call_id: "call_post_hook".to_string(),
        payload: ToolPayload::Function {
            arguments: json!({ "command": "Write-Output original", "timeout_ms": 5000 }),
        },
    };

    let result = router
        .dispatch(call, ctx.clone(), false)
        .await
        .expect("post hook should allow dispatch");

    assert_eq!(
        result.result.code_mode_result()["output"],
        "replaced by post hook"
    );
    assert!(ctx.events().iter().any(|event| matches!(
        event,
        ToolRuntimeEvent::ToolFinished {
            call_id,
            success: true,
            ..
        } if call_id == "call_post_hook"
    )));
}

#[tokio::test]
async fn pass_shell_runtime_records_stdout_stderr_delta_events() {
    let _guard = env_lock().await;
    std::env::set_var("TURA_COMMAND_RUN_SHELL", "shell_command");
    let root = temp_workspace("stream-delta");
    let command = if cfg!(windows) {
        "Write-Output out-delta; [Console]::Error.WriteLine('err-delta')"
    } else {
        "echo out-delta; echo err-delta >&2"
    };
    let ctx = ToolContext::new(root);
    let router = ToolRouter::new();
    let call = ToolCall {
        tool_name: "shell_command".to_string(),
        call_id: "call_delta".to_string(),
        payload: ToolPayload::Function {
            arguments: json!({ "command": command, "timeout_ms": 5000 }),
        },
    };

    let result = router
        .dispatch(call, ctx.clone(), false)
        .await
        .expect("streaming command should succeed");

    assert_eq!(result.result.success, Some(true));
    let events = ctx.events();
    assert!(events.iter().any(|event| matches!(
        event,
        ToolRuntimeEvent::OutputDelta {
            call_id,
            stream,
            text,
        } if call_id == "call_delta" && stream == "stdout" && text.contains("out-delta")
    )));
    assert!(events.iter().any(|event| matches!(
        event,
        ToolRuntimeEvent::OutputDelta {
            call_id,
            stream,
            text,
        } if call_id == "call_delta" && stream == "stderr" && text.contains("err-delta")
    )));
}

#[tokio::test]
async fn fail_turn_cancellation_aborts_running_shell_command() {
    let _guard = env_lock().await;
    std::env::set_var("TURA_COMMAND_RUN_SHELL", "shell_command");
    let root = temp_workspace("cancel");
    let command = if cfg!(windows) {
        "Start-Sleep -Seconds 10"
    } else {
        "sleep 10"
    };
    let ctx = ToolContext::new(root);
    let cancellation = ctx.cancellation.clone();
    let router = ToolRouter::new();
    let call = ToolCall {
        tool_name: "shell_command".to_string(),
        call_id: "call_cancel".to_string(),
        payload: ToolPayload::Function {
            arguments: json!({ "command": command, "timeout_ms": 30000 }),
        },
    };
    let started = Instant::now();
    let task = tokio::spawn(async move { router.dispatch(call, ctx, false).await });

    tokio::time::sleep(Duration::from_millis(200)).await;
    cancellation.cancel();
    let result = task
        .await
        .expect("dispatch task should join")
        .expect("dispatch should return model-visible failure output");

    assert!(started.elapsed() < Duration::from_secs(5));
    assert_eq!(result.result.success, Some(false));
    assert!(result.result.code_mode_result()["stderr"]
        .as_str()
        .unwrap_or_default()
        .contains("tool task aborted"));
}

#[tokio::test]
async fn pass_concurrent_shell_command_runs_do_not_block_async_runtime_or_cross_workspaces() {
    let _guard = env_lock().await;
    std::env::set_var("TURA_COMMAND_RUN_SHELL", "shell_command");
    let roots = (0..4)
        .map(|index| temp_workspace(&format!("concurrent-shell-{index}")))
        .collect::<Vec<_>>();
    let run_all = async {
        let mut tasks = Vec::new();
        for (index, root) in roots.iter().enumerate() {
            let root = root.clone();
            let marker = format!("command-run-concurrent-{index}");
            let file_name = format!("concurrent-{index}.txt");
            tasks.push(tokio::spawn(async move {
                let output = command_run::execute_async_value(
                    json!({
                        "commands": [{
                            "command": "shell_command",
                            "command_line": json!({
                                "command": format!("echo {marker} > {file_name}"),
                                "timeout_ms": 5000
                            }).to_string(),
                            "step": 1
                        }]
                    }),
                    root.clone(),
                )
                .await;
                (index, root, marker, file_name, output)
            }));
        }

        let mut completed = Vec::new();
        for task in tasks {
            completed.push(task.await.expect("command_run task should join"));
        }
        completed
    };

    let mut completed = tokio::time::timeout(Duration::from_secs(20), run_all)
        .await
        .expect("concurrent shell command_run calls should finish before the business timeout");
    completed.sort_by_key(|(index, _, _, _, _)| *index);

    for (index, root, marker, file_name, output) in completed {
        assert_eq!(output["results"][0]["command_type"], "shell_command");
        assert_eq!(
            output["results"][0]["success"], true,
            "concurrent shell command should succeed: {output}"
        );
        let text = fs::read_to_string(root.join(&file_name)).unwrap_or_else(|error| {
            panic!(
                "workspace {} should contain {file_name}: {error}",
                root.display()
            )
        });
        assert!(text.contains(&marker));
        let serialized = output.to_string();
        assert!(serialized.contains("shell_command"));
        for other in 0..roots.len() {
            if other != index {
                assert!(
                    !serialized.contains(&format!("concurrent-{other}.txt")),
                    "command_run result {index} should not mention another workspace: {output}"
                );
            }
        }
    }
}

#[tokio::test]
async fn fail_timeout_aborts_reader_drain_for_pipe_holding_descendants() {
    let _guard = env_lock().await;
    std::env::set_var("TURA_COMMAND_RUN_SHELL", "bash");
    let root = temp_workspace("reader-drain");
    let router = ToolRouter::new();
    let call = ToolCall {
        tool_name: "bash".to_string(),
        call_id: "call_drain".to_string(),
        payload: ToolPayload::Function {
            arguments: json!({ "command": "sh -c 'sleep 10 & wait'", "timeout_ms": 500 }),
        },
    };
    let started = Instant::now();

    let result = router
        .dispatch(call, ToolContext::new(root), false)
        .await
        .expect("timeout should be reported as tool output");

    assert!(started.elapsed() < Duration::from_secs(5));
    assert_eq!(result.result.success, Some(false));
    assert!(result.result.code_mode_result()["stderr"]
        .as_str()
        .unwrap_or_default()
        .contains("Timed out"));
}
