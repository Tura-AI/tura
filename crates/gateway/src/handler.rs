use anyhow::Result;
use async_trait::async_trait;

use crate::types::{OutboundAction, ProcessedInboundMessage};

#[async_trait]
pub trait ProcessedMessageHandler: Send + Sync {
    async fn handle_processed_message(
        &self,
        message: ProcessedInboundMessage,
    ) -> Result<Vec<OutboundAction>>;
}
