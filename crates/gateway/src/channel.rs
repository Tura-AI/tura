use anyhow::Result;
use async_trait::async_trait;

use crate::types::OutboundAction;

#[async_trait]
pub trait ChannelSender: Send + Sync {
    async fn send_action(&self, conversation_id: &str, action: OutboundAction) -> Result<()>;
}
