//! Typed runtime checkpoint client.

use anyhow::Result;
use session_log_contract::CommandCheckpoint;

#[derive(Debug, Clone, Default)]
pub struct CheckpointClient {
    inner: crate::session_log_client::SessionLogClient,
}

pub type SessionDbClient = CheckpointClient;

impl CheckpointClient {
    pub fn discover() -> Result<Self> {
        Ok(Self {
            inner: crate::session_log_client::SessionLogClient::discover()?,
        })
    }

    pub fn checkpoint_command_finished(&self, checkpoint: CommandCheckpoint) -> Result<()> {
        self.inner.apply_command_checkpoint(checkpoint)
    }
}
