use anyhow::{anyhow, Context, Result};
use serde_json::json;
use std::path::{Path, PathBuf};
use tura_router::manager::ServiceManager;
use tura_router::models::{CallContext, WorkerSpec};

#[tokio::test]
async fn router_persistent_worker_business_flow_rejects_malformed_json_and_cleans_up() -> Result<()>
{
    let temp = tempfile::tempdir().context("temp malformed worker dir")?;
    let script = write_malformed_worker_script(temp.path())?;
    let manager = ServiceManager::new();
    let spec = WorkerSpec {
        key: "runtime_worker:malformed-json".to_string(),
        service_name: "runtime".to_string(),
        executable: python_executable()?,
        args: vec![script.to_string_lossy().to_string()],
        env: Vec::new(),
    };

    let handle = manager.ensure_worker(spec.clone()).await?;
    let reused = manager.ensure_worker(spec.clone()).await?;
    assert_eq!(
        handle.worker_id, reused.worker_id,
        "healthy malformed-output worker should still be reused before invocation failure"
    );
    assert_eq!(manager.count_workers_with_prefix("runtime_worker:"), 1);

    let error = manager
        .call_worker(
            &handle.worker_id,
            CallContext::new(
                "runtime.run".to_string(),
                "/runtime".to_string(),
                json!({ "prompt": "return malformed json" }),
            ),
        )
        .await
        .expect_err("persistent worker malformed JSON should be rejected");

    assert!(
        error
            .to_string()
            .contains("worker returned invalid response"),
        "malformed worker output should be mapped to a bounded router error: {error:#}"
    );
    assert_eq!(
        manager.count_workers_with_prefix("runtime_worker:"),
        0,
        "malformed response should stop and remove the worker before reuse"
    );
    assert_eq!(manager.count_workers_with_prefix("runtime_worker:"), 0);
    assert!(!manager.stop_worker(&handle.worker_id).await);
    let stale_error = manager
        .call_worker(
            &handle.worker_id,
            CallContext::new(
                "runtime.run".to_string(),
                "/runtime/stale-malformed".to_string(),
                json!({ "prompt": "old id" }),
            ),
        )
        .await
        .expect_err("malformed worker id should already be removed");
    assert!(
        stale_error.to_string().contains("worker not found"),
        "stale malformed worker id should fail cleanly: {stale_error:#}"
    );

    let restarted = manager.ensure_worker(spec).await?;
    assert_ne!(
        handle.worker_id, restarted.worker_id,
        "malformed worker should be replaced by a fresh process"
    );
    assert_eq!(manager.stop_workers_with_prefix("runtime_worker:").await, 1);
    assert_eq!(manager.count_workers_with_prefix("runtime_worker:"), 0);
    Ok(())
}

fn python_executable() -> Result<PathBuf> {
    if let Some(path) = std::env::var_os("PYTHON") {
        let path = PathBuf::from(path);
        if path.exists() {
            return Ok(path);
        }
    }
    for candidate in ["python", "python3"] {
        if let Ok(output) = std::process::Command::new(candidate)
            .arg("--version")
            .output()
        {
            if output.status.success() {
                return Ok(PathBuf::from(candidate));
            }
        }
    }
    Err(anyhow!(
        "python or python3 is required for router malformed output business flow"
    ))
}

fn write_malformed_worker_script(dir: &Path) -> Result<PathBuf> {
    let script = dir.join("router_worker_malformed_business.py");
    std::fs::write(
        &script,
        r#"
import json
import sys

for raw in sys.stdin:
    raw = raw.strip()
    if not raw:
        continue
    message = json.loads(raw)
    if message.get("kind") == "health_check":
        print(json.dumps({"ok": True}), flush=True)
        continue
    print("{ this is not valid json", flush=True)
"#,
    )
    .with_context(|| format!("write malformed worker script {}", script.display()))?;
    Ok(script)
}
