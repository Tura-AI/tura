//! End-to-end interceptor tests.
//!
//! These tests drive the real `command_run` entry point with the bash surface
//! and *actually execute* commands. The point is to prove the interceptor's
//! effect on real execution: dangerous commands must be blocked before they run
//! (verified by checking that their destructive side effect never happened),
//! while safe commands must still run for real (verified by their side effect).
//!
//! Designed to run on Linux inside the Docker harness under
//! `tests/docker/` (see `scripts/run_interceptor_e2e_docker.sh`). The whole file
//! is gated to Unix so it only executes where a real `bash` is present.
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

fn result_text(output: &serde_json::Value) -> String {
    output["results"][0]["output"]
        .as_str()
        .unwrap_or_default()
        .to_string()
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

#[test]
fn dangerous_rm_rf_is_blocked_and_target_survives() {
    let root = workspace("rm-rf");
    let target = root.join("precious");
    fs::create_dir_all(&target).expect("create target");
    fs::write(target.join("keep.txt"), "data").expect("write file");

    let output = run_bash(&root, "rm -rf precious");

    assert_blocked(&output);
    assert!(
        target.join("keep.txt").exists(),
        "dangerous rm must not have executed"
    );
}

#[test]
fn sudo_wrapped_rm_is_blocked_and_target_survives() {
    let root = workspace("sudo-rm");
    let target = root.join("vault");
    fs::create_dir_all(&target).expect("create target");
    fs::write(target.join("keep.txt"), "data").expect("write file");

    let output = run_bash(&root, "sudo rm -rf vault");

    assert_blocked(&output);
    assert!(target.join("keep.txt").exists());
}

#[test]
fn timeout_wrapped_rm_is_blocked_and_target_survives() {
    let root = workspace("timeout-rm");
    let target = root.join("cache");
    fs::create_dir_all(&target).expect("create target");
    fs::write(target.join("keep.txt"), "data").expect("write file");

    let output = run_bash(&root, "timeout 5 rm -rf cache");

    assert_blocked(&output);
    assert!(target.join("keep.txt").exists());
}

#[test]
fn chained_rm_after_safe_command_is_blocked_and_target_survives() {
    let root = workspace("chained-rm");
    let target = root.join("data");
    fs::create_dir_all(&target).expect("create target");
    fs::write(target.join("keep.txt"), "data").expect("write file");

    let output = run_bash(&root, "echo starting && rm -rf data");

    assert_blocked(&output);
    assert!(
        target.join("keep.txt").exists(),
        "chained dangerous command must block the whole segment"
    );
}

#[test]
fn nested_bash_c_rm_is_blocked_and_target_survives() {
    let root = workspace("nested-rm");
    let target = root.join("inner");
    fs::create_dir_all(&target).expect("create target");
    fs::write(target.join("keep.txt"), "data").expect("write file");

    let output = run_bash(&root, "bash -c \"rm -rf inner\"");

    assert_blocked(&output);
    assert!(target.join("keep.txt").exists());
}

#[test]
fn python_library_smuggled_rm_is_blocked_and_target_survives() {
    let root = workspace("py-smuggle");
    let target = root.join("loot");
    fs::create_dir_all(&target).expect("create target");
    fs::write(target.join("keep.txt"), "data").expect("write file");

    let output = run_bash(&root, "python3 -c \"import os; os.system('rm -rf loot')\"");

    assert_blocked(&output);
    assert!(
        target.join("keep.txt").exists(),
        "library-smuggled dangerous command must not execute"
    );
}

#[test]
fn node_library_smuggled_rm_is_blocked_and_target_survives() {
    let root = workspace("node-smuggle");
    let target = root.join("stash");
    fs::create_dir_all(&target).expect("create target");
    fs::write(target.join("keep.txt"), "data").expect("write file");

    let output = run_bash(
        &root,
        "node -e \"require('child_process').execSync('rm -rf stash')\"",
    );

    assert_blocked(&output);
    assert!(target.join("keep.txt").exists());
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
fn interceptor_opt_out_allows_dangerous_command_to_execute() {
    let _guard = ENV_LOCK.lock().expect("env lock");
    std::env::set_var("TURA_COMMAND_RUN_SHELL", "bash");
    std::env::set_var("TURA_COMMAND_INTERCEPTOR_DISABLED", "1");

    let root = workspace("opt-out");
    let target = root.join("removable");
    fs::create_dir_all(&target).expect("create target");
    fs::write(target.join("keep.txt"), "data").expect("write file");

    let output = command_run::execute(
        &json!({
            "commands": [
                { "command": "bash", "command_line": json!({ "command": "rm -rf removable", "timeout_ms": 8000 }).to_string() }
            ]
        }),
        &root,
    );

    std::env::remove_var("TURA_COMMAND_INTERCEPTOR_DISABLED");

    assert_eq!(
        output["results"][0]["success"], true,
        "with interceptor disabled the command should run: {output}"
    );
    assert!(
        !target.exists(),
        "with interceptor disabled the dangerous command really executes"
    );
}
