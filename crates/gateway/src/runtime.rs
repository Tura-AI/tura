use anyhow::Result;

use crate::{
    channel::ChannelSender, handler::ProcessedMessageHandler, media::GatewayMediaProcessor,
    types::InboundMessage,
};

pub struct GatewayRuntime<H, C> {
    media_processor: GatewayMediaProcessor,
    handler: H,
    sender: C,
}

impl<H, C> GatewayRuntime<H, C> {
    pub fn new(media_processor: GatewayMediaProcessor, handler: H, sender: C) -> Self {
        Self {
            media_processor,
            handler,
            sender,
        }
    }
}

impl<H, C> GatewayRuntime<H, C>
where
    H: ProcessedMessageHandler,
    C: ChannelSender,
{
    pub async fn process_message(&self, message: InboundMessage) -> Result<()> {
        let conversation_id = message.conversation_id.clone();

        let processed = self.media_processor.process_message(message).await?;
        let actions = self.handler.handle_processed_message(processed).await?;

        for action in actions {
            self.sender.send_action(&conversation_id, action).await?;
        }

        Ok(())
    }
}
