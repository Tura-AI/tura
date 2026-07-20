use anyhow::{anyhow, Context, Result};
use runtime_contract::CallContext;
use serde_json::json;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::time::Duration;
use sysinfo::{Pid, System};
use tokio::sync::Barrier;
use tura_router::manager::ServiceManager;
use tura_router::models::WorkerSpec;
use tura_router::runtime_orphans::cleanup_orphan_runtime_workers;

#[tokio::test]
async fn router_worker_business_flow_reuses_replaces_and_cleans_up_processes() -> Result<()> {
    let temp = tempfile::tempdir().context("temp worker dir")?;
    let script = write_worker_script(temp.path())?;
    let python = python_executable()?;
    let manager = ServiceManager::new();

    let spec = WorkerSpec {
        key: "runtime_worker:business-session".to_string(),
        service_name: "runtime".to_string(),
        executable: python.clone(),
        args: vec![script.to_string_lossy().to_string()],
        env: vec![("TURA_TEST_WORKER_KIND".to_string(), "business".to_string())],
    };

    let first = manager.ensure_worker(spec.clone()).await?;
    let reused = manager.ensure_worker(spec.clone()).await?;
    assert_eq!(
        first.worker_id, reused.worker_id,
        "healthy workers must be reused by key"
    );
    assert_eq!(manager.count_workers_with_prefix("runtime_worker:"), 1);

    let first_response = manager
        .call_worker(
            &first.worker_id,
            CallContext::new(
                "runtime.run".to_string(),
                "/runtime".to_string(),
                json!({ "prompt": "hello", "seq": 1 }),
            ),
        )
        .await?;
    assert_eq!(first_response["ok"], true);
    assert_eq!(first_response["method"], "runtime.run");
    assert!(
        first_response["request_id"]
            .as_str()
            .is_some_and(|value| !value.trim().is_empty()),
        "router should forward a generated request id to the worker"
    );
    assert_eq!(first_response["path"], "/runtime");
    assert_eq!(first_response["input"]["prompt"], "hello");
    assert_eq!(first_response["worker_kind"], "business");

    let manager = Arc::new(manager);
    let mut concurrent_calls = Vec::new();
    for seq in 0..8 {
        let manager = Arc::clone(&manager);
        let worker_id = first.worker_id.clone();
        concurrent_calls.push(tokio::spawn(async move {
            let prompt = format!("parallel-{seq}");
            let response = manager
                .call_worker(
                    &worker_id,
                    CallContext {
                        request_id: format!("parallel-request-{seq}"),
                        method: "runtime.run".to_string(),
                        path: format!("/runtime/session/{seq}"),
                        input: json!({ "prompt": prompt, "seq": seq }),
                    },
                )
                .await?;
            Ok::<_, anyhow::Error>((seq, response))
        }));
    }

    let mut seen = Vec::new();
    for call in concurrent_calls {
        let (seq, response) = call.await??;
        assert_eq!(response["ok"], true);
        assert_eq!(response["method"], "runtime.run");
        assert_eq!(response["request_id"], format!("parallel-request-{seq}"));
        assert_eq!(response["path"], format!("/runtime/session/{seq}"));
        assert_eq!(response["input"]["seq"], seq);
        assert_eq!(response["input"]["prompt"], format!("parallel-{seq}"));
        assert_eq!(response["worker_kind"], "business");
        seen.push(seq);
    }
    seen.sort_unstable();
    assert_eq!(seen, (0..8).collect::<Vec<_>>());

    let exit_error = manager
        .call_worker(
            &first.worker_id,
            CallContext::new(
                "runtime.run".to_string(),
                "/runtime".to_string(),
                json!({ "exit_after_read": true }),
            ),
        )
        .await
        .expect_err("worker exits before responding");
    assert!(
        exit_error.to_string().contains("empty response")
            || exit_error.to_string().contains("broken pipe")
            || exit_error
                .to_string()
                .contains("closed stdout before response")
            || exit_error.to_string().contains("reset"),
        "unexpected worker exit error: {exit_error:#}"
    );
    tokio::time::sleep(Duration::from_millis(250)).await;

    let replacement = manager.ensure_worker(spec.clone()).await?;
    assert_ne!(
        replacement.worker_id, first.worker_id,
        "dead worker must be replaced for the same key"
    );
    assert_eq!(manager.count_workers_with_prefix("runtime_worker:"), 1);

    let other = manager
        .ensure_worker(WorkerSpec {
            key: "runtime_worker:other-session".to_string(),
            service_name: "runtime".to_string(),
            executable: python,
            args: vec![script.to_string_lossy().to_string()],
            env: vec![("TURA_TEST_WORKER_KIND".to_string(), "other".to_string())],
        })
        .await?;
    assert_ne!(other.worker_id, replacement.worker_id);
    assert_eq!(manager.count_workers_with_prefix("runtime_worker:"), 2);

    assert_eq!(manager.stop_workers_with_prefix("runtime_worker:").await, 2);
    assert_eq!(manager.count_workers_with_prefix("runtime_worker:"), 0);
    assert!(!manager.stop_worker(&replacement.worker_id).await);
    Ok(())
}

#[tokio::test]
async fn router_worker_business_flow_concurrent_ensure_keeps_single_owner_per_key() -> Result<()> {
    let temp = tempfile::tempdir().context("temp concurrent worker dir")?;
    let script = write_worker_script(temp.path())?;
    let python = python_executable()?;
    let manager = Arc::new(ServiceManager::new());
    let spec = WorkerSpec {
        key: "runtime_worker:concurrent-ensure".to_string(),
        service_name: "runtime".to_string(),
        executable: python,
        args: vec![script.to_string_lossy().to_string()],
        env: vec![(
            "TURA_TEST_WORKER_KIND".to_string(),
            "concurrent-owner".to_string(),
        )],
    };

    let callers = 16;
    let barrier = Arc::new(Barrier::new(callers));
    let mut tasks = Vec::new();
    for _ in 0..callers {
        let manager = Arc::clone(&manager);
        let spec = spec.clone();
        let barrier = Arc::clone(&barrier);
        tasks.push(tokio::spawn(async move {
            barrier.wait().await;
            manager
                .ensure_worker(spec)
                .await
                .map(|handle| handle.worker_id)
        }));
    }

    let mut worker_ids = Vec::new();
    for task in tasks {
        worker_ids.push(task.await??);
    }
    worker_ids.sort();
    worker_ids.dedup();
    assert_eq!(
        worker_ids.len(),
        1,
        "concurrent ensure_worker calls for one key must converge on one worker"
    );
    assert_eq!(manager.count_workers_with_prefix("runtime_worker:"), 1);

    let response = manager
        .call_worker(
            &worker_ids[0],
            CallContext::new(
                "runtime.run".to_string(),
                "/runtime/concurrent".to_string(),
                json!({ "prompt": "after concurrent ensure", "seq": 99 }),
            ),
        )
        .await?;
    assert_eq!(response["ok"], true);
    assert_eq!(response["worker_kind"], "concurrent-owner");
    assert_eq!(response["input"]["seq"], 99);

    assert_eq!(manager.stop_workers_with_prefix("runtime_worker:").await, 1);
    assert_eq!(manager.count_workers_with_prefix("runtime_worker:"), 0);
    Ok(())
}

#[tokio::test]
async fn router_worker_business_flow_skips_non_protocol_stdout_noise_before_response() -> Result<()>
{
    let temp = tempfile::tempdir().context("temp noisy worker dir")?;
    let script = write_worker_script(temp.path())?;
    let python = python_executable()?;
    let manager = ServiceManager::new();
    let spec = WorkerSpec {
        key: "runtime_worker:noisy-stdout".to_string(),
        service_name: "runtime".to_string(),
        executable: python,
        args: vec![script.to_string_lossy().to_string()],
        env: vec![("TURA_TEST_WORKER_KIND".to_string(), "noisy".to_string())],
    };

    let worker = manager.ensure_worker(spec).await?;
    let response = manager
        .call_worker(
            &worker.worker_id,
            CallContext::new(
                "runtime.run".to_string(),
                "/runtime/noisy".to_string(),
                json!({ "prompt": "skip stdout noise", "noisy_stdout": true }),
            ),
        )
        .await?;

    assert_eq!(response["ok"], true);
    assert_eq!(response["worker_kind"], "noisy");
    assert_eq!(response["input"]["prompt"], "skip stdout noise");
    assert_eq!(manager.stop_workers_with_prefix("runtime_worker:").await, 1);
    Ok(())
}

#[tokio::test]
async fn router_worker_business_flow_stop_by_key_only_cleans_target_session_and_allows_recreate(
) -> Result<()> {
    let temp = tempfile::tempdir().context("temp stop-by-key worker dir")?;
    let script = write_worker_script(temp.path())?;
    let python = python_executable()?;
    let manager = ServiceManager::new();

    let target_spec = WorkerSpec {
        key: "runtime_worker:target-session".to_string(),
        service_name: "runtime".to_string(),
        executable: python.clone(),
        args: vec![script.to_string_lossy().to_string()],
        env: vec![("TURA_TEST_WORKER_KIND".to_string(), "target".to_string())],
    };
    let survivor_spec = WorkerSpec {
        key: "runtime_worker:survivor-session".to_string(),
        service_name: "runtime".to_string(),
        executable: python,
        args: vec![script.to_string_lossy().to_string()],
        env: vec![("TURA_TEST_WORKER_KIND".to_string(), "survivor".to_string())],
    };

    let target = manager.ensure_worker(target_spec.clone()).await?;
    let survivor = manager.ensure_worker(survivor_spec.clone()).await?;
    assert_ne!(target.worker_id, survivor.worker_id);
    assert_eq!(manager.count_workers_with_prefix("runtime_worker:"), 2);

    assert!(
        manager
            .stop_worker_by_key("runtime_worker:target-session")
            .await,
        "target key should remove exactly the target worker"
    );
    assert_eq!(manager.count_workers_with_prefix("runtime_worker:"), 1);
    assert!(
        !manager.stop_worker(&target.worker_id).await,
        "target worker id should no longer be registered after key cleanup"
    );

    let survivor_response = manager
        .call_worker(
            &survivor.worker_id,
            CallContext {
                request_id: "survivor-after-target-stop".to_string(),
                method: "runtime.run".to_string(),
                path: "/runtime/survivor".to_string(),
                input: json!({ "prompt": "survivor still alive", "seq": 42 }),
            },
        )
        .await?;
    assert_eq!(survivor_response["ok"], true);
    assert_eq!(survivor_response["worker_kind"], "survivor");
    assert_eq!(
        survivor_response["request_id"],
        "survivor-after-target-stop"
    );
    assert_eq!(survivor_response["input"]["seq"], 42);

    let recreated = manager.ensure_worker(target_spec).await?;
    assert_ne!(
        recreated.worker_id, target.worker_id,
        "ensuring the stopped key should create a fresh target worker"
    );
    assert_ne!(recreated.worker_id, survivor.worker_id);
    assert_eq!(manager.count_workers_with_prefix("runtime_worker:"), 2);

    let recreated_response = manager
        .call_worker(
            &recreated.worker_id,
            CallContext::new(
                "runtime.run".to_string(),
                "/runtime/recreated".to_string(),
                json!({ "prompt": "recreated target", "seq": 7 }),
            ),
        )
        .await?;
    assert_eq!(recreated_response["ok"], true);
    assert_eq!(recreated_response["worker_kind"], "target");
    assert_eq!(recreated_response["path"], "/runtime/recreated");
    assert_eq!(recreated_response["input"]["seq"], 7);

    assert_eq!(manager.stop_workers_with_prefix("runtime_worker:").await, 2);
    assert_eq!(manager.count_workers_with_prefix("runtime_worker:"), 0);
    Ok(())
}

#[tokio::test]
async fn router_worker_business_flow_stale_worker_id_fails_cleanly_after_key_stop_and_recreate(
) -> Result<()> {
    let temp = tempfile::tempdir().context("temp stale worker id dir")?;
    let script = write_worker_script(temp.path())?;
    let python = python_executable()?;
    let manager = ServiceManager::new();
    let spec = WorkerSpec {
        key: "runtime_worker:stale-id-session".to_string(),
        service_name: "runtime".to_string(),
        executable: python,
        args: vec![script.to_string_lossy().to_string()],
        env: vec![("TURA_TEST_WORKER_KIND".to_string(), "stale-id".to_string())],
    };

    let first = manager.ensure_worker(spec.clone()).await?;
    let first_response = manager
        .call_worker(
            &first.worker_id,
            CallContext::new(
                "runtime.run".to_string(),
                "/runtime/stale/first".to_string(),
                json!({ "prompt": "before stale id", "seq": 1 }),
            ),
        )
        .await?;
    assert_eq!(first_response["ok"], true);
    assert_eq!(first_response["worker_kind"], "stale-id");

    assert!(
        manager
            .stop_worker_by_key("runtime_worker:stale-id-session")
            .await,
        "key stop should remove the registered worker"
    );
    assert_eq!(manager.count_workers_with_prefix("runtime_worker:"), 0);
    let stale_error = manager
        .call_worker(
            &first.worker_id,
            CallContext::new(
                "runtime.run".to_string(),
                "/runtime/stale/old-id".to_string(),
                json!({ "prompt": "must not reach stopped worker", "seq": 2 }),
            ),
        )
        .await
        .expect_err("stale worker id should not be callable after key cleanup");
    assert!(
        stale_error
            .to_string()
            .contains(&format!("worker not found: {}", first.worker_id)),
        "stale worker id error should name the missing worker: {stale_error:#}"
    );

    let recreated = manager.ensure_worker(spec).await?;
    assert_ne!(
        recreated.worker_id, first.worker_id,
        "recreated key should get a fresh worker id after stale id cleanup"
    );
    assert_eq!(manager.count_workers_with_prefix("runtime_worker:"), 1);
    let recreated_response = manager
        .call_worker(
            &recreated.worker_id,
            CallContext::new(
                "runtime.run".to_string(),
                "/runtime/stale/recreated".to_string(),
                json!({ "prompt": "after stale id cleanup", "seq": 3 }),
            ),
        )
        .await?;
    assert_eq!(recreated_response["ok"], true);
    assert_eq!(recreated_response["path"], "/runtime/stale/recreated");
    assert_eq!(recreated_response["input"]["seq"], 3);
    assert_eq!(recreated_response["worker_kind"], "stale-id");

    assert_eq!(manager.stop_workers_with_prefix("runtime_worker:").await, 1);
    assert_eq!(manager.count_workers_with_prefix("runtime_worker:"), 0);
    Ok(())
}

#[tokio::test]
async fn router_worker_business_flow_stop_by_key_interrupts_slow_worker_without_registry_leak(
) -> Result<()> {
    let temp = tempfile::tempdir().context("temp slow stop worker dir")?;
    let script = write_worker_script(temp.path())?;
    let python = python_executable()?;
    let manager = Arc::new(ServiceManager::new());
    let spec = WorkerSpec {
        key: "runtime_worker:slow-stop-session".to_string(),
        service_name: "runtime".to_string(),
        executable: python,
        args: vec![script.to_string_lossy().to_string()],
        env: vec![("TURA_TEST_WORKER_KIND".to_string(), "slow-stop".to_string())],
    };

    let handle = manager.ensure_worker(spec.clone()).await?;
    let slow_call_manager = Arc::clone(&manager);
    let slow_worker_id = handle.worker_id.clone();
    let slow_call = tokio::spawn(async move {
        slow_call_manager
            .call_worker(
                &slow_worker_id,
                CallContext::new(
                    "runtime.run".to_string(),
                    "/runtime/slow-stop".to_string(),
                    json!({ "prompt": "slow call should be interrupted", "sleep_ms": 5000 }),
                ),
            )
            .await
    });

    tokio::time::sleep(Duration::from_millis(200)).await;
    let stopped = tokio::time::timeout(
        Duration::from_secs(3),
        manager.stop_worker_by_key("runtime_worker:slow-stop-session"),
    )
    .await
    .expect("stopping a slow worker should not hang");
    assert!(stopped, "slow worker should be removed by key");
    assert_eq!(
        manager.count_workers_with_prefix("runtime_worker:"),
        0,
        "registry should be cleared as soon as the slow worker is stopped"
    );

    let slow_error = slow_call
        .await
        .expect("slow call task should join")
        .expect_err("interrupted slow worker call should fail");
    assert_worker_interruption_error(&slow_error);

    let recreated = manager.ensure_worker(spec).await?;
    assert_ne!(
        recreated.worker_id, handle.worker_id,
        "slow worker key should recreate with a fresh worker id after cleanup"
    );
    let response = manager
        .call_worker(
            &recreated.worker_id,
            CallContext::new(
                "runtime.run".to_string(),
                "/runtime/slow-stop/recreated".to_string(),
                json!({ "prompt": "after slow stop", "seq": 11 }),
            ),
        )
        .await?;
    assert_eq!(response["ok"], true);
    assert_eq!(response["worker_kind"], "slow-stop");
    assert_eq!(response["path"], "/runtime/slow-stop/recreated");
    assert_eq!(response["input"]["seq"], 11);

    assert_eq!(manager.stop_workers_with_prefix("runtime_worker:").await, 1);
    assert_eq!(manager.count_workers_with_prefix("runtime_worker:"), 0);
    Ok(())
}

#[tokio::test]
async fn router_worker_business_flow_prefix_stop_interrupts_many_in_flight_workers_and_recovers(
) -> Result<()> {
    let temp = tempfile::tempdir().context("temp prefix stop worker dir")?;
    let script = write_worker_script(temp.path())?;
    let python = python_executable()?;
    let manager = Arc::new(ServiceManager::new());

    let prefix_specs = (0..4)
        .map(|index| WorkerSpec {
            key: format!("runtime_worker:prefix-stop-{index}"),
            service_name: "runtime".to_string(),
            executable: python.clone(),
            args: vec![script.to_string_lossy().to_string()],
            env: vec![(
                "TURA_TEST_WORKER_KIND".to_string(),
                format!("prefix-stop-{index}"),
            )],
        })
        .collect::<Vec<_>>();
    let survivor_spec = WorkerSpec {
        key: "other_worker:prefix-stop-survivor".to_string(),
        service_name: "runtime".to_string(),
        executable: python,
        args: vec![script.to_string_lossy().to_string()],
        env: vec![(
            "TURA_TEST_WORKER_KIND".to_string(),
            "prefix-stop-survivor".to_string(),
        )],
    };

    let mut prefix_handles = Vec::new();
    for spec in &prefix_specs {
        prefix_handles.push(manager.ensure_worker(spec.clone()).await?);
    }
    let survivor = manager.ensure_worker(survivor_spec.clone()).await?;
    assert_eq!(manager.count_workers_with_prefix("runtime_worker:"), 4);
    assert_eq!(manager.count_workers_with_prefix("other_worker:"), 1);

    let mut slow_calls = Vec::new();
    for (index, handle) in prefix_handles.iter().enumerate() {
        let manager = Arc::clone(&manager);
        let worker_id = handle.worker_id.clone();
        slow_calls.push(tokio::spawn(async move {
            manager
                .call_worker(
                    &worker_id,
                    CallContext::new(
                        "runtime.run".to_string(),
                        format!("/runtime/prefix-stop/{index}"),
                        json!({
                            "prompt": "prefix cleanup should interrupt this call",
                            "seq": index,
                            "sleep_ms": 5000
                        }),
                    ),
                )
                .await
        }));
    }

    tokio::time::sleep(Duration::from_millis(250)).await;
    let stopped = tokio::time::timeout(
        Duration::from_secs(5),
        manager.stop_workers_with_prefix("runtime_worker:"),
    )
    .await
    .expect("prefix stop should not hang behind slow worker calls");
    assert_eq!(stopped, 4);
    assert_eq!(
        manager.count_workers_with_prefix("runtime_worker:"),
        0,
        "runtime_worker registry entries must be gone before callers retry"
    );
    assert_eq!(
        manager.count_workers_with_prefix("other_worker:"),
        1,
        "prefix cleanup must not remove unrelated worker keys"
    );

    for slow_call in slow_calls {
        let slow_error = slow_call
            .await
            .expect("slow prefix call should join")
            .expect_err("prefix-stopped slow call should fail");
        assert_worker_interruption_error(&slow_error);
    }

    for handle in &prefix_handles {
        let stale_error = manager
            .call_worker(
                &handle.worker_id,
                CallContext::new(
                    "runtime.run".to_string(),
                    "/runtime/prefix-stop/stale".to_string(),
                    json!({ "prompt": "stale worker id after prefix stop" }),
                ),
            )
            .await
            .expect_err("stale ids removed by prefix stop must not dispatch");
        assert!(
            stale_error
                .to_string()
                .contains(&format!("worker not found: {}", handle.worker_id)),
            "stale prefix worker id error should name the missing worker: {stale_error:#}"
        );
    }

    let survivor_response = manager
        .call_worker(
            &survivor.worker_id,
            CallContext::new(
                "runtime.run".to_string(),
                "/runtime/prefix-stop/survivor".to_string(),
                json!({ "prompt": "survivor must remain available", "seq": 101 }),
            ),
        )
        .await?;
    assert_eq!(survivor_response["ok"], true);
    assert_eq!(survivor_response["worker_kind"], "prefix-stop-survivor");
    assert_eq!(survivor_response["input"]["seq"], 101);

    let recreated = manager.ensure_worker(prefix_specs[0].clone()).await?;
    assert!(
        prefix_handles
            .iter()
            .all(|old| old.worker_id != recreated.worker_id),
        "prefix-stopped key should restart with a fresh worker id"
    );
    let recreated_response = manager
        .call_worker(
            &recreated.worker_id,
            CallContext::new(
                "runtime.run".to_string(),
                "/runtime/prefix-stop/recreated".to_string(),
                json!({ "prompt": "recreated after prefix stop", "seq": 202 }),
            ),
        )
        .await?;
    assert_eq!(recreated_response["ok"], true);
    assert_eq!(recreated_response["worker_kind"], "prefix-stop-0");
    assert_eq!(recreated_response["input"]["seq"], 202);
    assert_eq!(manager.stop_workers_with_prefix("runtime_worker:").await, 1);
    assert_eq!(manager.stop_workers_with_prefix("other_worker:").await, 1);
    assert_eq!(manager.count_workers_with_prefix("runtime_worker:"), 0);
    assert_eq!(manager.count_workers_with_prefix("other_worker:"), 0);
    Ok(())
}

#[tokio::test]
async fn router_worker_business_flow_startup_cleanup_kills_orphan_runtime_processes() -> Result<()>
{
    let _guard = ENV_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let temp = tempfile::tempdir().context("temp orphan runtime dir")?;
    let home = temp.path().join("tura-home");
    std::fs::create_dir_all(&home).context("create orphan cleanup TURA_HOME")?;
    let previous_home = std::env::var_os("TURA_HOME");
    let previous_session_root = std::env::var_os("SESSION_LOG_DB_ROOT");
    let previous_tura_root = std::env::var_os("TURA_DB_ROOT");
    std::env::set_var("TURA_HOME", &home);
    std::env::remove_var("SESSION_LOG_DB_ROOT");
    std::env::remove_var("TURA_DB_ROOT");

    let mut child = ChildCleanup::new(
        Command::new(python_executable()?)
            .arg("-c")
            .arg("import time; time.sleep(60)")
            .env("TURA_RUNTIME_WORKER", "1")
            .env("TURA_ROLE", "runtime_worker")
            .env("TURA_HOME", &home)
            .env("TURA_ROUTER_PARENT_PID", "999999")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .context("spawn orphan runtime marker process")?,
    );
    let child_pid = child.id();
    wait_for_process_alive(child_pid, Duration::from_secs(5))
        .with_context(|| format!("orphan marker pid {child_pid} should start"))?;

    let report = cleanup_orphan_runtime_workers();
    wait_for_process_dead(child_pid, Duration::from_secs(10))
        .with_context(|| format!("orphan marker pid {child_pid} should be killed"))?;
    child.wait();

    restore_env("TURA_HOME", previous_home);
    restore_env("SESSION_LOG_DB_ROOT", previous_session_root);
    restore_env("TURA_DB_ROOT", previous_tura_root);

    assert!(
        report.killed.contains(&child_pid),
        "cleanup report should include the orphan runtime pid {child_pid}: {report:?}"
    );
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
        "python or python3 is required for router worker business flow"
    ))
}

fn write_worker_script(dir: &Path) -> Result<PathBuf> {
    let script = dir.join("router_worker_business.py");
    let source = r#"
import json
import os
import sys
import time

worker_kind = os.environ.get("TURA_TEST_WORKER_KIND", "")

for raw in sys.stdin:
    raw = raw.strip()
    if not raw:
        continue
    message = json.loads(raw)
    if message.get("kind") == "health_check":
        print(json.dumps({"ok": True, "version": "__TURA_WORKER_VERSION__"}), flush=True)
        continue
    payload = message.get("payload") or {}
    input_payload = (payload.get("input") or {})
    request_id = input_payload.get("request_id")
    method = input_payload.get("method")
    path = input_payload.get("path")
    value = input_payload.get("input") or {}
    if value.get("exit_after_read"):
        sys.exit(7)
    if value.get("never_respond"):
        time.sleep(30)
        continue
    if value.get("sleep_ms"):
        time.sleep(float(value.get("sleep_ms")) / 1000.0)
    if value.get("noisy_stdout"):
        print("worker debug log on stdout before protocol response", flush=True)
    print(json.dumps({
        "ok": True,
        "request_id": request_id,
        "method": method,
        "path": path,
        "input": value,
        "worker_kind": worker_kind,
    }), flush=True)
"#
    .replace("__TURA_WORKER_VERSION__", &tura_path::instance_version());
    std::fs::write(&script, source)
        .with_context(|| format!("write worker script {}", script.display()))?;
    Ok(script)
}

fn assert_worker_interruption_error(error: &anyhow::Error) {
    let message = error.to_string();
    assert!(
        message.contains("empty response")
            || message.contains("broken pipe")
            || message.contains("reset")
            || message.contains("closed stdout before response")
            || message.contains("cancelled"),
        "unexpected worker interruption error: {error:#}"
    );
}

static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

struct ChildCleanup {
    child: Option<Child>,
}

impl ChildCleanup {
    fn new(child: Child) -> Self {
        Self { child: Some(child) }
    }

    fn id(&self) -> u32 {
        self.child.as_ref().map(Child::id).unwrap_or(0)
    }

    fn wait(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.wait();
        }
    }
}

impl Drop for ChildCleanup {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

fn wait_for_process_alive(pid: u32, timeout: Duration) -> Result<()> {
    let started = std::time::Instant::now();
    while started.elapsed() < timeout {
        if process_alive(pid) {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    Err(anyhow!(
        "pid {pid} was not alive after {}ms",
        timeout.as_millis()
    ))
}

fn wait_for_process_dead(pid: u32, timeout: Duration) -> Result<()> {
    let started = std::time::Instant::now();
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
    system.process(Pid::from_u32(pid)).is_some_and(|process| {
        !matches!(
            process.status(),
            sysinfo::ProcessStatus::Zombie | sysinfo::ProcessStatus::Dead
        )
    })
}

fn restore_env(key: &str, previous: Option<std::ffi::OsString>) {
    if let Some(value) = previous {
        std::env::set_var(key, value);
    } else {
        std::env::remove_var(key);
    }
}
