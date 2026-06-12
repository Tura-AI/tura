use anyhow::{anyhow, Context, Result};
use serde_json::json;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use sysinfo::{Pid, System};
use tura_router::manager::ServiceManager;
use tura_router::models::{CallContext, WorkerSpec};

#[tokio::test]
async fn router_one_shot_fallback_business_flow_executes_raw_input_and_reuses_key() -> Result<()> {
    let temp = tempfile::tempdir().context("temp one-shot worker dir")?;
    let script = write_one_shot_worker_script(temp.path())?;
    let manager = ServiceManager::new();
    let spec = one_shot_spec(
        "runtime_worker:oneshot-session",
        &python_executable()?,
        &script,
        "ok",
    );

    let first = manager.ensure_worker(spec.clone()).await?;
    let reused = manager.ensure_worker(spec.clone()).await?;
    assert_eq!(
        first.worker_id, reused.worker_id,
        "one-shot fallback workers are still reused by router key"
    );
    assert_eq!(manager.count_workers_with_prefix("runtime_worker:"), 1);

    let response = manager
        .call_worker(
            &first.worker_id,
            CallContext::new(
                "runtime.run".to_string(),
                "/runtime".to_string(),
                json!({ "prompt": "run one-shot", "turn": 1 }),
            ),
        )
        .await?;

    assert_eq!(response["ok"], true);
    assert_eq!(response["mode"], "ok");
    assert_eq!(response["input"]["prompt"], "run one-shot");
    assert!(
        manager
            .stop_worker_by_key("runtime_worker:oneshot-session")
            .await
    );
    assert_eq!(manager.count_workers_with_prefix("runtime_worker:"), 0);
    Ok(())
}

#[tokio::test]
async fn router_one_shot_fallback_business_flow_reports_child_failure_and_cleans_key() -> Result<()>
{
    let temp = tempfile::tempdir().context("temp one-shot failure worker dir")?;
    let script = write_one_shot_worker_script(temp.path())?;
    let manager = ServiceManager::new();
    let spec = one_shot_spec(
        "runtime_worker:oneshot-failure",
        &python_executable()?,
        &script,
        "fail",
    );

    let handle = manager.ensure_worker(spec).await?;
    let error = manager
        .call_worker(
            &handle.worker_id,
            CallContext::new(
                "runtime.run".to_string(),
                "/runtime".to_string(),
                json!({ "prompt": "fail one-shot" }),
            ),
        )
        .await
        .expect_err("failing one-shot worker should report execution failure");

    assert!(
        error.to_string().contains("worker execution failed"),
        "unexpected one-shot failure error: {error:#}"
    );
    assert_eq!(manager.stop_workers_with_prefix("runtime_worker:").await, 1);
    assert_eq!(manager.count_workers_with_prefix("runtime_worker:"), 0);
    Ok(())
}

#[tokio::test]
async fn router_one_shot_fallback_business_flow_restarts_child_for_each_invocation() -> Result<()> {
    let temp = tempfile::tempdir().context("temp one-shot restart worker dir")?;
    let script = write_one_shot_worker_script(temp.path())?;
    let counter = temp.path().join("invocation-count.txt");
    let manager = ServiceManager::new();
    let spec = one_shot_spec_with_env(
        "runtime_worker:oneshot-restart",
        &python_executable()?,
        &script,
        "ok",
        vec![(
            "TURA_TEST_ONESHOT_COUNTER".to_string(),
            counter.to_string_lossy().to_string(),
        )],
    );

    let handle = manager.ensure_worker(spec.clone()).await?;
    for turn in 1..=3 {
        let reused = manager.ensure_worker(spec.clone()).await?;
        assert_eq!(
            reused.worker_id, handle.worker_id,
            "router key should continue reusing the one-shot worker handle"
        );
        let response = manager
            .call_worker(
                &handle.worker_id,
                CallContext::new(
                    "runtime.run".to_string(),
                    format!("/runtime/oneshot/restart/{turn}"),
                    json!({ "prompt": "one-shot restart", "turn": turn }),
                ),
            )
            .await?;
        assert_eq!(response["ok"], true);
        assert_eq!(response["mode"], "ok");
        assert_eq!(response["invocation_index"], turn);
        assert_eq!(response["input"]["turn"], turn);
    }

    let final_count = std::fs::read_to_string(&counter)
        .with_context(|| format!("read one-shot counter {}", counter.display()))?;
    assert_eq!(final_count.trim(), "3");
    assert_eq!(manager.stop_workers_with_prefix("runtime_worker:").await, 1);
    assert_eq!(manager.count_workers_with_prefix("runtime_worker:"), 0);
    Ok(())
}

#[tokio::test]
async fn router_one_shot_fallback_business_flow_rejects_successful_child_with_invalid_json(
) -> Result<()> {
    let temp = tempfile::tempdir().context("temp one-shot invalid json worker dir")?;
    let script = write_one_shot_worker_script(temp.path())?;
    let manager = ServiceManager::new();
    let spec = one_shot_spec(
        "runtime_worker:oneshot-invalid-json",
        &python_executable()?,
        &script,
        "invalid-json",
    );

    let handle = manager.ensure_worker(spec).await?;
    let error = manager
        .call_worker(
            &handle.worker_id,
            CallContext::new(
                "runtime.run".to_string(),
                "/runtime/oneshot/invalid-json".to_string(),
                json!({ "prompt": "invalid json after success" }),
            ),
        )
        .await
        .expect_err("successful one-shot child with invalid JSON should be rejected");
    assert!(
        error
            .to_string()
            .contains("worker returned invalid response"),
        "invalid one-shot stdout should be mapped to a bounded router error: {error:#}"
    );
    assert_eq!(manager.stop_workers_with_prefix("runtime_worker:").await, 1);
    assert_eq!(manager.count_workers_with_prefix("runtime_worker:"), 0);
    assert!(!manager.stop_worker(&handle.worker_id).await);
    Ok(())
}

#[tokio::test]
async fn router_one_shot_fallback_business_flow_handles_health_handshake_rejections() -> Result<()>
{
    let temp = tempfile::tempdir().context("temp one-shot health rejection worker dir")?;
    let script = write_one_shot_worker_script(temp.path())?;
    let python = python_executable()?;
    let manager = ServiceManager::new();

    for (key_suffix, mode, expected_health_reason) in [
        (
            "missing-ok",
            "health-missing-ok",
            "persistent health response omits the required ok flag",
        ),
        (
            "version-mismatch",
            "health-version-mismatch",
            "persistent health response advertises a different build",
        ),
    ] {
        let key = format!("runtime_worker:oneshot-health-{key_suffix}");
        let spec = one_shot_spec(&key, &python, &script, mode);
        let handle = manager.ensure_worker(spec.clone()).await.with_context(|| {
            format!("fallback should register a one-shot worker after {expected_health_reason}")
        })?;
        let reused = manager.ensure_worker(spec).await?;
        assert_eq!(
            reused.worker_id, handle.worker_id,
            "fallback worker handle should remain reusable after {expected_health_reason}"
        );
        let response = manager
            .call_worker(
                &handle.worker_id,
                CallContext::new(
                    "runtime.run".to_string(),
                    format!("/runtime/oneshot/health/{key_suffix}"),
                    json!({
                        "prompt": "health rejection fallback",
                        "case": key_suffix
                    }),
                ),
            )
            .await?;
        assert_eq!(response["ok"], true);
        assert_eq!(response["mode"], mode);
        assert_eq!(response["input"]["case"], key_suffix);
        assert!(
            manager.stop_worker_by_key(&key).await,
            "health rejection fallback worker should be removable by key"
        );
        assert!(!manager.stop_worker(&handle.worker_id).await);
    }

    assert_eq!(manager.count_workers_with_prefix("runtime_worker:"), 0);
    Ok(())
}

#[tokio::test]
async fn router_one_shot_fallback_business_flow_timeout_kills_spawned_child_tree() -> Result<()> {
    let temp = tempfile::tempdir().context("temp one-shot timeout worker dir")?;
    let script = write_one_shot_worker_script(temp.path())?;
    let child_pid_file = temp.path().join("timeout-child.pid");
    let manager = ServiceManager::new();
    let spec = one_shot_spec_with_env(
        "runtime_worker:oneshot-timeout-tree",
        &python_executable()?,
        &script,
        "timeout-tree",
        vec![
            (
                "TURA_WORKER_INVOKE_TIMEOUT_SECS".to_string(),
                "1".to_string(),
            ),
            (
                "TURA_TEST_CHILD_PID_FILE".to_string(),
                child_pid_file.to_string_lossy().to_string(),
            ),
        ],
    );

    let handle = manager.ensure_worker(spec).await?;
    let error = manager
        .call_worker(
            &handle.worker_id,
            CallContext::new(
                "runtime.run".to_string(),
                "/runtime/oneshot/timeout-tree".to_string(),
                json!({ "prompt": "timeout and cleanup child tree" }),
            ),
        )
        .await
        .expect_err("slow one-shot worker should time out");
    assert!(
        error.to_string().contains("one-shot worker timed out"),
        "unexpected timeout error: {error:#}"
    );

    let child_pid = read_pid_file(&child_pid_file)?;
    wait_for_process_dead(child_pid, Duration::from_secs(10))
        .with_context(|| format!("one-shot timeout child pid {child_pid} should be killed"))?;
    assert_eq!(manager.stop_workers_with_prefix("runtime_worker:").await, 1);
    assert_eq!(manager.count_workers_with_prefix("runtime_worker:"), 0);
    Ok(())
}

fn one_shot_spec(key: &str, python: &Path, script: &Path, mode: &str) -> WorkerSpec {
    one_shot_spec_with_env(key, python, script, mode, Vec::new())
}

fn one_shot_spec_with_env(
    key: &str,
    python: &Path,
    script: &Path,
    mode: &str,
    extra_env: Vec<(String, String)>,
) -> WorkerSpec {
    let mut env = vec![("TURA_TEST_ONESHOT_MODE".to_string(), mode.to_string())];
    env.extend(extra_env);
    WorkerSpec {
        key: key.to_string(),
        service_name: "runtime".to_string(),
        executable: python.to_path_buf(),
        args: vec![script.to_string_lossy().to_string()],
        env,
    }
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
        "python or python3 is required for router one-shot business flow"
    ))
}

fn write_one_shot_worker_script(dir: &Path) -> Result<PathBuf> {
    let script = dir.join("router_worker_oneshot_business.py");
    std::fs::write(
        &script,
        r#"
import json
import os
import subprocess
import sys
import time

chunks = []
while True:
    char = sys.stdin.read(1)
    if char == "":
        break
    chunks.append(char)
    if char == "\n":
        break
raw = "".join(chunks).strip()
message = json.loads(raw) if raw else {}

if message.get("kind") == "health_check":
    mode = os.environ.get("TURA_TEST_ONESHOT_MODE", "ok")
    if mode == "health-missing-ok":
        print(json.dumps({"status": "ready"}), flush=True)
        sys.exit(0)
    if mode == "health-version-mismatch":
        print(json.dumps({"ok": True, "version": "not-the-router-build"}), flush=True)
        sys.exit(0)
    print("health refused for one-shot fallback", file=sys.stderr, flush=True)
    sys.exit(3)

mode = os.environ.get("TURA_TEST_ONESHOT_MODE", "ok")
if mode == "fail":
    print("intentional one-shot failure", file=sys.stderr, flush=True)
    sys.exit(9)
if mode == "invalid-json":
    print("{ still not json", flush=True)
    sys.exit(0)
if mode == "timeout-tree":
    child_pid_file = os.environ["TURA_TEST_CHILD_PID_FILE"]
    child = subprocess.Popen(
        [sys.executable, "-c", "import time; time.sleep(30)"],
        stdin=subprocess.DEVNULL,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    with open(child_pid_file, "w", encoding="utf-8") as writer:
        writer.write(str(child.pid))
        writer.flush()
    time.sleep(30)

counter_path = os.environ.get("TURA_TEST_ONESHOT_COUNTER")
invocation_index = None
if counter_path:
    try:
        with open(counter_path, "r", encoding="utf-8") as reader:
            current = int((reader.read() or "0").strip() or "0")
    except FileNotFoundError:
        current = 0
    invocation_index = current + 1
    with open(counter_path, "w", encoding="utf-8") as writer:
        writer.write(str(invocation_index))

print(json.dumps({
    "ok": True,
    "mode": mode,
    "input": message,
    "invocation_index": invocation_index,
}), flush=True)
"#,
    )
    .with_context(|| format!("write one-shot worker script {}", script.display()))?;
    Ok(script)
}

fn read_pid_file(path: &Path) -> Result<u32> {
    let raw = std::fs::read_to_string(path)
        .with_context(|| format!("read child pid file {}", path.display()))?;
    raw.trim()
        .parse::<u32>()
        .with_context(|| format!("parse child pid from {}", path.display()))
}

fn wait_for_process_dead(pid: u32, timeout: Duration) -> Result<()> {
    let started = Instant::now();
    while started.elapsed() < timeout {
        if !process_alive(pid) {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    Err(anyhow!(
        "pid {pid} was still alive after {}ms",
        timeout.as_millis()
    ))
}

fn process_alive(pid: u32) -> bool {
    let mut system = System::new_all();
    system.refresh_processes();
    system.process(Pid::from_u32(pid)).is_some()
}
