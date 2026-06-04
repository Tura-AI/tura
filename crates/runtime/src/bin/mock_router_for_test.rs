//! Test-only mock of the `tura_router run-agent` CLI contract: reads a
//! `RunAgentRequest` JSON from stdin and writes the result JSON to stdout.
//! Supports self-recursion (re-invokes itself by depth) to exercise the
//! runtime↔router CLI recursive dispatch path.
//!
//! Behavior is driven by env vars (each child agent gets its own set in tests):
//! - `MOCK_RECURSE_TO_DEPTH`: when `depth < target`, the mock re-invokes its
//!   own binary once via CLI and folds the child summary into the result.
//! - `MOCK_AGENT_SUMMARY`: summary string returned at this level.
//! - `MOCK_FAIL`: when set, the mock returns ok=false.
//!
//! No URL traffic is introduced: recursion goes through stdin/stdout CLI calls
//! to the mock's own binary.

use std::io::{Read, Write};
use std::process::{Command, Stdio};

use serde_json::{json, Value};

fn main() {
    let mut args = std::env::args().skip(1);
    let sub = args.next().unwrap_or_default();
    if sub != "run-agent" {
        eprintln!("mock_router_for_test: unsupported subcommand `{sub}`");
        std::process::exit(2);
    }

    let mut raw = String::new();
    if let Err(error) = std::io::stdin().read_to_string(&mut raw) {
        emit_error(&format!("stdin read failed: {error}"));
        return;
    }
    let req: Value = match serde_json::from_str(raw.trim()) {
        Ok(v) => v,
        Err(error) => {
            emit_error(&format!("invalid json: {error}"));
            return;
        }
    };

    if std::env::var("MOCK_FAIL").is_ok() {
        let session_id = req
            .get("session_id")
            .and_then(Value::as_str)
            .unwrap_or("mock-session")
            .to_string();
        let body = json!({
            "ok": false,
            "session_id": session_id,
            "error": "mock failure",
        });
        let _ = writeln!(std::io::stdout(), "{body}");
        return;
    }

    let depth = req.get("depth").and_then(Value::as_u64).unwrap_or(0) as usize;
    let agent = req
        .get("agent")
        .and_then(Value::as_str)
        .unwrap_or("unknown")
        .to_string();
    let session_id = req
        .get("session_id")
        .and_then(Value::as_str)
        .unwrap_or("mock-session")
        .to_string();
    let my_summary =
        std::env::var("MOCK_AGENT_SUMMARY").unwrap_or_else(|_| format!("{agent}@depth{depth}"));

    let target_depth = std::env::var("MOCK_RECURSE_TO_DEPTH")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(0);

    let mut child_summary: Option<String> = None;
    if depth < target_depth {
        if let Ok(self_bin) = std::env::current_exe() {
            let child_req = json!({
                "session_id": format!("{session_id}-child"),
                "agent": format!("{agent}.child"),
                "prompt": "recursive-child",
                "parent_session_id": session_id,
                "depth": depth + 1,
            });
            let body = serde_json::to_string(&child_req).unwrap_or_default();
            let mut child = match Command::new(&self_bin)
                .arg("run-agent")
                .env_remove("MOCK_RECURSE_TO_DEPTH")
                .env(
                    "MOCK_AGENT_SUMMARY",
                    format!("{agent}.child@depth{}", depth + 1),
                )
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .spawn()
            {
                Ok(c) => c,
                Err(error) => {
                    emit_error(&format!("recursive spawn failed: {error}"));
                    return;
                }
            };
            if let Some(mut stdin) = child.stdin.take() {
                let _ = stdin.write_all(body.as_bytes());
            }
            let out = match child.wait_with_output() {
                Ok(o) => o,
                Err(error) => {
                    emit_error(&format!("recursive wait failed: {error}"));
                    return;
                }
            };
            let stdout = String::from_utf8_lossy(&out.stdout);
            if let Ok(v) = serde_json::from_str::<Value>(stdout.trim()) {
                let s = v
                    .get("result")
                    .and_then(|r| r.get("summary"))
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                child_summary = Some(s);
            }
        }
    }

    let combined = match child_summary {
        Some(child) if !child.is_empty() => format!("{my_summary} | child=[{child}]"),
        _ => my_summary,
    };

    let body = json!({
        "ok": true,
        "session_id": session_id,
        "worker_id": "mock-worker",
        "agent": agent,
        "result": {
            "ok": true,
            "summary": combined,
            "depth": depth,
        },
    });
    let _ = writeln!(std::io::stdout(), "{body}");
}

fn emit_error(message: &str) {
    let body = json!({ "ok": false, "error": message });
    let _ = writeln!(std::io::stdout(), "{body}");
}
