//! Child sub-session dispatch: the runtime spawns child agents by invoking the
//! **router CLI subprocess** (`tura_router run-agent`) — never over URL/HTTP.
//! Writes a `RunAgentRequest` JSON to stdin, reads the result JSON from stdout.
//!
//! Binding rule: internal runtime↔router communication is always CLI; router
//! and gateway live in the same process; all runtimes are subprocesses. No
//! internal URL communication may be introduced here.

use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use serde_json::{json, Value};

/// Request to dispatch a single child agent.
pub struct ChildAgentRequest {
    pub agent: String,
    pub prompt: String,
    pub directory: Option<PathBuf>,
    pub parent_session_id: String,
    pub depth: usize,
}

/// Result of a child-agent dispatch (normalized summary).
pub struct ChildAgentSummary {
    pub agent: String,
    pub session_id: String,
    pub ok: bool,
    pub summary: String,
    pub raw: Value,
}

/// Per-process unique suffix (nanos timestamp + monotonic counter); avoids pulling in a uuid dependency.
fn unique_suffix() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let seq = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{nanos:x}{seq:x}")
}

/// Resolve the router binary: prefer `TURA_ROUTER_BIN`, otherwise probe repo `target/{release,debug}`.
fn resolve_router_binary() -> Result<PathBuf, String> {
    if let Ok(explicit) = std::env::var("TURA_ROUTER_BIN") {
        let path = PathBuf::from(explicit);
        if path.exists() {
            return Ok(path);
        }
    }

    let exe_name = if cfg!(windows) {
        "tura_router.exe"
    } else {
        "tura_router"
    };

    let mut roots: Vec<PathBuf> = Vec::new();
    if let Ok(root) = std::env::var("TURA_PROJECT_ROOT") {
        roots.push(PathBuf::from(root));
    }
    if let Ok(current) = std::env::current_dir() {
        roots.push(current.clone());
        // Walk parents to find a repo root that contains `target/`.
        let mut cursor = current;
        while let Some(parent) = cursor.parent() {
            roots.push(parent.to_path_buf());
            cursor = parent.to_path_buf();
        }
    }
    // Sibling of the current exe (worker and router share the same target dir).
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let candidate = dir.join(exe_name);
            if candidate.exists() {
                return Ok(candidate);
            }
        }
    }

    for root in roots {
        for profile in ["release", "debug"] {
            let candidate = root.join("target").join(profile).join(exe_name);
            if candidate.exists() {
                return Ok(candidate);
            }
        }
    }

    Err(format!(
        "router binary `{exe_name}` not found (set TURA_ROUTER_BIN or build target/{{release,debug}})"
    ))
}

/// Dispatch a single child agent through a router CLI subprocess; blocks on the result JSON.
pub fn dispatch_child_agent(req: &ChildAgentRequest) -> Result<ChildAgentSummary, String> {
    let router_bin = resolve_router_binary()?;

    let payload = json!({
        "session_id": format!("{}-child-{}", req.parent_session_id, unique_suffix()),
        "agent": req.agent,
        "prompt": req.prompt,
        "directory": req.directory.as_ref().map(|d| d.to_string_lossy().to_string()),
        "parent_session_id": req.parent_session_id,
        "depth": req.depth,
    });
    let body = serde_json::to_string(&payload)
        .map_err(|error| format!("failed to encode child request: {error}"))?;

    let mut command = Command::new(&router_bin);
    command
        .arg("run-agent")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    hide_child_window(&mut command);
    let mut child = command
        .spawn()
        .map_err(|error| format!("failed to spawn router CLI {router_bin:?}: {error}"))?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(body.as_bytes())
            .map_err(|error| format!("failed to write child request: {error}"))?;
    }

    let output = child
        .wait_with_output()
        .map_err(|error| format!("router CLI wait failed: {error}"))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let raw: Value = serde_json::from_str(stdout.trim())
        .map_err(|error| format!("router CLI returned invalid json: {error}; raw={stdout}"))?;

    let ok = raw.get("ok").and_then(Value::as_bool).unwrap_or(false);
    let session_id = raw
        .get("session_id")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let summary = summarize_child_result(&raw);

    Ok(ChildAgentSummary {
        agent: req.agent.clone(),
        session_id,
        ok,
        summary,
        raw,
    })
}

fn hide_child_window(_command: &mut Command) {
    #[cfg(windows)]
    {
        #[allow(unused_imports)]
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        _command.creation_flags(CREATE_NO_WINDOW);
    }
}

/// Dispatch N child agents concurrently (each its own router CLI subprocess); returns once all summaries are collected.
pub fn dispatch_child_agents_concurrent(
    requests: Vec<ChildAgentRequest>,
) -> Vec<Result<ChildAgentSummary, String>> {
    let handles: Vec<_> = requests
        .into_iter()
        .map(|req| std::thread::spawn(move || dispatch_child_agent(&req)))
        .collect();

    handles
        .into_iter()
        .map(|handle| {
            handle
                .join()
                .unwrap_or_else(|_| Err("child dispatch thread panicked".to_string()))
        })
        .collect()
}

/// Extract a readable summary from the router result (result.message / output_text / final text).
fn summarize_child_result(raw: &Value) -> String {
    let result = raw.get("result").unwrap_or(raw);
    for key in ["summary", "message", "output_text", "final_text", "text"] {
        if let Some(text) = result.get(key).and_then(Value::as_str) {
            if !text.trim().is_empty() {
                return text.trim().to_string();
            }
        }
    }
    if let Some(error) = raw.get("error").and_then(Value::as_str) {
        return format!("error: {error}");
    }
    result.to_string()
}
