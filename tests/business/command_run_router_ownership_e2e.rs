use serde_json::{json, Value};
use std::sync::{Mutex, OnceLock};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;

static MOCK_ROUTER_ADDR: OnceLock<String> = OnceLock::new();
static MOCK_ROUTER_INIT: Mutex<()> = Mutex::new(());

#[tokio::test]
async fn command_run_without_router_addr_does_not_execute_in_runtime_process() {
    let previous = std::env::var("TURA_ROUTER_ADDR").ok();
    std::env::remove_var("TURA_ROUTER_ADDR");
    let workspace = tempfile::tempdir().expect("workspace");
    let marker = workspace.path().join("must-not-exist.txt");

    let output = runtime::router_command_run::execute_command_run_value_or_error(
        json!({
            "commands": [{
                "command": "shell_command",
                "command_line": json!({
                    "command": "echo wrong-owner > must-not-exist.txt",
                    "timeout_ms": 2000
                }).to_string()
            }]
        }),
        workspace.path().to_path_buf(),
        Some("session-no-router"),
        Some("runtime-no-router"),
        None,
    )
    .await;

    if let Some(value) = previous {
        std::env::set_var("TURA_ROUTER_ADDR", value);
    }
    assert_eq!(output["ok"], false);
    assert!(
        !marker.exists(),
        "runtime must not execute command_run locally when router ownership is unavailable"
    );
}

#[tokio::test]
async fn command_run_executes_only_after_runtime_hands_request_to_router() {
    let router_addr = ensure_mock_router();
    std::env::set_var("TURA_ROUTER_ADDR", router_addr);
    let workspace = tempfile::tempdir().expect("workspace");

    let output = runtime::router_command_run::execute_command_run_value_or_error(
        json!({
            "commands": [{
                "command": "shell_command",
                "command_line": json!({
                    "command": "echo router-owned > router-owned.txt",
                    "timeout_ms": 5000
                }).to_string()
            }]
        }),
        workspace.path().to_path_buf(),
        Some("session-router-owned"),
        Some("runtime-router-owned"),
        None,
    )
    .await;

    assert_eq!(output["results"][0]["command_type"], "shell_command");
    assert_eq!(output["results"][0]["success"], true);
    let text = std::fs::read_to_string(workspace.path().join("router-owned.txt"))
        .expect("router-owned command created workspace artifact");
    assert!(text.contains("router-owned"));
}

fn ensure_mock_router() -> String {
    if let Some(addr) = MOCK_ROUTER_ADDR.get() {
        return addr.clone();
    }
    let _guard = MOCK_ROUTER_INIT.lock().expect("mock router init lock");
    if let Some(addr) = MOCK_ROUTER_ADDR.get() {
        return addr.clone();
    }

    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind mock router");
    listener
        .set_nonblocking(true)
        .expect("mock router nonblocking");
    let addr = listener.local_addr().expect("mock router addr").to_string();
    std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("mock router runtime");
        runtime.block_on(async move {
            let listener = TcpListener::from_std(listener).expect("tokio listener");
            while let Ok((stream, _)) = listener.accept().await {
                tokio::spawn(async move {
                    let (read, mut write) = stream.into_split();
                    let mut reader = BufReader::new(read);
                    let mut line = String::new();
                    if reader.read_line(&mut line).await.unwrap_or(0) == 0 {
                        return;
                    }
                    let response = mock_router_response(&line).await;
                    let _ = write.write_all(format!("{response}\n").as_bytes()).await;
                    let _ = write.flush().await;
                });
            }
        });
    });
    MOCK_ROUTER_ADDR
        .set(addr.clone())
        .expect("mock router addr set once");
    addr
}

async fn mock_router_response(raw: &str) -> Value {
    let request: Value = match serde_json::from_str(raw.trim()) {
        Ok(request) => request,
        Err(error) => {
            return json!({
                "request_id": "invalid",
                "ok": false,
                "error": format!("invalid request: {error}")
            });
        }
    };
    let request_id = request
        .get("request_id")
        .and_then(Value::as_str)
        .unwrap_or("missing")
        .to_string();
    if request.get("method").and_then(Value::as_str) != Some("execution.command_run") {
        return json!({
            "request_id": request_id,
            "ok": false,
            "error": "unsupported mock router method"
        });
    }
    let payload = &request["payload"];
    let Some(session_directory) = payload.get("session_directory").and_then(Value::as_str) else {
        return json!({
            "request_id": request_id,
            "ok": false,
            "error": "session_directory missing"
        });
    };
    let output = code_tools::command_run::execute_async_value(
        payload
            .get("arguments")
            .cloned()
            .unwrap_or_else(|| json!({})),
        std::path::PathBuf::from(session_directory),
    )
    .await;
    json!({
        "request_id": request_id,
        "ok": true,
        "payload": {
            "status": "finished",
            "owner": "router",
            "result": output
        }
    })
}
