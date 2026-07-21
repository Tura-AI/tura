//! Narrow gateway write path for session lifecycle operations.
//!
//! Runtime owns turn/checkpoint writes. Gateway only writes session lifecycle
//! mutations that are initiated directly by frontend actions.

use anyhow::{anyhow, Result};
use session_log_contract::{SessionLogCommand, SessionLogResponse};

pub fn write_session_log(command: SessionLogCommand) -> Result<()> {
    if session_log_contract::client::service_is_running() {
        match session_log_contract::client::call_service(&command) {
            Ok(SessionLogResponse::Ok) => return Ok(()),
            Ok(SessionLogResponse::Error { error }) => {
                return Err(anyhow!("session_log write failed: {error}"));
            }
            Ok(response) => {
                return Err(anyhow!(
                    "unexpected session_log write response: {response:?}"
                ))
            }
            Err(error) => {
                tracing::warn!(error = %error, "session_log IPC write failed; enqueueing write");
            }
        }
    }

    session_log_contract::client::enqueue_command(&command)?;
    Ok(())
}
