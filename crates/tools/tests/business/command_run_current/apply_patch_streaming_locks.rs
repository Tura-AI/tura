use super::helpers::*;

#[test]
fn pass_apply_patch_success_and_fail_context_mismatch() {
    let root = temp_workspace("patch");
    fs::write(root.join("app.txt"), "old\n").expect("fixture should be written");

    let pass = command_run::execute(
        &json!({
            "commands": [
                {
                    "command": "apply_patch",
                    "command_line": "*** Begin Patch\n*** Update File: app.txt\n@@\n-old\n+new\n*** End Patch\n"
                }
            ]
        }),
        &root,
    );
    assert_eq!(pass["results"][0]["success"], true);
    assert_eq!(
        fs::read_to_string(root.join("app.txt")).expect("patched file should be readable"),
        "new\n"
    );

    let fail = command_run::execute(
        &json!({
            "commands": [
                {
                    "command": "apply_patch",
                    "command_line": "*** Begin Patch\n*** Update File: app.txt\n@@\n-missing\n+value\n*** End Patch\n"
                }
            ]
        }),
        &root,
    );
    assert_eq!(fail["results"][0]["success"], false);
}

#[test]
fn pass_apply_patch_add_delete_and_move_are_tracked_in_output() {
    let root = temp_workspace("patch-add-delete-move");
    fs::write(root.join("move-me.txt"), "alpha\n").expect("move fixture should be written");
    fs::write(root.join("delete-me.txt"), "gone\n").expect("delete fixture should be written");

    let output = command_run::execute(
        &json!({
            "commands": [
                {
                    "step": 1,
                    "command": "apply_patch",
                    "command_line": "*** Begin Patch\n*** Add File: added.txt\n+hello\n*** Update File: move-me.txt\n*** Move to: moved.txt\n@@\n-alpha\n+beta\n*** Delete File: delete-me.txt\n*** End Patch\n"
                }
            ]
        }),
        &root,
    );

    assert_eq!(output["results"][0]["success"], true, "{output}");
    assert_eq!(
        fs::read_to_string(root.join("added.txt")).expect("added file should be readable"),
        "hello\n"
    );
    assert!(!root.join("move-me.txt").exists());
    assert_eq!(
        fs::read_to_string(root.join("moved.txt")).expect("moved file should be readable"),
        "beta\n"
    );
    assert!(!root.join("delete-me.txt").exists());
    let changes = output["results"][0]["output"]["changes"]
        .as_array()
        .expect("changes should be an array");
    assert!(changes.iter().any(|change| change["kind"] == "add"));
    assert!(changes
        .iter()
        .any(|change| change["move_path"] == "moved.txt"));
    assert!(changes.iter().any(|change| change["kind"] == "delete"));
}

#[test]
fn pass_apply_patch_allows_path_outside_workspace_without_cli_sandbox() {
    let _guard = env_lock_blocking();
    let previous_sandbox = std::env::var_os("TURA_COMMAND_RUN_SANDBOX");
    std::env::remove_var("TURA_COMMAND_RUN_SANDBOX");
    let root = temp_workspace("patch-outside-default");
    let outside = root
        .parent()
        .expect("temp workspace should have a parent")
        .join("outside-command-run-default-test.txt");
    let _ = fs::remove_file(&outside);

    let output = command_run::execute(
        &json!({
            "commands": [
                {
                    "command": "apply_patch",
                    "command_line": format!("*** Begin Patch\n*** Add File: {}\n+ok\n*** End Patch\n", outside.display())
                }
            ]
        }),
        &root,
    );

    assert_eq!(output["results"][0]["success"], true, "{output}");
    assert_eq!(
        fs::read_to_string(&outside).expect("outside file should be written"),
        "ok\n"
    );
    let _ = fs::remove_file(outside);
    restore_env_var("TURA_COMMAND_RUN_SANDBOX", previous_sandbox);
}

#[test]
fn fail_apply_patch_rejects_path_outside_workspace_when_cli_sandboxed() {
    let _guard = env_lock_blocking();
    let previous_sandbox = std::env::var_os("TURA_COMMAND_RUN_SANDBOX");
    std::env::set_var("TURA_COMMAND_RUN_SANDBOX", "1");
    let root = temp_workspace("patch-outside");
    let outside = root
        .parent()
        .expect("temp workspace should have a parent")
        .join("outside-command-run-test.txt");
    let _ = fs::remove_file(&outside);

    let output = command_run::execute(
        &json!({
            "commands": [
                {
                    "command": "apply_patch",
                    "command_line": format!("*** Begin Patch\n*** Add File: {}\n+bad\n*** End Patch\n", outside.display())
                }
            ]
        }),
        &root,
    );

    assert_eq!(output["results"][0]["success"], false);
    assert!(output["results"][0]["output"]["stderr"]
        .as_str()
        .unwrap_or_default()
        .contains("outside"));
    assert!(!outside.exists());
    restore_env_var("TURA_COMMAND_RUN_SANDBOX", previous_sandbox);
}

#[test]
fn pass_shell_embedded_apply_patch_is_intercepted_before_shell_execution() {
    let _guard = env_lock_blocking();
    std::env::set_var("TURA_COMMAND_RUN_SHELL", "shell_command");
    let root = temp_workspace("embedded-patch");
    fs::write(root.join("app.txt"), "old\n").expect("fixture should be written");
    let command_line = "@'\n*** Begin Patch\n*** Update File: app.txt\n@@\n-old\n+new\n*** End Patch\n'@ | apply_patch";

    let output = command_run::execute(
        &json!({
            "commands": [
                { "command": "shell_command", "command_line": json!({ "command": command_line }).to_string() }
            ]
        }),
        &root,
    );

    assert_eq!(output["results"][0]["success"], true, "{output}");
    assert_eq!(
        fs::read_to_string(root.join("app.txt")).expect("patched file should be readable"),
        "new\n"
    );
}

#[test]
fn pass_command_line_wrapped_apply_patch_routes_to_apply_patch() {
    let root = temp_workspace("patch-payload-route");
    fs::write(root.join("app.txt"), "old\n").expect("fixture");

    let output = command_run::execute(
        &json!({
            "commands": [
                {
                    "command": "shell_command",
                    "command_line": "apply_patch <<'PATCH'\n*** Begin Patch\n*** Update File: app.txt\n@@\n-old\n+new\n*** End Patch\nPATCH",
                    "step": 1
                }
            ]
        }),
        &root,
    );

    assert_eq!(output["results"][0]["success"], true);
    assert_eq!(output["results"][0]["command_type"], "apply_patch");
    assert_eq!(
        fs::read_to_string(root.join("app.txt")).expect("patched file should be readable"),
        "new\n"
    );
}

#[test]
fn pass_aliases_cmd_and_command_line_are_accepted() {
    let _guard = env_lock_blocking();
    std::env::set_var("TURA_COMMAND_RUN_SHELL", "shell_command");
    let root = temp_workspace("aliases");
    let output = command_run::execute(
        &json!({
            "commands": [
                { "cmd": "shell_command", "commandLine": json!({ "command": "Write-Output ok" }).to_string(), "step": 1 }
            ]
        }),
        &root,
    );

    assert_eq!(output["results"][0]["success"], true);
    assert_eq!(output["results"][0]["command_type"], "shell_command");
}

#[test]
fn pass_single_shell_object_without_commands_is_wrapped() {
    let _guard = env_lock_blocking();
    std::env::set_var("TURA_COMMAND_RUN_SHELL", "shell_command");
    let root = temp_workspace("single-shell-object");
    let output = command_run::execute(
        &json!({
            "command": json!({ "command": "Write-Output ok", "timeout_ms": 5000 }).to_string(),
            "timeoutMs": 120000
        }),
        &root,
    );

    assert!(
        output["results"][0]["success"].as_bool().unwrap_or(false),
        "single shell object should execute successfully: {output}"
    );
}

#[test]
fn pass_command_only_here_string_patch_is_routed_to_apply_patch() {
    let root = temp_workspace("patch-route");
    fs::write(root.join("app.txt"), "old\n").expect("fixture");

    let output = command_run::execute(
        &json!({
            "commands": [
                {
                    "command": "@'\n*** Begin Patch\n*** Update File: app.txt\n@@\n-old\n+new\n*** End Patch\n'@",
                    "step": 1
                }
            ]
        }),
        &root,
    );

    assert_eq!(output["results"][0]["success"], true);
    assert_eq!(output["results"][0]["command_type"], "apply_patch");
    assert_eq!(
        fs::read_to_string(root.join("app.txt")).expect("patched file should be readable"),
        "new\n"
    );
}

#[test]
fn fail_later_batch_commands_stop_after_apply_patch_failure() {
    let root = temp_workspace("patch-failure-stop");
    fs::write(root.join("app.txt"), "actual\n").expect("fixture");

    let output = command_run::execute(
        &json!({
            "commands": [
                {
                    "command": "apply_patch",
                    "command_line": "*** Begin Patch\n*** Update File: app.txt\n@@\n-missing\n+new\n*** End Patch\n",
                    "step": 1
                },
                {
                    "command": "shell_command",
                    "command_line": "echo after",
                    "step": 1
                },
                {
                    "command": "shell_command",
                    "command_line": "echo next-step",
                    "step": 2
                }
            ]
        }),
        &root,
    );

    assert_eq!(output["cancelled"], true);
    assert!(output["cancel_reason"]
        .as_str()
        .is_some_and(|text| text.contains("apply_patch failed")));
    assert_eq!(output["results"].as_array().expect("results").len(), 1);
    assert_eq!(output["results"][0]["success"], false);
    assert_eq!(
        output["results"][0]["output"]["output"]["error_type"],
        "ContextMismatch"
    );
}

#[test]
fn pass_streaming_executor_returns_apply_patch_result_without_finish() {
    let root = temp_workspace("streaming-immediate");
    fs::write(root.join("app.txt"), "old\n").expect("fixture");
    let runtime = tokio::runtime::Runtime::new().expect("runtime");
    let mut executor = command_run::StreamingCommandRunExecutor::new(root.clone());

    let immediate = runtime.block_on(executor.push_command_value(json!({
        "command": "apply_patch",
        "command_line": "*** Begin Patch\n*** Update File: app.txt\n@@\n-old\n+new\n*** End Patch\n",
        "step": 1
    })));

    assert_eq!(immediate.len(), 1);
    assert_eq!(immediate[0]["command_type"], "apply_patch");
    assert_eq!(immediate[0]["success"], true);
    assert_eq!(
        fs::read_to_string(root.join("app.txt")).expect("patched file should be readable"),
        "new\n"
    );
    let final_results = runtime.block_on(executor.finish());
    assert!(final_results.is_empty());
}

#[test]
fn pass_streaming_executor_strips_apply_patch_tool_prefix() {
    let root = temp_workspace("streaming-prefixed-patch");
    fs::write(root.join("app.txt"), "old\n").expect("fixture");
    let runtime = tokio::runtime::Runtime::new().expect("runtime");
    let mut executor = command_run::StreamingCommandRunExecutor::new(root.clone());

    let immediate = runtime.block_on(executor.push_command_value(json!({
        "command_type": "apply_patch",
        "command_line": "apply_patch\n*** Begin Patch\n*** Update File: app.txt\n@@\n-old\n+new\n*** End Patch\n",
        "step": 1
    })));

    assert_eq!(immediate.len(), 1);
    assert_eq!(immediate[0]["command_type"], "apply_patch");
    assert_eq!(immediate[0]["success"], true);
    assert_eq!(
        fs::read_to_string(root.join("app.txt")).expect("patched file should be readable"),
        "new\n"
    );
}

#[test]
fn fail_streaming_executor_ignores_commands_after_failed_apply_patch() {
    let root = temp_workspace("streaming-patch-stop");
    fs::write(root.join("app.txt"), "actual\n").expect("fixture");
    let runtime = tokio::runtime::Runtime::new().expect("runtime");
    let mut executor = command_run::StreamingCommandRunExecutor::new(root.clone());

    let failed = runtime.block_on(executor.push_command_value(json!({
        "command": "apply_patch",
        "command_line": "*** Begin Patch\n*** Update File: app.txt\n@@\n-missing\n+new\n*** End Patch\n",
        "step": 1
    })));
    let ignored = runtime.block_on(executor.push_command_value(json!({
        "command": "shell_command",
        "command_line": "echo after",
        "step": 1
    })));
    let final_results = runtime.block_on(executor.finish());

    assert_eq!(failed.len(), 1);
    assert_eq!(failed[0]["command_type"], "apply_patch");
    assert_eq!(failed[0]["success"], false);
    assert!(ignored.is_empty());
    assert!(final_results.is_empty());
    assert_eq!(
        fs::read_to_string(root.join("app.txt")).expect("fixture file should be readable"),
        "actual\n"
    );
}

#[test]
fn pass_streaming_executor_exposes_output_deltas_before_command_finishes() {
    let root = temp_workspace("streaming-output-deltas");
    let continue_file = root.join("continue.flag");
    let runtime = tokio::runtime::Runtime::new().expect("runtime");
    let mut executor = command_run::StreamingCommandRunExecutor::new(root);
    let event_ctx = executor.event_context();
    let command_line = if cfg!(windows) {
        format!(
            "$gate = '{}'; Write-Output 'stream-live-1'; while (-not (Test-Path -LiteralPath \
             $gate)) {{ Start-Sleep -Milliseconds 50 }}; Write-Output 'stream-live-2'",
            single_quoted_powershell_path(&continue_file)
        )
    } else {
        format!(
            "printf 'stream-live-1\\n'; while [ ! -f '{}' ]; do sleep 0.05; done; printf \
             'stream-live-2\\n'",
            single_quoted_posix_path(&continue_file)
        )
    };
    let handle = thread::spawn(move || {
        runtime.block_on(executor.push_command_value(json!({
            "command_type": commands::active_shell_command_name(),
            "command_line": command_line,
            "timeout_ms": 10000,
            "step": 1
        })))
    });

    let deadline = Instant::now() + Duration::from_secs(5);
    let mut saw_delta_before_finish = false;
    while Instant::now() < deadline {
        saw_delta_before_finish = event_ctx.events().iter().any(|event| {
            matches!(
                event,
                ToolRuntimeEvent::OutputDelta { text, .. } if text.contains("stream-live-1")
            )
        });
        if saw_delta_before_finish {
            break;
        }
        thread::sleep(Duration::from_millis(50));
    }
    let finished_before_gate = handle.is_finished();
    fs::write(&continue_file, "continue").expect("release streaming command");

    assert!(
        saw_delta_before_finish && !finished_before_gate,
        "expected stdout delta while the shell command was still waiting"
    );
    let results = handle.join().expect("streaming command thread");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0]["success"], true);
}

#[test]
fn pass_streaming_executor_repairs_scrambled_steps_without_accidental_grouping() {
    let root = temp_workspace("streaming-scrambled-steps");
    let runtime = tokio::runtime::Runtime::new().expect("runtime");
    let mut executor = command_run::StreamingCommandRunExecutor::new(root);
    let mut results = Vec::new();

    for (step, detail) in [(3, "three"), (2, "two"), (4, "four"), (1, "one")] {
        results.extend(runtime.block_on(executor.push_command_value(json!({
            "command": "task_status",
            "command_line": json!({ "status": "doing", "task_group": detail }).to_string(),
            "step": step
        }))));
    }
    results.extend(runtime.block_on(executor.finish()));

    assert_eq!(
        results
            .iter()
            .map(|result| result["step"].as_u64().expect("step"))
            .collect::<Vec<_>>(),
        vec![3, 4, 5, 6]
    );
    assert_eq!(
        results
            .iter()
            .map(|result| {
                result["output"]["task_status"]["task_group"]
                    .as_str()
                    .expect("task group")
                    .to_string()
            })
            .collect::<Vec<_>>(),
        vec!["three", "two", "four", "one"]
    );
}

#[test]
fn pass_mutating_commands_are_barriers_between_read_batches() {
    let _guard = env_lock_blocking();
    std::env::set_var("TURA_COMMAND_RUN_SHELL", "shell_command");
    let root = temp_workspace("barrier");
    fs::write(root.join("state.txt"), "before\n").expect("state fixture should be written");

    let output = command_run::execute(
        &json!({
            "commands": [
                { "step": 1, "command": "shell_command", "command_line": json!({ "command": "Get-Content state.txt" }).to_string() },
                {
                    "step": 1,
                    "command": "apply_patch",
                    "command_line": "*** Begin Patch\n*** Update File: state.txt\n@@\n-before\n+after\n*** End Patch\n"
                },
                { "step": 1, "command": "shell_command", "command_line": json!({ "command": "Get-Content state.txt" }).to_string() }
            ]
        }),
        &root,
    );

    assert_eq!(output["results"][0]["success"], true);
    assert_eq!(output["results"][1]["success"], true);
    assert_eq!(output["results"][2]["success"], true);
    assert!(output["results"][0]["output"]
        .as_str()
        .unwrap_or_default()
        .contains("before"));
    assert!(output["results"][2]["output"]
        .as_str()
        .unwrap_or_default()
        .contains("after"));
}

#[test]
fn pass_same_step_commands_keep_dependency_group() {
    let _guard = env_lock_blocking();
    std::env::set_var("TURA_COMMAND_RUN_SHELL", "shell_command");
    let root = temp_workspace("shared-read-step");
    fs::write(root.join("state.txt"), "ready\n").expect("state fixture should be written");
    let command_a = if cfg!(windows) {
        "Test-Path state.txt; Write-Output read-a"
    } else {
        "pwd; echo read-a"
    };
    let command_b = if cfg!(windows) {
        "Test-Path state.txt; Write-Output read-b"
    } else {
        "pwd; echo read-b"
    };

    let output = command_run::execute(
        &json!({
            "commands": [
                { "step": 1, "command": "shell_command", "command_line": json!({ "command": command_a, "timeout_ms": 5000 }).to_string() },
                { "step": 1, "command": "shell_command", "command_line": json!({ "command": command_b, "timeout_ms": 5000 }).to_string() }
            ]
        }),
        &root,
    );

    assert_eq!(output["results"][0]["success"], true);
    assert_eq!(output["results"][1]["success"], true);
    assert_eq!(output["results"][0]["step"], 1);
    assert_eq!(output["results"][1]["step"], 1);
}

#[tokio::test]
async fn pass_later_steps_wait_while_step_two_is_running() {
    let _guard = env_lock().await;
    std::env::set_var("TURA_COMMAND_RUN_SHELL", "shell_command");
    let root = temp_workspace("step-two-running-barrier");
    let gate = root.join("release-step-two.flag");
    let step_two = if cfg!(windows) {
        format!(
            "Set-Content -Path step2-started.txt -Value started; while (-not (Test-Path \
             -LiteralPath '{}')) {{ Start-Sleep -Milliseconds 50 }}; Set-Content -Path \
             step2-done.txt -Value done",
            single_quoted_powershell_path(&gate)
        )
    } else {
        format!(
            "printf started > step2-started.txt; while [ ! -f '{}' ]; do sleep 0.05; done; \
             printf done > step2-done.txt",
            single_quoted_posix_path(&gate)
        )
    };
    let step_command = |file_name: &str, text: &str| {
        if cfg!(windows) {
            format!("Set-Content -Path {file_name} -Value {text}")
        } else {
            format!("printf {text} > {file_name}")
        }
    };
    let run_root = root.clone();
    let run = tokio::spawn(async move {
        command_run::execute_async_value(
            json!({
                "commands": [
                    {
                        "step": 1,
                        "command": "shell_command",
                        "command_line": json!({
                            "command": step_command("step1.txt", "one"),
                            "timeout_ms": 5000
                        }).to_string()
                    },
                    {
                        "step": 2,
                        "command": "shell_command",
                        "command_line": json!({
                            "command": step_two,
                            "timeout_ms": 10000
                        }).to_string()
                    },
                    {
                        "step": 3,
                        "command": "shell_command",
                        "command_line": json!({
                            "command": step_command("step3.txt", "three"),
                            "timeout_ms": 5000
                        }).to_string()
                    },
                    {
                        "step": 4,
                        "command": "shell_command",
                        "command_line": json!({
                            "command": step_command("step4.txt", "four"),
                            "timeout_ms": 5000
                        }).to_string()
                    }
                ]
            }),
            run_root,
        )
        .await
    });

    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline && !root.join("step2-started.txt").exists() {
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert!(
        root.join("step2-started.txt").exists(),
        "step 2 should start before testing the dependency barrier"
    );
    tokio::time::sleep(Duration::from_millis(250)).await;
    assert!(
        !root.join("step3.txt").exists() && !root.join("step4.txt").exists(),
        "steps 3 and 4 must not run while step 2 is still blocked"
    );

    fs::write(&gate, "release").expect("release step two");
    let output = tokio::time::timeout(Duration::from_secs(10), run)
        .await
        .expect("command_run should finish after releasing step 2")
        .expect("command_run task should join");
    let results = output["results"]
        .as_array()
        .expect("command_run output should contain results");
    assert_eq!(results.len(), 4);
    assert_eq!(
        results
            .iter()
            .map(|result| result["step"].as_u64().expect("step"))
            .collect::<Vec<_>>(),
        vec![1, 2, 3, 4]
    );
    assert!(root.join("step3.txt").exists());
    assert!(root.join("step4.txt").exists());
}

#[test]
fn pass_scrambled_steps_are_repaired_without_accidental_parallel_grouping() {
    let _guard = env_lock_blocking();
    std::env::set_var("TURA_COMMAND_RUN_SHELL", "shell_command");
    let root = temp_workspace("scrambled-steps");

    let output = command_run::execute(
        &json!({
            "commands": [
                { "step": 3, "command": "task_status", "command_line": json!({ "status": "doing", "task_group": "three" }).to_string() },
                { "step": 2, "command": "task_status", "command_line": json!({ "status": "doing", "task_group": "two" }).to_string() },
                { "step": 4, "command": "task_status", "command_line": json!({ "status": "doing", "task_group": "four" }).to_string() },
                { "step": 1, "command": "task_status", "command_line": json!({ "status": "done", "task_group": "one" }).to_string() }
            ]
        }),
        &root,
    );

    let results = output["results"].as_array().expect("results");
    assert_eq!(
        results
            .iter()
            .map(|result| result["step"].as_u64().expect("step"))
            .collect::<Vec<_>>(),
        vec![3, 4, 5, 6]
    );
    assert_eq!(
        results
            .iter()
            .map(|result| {
                result["output"]["task_status"]["task_group"]
                    .as_str()
                    .expect("task group")
                    .to_string()
            })
            .collect::<Vec<_>>(),
        vec!["three", "two", "four", "one"]
    );
}

#[test]
fn pass_file_lock_allows_parallel_reads_and_blocks_write() {
    let read_access = Access {
        read_paths: vec!["same.txt".to_string()],
        ..Access::default()
    };
    let write_access = Access {
        write_paths: vec!["same.txt".to_string()],
        ..Access::default()
    };
    let read_a = file_locks::acquire(&read_access);
    let read_b = file_locks::acquire(&read_access);
    let started = Instant::now();
    let writer = std::thread::spawn(move || {
        let _write = file_locks::acquire(&write_access);
        started.elapsed()
    });

    std::thread::sleep(Duration::from_millis(250));
    assert!(
        !writer.is_finished(),
        "write lock must wait for active readers"
    );
    drop(read_a);
    assert!(
        !writer.is_finished(),
        "write lock must wait for all readers"
    );
    drop(read_b);
    let waited = writer.join().expect("writer thread should finish");
    assert!(waited >= Duration::from_millis(200));
}

#[test]
fn pass_file_lock_many_readers_release_before_writer() {
    let reader_count = 16;
    let key = format!("many-readers-{}", std::process::id());
    let read_access = Access {
        read_paths: vec![key.clone()],
        ..Access::default()
    };
    let write_access = Access {
        write_paths: vec![key],
        ..Access::default()
    };
    let readers_ready = Arc::new(Barrier::new(reader_count + 1));
    let release_readers = Arc::new(Barrier::new(reader_count + 1));
    let mut readers = Vec::new();

    for _ in 0..reader_count {
        let read_access = read_access.clone();
        let readers_ready = Arc::clone(&readers_ready);
        let release_readers = Arc::clone(&release_readers);
        readers.push(thread::spawn(move || {
            let _read = file_locks::acquire(&read_access);
            readers_ready.wait();
            release_readers.wait();
        }));
    }

    readers_ready.wait();
    let started = Instant::now();
    let writer = thread::spawn(move || {
        let _write = file_locks::acquire(&write_access);
        started.elapsed()
    });
    thread::sleep(Duration::from_millis(100));
    assert!(
        !writer.is_finished(),
        "writer must wait while many readers hold the dependency group"
    );
    release_readers.wait();
    for reader in readers {
        reader.join().expect("reader should release cleanly");
    }
    let waited = writer
        .join()
        .expect("writer should acquire after readers release");
    assert!(waited >= Duration::from_millis(80));
}

#[test]
fn pass_file_lock_releases_after_panic_and_allows_retry() {
    let key = format!("panic-release-{}", std::process::id());
    let first_access = Access {
        write_paths: vec![key.clone()],
        ..Access::default()
    };
    let retry_access = Access {
        write_paths: vec![key],
        ..Access::default()
    };

    let panicked = thread::spawn(move || {
        let _write = file_locks::acquire(&first_access);
        panic!("intentional panic while holding file lock");
    })
    .join();
    assert!(panicked.is_err());

    let retry = thread::spawn(move || {
        let _write = file_locks::acquire(&retry_access);
        true
    })
    .join()
    .expect("retry should not panic");
    assert!(retry, "retry should acquire after panic drops the guard");
}

#[test]
fn pass_same_step_mutating_shell_commands_are_serialized_by_workspace_lock() {
    let _guard = env_lock_blocking();
    std::env::set_var("TURA_COMMAND_RUN_SHELL", "shell_command");
    let root = temp_workspace("same-step-mutating-serialized");
    let command = |label: &str| {
        if cfg!(windows) {
            format!(
                "$guard = 'guard.lock'; if (Test-Path $guard) {{ Write-Error 'overlap'; exit 7 }}; New-Item -ItemType File -Path $guard -Force | Out-Null; Start-Sleep -Milliseconds 150; Remove-Item $guard; Add-Content -Path trace.txt -Value '{label}'"
            )
        } else {
            format!(
                "if [ -e guard.lock ]; then echo overlap >&2; exit 7; fi; touch guard.lock; sleep 0.15; rm guard.lock; printf '%s\\n' '{label}' >> trace.txt"
            )
        }
    };
    let commands = (0..6)
        .map(|index| {
            json!({
                "step": 1,
                "command": "shell_command",
                "command_line": json!({
                    "command": command(&format!("serialized-{index}")),
                    "timeout_ms": 5000
                }).to_string()
            })
        })
        .collect::<Vec<_>>();

    let output = command_run::execute(&json!({ "commands": commands }), &root);
    let results = output["results"]
        .as_array()
        .expect("command_run output should contain results");
    assert_eq!(results.len(), 6);
    assert!(
        results
            .iter()
            .all(|result| result["step"] == 1 && result["success"] == true),
        "same-step mutating commands should keep step 1 and serialize without overlap: {output}"
    );
    let trace = fs::read_to_string(root.join("trace.txt")).expect("trace should be written");
    for index in 0..6 {
        assert!(trace.contains(&format!("serialized-{index}")));
    }
    assert!(
        !root.join("guard.lock").exists(),
        "guard file should be removed after serialized commands finish"
    );
}

#[tokio::test]
async fn pass_failed_mutating_command_releases_workspace_lock_for_retry() {
    let _guard = env_lock().await;
    std::env::set_var("TURA_COMMAND_RUN_SHELL", "shell_command");
    let root = temp_workspace("failed-mutating-retry");
    let failing_command = if cfg!(windows) {
        "Set-Content -Path failed-before-retry.txt -Value before; exit 9"
    } else {
        "printf before > failed-before-retry.txt; exit 9"
    };
    let retry_command = if cfg!(windows) {
        "Set-Content -Path retry-after-failure.txt -Value retry-ok"
    } else {
        "printf retry-ok > retry-after-failure.txt"
    };

    let first = tokio::time::timeout(
        Duration::from_secs(10),
        command_run::execute_async_value(
            json!({
                "commands": [{
                    "step": 1,
                    "command": "shell_command",
                    "command_line": json!({ "command": failing_command, "timeout_ms": 5000 }).to_string()
                }]
            }),
            root.clone(),
        ),
    )
    .await
    .expect("failed mutating command must return instead of leaking a lock");
    assert_eq!(first["results"][0]["success"], false);

    let retry = tokio::time::timeout(
        Duration::from_secs(10),
        command_run::execute_async_value(
            json!({
                "commands": [{
                    "step": 1,
                    "command": "shell_command",
                    "command_line": json!({ "command": retry_command, "timeout_ms": 5000 }).to_string()
                }]
            }),
            root.clone(),
        ),
    )
    .await
    .expect("retry should not block on a leaked workspace lock");
    assert_eq!(retry["results"][0]["success"], true);
    assert!(
        fs::read_to_string(root.join("retry-after-failure.txt"))
            .expect("retry file should exist")
            .contains("retry-ok"),
        "retry command should write after failed command releases the lock"
    );
}
