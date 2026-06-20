//! `tura_runtime` — the standalone per-session agent worker.
//!
//! Spawned by the router (one per session, killed on completion). It is no
//! longer the gateway binary re-invoked by role: it is its own binary so the
//! gateway/router no longer link the runtime execution entry. The router
//! performs a version handshake against the `health_check` reply before
//! dispatching work to it.

fn main() -> std::io::Result<()> {
    tura_path::process_hardening::harden_current_process("runtime_worker");
    std::env::set_var("TURA_ROLE", "runtime_worker");
    std::env::set_var("TURA_RUNTIME_WORKER", "1");
    runtime::worker::run()
}
