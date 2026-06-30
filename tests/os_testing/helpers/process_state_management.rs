//! Required workspace-wide process and state management E2E tests.
//!
//! This required root-package business E2E is wired into the root workspace
//! package, so process lifecycle and state recovery run as mandatory local
//! correctness coverage instead of optional performance or live scripts.

pub(crate) use anyhow::{anyhow, bail, Context, Result};
pub(crate) use serde_json::json;
pub(crate) use std::{
    io::{BufRead, BufReader, Read, Write},
    net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream},
    path::{Path, PathBuf},
    process::{Child, Command, ExitStatus, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

pub(crate) static SERIAL: Mutex<()> = Mutex::new(());
pub(crate) const PROCESS_EXIT_TIMEOUT: Duration = Duration::from_secs(30);

pub(crate) fn stale_endpoints_are_replaced_gateway_restarts_and_conflicts_fail(
    repo: &Path,
) -> Result<()> {
    let root = temp_root("workspace-process-stale-restart")?;
    let home = root.join("home");
    let workspace = root.join("workspace");
    std::fs::create_dir_all(&home)?;
    std::fs::create_dir_all(&workspace)?;

    write_stale_router_endpoint(&home)?;
    write_stale_session_db_endpoint(&home)?;
    assert!(
        !endpoint_reachable(&router_addr_path(&home))?,
        "seeded stale router endpoint should not be reachable"
    );
    assert!(
        !endpoint_reachable(&service_addr_path(&home))?,
        "seeded stale session_db endpoint should not be reachable"
    );

    let first_port = free_port()?;
    let mut gateway = GatewayGuard::start(repo, &home, &workspace, first_port)?;
    if let Err(error) = wait_for_http_ok(first_port, "/global/health", Duration::from_secs(30)) {
        bail!(
            "stale/restart first gateway did not become healthy: {error}; {}",
            gateway.health_context()
        );
    }
    let router_addr =
        wait_for_reachable_endpoint(&router_addr_path(&home), Duration::from_secs(30))
            .context("stale/restart first router endpoint did not become reachable")?;
    let service_addr =
        wait_for_reachable_endpoint(&service_addr_path(&home), Duration::from_secs(30))
            .context("stale/restart first session_db endpoint did not become reachable")?;
    assert!(!router_addr.is_empty());
    assert!(!service_addr.is_empty());

    let status = wait_for_gateway_router_running(first_port, Duration::from_secs(30))
        .context("stale/restart gateway service status did not report router running")?;
    assert_eq!(
        status["router"]["status"], "running",
        "gateway should report a running router after replacing stale endpoints: {status}"
    );

    let conflict = spawn_conflicting_gateway(repo, &home, &workspace, first_port)?;
    assert!(
        !conflict.status.success(),
        "second gateway with the same TURA_HOME must fail, stdout={}, stderr={}",
        conflict.stdout,
        conflict.stderr
    );
    assert!(
        conflict
            .stderr
            .contains("gateway ownership lock refused startup"),
        "conflict stderr should explain the ownership lock refusal, got: {}",
        conflict.stderr
    );

    let shutdown = shutdown_router(&home).context("stale/restart first router shutdown failed")?;
    assert!(
        shutdown["ok"].as_bool().unwrap_or(false),
        "router shutdown should succeed: {shutdown}"
    );
    assert_eq!(shutdown["payload"]["status"], "shutting_down");
    wait_for_missing(&router_addr_path(&home), Duration::from_secs(10))?;
    wait_for_missing(&service_addr_path(&home), Duration::from_secs(10))?;
    let health_after_shutdown = http_get(first_port, "/global/health", Duration::from_secs(2))
        .context("stale/restart gateway health after router shutdown failed")?;
    assert!(
        health_after_shutdown.starts_with("HTTP/1.1 200"),
        "gateway should remain alive until its front process is stopped"
    );
    gateway.stop()?;
    assert_endpoints_cleaned(&home)?;

    let restart_port = free_port()?;
    let mut restarted = GatewayGuard::start(repo, &home, &workspace, restart_port)?;
    wait_for_http_ok(restart_port, "/global/health", Duration::from_secs(30))
        .context("stale/restart second gateway did not become healthy")?;
    wait_for_reachable_endpoint(&router_addr_path(&home), Duration::from_secs(30))
        .context("stale/restart second router endpoint did not become reachable")?;
    wait_for_reachable_endpoint(&service_addr_path(&home), Duration::from_secs(30))
        .context("stale/restart second session_db endpoint did not become reachable")?;
    let restarted_status =
        wait_for_gateway_router_running(restart_port, Duration::from_secs(30))
            .context("stale/restart second gateway service status did not report router running")?;
    assert_eq!(
        restarted_status["router"]["status"], "running",
        "gateway restart should recreate a healthy router/session_db pair: {restarted_status}"
    );
    restarted.stop()?;
    assert_endpoints_cleaned(&home)?;

    Ok(())
}

pub(crate) fn foreign_gateway_port_conflict_does_not_start_backend(repo: &Path) -> Result<()> {
    let root = temp_root("workspace-process-foreign-port")?;
    let home = root.join("home");
    let workspace = root.join("workspace");
    std::fs::create_dir_all(&home)?;
    std::fs::create_dir_all(&workspace)?;

    let listener = TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0)))?;
    let occupied_port = listener.local_addr()?.port();
    let output = spawn_gateway_until_exit(
        repo,
        &home,
        &workspace,
        occupied_port,
        Duration::from_secs(10),
        "foreign-port gateway",
    )?;
    assert!(
        !output.status.success(),
        "gateway should fail when the requested port is owned by a foreign process, stdout={}, stderr={}",
        output.stdout,
        output.stderr
    );
    assert!(
        output.stderr.contains("gateway port")
            && output.stderr.contains("occupied by a foreign process"),
        "foreign port stderr should explain the port conflict, got: {}",
        output.stderr
    );
    assert_endpoints_cleaned(&home)?;
    assert!(
        lock_files(&home, "gateway")?.is_empty(),
        "failed gateway startup should release its gateway owner lock"
    );
    drop(listener);
    Ok(())
}

pub(crate) fn gateway_status_restarts_crashed_router_and_adopts_session_db(
    repo: &Path,
) -> Result<()> {
    let root = temp_root("workspace-process-router-crash")?;
    let home = root.join("home");
    let workspace = root.join("workspace");
    std::fs::create_dir_all(&home)?;
    std::fs::create_dir_all(&workspace)?;

    let port = free_port()?;
    let mut gateway = GatewayGuard::start(repo, &home, &workspace, port)?;
    wait_for_http_ok(port, "/global/health", Duration::from_secs(30))
        .context("router crash gateway did not become healthy")?;
    let router_before =
        wait_for_reachable_endpoint(&router_addr_path(&home), Duration::from_secs(30))
            .context("router crash initial router endpoint did not become reachable")?;
    let router_before_endpoint = read_endpoint_json(&router_addr_path(&home))?;
    let service_before =
        wait_for_reachable_endpoint(&service_addr_path(&home), Duration::from_secs(30))
            .context("router crash initial session_db endpoint did not become reachable")?;
    let router_pid = endpoint_pid(&router_before_endpoint)
        .or_else(|| wait_for_process_pid("tura_router", &workspace, Duration::from_secs(10)).ok())
        .context("router crash initial endpoint did not expose a pid")?;
    assert_process_alive(router_pid, "router crash initial endpoint pid")?;

    kill_process(router_pid).with_context(|| format!("kill router pid {router_pid}"))?;
    wait_for_addr_unreachable(&router_before, Duration::from_secs(10))
        .context("killed router socket should become unreachable")?;

    let status = wait_for_gateway_router_running(port, Duration::from_secs(30))
        .context("gateway status did not restart crashed router")?;
    assert_eq!(
        status["router"]["status"], "running",
        "gateway status should report the restarted router as running: {status}"
    );
    let router_after =
        wait_for_reachable_endpoint(&router_addr_path(&home), Duration::from_secs(30))
            .context("router crash restarted router endpoint did not become reachable")?;
    let router_after_endpoint = read_endpoint_json(&router_addr_path(&home))?;
    assert_ne!(
        router_after, router_before,
        "gateway should publish a fresh router endpoint after a crash restart"
    );
    let restarted_router_pid = endpoint_pid(&router_after_endpoint)
        .or_else(|| {
            wait_for_process_pid_change(
                "tura_router",
                &workspace,
                router_pid,
                Duration::from_secs(10),
            )
            .ok()
        })
        .context("router crash restarted endpoint did not expose a pid")?;
    assert_ne!(
        restarted_router_pid, router_pid,
        "router restart should be owned by a different process"
    );
    assert_process_alive(restarted_router_pid, "router crash restarted endpoint pid")?;

    let service_after =
        wait_for_reachable_endpoint(&service_addr_path(&home), Duration::from_secs(30))
            .context("router crash adopted session_db endpoint did not remain reachable")?;
    assert_eq!(
        service_after, service_before,
        "restarted router should adopt the orphaned session_db instead of replacing it"
    );

    gateway.stop()?;
    assert_endpoints_cleaned(&home)?;
    Ok(())
}

pub(crate) fn gateway_status_kills_unresponsive_router_and_restarts(repo: &Path) -> Result<()> {
    let root = temp_root("workspace-process-router-unresponsive")?;
    let home = root.join("home");
    let workspace = root.join("workspace");
    std::fs::create_dir_all(&home)?;
    std::fs::create_dir_all(&workspace)?;

    let port = free_port()?;
    let mut gateway = GatewayGuard::start(repo, &home, &workspace, port)?;
    wait_for_http_ok(port, "/global/health", Duration::from_secs(30))
        .context("router unresponsive gateway did not become healthy")?;
    let router_before =
        wait_for_reachable_endpoint(&router_addr_path(&home), Duration::from_secs(30))
            .context("router unresponsive initial router endpoint did not become reachable")?;
    let service_before =
        wait_for_reachable_endpoint(&service_addr_path(&home), Duration::from_secs(30))
            .context("router unresponsive initial session_db endpoint did not become reachable")?;
    let original_endpoint = read_endpoint_json(&router_addr_path(&home))?;
    let router_pid = endpoint_pid(&original_endpoint)
        .or_else(|| wait_for_process_pid("tura_router", &workspace, Duration::from_secs(10)).ok())
        .context("router unresponsive endpoint did not expose a pid")?;

    let fake = UnresponsiveEndpoint::start()?;
    let mut unresponsive_endpoint = original_endpoint;
    unresponsive_endpoint["addr"] = json!(fake.addr.clone());
    publish_router_endpoint(&home, &unresponsive_endpoint)?;

    let status = wait_for_gateway_router_running_with_http_timeout(
        port,
        Duration::from_secs(120),
        Duration::from_secs(90),
    )
    .context("gateway status did not kill and restart unresponsive router")?;
    assert_eq!(
        status["router"]["status"], "running",
        "gateway status should report restarted router after unresponsive endpoint: {status}"
    );

    wait_for_process_dead(router_pid, Duration::from_secs(10))
        .with_context(|| format!("unresponsive router pid {router_pid} should be killed"))?;
    let router_after =
        wait_for_reachable_endpoint(&router_addr_path(&home), Duration::from_secs(30))
            .context("router unresponsive restarted router endpoint did not become reachable")?;
    assert_ne!(
        router_after, router_before,
        "gateway should publish a fresh router endpoint after replacing an unresponsive router"
    );
    assert_ne!(
        router_after, fake.addr,
        "gateway must replace the unresponsive fake router endpoint"
    );
    let router_after_endpoint = read_endpoint_json(&router_addr_path(&home))?;
    let restarted_router_pid = endpoint_pid(&router_after_endpoint)
        .or_else(|| {
            wait_for_process_pid_change(
                "tura_router",
                &workspace,
                router_pid,
                Duration::from_secs(10),
            )
            .ok()
        })
        .context("router unresponsive restarted endpoint did not expose a pid")?;
    assert_ne!(
        restarted_router_pid, router_pid,
        "unresponsive router restart should be owned by a different process"
    );
    assert_process_alive(
        restarted_router_pid,
        "router unresponsive restarted endpoint pid",
    )?;
    let service_after =
        wait_for_reachable_endpoint(&service_addr_path(&home), Duration::from_secs(30)).context(
            "router unresponsive recovered session_db endpoint did not become reachable",
        )?;
    if service_after != service_before {
        wait_for_addr_unreachable(&service_before, Duration::from_secs(10)).with_context(|| {
            format!("replaced session_db endpoint {service_before} should not remain reachable")
        })?;
    }

    gateway.stop()?;
    assert_endpoints_cleaned(&home)?;
    Ok(())
}

pub(crate) fn router_health_restarts_crashed_session_db(repo: &Path) -> Result<()> {
    let root = temp_root("workspace-process-session-db-crash")?;
    let home = root.join("home");
    let workspace = root.join("workspace");
    std::fs::create_dir_all(&home)?;
    std::fs::create_dir_all(&workspace)?;

    let mut router = RouterGuard::start(repo, &home, &workspace)?;
    wait_for_reachable_endpoint(&router_addr_path(&home), Duration::from_secs(30))
        .context("session_db crash initial router endpoint did not become reachable")?;
    let service_before =
        wait_for_reachable_endpoint(&service_addr_path(&home), Duration::from_secs(30))
            .context("session_db crash initial service endpoint did not become reachable")?;
    let session_db_pid =
        wait_for_process_pid("tura_session_db", &workspace, Duration::from_secs(10))?;

    kill_process(session_db_pid)
        .with_context(|| format!("kill session_db pid {session_db_pid}"))?;
    wait_for_addr_unreachable(&service_before, Duration::from_secs(10))
        .context("killed session_db socket should become unreachable")?;

    let health = wait_for_router_session_db_running(&home, Duration::from_secs(30))
        .context("router health did not restart crashed session_db")?;
    assert_eq!(
        health["payload"]["session_db"]["status"], "running",
        "router health should report restarted session_db as running: {health}"
    );
    let service_after =
        wait_for_reachable_endpoint(&service_addr_path(&home), Duration::from_secs(30))
            .context("session_db crash restarted service endpoint did not become reachable")?;
    assert_ne!(
        service_after, service_before,
        "router should publish a fresh session_db endpoint after a crash restart"
    );
    let restarted_session_db_pid = wait_for_process_pid_change(
        "tura_session_db",
        &workspace,
        session_db_pid,
        Duration::from_secs(10),
    )?;
    assert_ne!(
        restarted_session_db_pid, session_db_pid,
        "session_db restart should be owned by a different process"
    );

    let shutdown = shutdown_router(&home)?;
    assert!(
        shutdown["ok"].as_bool().unwrap_or(false),
        "router shutdown should succeed after session_db crash recovery: {shutdown}"
    );
    router.wait_for_exit(PROCESS_EXIT_TIMEOUT)?;
    wait_for_missing(&router_addr_path(&home), Duration::from_secs(10))?;
    wait_for_missing(&service_addr_path(&home), Duration::from_secs(10))?;
    assert_endpoints_cleaned(&home)?;
    Ok(())
}

pub(crate) fn router_health_restarts_unresponsive_session_db(repo: &Path) -> Result<()> {
    let root = temp_root("workspace-process-session-db-unresponsive")?;
    let home = root.join("home");
    let workspace = root.join("workspace");
    std::fs::create_dir_all(&home)?;
    std::fs::create_dir_all(&workspace)?;

    let mut router = RouterGuard::start(repo, &home, &workspace)?;
    wait_for_reachable_endpoint(&router_addr_path(&home), Duration::from_secs(30))
        .context("session_db unresponsive initial router endpoint did not become reachable")?;
    let service_before =
        wait_for_reachable_endpoint(&service_addr_path(&home), Duration::from_secs(30))
            .context("session_db unresponsive initial service endpoint did not become reachable")?;
    let session_db_pid =
        wait_for_process_pid("tura_session_db", &workspace, Duration::from_secs(10))?;
    assert_process_alive(
        session_db_pid,
        "managed session_db before unresponsive endpoint swap",
    )?;

    let fake = UnresponsiveEndpoint::start()?;
    publish_session_db_endpoint(&home, &fake.addr)?;
    assert_process_alive(
        session_db_pid,
        "managed session_db after stale unresponsive endpoint is published",
    )?;

    let health = wait_for_router_session_db_running(&home, Duration::from_secs(30))
        .context("router health did not restart unresponsive session_db")?;
    assert_eq!(
        health["payload"]["session_db"]["status"], "running",
        "router health should report restarted session_db as running: {health}"
    );
    let service_after =
        wait_for_reachable_endpoint(&service_addr_path(&home), Duration::from_secs(30)).context(
            "session_db unresponsive restarted service endpoint did not become reachable",
        )?;
    assert_ne!(
        service_after, fake.addr,
        "router must replace the unresponsive published session_db endpoint"
    );
    assert_ne!(
        service_after, service_before,
        "router should publish a fresh session_db endpoint after replacing an unresponsive service"
    );
    wait_for_process_dead(session_db_pid, Duration::from_secs(10)).with_context(|| {
        format!("unresponsive managed session_db pid {session_db_pid} should die")
    })?;
    let restarted_session_db_pid = wait_for_process_pid_change(
        "tura_session_db",
        &workspace,
        session_db_pid,
        Duration::from_secs(10),
    )?;
    assert_ne!(
        restarted_session_db_pid, session_db_pid,
        "unresponsive session_db restart should be owned by a different process"
    );

    let shutdown = shutdown_router(&home)?;
    assert!(
        shutdown["ok"].as_bool().unwrap_or(false),
        "router shutdown should succeed after session_db unresponsive recovery: {shutdown}"
    );
    router.wait_for_exit(PROCESS_EXIT_TIMEOUT)?;
    wait_for_missing(&router_addr_path(&home), Duration::from_secs(10))?;
    wait_for_missing(&service_addr_path(&home), Duration::from_secs(10))?;
    assert_endpoints_cleaned(&home)?;
    Ok(())
}

pub(crate) fn orphan_session_db_is_adopted_and_stopped_by_router(repo: &Path) -> Result<()> {
    let root = temp_root("workspace-process-orphan-session-db")?;
    let home = root.join("home");
    let workspace = root.join("workspace");
    std::fs::create_dir_all(&home)?;
    std::fs::create_dir_all(&workspace)?;

    let mut session_db = SessionDbGuard::start(repo, &home)?;
    let service_before =
        wait_for_reachable_endpoint(&service_addr_path(&home), Duration::from_secs(30))?;

    let mut router = RouterGuard::start(repo, &home, &workspace)?;
    wait_for_reachable_endpoint(&router_addr_path(&home), Duration::from_secs(30))?;
    let service_after =
        wait_for_reachable_endpoint(&service_addr_path(&home), Duration::from_secs(30))?;
    assert_eq!(
        service_after, service_before,
        "router should adopt the already-running orphan session_db instead of replacing it"
    );

    let health = router_health(&home)?;
    assert_eq!(
        health["payload"]["session_db"]["status"], "running",
        "router health should report the adopted session_db as running: {health}"
    );

    let shutdown = shutdown_router(&home)?;
    assert!(
        shutdown["ok"].as_bool().unwrap_or(false),
        "router shutdown should succeed for adopted session_db: {shutdown}"
    );
    router.wait_for_exit(PROCESS_EXIT_TIMEOUT)?;
    session_db.wait_for_exit(PROCESS_EXIT_TIMEOUT)?;
    wait_for_missing(&router_addr_path(&home), Duration::from_secs(10))?;
    wait_for_missing(&service_addr_path(&home), Duration::from_secs(10))?;
    assert_endpoints_cleaned(&home)?;

    Ok(())
}

pub(crate) fn orphan_router_is_adopted_and_stopped_by_gateway(repo: &Path) -> Result<()> {
    let root = temp_root("workspace-process-orphan-router")?;
    let home = root.join("home");
    let workspace = root.join("workspace");
    std::fs::create_dir_all(&home)?;
    std::fs::create_dir_all(&workspace)?;

    let mut router = RouterGuard::start(repo, &home, &workspace)?;
    let router_before =
        wait_for_reachable_endpoint(&router_addr_path(&home), Duration::from_secs(30))?;
    wait_for_reachable_endpoint(&service_addr_path(&home), Duration::from_secs(30))?;

    let port = free_port()?;
    let mut gateway = GatewayGuard::start(repo, &home, &workspace, port)?;
    wait_for_http_ok(port, "/global/health", Duration::from_secs(30))?;
    let router_after =
        wait_for_reachable_endpoint(&router_addr_path(&home), Duration::from_secs(30))?;
    assert_eq!(
        router_after, router_before,
        "gateway should adopt the already-running orphan router instead of starting a second one"
    );

    let status = wait_for_gateway_router_running(port, Duration::from_secs(30))
        .context("orphan router gateway service status did not report router running")?;
    assert_eq!(
        status["router"]["status"], "running",
        "gateway should report the adopted router as running: {status}"
    );

    gateway.stop()?;
    router.wait_for_exit(PROCESS_EXIT_TIMEOUT)?;
    wait_for_missing(&router_addr_path(&home), Duration::from_secs(10))?;
    wait_for_missing(&service_addr_path(&home), Duration::from_secs(10))?;
    assert_endpoints_cleaned(&home)?;

    Ok(())
}

pub(crate) fn router_keeps_command_run_when_runtime_socket_disconnects(repo: &Path) -> Result<()> {
    let root = temp_root("workspace-process-router-command-run-abort")?;
    let home = root.join("home");
    let workspace = root.join("workspace");
    std::fs::create_dir_all(&home)?;
    std::fs::create_dir_all(&workspace)?;

    let mut router = RouterGuard::start(repo, &home, &workspace)?;
    let addr = wait_for_reachable_endpoint(&router_addr_path(&home), Duration::from_secs(30))?;
    wait_for_router_session_db_running(&home, Duration::from_secs(30))?;

    let pid_file = workspace.join("router-command-run-child.pid");
    let done_file = workspace.join("router-command-run-child.done");
    let shell_command = code_tools::commands::active_shell_command_name();
    let request = json!({
        "request_id": "router-command-run-survive-disconnect",
        "kind": "call",
        "method": "execution.command_run",
        "payload": {
            "session_id": "router-command-run-survive-session",
            "runtime_id": "router-command-run-survive-runtime",
            "session_directory": workspace.display().to_string(),
            "arguments": {
                "commands": [{
                    "command": shell_command,
                    "command_line": json!({
                        "command": command_run_survival_script(&pid_file, &done_file),
                        "timeout_ms": 10000
                    }).to_string()
                }]
            },
            "allowed_commands": [shell_command]
        }
    });

    let socket: SocketAddr = addr.parse().context("parse router command_run addr")?;
    let mut stream = TcpStream::connect_timeout(&socket, Duration::from_secs(2))
        .context("connect router for command_run disconnect survival")?;
    stream.set_write_timeout(Some(Duration::from_secs(2)))?;
    stream.write_all(serde_json::to_string(&request)?.as_bytes())?;
    stream.write_all(b"\n")?;
    stream.flush()?;

    let child_pid = wait_for_pid_file(&pid_file, Duration::from_secs(10))?;
    assert_process_alive(
        child_pid,
        "router-owned command_run child before socket close",
    )?;
    drop(stream);
    wait_for_process_dead(child_pid, Duration::from_secs(10))
        .with_context(|| format!("router-owned command_run child pid {child_pid} should exit"))?;
    wait_for_path(&done_file, Duration::from_secs(2))
        .context("router-owned command_run should finish after runtime socket disconnect")?;

    router.stop()?;
    assert_endpoints_cleaned(&home)?;
    Ok(())
}

pub(crate) fn gateway_stdin_eof_shuts_down_router_session_db_and_runtime(
    repo: &Path,
) -> Result<()> {
    let root = temp_root("workspace-process-gateway-router-idle")?;
    let home = root.join("home");
    let workspace = root.join("workspace");
    std::fs::create_dir_all(&home)?;
    std::fs::create_dir_all(&workspace)?;

    let port = free_port()?;
    let mut child = Command::new(debug_bin(repo, "tura_gateway"))
        .current_dir(&workspace)
        .envs(gateway_env(repo, &home, &workspace, port))
        .env("TURA_GATEWAY_SHUTDOWN_ON_STDIN_EOF", "1")
        .env("TURA_GATEWAY_ROUTER_LEASE_TTL_SECS", "1")
        .env("TURA_ROUTER_IDLE_SHUTDOWN_SECS", "2")
        .stdin(Stdio::piped())
        .stdout(Stdio::from(process_log_file(
            &home,
            "gateway-stdin-eof.stdout.log",
        )?))
        .stderr(Stdio::from(process_log_file(
            &home,
            "gateway-stdin-eof.stderr.log",
        )?))
        .spawn()
        .context("spawn stdin-eof gateway")?;

    wait_for_http_ok(port, "/global/health", Duration::from_secs(30))
        .context("stdin-eof gateway did not become healthy")?;
    wait_for_reachable_endpoint(&router_addr_path(&home), Duration::from_secs(30))
        .context("stdin-eof router endpoint did not become reachable")?;
    wait_for_reachable_endpoint(&service_addr_path(&home), Duration::from_secs(30))
        .context("stdin-eof session_db endpoint did not become reachable")?;
    wait_for_router_fronts(&home, 1, Duration::from_secs(10))
        .context("router did not receive gateway heartbeat before stdin EOF")?;
    let router_pid = wait_for_process_pid("tura_router", &workspace, Duration::from_secs(10))
        .context("stdin-eof router pid should be discoverable before idle shutdown")?;

    drop(child.stdin.take());
    let status = wait_for_process_exit(&mut child, Duration::from_secs(20), "stdin-eof gateway")?;
    assert!(
        status.success(),
        "stdin-eof gateway should exit cleanly after frontend pipe closes: {status}"
    );
    if let Err(error) = wait_for_file_missing(&router_addr_path(&home), PROCESS_EXIT_TIMEOUT) {
        let lifecycle = router_lifecycle_status(&home)
            .unwrap_or_else(|status_error| json!({ "status_error": status_error.to_string() }));
        bail!("gateway EOF must explicitly shut down router/session_db/runtime-owned work: {error}; lifecycle={lifecycle}");
    }
    wait_for_file_missing(&service_addr_path(&home), PROCESS_EXIT_TIMEOUT)?;
    wait_for_process_dead(router_pid, PROCESS_EXIT_TIMEOUT).with_context(|| {
        format!("stdin-eof router pid {router_pid} should exit after idle self-shutdown")
    })?;
    assert_endpoints_cleaned(&home)?;
    Ok(())
}

pub(crate) fn session_db_restart_marks_running_sessions_interrupted_without_losing_history(
    repo: &Path,
) -> Result<()> {
    let root = temp_root("workspace-process-session-db-recovery")?;
    let home = root.join("home");
    let workspace = root.join("workspace with spaces");
    std::fs::create_dir_all(&home)?;
    std::fs::create_dir_all(&workspace)?;

    let session_id = format!("process-recovery-{}", std::process::id());
    let message_id = "process-recovery-message-1";
    let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis() as i64;
    let mut session_db = SessionDbGuard::start(repo, &home)?;
    wait_for_reachable_endpoint(&service_addr_path(&home), Duration::from_secs(30))
        .context("recovery initial session_db endpoint did not become reachable")?;
    let upsert = session_db_call(
        &home,
        &json!({
            "command": "upsert_session",
            "session": running_session_payload(&session_id, &workspace, timestamp),
            "messages": [
                message_payload(&session_id, message_id, "user", timestamp, "resume this work")
            ],
            "todos": [
                {"id": "todo-recovery", "content": "keep history across restart", "status": "doing"}
            ]
        }),
    )?;
    assert_eq!(
        upsert["kind"], "ok",
        "upsert through session_db failed: {upsert}"
    );

    let before = session_db_call(
        &home,
        &json!({
            "command": "get_session",
            "session_id": session_id,
        }),
    )?;
    assert_eq!(before["session"]["state"], "running");
    assert_eq!(before["session"]["status"], "busy");
    assert_eq!(before["session"]["message_count"], 1);

    let shutdown = shutdown_session_db(&home)?;
    assert_eq!(
        shutdown["kind"], "ok",
        "session_db shutdown before recovery restart failed: {shutdown}"
    );
    session_db.wait_for_exit(PROCESS_EXIT_TIMEOUT)?;
    wait_for_missing(&service_addr_path(&home), Duration::from_secs(10))?;

    let mut restarted = SessionDbGuard::start(repo, &home)?;
    wait_for_reachable_endpoint(&service_addr_path(&home), Duration::from_secs(30))
        .context("recovery restarted session_db endpoint did not become reachable")?;

    let after = session_db_call(
        &home,
        &json!({
            "command": "get_session",
            "session_id": session_id,
        }),
    )?;
    assert_eq!(
        after["session"]["state"], "interrupted",
        "session_db startup recovery must interrupt in-flight sessions without dropping them: {after}"
    );
    assert_eq!(after["session"]["status"], "error");
    assert_eq!(after["session"]["message_count"], 1);
    assert_eq!(after["session"]["session"]["id"], session_id);
    assert_eq!(after["session"]["management"]["state"], "interrupted");
    assert!(
        after["session"]["management"]["session_last_update_at"]
            .as_str()
            .unwrap_or_default()
            .contains('T'),
        "interrupted management timestamp should remain an RFC3339 string: {after}"
    );

    let records = session_db_call(
        &home,
        &json!({
            "command": "list_session_records",
            "session_id": session_id,
            "page": 0,
            "page_size": 10,
        }),
    )?;
    assert_eq!(records["page"]["total"], 1);
    assert_eq!(records["records"][0]["message_id"], message_id);
    assert_eq!(
        records["records"][0]["record"]["content"],
        "resume this work"
    );

    let workspace_key = workspace.to_string_lossy().replace('\\', "/");
    let listed = session_db_call(
        &home,
        &json!({
            "command": "list_sessions",
            "workspace": workspace_key,
            "page": 0,
            "page_size": 10,
        }),
    )?;
    assert_eq!(listed["page"]["total"], 1);
    assert_eq!(listed["sessions"][0]["session_id"], session_id);
    assert_eq!(listed["sessions"][0]["state"], "interrupted");
    assert_eq!(listed["sessions"][0]["message_count"], 1);

    let shutdown = shutdown_session_db(&home)?;
    assert_eq!(
        shutdown["kind"], "ok",
        "session_db shutdown after recovery assertions failed: {shutdown}"
    );
    restarted.wait_for_exit(PROCESS_EXIT_TIMEOUT)?;
    wait_for_missing(&service_addr_path(&home), Duration::from_secs(10))?;
    assert_endpoints_cleaned(&home)?;
    Ok(())
}

pub(crate) struct GatewayGuard {
    child: Option<Child>,
    home: PathBuf,
    stdout_log: PathBuf,
    stderr_log: PathBuf,
}

impl GatewayGuard {
    fn start(repo: &Path, home: &Path, workspace: &Path, port: u16) -> Result<Self> {
        let stdout_log = process_log_path(home, &format!("gateway-{port}.stdout.log"));
        let stderr_log = process_log_path(home, &format!("gateway-{port}.stderr.log"));
        let child = Command::new(debug_bin(repo, "tura_gateway"))
            .current_dir(workspace)
            .envs(gateway_env(repo, home, workspace, port))
            .stdin(Stdio::null())
            .stdout(Stdio::from(process_log_file_at(&stdout_log)?))
            .stderr(Stdio::from(process_log_file_at(&stderr_log)?))
            .spawn()
            .context("spawn tura_gateway")?;
        Ok(Self {
            child: Some(child),
            home: home.to_path_buf(),
            stdout_log,
            stderr_log,
        })
    }

    fn health_context(&mut self) -> String {
        let child_status = match self.child.as_mut() {
            Some(child) => match child.try_wait() {
                Ok(Some(status)) => format!("exited with {status}"),
                Ok(None) => "still running".to_string(),
                Err(error) => format!("status unavailable: {error}"),
            },
            None => "already reaped".to_string(),
        };
        format!(
            "gateway process {child_status}; stdout tail: {}; stderr tail: {}",
            file_tail(&self.stdout_log, 4096),
            file_tail(&self.stderr_log, 4096)
        )
    }

    fn stop(&mut self) -> Result<()> {
        if shutdown_router(&self.home).is_ok() {
            wait_for_missing(&router_addr_path(&self.home), Duration::from_secs(10))?;
            wait_for_missing(&service_addr_path(&self.home), Duration::from_secs(10))?;
        }
        if let Some(mut child) = self.child.take() {
            if child.try_wait()?.is_none() {
                child.kill().context("kill gateway")?;
            }
            let _ = child.wait();
        }
        Ok(())
    }
}

impl Drop for GatewayGuard {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

pub(crate) struct RouterGuard {
    child: Option<Child>,
    home: PathBuf,
}

impl RouterGuard {
    fn start(repo: &Path, home: &Path, workspace: &Path) -> Result<Self> {
        let child = Command::new(debug_bin(repo, "tura_router"))
            .arg("serve-socket")
            .current_dir(workspace)
            .envs(base_process_env(repo, home, workspace))
            .stdin(Stdio::null())
            .stdout(Stdio::from(process_log_file(
                home,
                "orphan-router.stdout.log",
            )?))
            .stderr(Stdio::from(process_log_file(
                home,
                "orphan-router.stderr.log",
            )?))
            .spawn()
            .context("spawn tura_router serve-socket")?;
        Ok(Self {
            child: Some(child),
            home: home.to_path_buf(),
        })
    }

    fn wait_for_exit(&mut self, timeout: Duration) -> Result<()> {
        wait_for_child_exit(&mut self.child, timeout, "tura_router")
    }

    fn stop(&mut self) -> Result<()> {
        if shutdown_router(&self.home).is_ok() {
            wait_for_missing(&router_addr_path(&self.home), Duration::from_secs(10))?;
            wait_for_missing(&service_addr_path(&self.home), Duration::from_secs(10))?;
        }
        if let Some(mut child) = self.child.take() {
            if child.try_wait()?.is_none() {
                child.kill().context("kill tura_router")?;
            }
            let _ = child.wait();
        }
        Ok(())
    }
}

impl Drop for RouterGuard {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

pub(crate) struct SessionDbGuard {
    child: Option<Child>,
    home: PathBuf,
}

impl SessionDbGuard {
    fn start(repo: &Path, home: &Path) -> Result<Self> {
        let child = Command::new(debug_bin(repo, "tura_session_db"))
            .env("TURA_HOME", home)
            .env_remove("SESSION_LOG_DB_ROOT")
            .env_remove("TURA_DB_ROOT")
            .stdin(Stdio::null())
            .stdout(Stdio::from(process_log_file(
                home,
                "orphan-session-db.stdout.log",
            )?))
            .stderr(Stdio::from(process_log_file(
                home,
                "orphan-session-db.stderr.log",
            )?))
            .spawn()
            .context("spawn tura_session_db")?;
        Ok(Self {
            child: Some(child),
            home: home.to_path_buf(),
        })
    }

    fn wait_for_exit(&mut self, timeout: Duration) -> Result<()> {
        wait_for_child_exit(&mut self.child, timeout, "tura_session_db")
    }

    fn stop(&mut self) -> Result<()> {
        if shutdown_session_db(&self.home).is_ok() {
            wait_for_missing(&service_addr_path(&self.home), Duration::from_secs(10))?;
        }
        if let Some(mut child) = self.child.take() {
            if child.try_wait()?.is_none() {
                child.kill().context("kill tura_session_db")?;
            }
            let _ = child.wait();
        }
        Ok(())
    }
}

impl Drop for SessionDbGuard {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

pub(crate) struct CommandOutput {
    status: ExitStatus,
    stdout: String,
    stderr: String,
}

pub(crate) fn spawn_conflicting_gateway(
    repo: &Path,
    home: &Path,
    workspace: &Path,
    existing_port: u16,
) -> Result<CommandOutput> {
    let conflict_port = loop {
        let candidate = free_port()?;
        if candidate != existing_port {
            break candidate;
        }
    };
    spawn_gateway_until_exit(
        repo,
        home,
        workspace,
        conflict_port,
        Duration::from_secs(10),
        "conflicting gateway",
    )
}

pub(crate) fn spawn_gateway_until_exit(
    repo: &Path,
    home: &Path,
    workspace: &Path,
    port: u16,
    timeout: Duration,
    label: &str,
) -> Result<CommandOutput> {
    let mut child = Command::new(debug_bin(repo, "tura_gateway"))
        .current_dir(workspace)
        .envs(gateway_env(repo, home, workspace, port))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("spawn {label}"))?;
    let status = wait_for_process_exit(&mut child, timeout, label)?;
    let stdout = read_pipe(child.stdout.take());
    let stderr = read_pipe(child.stderr.take());
    Ok(CommandOutput {
        status,
        stdout,
        stderr,
    })
}

pub(crate) fn wait_for_child_exit(
    child: &mut Option<Child>,
    timeout: Duration,
    label: &str,
) -> Result<()> {
    let Some(handle) = child.as_mut() else {
        return Ok(());
    };
    let status = wait_for_process_exit(handle, timeout, label)?;
    if !status.success() {
        bail!("{label} exited with {status}");
    }
    *child = None;
    Ok(())
}

pub(crate) fn wait_for_process_exit(
    child: &mut Child,
    timeout: Duration,
    label: &str,
) -> Result<ExitStatus> {
    let started = Instant::now();
    while started.elapsed() < timeout {
        if let Some(status) = child.try_wait()? {
            return Ok(status);
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    child.kill().with_context(|| format!("kill hung {label}"))?;
    let _ = child.wait();
    bail!("{label} did not exit within {}ms", timeout.as_millis())
}

pub(crate) fn gateway_env(
    repo: &Path,
    home: &Path,
    workspace: &Path,
    port: u16,
) -> Vec<(String, String)> {
    let mut env = base_process_env(repo, home, workspace);
    env.push(("PORT".to_string(), port.to_string()));
    env.push(("TURA_GATEWAY_PORT".to_string(), port.to_string()));
    env.push((
        "TURA_GATEWAY_URL".to_string(),
        format!("http://127.0.0.1:{port}"),
    ));
    env
}

pub(crate) fn base_process_env(
    repo: &Path,
    home: &Path,
    workspace: &Path,
) -> Vec<(String, String)> {
    vec![
        ("TURA_HOME".to_string(), home.display().to_string()),
        ("TURA_PROJECT_ROOT".to_string(), repo.display().to_string()),
        ("TURA_CWD".to_string(), workspace.display().to_string()),
        (
            "TURA_ROUTER_IDLE_SHUTDOWN_SECS".to_string(),
            "120".to_string(),
        ),
        (
            "TURA_GATEWAY_ROUTER_LEASE_TTL_SECS".to_string(),
            "15".to_string(),
        ),
        (
            "TURA_PROVIDER_CONFIG".to_string(),
            repo.join("crates")
                .join("provider")
                .join("config")
                .join("provider_config.json")
                .display()
                .to_string(),
        ),
    ]
}

pub(crate) fn shutdown_router(home: &Path) -> Result<serde_json::Value> {
    let path = router_addr_path(home);
    if !path.exists() {
        return Err(anyhow!(
            "router endpoint does not exist: {}",
            path.display()
        ));
    }
    let addr = read_endpoint_addr(&path)?;
    let response = call_jsonl(
        &addr,
        &json!({
            "request_id": "workspace-process-shutdown",
            "kind": "call",
            "method": "execution.shutdown",
            "payload": {}
        }),
        Duration::from_secs(5),
    )?;
    if response["ok"].as_bool().unwrap_or(false) {
        wait_for_addr_unreachable(&addr, Duration::from_secs(10))?;
    }
    Ok(response)
}

pub(crate) fn shutdown_session_db(home: &Path) -> Result<serde_json::Value> {
    let path = service_addr_path(home);
    if !path.exists() {
        return Err(anyhow!(
            "session_db endpoint does not exist: {}",
            path.display()
        ));
    }
    let addr = read_endpoint_addr(&path)?;
    call_jsonl(
        &addr,
        &json!({
            "command": "shutdown"
        }),
        Duration::from_secs(5),
    )
}

pub(crate) fn router_health(home: &Path) -> Result<serde_json::Value> {
    let addr = read_endpoint_addr(&router_addr_path(home))?;
    call_jsonl(
        &addr,
        &json!({
            "request_id": "workspace-process-health",
            "kind": "health_check",
            "method": "health_check",
            "payload": {}
        }),
        Duration::from_secs(5),
    )
}

pub(crate) fn router_lifecycle_status(home: &Path) -> Result<serde_json::Value> {
    let addr = read_endpoint_addr(&router_addr_path(home))?;
    call_jsonl(
        &addr,
        &json!({
            "request_id": "workspace-process-lifecycle-status",
            "kind": "call",
            "method": "lifecycle.status",
            "payload": {}
        }),
        Duration::from_secs(5),
    )
}

pub(crate) fn session_db_call(
    home: &Path,
    payload: &serde_json::Value,
) -> Result<serde_json::Value> {
    let addr = read_endpoint_addr(&service_addr_path(home))?;
    call_jsonl(&addr, payload, Duration::from_secs(5))
}

pub(crate) fn wait_for_gateway_router_running(
    port: u16,
    timeout: Duration,
) -> Result<serde_json::Value> {
    wait_for_gateway_router_running_with_http_timeout(port, timeout, Duration::from_secs(45))
}

pub(crate) fn wait_for_gateway_router_running_with_http_timeout(
    port: u16,
    timeout: Duration,
    http_timeout: Duration,
) -> Result<serde_json::Value> {
    let started = Instant::now();
    let mut last_error = None;
    while started.elapsed() < timeout {
        match http_json_with_timeout(port, "/service/status", http_timeout) {
            Ok(status) if status["router"]["status"] == "running" => return Ok(status),
            Ok(status) => {
                last_error = Some(anyhow!(
                    "gateway router status was not running yet: {status}"
                ));
            }
            Err(error) => last_error = Some(error),
        }
        std::thread::sleep(Duration::from_millis(150));
    }
    Err(last_error.unwrap_or_else(|| anyhow!("gateway router status did not become running")))
}

pub(crate) fn wait_for_router_session_db_running(
    home: &Path,
    timeout: Duration,
) -> Result<serde_json::Value> {
    let started = Instant::now();
    let mut last_error = None;
    while started.elapsed() < timeout {
        match router_health(home) {
            Ok(health) if health["payload"]["session_db"]["status"] == "running" => {
                return Ok(health);
            }
            Ok(health) => {
                last_error = Some(anyhow!(
                    "router session_db status was not running yet: {health}"
                ));
            }
            Err(error) => last_error = Some(error),
        }
        std::thread::sleep(Duration::from_millis(150));
    }
    Err(last_error.unwrap_or_else(|| anyhow!("router session_db status did not become running")))
}

pub(crate) fn wait_for_router_fronts(
    home: &Path,
    minimum: u64,
    timeout: Duration,
) -> Result<serde_json::Value> {
    let started = Instant::now();
    let mut last_error = None;
    while started.elapsed() < timeout {
        match router_lifecycle_status(home) {
            Ok(status)
                if status["ok"].as_bool().unwrap_or(false)
                    && status["payload"]["active_fronts"].as_u64().unwrap_or(0) >= minimum =>
            {
                return Ok(status);
            }
            Ok(status) => {
                last_error = Some(anyhow!(
                    "router lifecycle did not report {minimum} active fronts yet: {status}"
                ));
            }
            Err(error) => last_error = Some(error),
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    Err(last_error.unwrap_or_else(|| anyhow!("router lifecycle active fronts did not appear")))
}

pub(crate) fn call_jsonl(
    addr: &str,
    payload: &serde_json::Value,
    timeout: Duration,
) -> Result<serde_json::Value> {
    let socket: SocketAddr = addr
        .parse()
        .with_context(|| format!("invalid socket address {addr}"))?;
    let mut stream = TcpStream::connect_timeout(&socket, Duration::from_secs(2))
        .with_context(|| format!("connect {addr}"))?;
    stream.set_read_timeout(Some(timeout))?;
    stream.set_write_timeout(Some(timeout))?;
    stream.write_all(serde_json::to_string(payload)?.as_bytes())?;
    stream.write_all(b"\n")?;
    stream.flush()?;
    let mut line = String::new();
    BufReader::new(stream).read_line(&mut line)?;
    if line.trim().is_empty() {
        bail!("socket {addr} closed without a response");
    }
    serde_json::from_str(line.trim()).context("parse JSONL response")
}

pub(crate) fn wait_for_reachable_endpoint(path: &Path, timeout: Duration) -> Result<String> {
    let mut last_error = None;
    let started = Instant::now();
    while started.elapsed() < timeout {
        match read_endpoint_addr(path) {
            Ok(addr) => {
                if socket_reachable(&addr)? {
                    return Ok(addr);
                }
                last_error = Some(anyhow!("endpoint {addr} is not reachable"));
            }
            Err(error) => last_error = Some(error),
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    Err(last_error.unwrap_or_else(|| anyhow!("endpoint {} did not appear", path.display())))
}

pub(crate) fn wait_for_missing(path: &Path, timeout: Duration) -> Result<()> {
    let started = Instant::now();
    while started.elapsed() < timeout {
        if !path.exists() && !endpoint_reachable(path)? {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    bail!(
        "endpoint {} was not cleaned up within {:?}: {}",
        path.display(),
        timeout,
        endpoint_debug(path)
    )
}

pub(crate) fn assert_endpoints_cleaned(home: &Path) -> Result<()> {
    let router_path = router_addr_path(home);
    wait_for_missing(&router_path, Duration::from_secs(10)).with_context(|| {
        format!(
            "router endpoint should be absent and unreachable after cleanup: {}",
            endpoint_debug(&router_path)
        )
    })?;
    let service_path = service_addr_path(home);
    wait_for_missing(&service_path, Duration::from_secs(10)).with_context(|| {
        format!(
            "session_db endpoint should be absent and unreachable after cleanup: {}",
            endpoint_debug(&service_path)
        )
    })?;
    Ok(())
}

pub(crate) fn endpoint_reachable(path: &Path) -> Result<bool> {
    let Ok(addr) = read_endpoint_addr(path) else {
        return Ok(false);
    };
    socket_reachable(&addr)
}

pub(crate) fn wait_for_file_missing(path: &Path, timeout: Duration) -> Result<()> {
    let started = Instant::now();
    while started.elapsed() < timeout {
        if !path.exists() {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    bail!(
        "file {} was not removed within {:?}",
        path.display(),
        timeout
    )
}

pub(crate) fn endpoint_debug(path: &Path) -> String {
    let exists = path.exists();
    match read_endpoint_addr(path) {
        Ok(addr) => {
            let reachable = socket_reachable(&addr)
                .map(|value| value.to_string())
                .unwrap_or_else(|error| format!("error: {error}"));
            format!(
                "path={}, exists={exists}, addr={addr}, reachable={reachable}",
                path.display()
            )
        }
        Err(error) => format!(
            "path={}, exists={exists}, addr_error={error}",
            path.display()
        ),
    }
}

pub(crate) fn socket_reachable(addr: &str) -> Result<bool> {
    let socket: SocketAddr = addr
        .parse()
        .with_context(|| format!("invalid socket address {addr}"))?;
    Ok(TcpStream::connect_timeout(&socket, Duration::from_millis(200)).is_ok())
}

pub(crate) fn read_endpoint_addr(path: &Path) -> Result<String> {
    let raw = std::fs::read_to_string(path)
        .with_context(|| format!("read endpoint {}", path.display()))?;
    let trimmed = raw.trim();
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(trimmed) {
        if let Some(addr) = value.get("addr").and_then(serde_json::Value::as_str) {
            return Ok(addr.to_string());
        }
    }
    if trimmed.is_empty() {
        bail!("endpoint {} is empty", path.display());
    }
    Ok(trimmed.to_string())
}

pub(crate) fn wait_for_addr_unreachable(addr: &str, timeout: Duration) -> Result<()> {
    let started = Instant::now();
    while started.elapsed() < timeout {
        if !socket_reachable(addr)? {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    bail!("address {addr} was still reachable after {:?}", timeout)
}

pub(crate) fn command_run_survival_script(pid_file: &Path, done_file: &Path) -> String {
    if cfg!(windows) {
        let pid_path = powershell_single_quoted_path(pid_file);
        let done_path = powershell_single_quoted_path(done_file);
        format!(
            "Set-Content -LiteralPath {pid_path} -Value $PID; Start-Sleep -Milliseconds 800; Set-Content -LiteralPath {done_path} -Value done"
        )
    } else {
        format!(
            "printf '%s' \"$$\" > '{}'; sleep 0.8; printf done > '{}'",
            pid_file.display(),
            done_file.display()
        )
    }
}

pub(crate) fn wait_for_path(path: &Path, timeout: Duration) -> Result<()> {
    let started = Instant::now();
    while started.elapsed() < timeout {
        if path.exists() {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    bail!(
        "path {} did not appear within {}ms",
        path.display(),
        timeout.as_millis()
    )
}

pub(crate) fn powershell_single_quoted_path(path: &Path) -> String {
    format!("'{}'", path.display().to_string().replace('\'', "''"))
}

pub(crate) fn wait_for_pid_file(path: &Path, timeout: Duration) -> Result<u32> {
    let started = Instant::now();
    let mut last_error = None;
    while started.elapsed() < timeout {
        match std::fs::read_to_string(path) {
            Ok(raw) => match raw.trim().parse::<u32>() {
                Ok(pid) if pid > 0 => return Ok(pid),
                Ok(_) => last_error = Some(anyhow!("pid file contained zero")),
                Err(error) => last_error = Some(error.into()),
            },
            Err(error) => last_error = Some(error.into()),
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    Err(last_error.unwrap_or_else(|| anyhow!("pid file {} did not appear", path.display())))
}

pub(crate) fn assert_process_alive(pid: u32, label: &str) -> Result<()> {
    if process_alive(pid) {
        return Ok(());
    }
    bail!("{label} pid {pid} was not alive")
}

pub(crate) fn wait_for_process_dead(pid: u32, timeout: Duration) -> Result<()> {
    let started = Instant::now();
    while started.elapsed() < timeout {
        if !process_alive(pid) {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    bail!("pid {pid} was still alive after {}ms", timeout.as_millis())
}

pub(crate) fn process_alive(pid: u32) -> bool {
    let mut system = sysinfo::System::new_all();
    system.refresh_processes();
    system
        .process(sysinfo::Pid::from_u32(pid))
        .is_some_and(|process| {
            !matches!(
                process.status(),
                sysinfo::ProcessStatus::Zombie | sysinfo::ProcessStatus::Dead
            )
        })
}

pub(crate) struct TargetBackendCleanup {
    pub(crate) repo: PathBuf,
}

impl Drop for TargetBackendCleanup {
    fn drop(&mut self) {
        let _ = cleanup_target_backend_processes(&self.repo, Duration::from_secs(10));
    }
}

pub(crate) fn cleanup_target_backend_processes(repo: &Path, timeout: Duration) -> Result<()> {
    let started = Instant::now();
    loop {
        let pids = target_backend_process_pids(repo)?;
        if pids.is_empty() {
            return Ok(());
        }
        for pid in pids {
            if pid == std::process::id() {
                continue;
            }
            terminate_process_quietly(pid);
        }
        if started.elapsed() >= timeout {
            bail!(
                "target backend processes remained after cleanup timeout: {:?}",
                target_backend_process_pids(repo)?
            );
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

pub(crate) fn target_backend_process_pids(repo: &Path) -> Result<Vec<u32>> {
    let target = canonical_or_self(&target_dir(repo));
    let mut system = sysinfo::System::new_all();
    system.refresh_processes();
    let mut pids = Vec::new();
    for (pid, process) in system.processes() {
        if ![
            "tura_router",
            "tura_session_db",
            "tura_gateway",
            "tura_runtime",
        ]
        .iter()
        .any(|name| process_name_matches(process.name(), name))
        {
            continue;
        }
        let Some(exe) = process.exe() else {
            continue;
        };
        if canonical_or_self(exe).starts_with(&target) {
            pids.push(pid.as_u32());
        }
    }
    pids.sort_unstable();
    Ok(pids)
}

pub(crate) fn terminate_process_quietly(pid: u32) {
    let mut system = sysinfo::System::new_all();
    system.refresh_processes();
    if let Some(process) = system.process(sysinfo::Pid::from_u32(pid)) {
        let _ = process.kill();
    }
}

pub(crate) fn wait_for_process_pid(binary: &str, cwd: &Path, timeout: Duration) -> Result<u32> {
    let started = Instant::now();
    let mut last_error = None;
    while started.elapsed() < timeout {
        match process_pids_by_name_and_cwd(binary, cwd) {
            Ok(pids) if !pids.is_empty() => return Ok(pids[0]),
            Ok(_) => {
                last_error = Some(anyhow!(
                    "no {binary} process with cwd {} found yet",
                    cwd.display()
                ))
            }
            Err(error) => last_error = Some(error),
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    Err(last_error.unwrap_or_else(|| anyhow!("{binary} process did not appear")))
}

pub(crate) fn wait_for_process_pid_change(
    binary: &str,
    cwd: &Path,
    previous: u32,
    timeout: Duration,
) -> Result<u32> {
    let started = Instant::now();
    let mut last_seen = None;
    while started.elapsed() < timeout {
        for pid in process_pids_by_name_and_cwd(binary, cwd)? {
            last_seen = Some(pid);
            if pid != previous {
                return Ok(pid);
            }
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    bail!(
        "{binary} process pid did not change from {previous}; last seen {:?}",
        last_seen
    )
}

pub(crate) fn process_pids_by_name_and_cwd(binary: &str, cwd: &Path) -> Result<Vec<u32>> {
    let expected_cwd = canonical_or_self(cwd);
    let mut system = sysinfo::System::new_all();
    system.refresh_processes();
    let mut pids = Vec::new();
    for (pid, process) in system.processes() {
        if !process_name_matches(process.name(), binary) {
            continue;
        }
        let Some(process_cwd) = process.cwd() else {
            continue;
        };
        if canonical_or_self(process_cwd) == expected_cwd {
            pids.push(pid.as_u32());
        }
    }
    pids.sort_unstable();
    Ok(pids)
}

pub(crate) fn process_name_matches(name: &str, binary: &str) -> bool {
    let normalize = |value: &str| value.trim().trim_end_matches(".exe").to_ascii_lowercase();
    normalize(name) == normalize(binary)
}

pub(crate) fn canonical_or_self(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

pub(crate) fn lock_files(home: &Path, kind: &str) -> Result<Vec<PathBuf>> {
    let lock_dir = home.join(".tura").join("locks");
    let mut files = Vec::new();
    let entries = match std::fs::read_dir(&lock_dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(files),
        Err(error) => {
            return Err(error).with_context(|| format!("read lock dir {}", lock_dir.display()));
        }
    };
    let prefix = format!("{kind}-");
    for entry in entries {
        let entry = entry?;
        let file_name = entry.file_name();
        let file_name = file_name.to_string_lossy();
        if !file_name.starts_with(&prefix) || !file_name.ends_with(".lock") {
            continue;
        }
        files.push(entry.path());
    }
    files.sort();
    Ok(files)
}

pub(crate) fn kill_process(pid: u32) -> Result<()> {
    if pid == std::process::id() {
        bail!("refusing to kill the current test process");
    }
    let pid_arg = pid.to_string();
    let status = if cfg!(windows) {
        Command::new("taskkill")
            .args(["/PID", pid_arg.as_str(), "/F"])
            .status()
            .with_context(|| format!("taskkill pid {pid}"))?
    } else {
        Command::new("kill")
            .args(["-9", pid_arg.as_str()])
            .status()
            .with_context(|| format!("kill pid {pid}"))?
    };
    if !status.success() {
        bail!("kill process {pid} failed with {status}");
    }
    Ok(())
}

pub(crate) fn write_stale_router_endpoint(home: &Path) -> Result<()> {
    let addr = reserved_closed_addr()?;
    let path = router_addr_path(home);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, serde_json::to_string(&json!({ "addr": addr }))?)?;
    Ok(())
}

pub(crate) fn write_stale_session_db_endpoint(home: &Path) -> Result<()> {
    let addr = reserved_closed_addr()?;
    let path = service_addr_path(home);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, addr)?;
    Ok(())
}

pub(crate) fn publish_session_db_endpoint(home: &Path, addr: &str) -> Result<()> {
    let path = service_addr_path(home);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, addr)?;
    Ok(())
}

pub(crate) fn read_endpoint_json(path: &Path) -> Result<serde_json::Value> {
    let raw = std::fs::read_to_string(path)
        .with_context(|| format!("read endpoint JSON {}", path.display()))?;
    serde_json::from_str(raw.trim())
        .with_context(|| format!("parse endpoint JSON {}", path.display()))
}

pub(crate) fn endpoint_pid(endpoint: &serde_json::Value) -> Option<u32> {
    endpoint
        .get("pid")
        .and_then(serde_json::Value::as_u64)
        .and_then(|value| u32::try_from(value).ok())
}

pub(crate) fn publish_router_endpoint(home: &Path, endpoint: &serde_json::Value) -> Result<()> {
    let path = router_addr_path(home);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, serde_json::to_string(endpoint)?)?;
    Ok(())
}

pub(crate) struct UnresponsiveEndpoint {
    addr: String,
    stop: Arc<AtomicBool>,
    handle: Option<thread::JoinHandle<Result<()>>>,
}

impl UnresponsiveEndpoint {
    fn start() -> Result<Self> {
        Self::start_holding(Duration::from_secs(2))
    }

    fn start_holding(connection_hold: Duration) -> Result<Self> {
        let listener = TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0)))?;
        listener.set_nonblocking(true)?;
        let addr = listener.local_addr()?.to_string();
        let stop = Arc::new(AtomicBool::new(false));
        let thread_stop = Arc::clone(&stop);
        let handle = thread::spawn(move || -> Result<()> {
            while !thread_stop.load(Ordering::SeqCst) {
                match listener.accept() {
                    Ok((stream, _)) => {
                        let stream_stop = Arc::clone(&thread_stop);
                        thread::spawn(move || {
                            let _stream = stream;
                            let started = Instant::now();
                            while !stream_stop.load(Ordering::SeqCst)
                                && started.elapsed() < connection_hold
                            {
                                thread::sleep(Duration::from_millis(25));
                            }
                        });
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(25));
                    }
                    Err(error) => return Err(error.into()),
                }
            }
            Ok(())
        });
        Ok(Self {
            addr,
            stop,
            handle: Some(handle),
        })
    }
}

impl Drop for UnresponsiveEndpoint {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        let _ = TcpStream::connect(&self.addr);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

pub(crate) fn running_session_payload(
    session_id: &str,
    workspace: &Path,
    timestamp: i64,
) -> serde_json::Value {
    json!({
        "id": session_id,
        "name": "Process Recovery",
        "directory": workspace.to_string_lossy(),
        "created_at": timestamp,
        "updated_at": timestamp,
        "management": {
            "session_id": session_id,
            "session_name": "Process Recovery",
            "session_directory": workspace.to_string_lossy(),
            "session_created_at": "2026-06-12T00:00:00.000Z",
            "session_last_update_at": "2026-06-12T00:00:00.000Z",
            "state": "running"
        }
    })
}

pub(crate) fn message_payload(
    session_id: &str,
    message_id: &str,
    role: &str,
    timestamp: i64,
    content: &str,
) -> serde_json::Value {
    json!({
        "id": message_id,
        "session_id": session_id,
        "role": role,
        "created_at": timestamp,
        "updated_at": timestamp,
        "content": content
    })
}

pub(crate) fn wait_for_http_ok(port: u16, path: &str, timeout: Duration) -> Result<()> {
    let started = Instant::now();
    let mut last_error = None;
    while started.elapsed() < timeout {
        match http_get(port, path, Duration::from_millis(900)) {
            Ok(response) if response.starts_with("HTTP/1.1 200") => return Ok(()),
            Ok(response) => last_error = Some(anyhow!("unexpected response: {response}")),
            Err(error) => last_error = Some(error),
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    Err(last_error.unwrap_or_else(|| anyhow!("HTTP {path} did not become healthy")))
}

pub(crate) fn http_json_with_timeout(
    port: u16,
    path: &str,
    timeout: Duration,
) -> Result<serde_json::Value> {
    let response = http_get(port, path, timeout)?;
    if !response.starts_with("HTTP/1.1 200") {
        bail!("GET {path} returned non-200 response: {response}");
    }
    let body = response
        .split("\r\n\r\n")
        .nth(1)
        .ok_or_else(|| anyhow!("HTTP response missing body"))?;
    serde_json::from_str(body.trim()).context("parse HTTP JSON body")
}

pub(crate) fn http_get(port: u16, path: &str, timeout: Duration) -> Result<String> {
    let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
    let mut stream = TcpStream::connect_timeout(&addr, timeout)
        .with_context(|| format!("connect gateway on {addr}"))?;
    stream.set_read_timeout(Some(timeout))?;
    stream.set_write_timeout(Some(timeout))?;
    let request =
        format!("GET {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n\r\n");
    stream
        .write_all(request.as_bytes())
        .with_context(|| format!("write HTTP request {path} to {addr}"))?;
    read_http_response(&mut stream, path, addr)
}

fn read_http_response(stream: &mut TcpStream, path: &str, addr: SocketAddr) -> Result<String> {
    let mut reader = BufReader::new(stream);
    let mut header = Vec::new();
    loop {
        let mut byte = [0_u8; 1];
        let read = reader
            .read(&mut byte)
            .with_context(|| format!("read HTTP response header {path} from {addr}"))?;
        if read == 0 {
            break;
        }
        header.push(byte[0]);
        if header.ends_with(b"\r\n\r\n") {
            break;
        }
    }
    if !header.ends_with(b"\r\n\r\n") {
        bail!(
            "HTTP response {path} from {addr} ended before complete headers: {}",
            String::from_utf8_lossy(&header)
        );
    }
    let header_text = String::from_utf8_lossy(&header);
    let content_length = header_text
        .lines()
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.eq_ignore_ascii_case("content-length")
                .then(|| value.trim().parse::<usize>().ok())
                .flatten()
        })
        .ok_or_else(|| anyhow!("HTTP response {path} from {addr} missing Content-Length"))?;
    let mut body = vec![0_u8; content_length];
    reader
        .read_exact(&mut body)
        .with_context(|| format!("read HTTP response body {path} from {addr}"))?;
    let mut response = header;
    response.extend_from_slice(&body);
    String::from_utf8(response).context("HTTP response was not valid UTF-8")
}

pub(crate) fn ensure_backend_binaries(repo: &Path) -> Result<()> {
    for (package, binary) in [
        ("session_log", "tura_session_db"),
        ("router", "tura_router"),
        ("gateway", "tura_gateway"),
    ] {
        let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
        let status = Command::new(cargo)
            .current_dir(repo)
            .args(["build", "-p", package, "--bin", binary])
            .status()
            .with_context(|| format!("build {package}::{binary}"))?;
        if !status.success() {
            bail!("cargo build -p {package} --bin {binary} failed with {status}");
        }
    }
    Ok(())
}

pub(crate) fn debug_bin(repo: &Path, binary: &str) -> PathBuf {
    target_dir(repo).join("debug").join(exe_name(binary))
}

pub(crate) fn target_dir(repo: &Path) -> PathBuf {
    std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| repo.join("target"))
}

pub(crate) fn exe_name(binary: &str) -> String {
    if cfg!(windows) {
        format!("{binary}.exe")
    } else {
        binary.to_string()
    }
}

pub(crate) fn router_addr_path(home: &Path) -> PathBuf {
    home.join("db").join("session_log").join("router.addr")
}

pub(crate) fn service_addr_path(home: &Path) -> PathBuf {
    home.join("db").join("session_log").join("service.addr")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn http_get_returns_complete_content_length_response_without_waiting_for_close() -> Result<()> {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?;
        let port = listener.local_addr()?.port();
        let handle = thread::spawn(move || -> Result<()> {
            let (mut stream, _) = listener.accept()?;
            stream.set_read_timeout(Some(Duration::from_secs(1)))?;
            let mut request_line = String::new();
            BufReader::new(stream.try_clone()?).read_line(&mut request_line)?;
            assert!(
                request_line.starts_with("GET /service/status HTTP/1.1"),
                "unexpected request line: {request_line}"
            );
            let body = r#"{"router":{"status":"running"}}"#;
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: keep-alive\r\n\r\n{}",
                body.len(),
                body
            )?;
            stream.flush()?;
            thread::sleep(Duration::from_secs(1));
            Ok(())
        });

        let response = http_get(port, "/service/status", Duration::from_millis(200))?;
        assert!(response.starts_with("HTTP/1.1 200"));
        assert!(response.ends_with(r#"{"router":{"status":"running"}}"#));
        handle
            .join()
            .map_err(|_| anyhow!("HTTP fixture thread panicked"))??;
        Ok(())
    }
}

pub(crate) fn temp_root(prefix: &str) -> Result<PathBuf> {
    let nonce = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let path = std::env::temp_dir().join(format!("{prefix}-{}-{nonce}", std::process::id()));
    std::fs::create_dir_all(&path)?;
    Ok(path)
}

pub(crate) fn free_port() -> Result<u16> {
    Ok(
        TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0)))?
            .local_addr()?
            .port(),
    )
}

pub(crate) fn reserved_closed_addr() -> Result<String> {
    Ok(SocketAddr::from((Ipv4Addr::LOCALHOST, 1)).to_string())
}

pub(crate) fn process_log_file(home: &Path, name: &str) -> Result<std::fs::File> {
    process_log_file_at(&process_log_path(home, name))
}

pub(crate) fn process_log_path(home: &Path, name: &str) -> PathBuf {
    home.join(".tura").join("test-logs").join(name)
}

pub(crate) fn process_log_file_at(path: &Path) -> Result<std::fs::File> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::File::create(path).with_context(|| format!("create process log {}", path.display()))
}

pub(crate) fn file_tail(path: &Path, max_bytes: usize) -> String {
    let Ok(bytes) = std::fs::read(path) else {
        return format!("{} unavailable", path.display());
    };
    let start = bytes.len().saturating_sub(max_bytes);
    String::from_utf8_lossy(&bytes[start..]).replace(['\r', '\n'], "\\n")
}

pub(crate) fn read_pipe(pipe: Option<impl Read>) -> String {
    let Some(mut pipe) = pipe else {
        return String::new();
    };
    let mut output = String::new();
    let _ = pipe.read_to_string(&mut output);
    output
}
