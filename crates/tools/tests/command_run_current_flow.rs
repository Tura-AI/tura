use code_tools::command_run;
use code_tools::commands;
use code_tools::runtime::file_locks::{self, Access};
use code_tools::runtime::tool::{
    FunctionToolOutput, ToolCall, ToolContext, ToolError, ToolPayload, ToolRouter, ToolRuntimeEvent,
};
use serde_json::json;
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, Instant};

static ENV_LOCK: Mutex<()> = Mutex::new(());

fn env_lock() -> std::sync::MutexGuard<'static, ()> {
    ENV_LOCK.lock().unwrap_or_else(|err| err.into_inner())
}

fn temp_workspace(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "tura-command-run-current-flow-{name}-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).expect("create temp workspace");
    path
}

#[test]
fn pass_current_style_command_run_output_shape() {
    let _guard = env_lock();
    std::env::set_var("TURA_COMMAND_RUN_SHELL", "shell_command");
    let root = temp_workspace("shape");

    let output = command_run::execute(
        &json!({
            "commands": [
                { "command": "shell_command", "command_line": "{\"command\":\"Write-Output ok\",\"timeout_ms\":5000}" }
            ]
        }),
        &root,
    );

    assert!(output.get("results").is_some());
    assert!(
        output.get("ok").is_none(),
        "current command_run does not expose top-level ok"
    );
    assert!(output.get("output_policy").is_none());
    assert!(output["results"][0].get("command").is_none());
    assert_eq!(output["results"][0]["command_type"], "shell_command");
    assert_eq!(output["results"][0]["success"], true);
}

#[tokio::test]
async fn pass_internal_command_rebuilds_tool_call_and_dispatches_router_handler() {
    let _guard = env_lock();
    std::env::set_var("TURA_COMMAND_RUN_SHELL", "shell_command");
    let root = temp_workspace("router");
    let router = ToolRouter::new();
    let call = ToolCall {
        tool_name: "shell_command".to_string(),
        call_id: "call_test".to_string(),
        payload: ToolPayload::Function {
            arguments: json!({ "command": "Write-Output router-ok", "timeout_ms": 5000 }),
        },
    };

    let result = router
        .dispatch(call, ToolContext::new(root), false)
        .await
        .expect("router dispatch should succeed");

    assert_eq!(result.call_id, "call_test");
    assert_eq!(result.result.success, Some(true));
    assert!(result.result.code_mode_result()["stdout"]
        .as_str()
        .unwrap_or_default()
        .contains("router-ok"));
}

#[test]
fn fail_empty_command_run_returns_current_style_failure_result() {
    let root = temp_workspace("empty");
    let output = command_run::execute(&json!({ "commands": [] }), &root);

    assert!(output["results"][0].get("command").is_none());
    assert_eq!(output["results"][0]["command_type"], "command_run");
    assert_eq!(output["results"][0]["success"], false);
    assert_eq!(
        output["results"][0]["error"],
        "command_run commands must not be empty"
    );
}

#[test]
fn fail_unsupported_internal_command_returns_model_visible_result() {
    let root = temp_workspace("unsupported");
    let output = command_run::execute(
        &json!({
            "commands": [
                { "command": "read_file", "command_line": "{}" }
            ]
        }),
        &root,
    );

    assert_eq!(output["results"][0]["success"], false);
    assert!(output["results"][0]["error"]
        .as_str()
        .unwrap()
        .contains("unsupported command_run command"));
}

#[test]
fn pass_read_media_image_returns_compact_visual_preview() {
    let root = temp_workspace("read-media-image");
    let image_path = root.join("sample.png");
    let mut image = image::RgbImage::new(64, 64);
    for (x, y, pixel) in image.enumerate_pixels_mut() {
        *pixel = if x > y {
            image::Rgb([220, 20, 20])
        } else {
            image::Rgb([20, 60, 220])
        };
    }
    image.save(&image_path).expect("save image fixture");

    let output = command_run::execute(
        &json!({
            "commands": [
                {
                    "command_type": "read_media",
                    "command_line": "{\"paths\":[\"sample.png\"],\"max_visuals\":1,\"max_side\":256}",
                    "step": 1
                }
            ]
        }),
        &root,
    );

    assert_eq!(output["results"][0]["command_type"], "read_media");
    assert_eq!(output["results"][0]["success"], true);
    let media = &output["results"][0]["output"]["media_results"][0];
    assert_eq!(media["media_type"], "image");
    assert_eq!(media["visual_preview_count"], 1);
    let url = media["visual_previews"][0]["image_url"]["url"]
        .as_str()
        .expect("image data url");
    assert!(url.starts_with("data:image/jpeg;base64,"));
    assert!(
        url.len() < 80_000,
        "preview should be compact, got {}",
        url.len()
    );
}

#[test]
fn pass_read_media_pdf_text_fallback_is_context_sized() {
    let root = temp_workspace("read-media-pdf");
    fs::write(
        root.join("brief.pdf"),
        b"%PDF-1.4\n1 0 obj <<>> stream\nBT /F1 12 Tf 72 720 Td (Quarterly media brief: blue chart and revenue table.) Tj ET\nendstream endobj\n%%EOF",
    )
    .expect("write simple pdf fixture");

    let output = command_run::execute(
        &json!({
            "commands": [
                {
                    "command_type": "read_media",
                    "command_line": "{\"paths\":[\"brief.pdf\"],\"include_text\":true,\"max_text_chars\":2000,\"max_visuals\":1}",
                    "step": 1
                }
            ]
        }),
        &root,
    );

    assert_eq!(output["results"][0]["command_type"], "read_media");
    assert_eq!(output["results"][0]["success"], true);
    let media = &output["results"][0]["output"]["media_results"][0];
    assert_eq!(media["media_type"], "pdf");
    assert!(media["extracted_text"]
        .as_str()
        .unwrap_or_default()
        .contains("Quarterly media brief"));
    assert!(
        serde_json::to_string(&output).expect("serialize").len() < 120_000,
        "read_media output should stay reasonably small"
    );
}

#[test]
fn pass_read_media_video_uses_available_frame_decoder() {
    let root = temp_workspace("read-media-video");
    let video_path = root.join("clip.mp4");
    let status = std::process::Command::new("python")
        .arg("-c")
        .arg(
            r#"
import cv2, sys
import numpy as np
out = cv2.VideoWriter(sys.argv[1], cv2.VideoWriter_fourcc(*"mp4v"), 2.0, (64, 64))
for i in range(6):
    frame = np.zeros((64,64,3), dtype=np.uint8)
    frame[:,:] = (i*30, 40, 220-i*20)
    cv2.putText(frame, str(i), (20,40), cv2.FONT_HERSHEY_SIMPLEX, 1, (255,255,255), 2)
    out.write(frame)
out.release()
"#,
        )
        .arg(&video_path)
        .status()
        .expect("run python");
    if !status.success() {
        eprintln!("python cv2 unavailable; skipping video decode fixture");
        return;
    }

    let output = command_run::execute(
        &json!({
            "commands": [
                {
                    "command_type": "read_media",
                    "command_line": "{\"paths\":[\"clip.mp4\"],\"max_visuals\":3,\"max_side\":128}",
                    "step": 1
                }
            ]
        }),
        &root,
    );

    assert_eq!(output["results"][0]["command_type"], "read_media");
    assert_eq!(output["results"][0]["success"], true);
    let media = &output["results"][0]["output"]["media_results"][0];
    assert_eq!(media["media_type"], "video");
    assert!(media["visual_preview_count"].as_u64().unwrap_or(0) >= 1);
}

#[test]
fn pass_missing_steps_default_to_original_order() {
    let _guard = env_lock();
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
fn pass_apply_patch_success_and_fail_context_mismatch() {
    let root = temp_workspace("patch");
    fs::write(root.join("app.txt"), "old\n").unwrap();

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
    assert_eq!(fs::read_to_string(root.join("app.txt")).unwrap(), "new\n");

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
    fs::write(root.join("move-me.txt"), "alpha\n").unwrap();
    fs::write(root.join("delete-me.txt"), "gone\n").unwrap();

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

    assert_eq!(output["results"][0]["success"], true);
    assert_eq!(
        fs::read_to_string(root.join("added.txt")).unwrap(),
        "hello\n"
    );
    assert!(!root.join("move-me.txt").exists());
    assert_eq!(
        fs::read_to_string(root.join("moved.txt")).unwrap(),
        "beta\n"
    );
    assert!(!root.join("delete-me.txt").exists());
    let changes = output["results"][0]["output"]["changes"]
        .as_array()
        .unwrap();
    assert!(changes.iter().any(|change| change["kind"] == "add"));
    assert!(changes
        .iter()
        .any(|change| change["move_path"] == "moved.txt"));
    assert!(changes.iter().any(|change| change["kind"] == "delete"));
}

#[test]
fn fail_apply_patch_rejects_path_outside_workspace() {
    let root = temp_workspace("patch-outside");
    let outside = root.parent().unwrap().join("outside-command-run-test.txt");
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
}

#[test]
fn pass_shell_embedded_apply_patch_is_intercepted_before_shell_execution() {
    let _guard = env_lock();
    std::env::set_var("TURA_COMMAND_RUN_SHELL", "shell_command");
    let root = temp_workspace("embedded-patch");
    fs::write(root.join("app.txt"), "old\n").unwrap();
    let command_line = "@'\n*** Begin Patch\n*** Update File: app.txt\n@@\n-old\n+new\n*** End Patch\n'@ | apply_patch";

    let output = command_run::execute(
        &json!({
            "commands": [
                { "command": "shell_command", "command_line": json!({ "command": command_line }).to_string() }
            ]
        }),
        &root,
    );

    assert_eq!(output["results"][0]["success"], true);
    assert_eq!(fs::read_to_string(root.join("app.txt")).unwrap(), "new\n");
}

#[test]
fn pass_mutating_commands_are_barriers_between_read_batches() {
    let _guard = env_lock();
    std::env::set_var("TURA_COMMAND_RUN_SHELL", "shell_command");
    let root = temp_workspace("barrier");
    fs::write(root.join("state.txt"), "before\n").unwrap();

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
fn pass_read_only_commands_in_same_step_run_concurrently() {
    let _guard = env_lock();
    std::env::set_var("TURA_COMMAND_RUN_SHELL", "shell_command");
    let root = temp_workspace("parallel-read-step");
    fs::write(root.join("state.txt"), "ready\n").unwrap();
    let command_a = if cfg!(windows) {
        "Test-Path state.txt; Start-Sleep -Milliseconds 900; Write-Output read-a"
    } else {
        "pwd; sleep 0.9; echo read-a"
    };
    let command_b = if cfg!(windows) {
        "Test-Path state.txt; Start-Sleep -Milliseconds 900; Write-Output read-b"
    } else {
        "pwd; sleep 0.9; echo read-b"
    };
    let started = Instant::now();

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
    assert!(
        started.elapsed() < Duration::from_millis(1700),
        "same-step read-only commands should run in parallel, elapsed {:?}",
        started.elapsed()
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
fn pass_timeout_returns_quick_failure() {
    let _guard = env_lock();
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
    assert!(started.elapsed() < Duration::from_secs(5));
}

#[test]
fn fail_timeout_kills_descendant_process_tree_quickly() {
    let _guard = env_lock();
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
    assert!(started.elapsed() < Duration::from_secs(5));
}

#[tokio::test]
async fn pass_async_command_run_entry_does_not_start_nested_runtime() {
    let _guard = env_lock();
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
    let _guard = env_lock();
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
fn pass_shell_surface_isolation_canonicalizes_to_one_active_shell() {
    let _guard = env_lock();

    std::env::set_var("TURA_COMMAND_RUN_SHELL", "bash");
    assert_eq!(commands::canonical_command("shell_command"), "bash");
    assert_eq!(commands::canonical_command("shll"), "bash");
    assert_eq!(commands::canonical_command("bash"), "bash");

    std::env::set_var("TURA_COMMAND_RUN_SHELL", "shll");
    assert_eq!(commands::canonical_command("bash"), "shell_command");
    assert_eq!(
        commands::canonical_command("shell_command"),
        "shell_command"
    );
}

#[tokio::test]
async fn fail_pre_tool_hook_blocks_tool_before_runtime() {
    let _guard = env_lock();
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
    let _guard = env_lock();
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
    let _guard = env_lock();
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
    let _guard = env_lock();
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
async fn fail_timeout_aborts_reader_drain_for_pipe_holding_descendants() {
    let _guard = env_lock();
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
