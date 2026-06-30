//! Required workspace-wide process and state management E2E tests.
//!
//! This required root-package business E2E is wired into the root workspace
//! package, so process lifecycle and state recovery run as mandatory local
//! correctness coverage instead of optional performance or live scripts.

#[path = "helpers/process_state_management.rs"]
mod helpers;

use helpers::*;

#[test]
fn process_state_management_handles_orphans_restarts_conflicts_and_cleanup() -> Result<()> {
    let _serial = SERIAL.lock().unwrap_or_else(|error| error.into_inner());
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    cleanup_target_backend_processes(&repo, Duration::from_secs(10))?;
    let _cleanup = TargetBackendCleanup { repo: repo.clone() };
    ensure_backend_binaries(&repo)?;

    stale_endpoints_are_replaced_gateway_restarts_and_conflicts_fail(&repo)
        .context("scenario stale endpoints, gateway restart, and same-home conflict")?;
    foreign_gateway_port_conflict_does_not_start_backend(&repo)
        .context("scenario foreign gateway port conflict")?;
    gateway_status_restarts_crashed_router_and_adopts_session_db(&repo)
        .context("scenario gateway restarts crashed router and adopts session_db")?;
    gateway_status_kills_unresponsive_router_and_restarts(&repo)
        .context("scenario gateway kills unresponsive router and restarts")?;
    router_health_restarts_crashed_session_db(&repo)
        .context("scenario router health restarts crashed session_db")?;
    router_health_restarts_unresponsive_session_db(&repo)
        .context("scenario router health restarts unresponsive session_db")?;
    orphan_session_db_is_adopted_and_stopped_by_router(&repo)
        .context("scenario router adopts orphan session_db")?;
    orphan_router_is_adopted_and_stopped_by_gateway(&repo)
        .context("scenario gateway adopts orphan router")?;
    router_keeps_command_run_when_runtime_socket_disconnects(&repo)
        .context("scenario router keeps command_run after runtime disconnect")?;
    gateway_stdin_eof_shuts_down_router_session_db_and_runtime(&repo)
        .context("scenario gateway EOF shuts down router/session_db/runtime")?;
    session_db_restart_marks_running_sessions_interrupted_without_losing_history(&repo)
        .context("scenario session_db restart marks running sessions interrupted")?;

    Ok(())
}
