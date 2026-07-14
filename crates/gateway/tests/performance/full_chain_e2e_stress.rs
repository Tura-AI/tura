use std::env;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

#[test]
fn full_chain_e2e_stress_covers_gateway_router_runtime_session_db() {
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repo root should resolve");
    let script = repo_root.join("tests/performance/full_chain_e2e_stress.mjs");
    assert!(
        script.exists(),
        "backend full-chain E2E harness missing: {}",
        script.display()
    );

    let total_timeout_ms = env::var("TURA_FULL_CHAIN_TOTAL_TIMEOUT_MS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(120_000);
    let timeout = Duration::from_millis(total_timeout_ms.saturating_add(15_000));
    let node = env::var("NODE").unwrap_or_else(|_| "node".to_string());
    let run_id = env::var("TURA_FULL_CHAIN_E2E_RUN_ID")
        .unwrap_or_else(|_| format!("gateway-full-chain-{}", std::process::id()));
    let summary_path = repo_root
        .join("target/full-chain-e2e-stress")
        .join(&run_id)
        .join("summary.json");

    let mut child = Command::new(&node)
        .arg(&script)
        .current_dir(&repo_root)
        .env("TURA_FULL_CHAIN_E2E_RUN_ID", &run_id)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .unwrap_or_else(|error| panic!("failed to start {node} {}: {error}", script.display()));

    let started = Instant::now();
    loop {
        if let Some(status) = child.try_wait().expect("poll full-chain E2E child") {
            assert!(
                status.success(),
                "backend full-chain E2E stress failed with {status}; summary: {}",
                summary_path.display()
            );
            return;
        }
        if started.elapsed() > timeout {
            let _ = child.kill();
            let _ = child.wait();
            panic!(
                "backend full-chain E2E stress exceeded {:?}; summary: {}",
                timeout,
                summary_path.display()
            );
        }
        thread::sleep(Duration::from_millis(500));
    }
}
