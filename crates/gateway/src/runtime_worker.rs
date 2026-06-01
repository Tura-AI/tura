//! runtime worker 角色入口（单二进制角色分发）。
//!
//! 由 router 以 `TURA_ROLE=runtime_worker` 拉起本二进制承载 runtime。
//! 与 router 的 `WorkerProcess` 持久化协议对齐：逐行读取 `{ "kind", "payload" }`，
//! 逐行回写 JSON（含 `ok` 标志）。
//!
//! 边界：runtime 仍是库；本入口仅负责 worker 进程承载 + 据 router 下发的 agent spec
//! 激活实体并执行一次 prompt，事件经现有 gateway 回调通道回报父 session。

use std::io::{BufRead, Write};
use std::path::PathBuf;

use code_tools_suite::state_machine::session_management::SessionInput;
use serde_json::{json, Value};

/// 进入 runtime worker 循环：阻塞读 stdin、写 stdout，直到对端关闭。
pub fn run() -> std::io::Result<()> {
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    let mut line = String::new();
    let mut reader = stdin.lock();

    loop {
        line.clear();
        let read = reader.read_line(&mut line)?;
        if read == 0 {
            return Ok(());
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let response = match serde_json::from_str::<Value>(trimmed) {
            Ok(envelope) => handle_envelope(&envelope),
            Err(error) => json!({ "ok": false, "error": format!("invalid envelope: {error}") }),
        };

        let encoded = serde_json::to_string(&response)
            .unwrap_or_else(|error| format!("{{\"ok\":false,\"error\":\"{error}\"}}"));
        stdout.write_all(encoded.as_bytes())?;
        stdout.write_all(b"\n")?;
        stdout.flush()?;
    }
}

fn handle_envelope(envelope: &Value) -> Value {
    match envelope.get("kind").and_then(Value::as_str) {
        Some("health_check") => json!({ "ok": true, "role": "runtime_worker" }),
        Some("call") => handle_call(envelope.get("payload").unwrap_or(&Value::Null)),
        Some(other) => json!({ "ok": false, "error": format!("unsupported kind: {other}") }),
        None => json!({ "ok": false, "error": "missing kind" }),
    }
}

/// call payload 形如 `{ "input": { "method", "input": <RuntimeWorkerCall> } }`
/// （对齐 router `invoke_persistent` 的封装）。
fn handle_call(payload: &Value) -> Value {
    let call = payload
        .get("input")
        .and_then(|value| value.get("input"))
        .cloned()
        .unwrap_or_else(|| payload.clone());

    let session_id = call
        .get("session_id")
        .and_then(Value::as_str)
        .map(str::to_string)
        .unwrap_or_else(|| format!("worker-{}", uuid::Uuid::new_v4()));
    let directory = call
        .get("directory")
        .and_then(Value::as_str)
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    let prompt = call
        .get("prompt")
        .or_else(|| call.get("message"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .unwrap_or_default();
    let agent = call
        .get("agent")
        .and_then(Value::as_str)
        .map(str::to_string);
    let runtime_context = call
        .get("runtime_context")
        .and_then(Value::as_str)
        .map(str::to_string);
    let agent_spec = call.get("agent_spec").cloned();

    if prompt.trim().is_empty() {
        return json!({ "ok": false, "session_id": session_id, "error": "empty prompt" });
    }

    let input = SessionInput {
        user_input: prompt,
        file_input: Vec::new(),
        agent,
        runtime_context,
    };

    if let Some(agent_spec) = agent_spec {
        std::env::set_var("TURA_ROUTER_AGENT_SPEC", agent_spec.to_string());
    } else {
        std::env::remove_var("TURA_ROUTER_AGENT_SPEC");
    }
    match code_tools_suite::mano::process_from_gateway_session_in_directory(
        session_id.clone(),
        input,
        directory,
    ) {
        Ok(_) => json!({ "ok": true, "session_id": session_id }),
        Err(error) => json!({ "ok": false, "session_id": session_id, "error": error }),
    }
}
