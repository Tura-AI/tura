//! Local business E2E interceptor tests.
//!
//! These tests drive the real `command_run` entry point with shell surfaces
//! and *actually execute* commands. The point is to prove the interceptor's
//! effect on real execution: dangerous commands must be blocked before they run
//! (verified by checking that their destructive side effect never happened),
//! while safe commands must still run for real (verified by their side effect).
//!
//! Designed to run on Linux inside the Docker harness under `tests/docker/`.
//! The whole file is gated to Unix so it only executes where a real POSIX shell
//! is present.
#![cfg(unix)]

use code_tools::command_run;
use serde_json::json;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

static ENV_LOCK: Mutex<()> = Mutex::new(());

fn workspace(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "tura-interceptor-e2e-{name}-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).expect("create workspace");
    path
}

fn run_bash(root: &Path, command: &str) -> serde_json::Value {
    let _guard = ENV_LOCK.lock().expect("env lock");
    std::env::set_var("TURA_COMMAND_RUN_SHELL", "bash");
    std::env::remove_var("TURA_COMMAND_INTERCEPTOR_DISABLED");
    command_run::execute(
        &json!({
            "commands": [
                { "command": "bash", "command_line": json!({ "command": command, "timeout_ms": 8000 }).to_string() }
            ]
        }),
        root,
    )
}

fn zsh_available() -> bool {
    std::process::Command::new("zsh")
        .arg("-c")
        .arg("print -r -- zsh-ok")
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn run_zsh(root: &Path, command: &str) -> serde_json::Value {
    let _guard = ENV_LOCK.lock().expect("env lock");
    std::env::set_var("TURA_COMMAND_RUN_SHELL", "zsh");
    std::env::remove_var("TURA_COMMAND_INTERCEPTOR_DISABLED");
    command_run::execute(
        &json!({
            "commands": [
                { "command": "zsh", "command_line": json!({ "command": command, "timeout_ms": 8000 }).to_string() }
            ]
        }),
        root,
    )
}

fn result_text(output: &serde_json::Value) -> String {
    let result = &output["results"][0];
    [
        result["error"].as_str(),
        result["output"].as_str(),
        result["output"]["error"].as_str(),
        result["output"]["stderr"].as_str(),
        result["output"]["stdout"].as_str(),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join("\n")
}

fn assert_blocked(output: &serde_json::Value) {
    assert_eq!(
        output["results"][0]["success"], false,
        "interceptor must report failure: {output}"
    );
    assert!(
        result_text(output).contains("Blocked by command interceptor"),
        "expected interceptor block message, got: {}",
        result_text(output)
    );
}

fn assert_success(output: &serde_json::Value) {
    assert_eq!(
        output["results"][0]["success"], true,
        "command should run successfully: {output}"
    );
}

#[test]
fn workspace_rm_rf_is_allowed_and_target_is_deleted() {
    let root = workspace("rm-rf");
    let target = root.join("precious");
    fs::create_dir_all(&target).expect("create target");
    fs::write(target.join("keep.txt"), "data").expect("write file");

    let output = run_bash(&root, "rm -rf precious");

    assert_success(&output);
    assert!(!target.exists(), "workspace-local rm should have executed");
}

#[test]
fn zsh_surface_allows_workspace_rm_rf_and_target_is_deleted() {
    if !zsh_available() {
        eprintln!("zsh unavailable; skipping zsh interceptor fixture");
        return;
    }
    let root = workspace("zsh-rm-rf");
    let target = root.join("precious-zsh");
    fs::create_dir_all(&target).expect("create target");
    fs::write(target.join("keep.txt"), "data").expect("write file");

    let output = run_zsh(&root, "rm -rf precious-zsh");

    assert_success(&output);
    assert!(
        !target.exists(),
        "workspace-local zsh rm should have executed"
    );
}

#[test]
fn sudo_wrapped_rm_inside_workspace_is_allowed() {
    let root = workspace("sudo-rm");
    let target = root.join("vault");
    fs::create_dir_all(&target).expect("create target");
    fs::write(target.join("keep.txt"), "data").expect("write file");

    let output = run_bash(&root, "sudo rm -rf vault");

    if result_text(&output).contains("sudo: command not found") {
        eprintln!("sudo unavailable; skipping sudo workspace-delete execution assertion");
        return;
    }
    assert_success(&output);
    assert!(!target.exists());
}

#[test]
fn timeout_wrapped_rm_inside_workspace_is_allowed() {
    let root = workspace("timeout-rm");
    let target = root.join("cache");
    fs::create_dir_all(&target).expect("create target");
    fs::write(target.join("keep.txt"), "data").expect("write file");

    let output = run_bash(&root, "timeout 5 rm -rf cache");

    assert_success(&output);
    assert!(!target.exists());
}

#[test]
fn chained_workspace_rm_after_safe_command_is_allowed() {
    let root = workspace("chained-rm");
    let target = root.join("data");
    fs::create_dir_all(&target).expect("create target");
    fs::write(target.join("keep.txt"), "data").expect("write file");

    let output = run_bash(&root, "echo starting && rm -rf data");

    assert_success(&output);
    assert!(
        !target.exists(),
        "workspace-local chained rm should execute"
    );
}

#[test]
fn nested_bash_c_workspace_rm_is_allowed() {
    let root = workspace("nested-rm");
    let target = root.join("inner");
    fs::create_dir_all(&target).expect("create target");
    fs::write(target.join("keep.txt"), "data").expect("write file");

    let output = run_bash(&root, "bash -c \"rm -rf inner\"");

    assert_success(&output);
    assert!(!target.exists());
}

#[test]
fn python_library_workspace_rm_is_allowed() {
    let root = workspace("py-smuggle");
    let target = root.join("loot");
    fs::create_dir_all(&target).expect("create target");
    fs::write(target.join("keep.txt"), "data").expect("write file");

    let output = run_bash(&root, "python3 -c \"import os; os.system('rm -rf loot')\"");

    assert_success(&output);
    assert!(!target.exists());
}

#[test]
fn node_library_workspace_rm_is_allowed() {
    let root = workspace("node-smuggle");
    let target = root.join("stash");
    fs::create_dir_all(&target).expect("create target");
    fs::write(target.join("keep.txt"), "data").expect("write file");

    let output = run_bash(
        &root,
        "node -e \"require('child_process').execSync('rm -rf stash')\"",
    );

    assert_success(&output);
    assert!(!target.exists());
}

#[test]
fn outside_workspace_rm_rf_is_blocked_and_target_survives() {
    let root = workspace("outside-rm-rf");
    let outside = root
        .parent()
        .expect("workspace should have parent")
        .join(format!("tura-interceptor-outside-{}", std::process::id()));
    let _ = fs::remove_dir_all(&outside);
    fs::create_dir_all(&outside).expect("create outside target");
    fs::write(outside.join("keep.txt"), "data").expect("write outside file");

    let output = run_bash(&root, &format!("rm -rf {}", outside.display()));

    assert_blocked(&output);
    assert!(outside.join("keep.txt").exists());
    let _ = fs::remove_dir_all(outside);
}

#[test]
fn safe_command_runs_for_real() {
    let root = workspace("safe-touch");

    let output = run_bash(&root, "echo hello > created.txt");

    assert_eq!(
        output["results"][0]["success"], true,
        "safe command must run: {output}"
    );
    let created = root.join("created.txt");
    assert!(
        created.exists(),
        "safe command should have created the file"
    );
    assert!(fs::read_to_string(&created)
        .unwrap_or_default()
        .contains("hello"));
}

#[test]
fn non_destructive_rm_of_single_file_still_runs() {
    let root = workspace("safe-rm");
    let scratch = root.join("scratch.txt");
    fs::write(&scratch, "temp").expect("write scratch");

    // Plain `rm file` (no force, no recurse, not a system path) is allowed and
    // should actually delete the file.
    let output = run_bash(&root, "rm scratch.txt");

    assert_eq!(
        output["results"][0]["success"], true,
        "plain rm of a single file should run: {output}"
    );
    assert!(!scratch.exists(), "allowed rm should have deleted the file");
}

#[test]
fn interceptor_opt_out_allows_safe_command_to_execute() {
    let _guard = ENV_LOCK.lock().expect("env lock");
    std::env::set_var("TURA_COMMAND_RUN_SHELL", "bash");
    std::env::set_var("TURA_COMMAND_INTERCEPTOR_DISABLED", "1");

    let root = workspace("opt-out");
    let marker = root.join("optout-safe.txt");

    let output = command_run::execute(
        &json!({
            "commands": [
                { "command": "bash", "command_line": json!({ "command": "echo optout-ok > optout-safe.txt", "timeout_ms": 8000 }).to_string() }
            ]
        }),
        &root,
    );

    std::env::remove_var("TURA_COMMAND_INTERCEPTOR_DISABLED");

    assert_eq!(
        output["results"][0]["success"], true,
        "with interceptor disabled a safe command should run: {output}"
    );
    assert!(
        marker.exists(),
        "with interceptor disabled the safe command should create its marker"
    );
}
