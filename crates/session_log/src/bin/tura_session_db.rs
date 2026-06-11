//! `tura_session_db` — the session-log SQLite owner.
//!
//! Gateway, router, runtime workers, and the CLI front reach the store through
//! its socket (`session_log::ipc`). Marking the role here keeps compatibility
//! with older role checks while the storage is embedded SQLite.

fn main() -> anyhow::Result<()> {
    std::env::set_var("TURA_ROLE", "session_db");
    session_log::service::run_socket_service()
}
