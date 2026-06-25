//! `tura_session_db` — the session-log SQLite owner.
//!
//! Gateway, router, runtime workers, and the CLI front reach the store through
//! its socket (`session_log::ipc`). The role marker identifies this process as
//! the embedded SQLite owner.

fn main() -> anyhow::Result<()> {
    tura_path::process_hardening::harden_current_process("session_db");
    std::env::set_var("TURA_ROLE", "session_db");
    session_log::service::run_socket_service()
}
