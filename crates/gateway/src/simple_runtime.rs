use anyhow::Result;
use async_trait::async_trait;

use crate::channel::ChannelSender;
use crate::types::{InboundMessage, OutboundAction};

#[async_trait]
pub trait SimpleMessageHandler: Send + Sync {
    async fn handle_message(&self, message: InboundMessage) -> Result<Vec<OutboundAction>>;
}

pub struct SimpleGatewayRuntime<H, C> {
    handler: H,
    sender: C,
}

impl<H, C> SimpleGatewayRuntime<H, C> {
    pub fn new(handler: H, sender: C) -> Self {
        Self { handler, sender }
    }
}

impl<H, C> SimpleGatewayRuntime<H, C>
where
    H: SimpleMessageHandler,
    C: ChannelSender,
{
    pub async fn process_message(&self, message: InboundMessage) -> Result<()> {
        let conversation_id = message.conversation_id.clone();
        let actions = self.handler.handle_message(message).await?;

        for action in actions {
            self.sender.send_action(&conversation_id, action).await?;
        }

        Ok(())
    }
}
