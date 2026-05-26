use std::process::Stdio;
use std::time::Duration;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let router_exe = std::env::current_exe()?
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("release")
        .join("tura_router.exe");

    let router_exe = if router_exe.exists() {
        router_exe
    } else {
        std::path::PathBuf::from("target/release/tura_router.exe")
    };

    println!("[TEST] Starting router: {:?}", router_exe);
    let mut router = tokio::process::Command::new(&router_exe)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    tokio::time::sleep(Duration::from_secs(3)).await;

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()?;

    println!("[TEST] Checking router health...");
    let health_resp = client.get("http://127.0.0.1:8181/health").send().await?;
    println!("[TEST] Health check: {}", health_resp.status());
    let health: serde_json::Value = health_resp.json().await?;
    println!(
        "[TEST] Health response: {}",
        serde_json::to_string_pretty(&health)?
    );

    println!("\n[TEST] === Starting LSP service for TypeScript ===");
    let start_lsp_req = serde_json::json!({
        "services_dir": "C:/Users/liuliu/RustroverProjects/turaOSv2/services/lsp",
        "input": {
            "start_lsp": true,
            "start_checks": ["ts"],
            "session_path": "C:/Users/liuliu/RustroverProjects/turaOSv2/temp/lsp_session"
        }
    });

    let resp = client
        .post("http://127.0.0.1:8181/run_service")
        .json(&start_lsp_req)
        .timeout(Duration::from_secs(60))
        .send()
        .await?;

    println!("[TEST] LSP start response status: {}", resp.status());
    let body: serde_json::Value = resp
        .json()
        .await
        .unwrap_or_else(|_| serde_json::json!({"raw": "failed to parse response"}));
    println!(
        "[TEST] LSP Start Response: {}",
        serde_json::to_string_pretty(&body)?
    );

    let worker_id = body
        .get("worker_id")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");
    println!("\n[TEST] === Worker ID: {} ===", worker_id);

    println!("\n[TEST] === Testing LSP symbols endpoint directly ===");
    let symbols_req = serde_json::json!({
        "textDocument": {
            "uri": "file:///C:/Users/liuliu/RustroverProjects/turaOSv2/test_project/ts_test/user.ts"
        }
    });

    let resp = client
        .post(format!(
            "http://127.0.0.1:8181/lsp/{}/check/symbols",
            worker_id
        ))
        .json(&symbols_req)
        .timeout(Duration::from_secs(30))
        .send()
        .await?;

    println!("[TEST] Symbols response status: {}", resp.status());
    let symbols_body: serde_json::Value = resp
        .json()
        .await
        .unwrap_or_else(|_| serde_json::json!({"raw": "failed to parse response"}));
    println!(
        "[TEST] Symbols Response: {}",
        serde_json::to_string_pretty(&symbols_body)?
    );

    println!("\n[TEST] === Testing get_file_outline tool via router ===");
    let tool_req = serde_json::json!({
        "tool": "get_file_outline",
        "input": [
            {
                "path": "C:/Users/liuliu/RustroverProjects/turaOSv2/test_project/ts_test/user.ts"
            }
        ],
        "lsp_worker_id": worker_id,
        "lsp_language": "ts"
    });

    let resp = client
        .post("http://127.0.0.1:8181/run_tool")
        .json(&tool_req)
        .timeout(Duration::from_secs(30))
        .send()
        .await?;

    println!("[TEST] Tool response status: {}", resp.status());
    let tool_body: serde_json::Value = resp
        .json()
        .await
        .unwrap_or_else(|_| serde_json::json!({"raw": "failed to parse response"}));
    println!(
        "[TEST] Tool Response: {}",
        serde_json::to_string_pretty(&tool_body)?
    );

    println!("\n[TEST] === Killing router ===");
    router.kill().await.ok();

    println!("\n[TEST] === Test completed successfully! ===");
    Ok(())
}
