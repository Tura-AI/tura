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

#[cfg(test)]
mod tests {
    use super::GatewayRuntime;
    use crate::{
        channel::ChannelSender,
        handler::ProcessedMessageHandler,
        media::GatewayMediaProcessor,
        types::{ChannelKind, InboundMessage, OutboundAction, ProcessedInboundMessage},
    };
    use anyhow::{anyhow, Result};
    use async_trait::async_trait;
    use std::collections::HashMap;
    use std::sync::Mutex;

    struct TestHandler {
        result: Result<Vec<OutboundAction>>,
    }

    #[async_trait]
    impl ProcessedMessageHandler for TestHandler {
        async fn handle_processed_message(
            &self,
            message: ProcessedInboundMessage,
        ) -> Result<Vec<OutboundAction>> {
            assert_eq!(message.history_text_entry, "hello");
            self.result
                .as_ref()
                .map(Clone::clone)
                .map_err(|error| anyhow!(error.to_string()))
        }
    }

    struct TestSender {
        sent: Mutex<Vec<(String, OutboundAction)>>,
        fail: bool,
    }

    #[async_trait]
    impl ChannelSender for TestSender {
        async fn send_action(&self, conversation_id: &str, action: OutboundAction) -> Result<()> {
            if self.fail {
                return Err(anyhow!("send failed"));
            }
            self.sent
                .lock()
                .expect("test sender mutex should not be poisoned")
                .push((conversation_id.to_string(), action));
            Ok(())
        }
    }

    fn inbound_message() -> InboundMessage {
        InboundMessage {
            channel: ChannelKind::Other("test".to_string()),
            conversation_id: "conversation-1".to_string(),
            message_id: "message-1".to_string(),
            sender_id: "sender-1".to_string(),
            text: "hello".to_string(),
            attachments: Vec::new(),
            metadata: HashMap::new(),
        }
    }

    #[tokio::test]
    async fn process_message_sends_all_handler_actions_to_original_conversation() {
        let sender = TestSender {
            sent: Mutex::new(Vec::new()),
            fail: false,
        };
        let runtime = GatewayRuntime::new(
            GatewayMediaProcessor::new(),
            TestHandler {
                result: Ok(vec![
                    OutboundAction::Typing,
                    OutboundAction::SendText {
                        text: "ok".to_string(),
                        reply_to_message_id: Some("message-1".to_string()),
                    },
                ]),
            },
            sender,
        );

        runtime
            .process_message(inbound_message())
            .await
            .expect("message should process");

        let sent = runtime
            .sender
            .sent
            .lock()
            .expect("test sender mutex should not be poisoned");
        assert_eq!(sent.len(), 2);
        assert_eq!(sent[0].0, "conversation-1");
        assert!(matches!(sent[0].1, OutboundAction::Typing));
        match &sent[1].1 {
            OutboundAction::SendText {
                text,
                reply_to_message_id,
            } => {
                assert_eq!(text, "ok");
                assert_eq!(reply_to_message_id.as_deref(), Some("message-1"));
            }
            other => panic!("unexpected outbound action: {other:?}"),
        }
    }

    #[tokio::test]
    async fn process_message_returns_handler_error_without_sending_actions() {
        let runtime = GatewayRuntime::new(
            GatewayMediaProcessor::new(),
            TestHandler {
                result: Err(anyhow!("handler failed")),
            },
            TestSender {
                sent: Mutex::new(Vec::new()),
                fail: false,
            },
        );

        let error = runtime
            .process_message(inbound_message())
            .await
            .expect_err("handler error should be returned");

        assert!(error.to_string().contains("handler failed"));
        assert!(runtime
            .sender
            .sent
            .lock()
            .expect("test sender mutex should not be poisoned")
            .is_empty());
    }

    #[tokio::test]
    async fn process_message_stops_on_sender_error() {
        let runtime = GatewayRuntime::new(
            GatewayMediaProcessor::new(),
            TestHandler {
                result: Ok(vec![OutboundAction::Typing]),
            },
            TestSender {
                sent: Mutex::new(Vec::new()),
                fail: true,
            },
        );

        let error = runtime
            .process_message(inbound_message())
            .await
            .expect_err("sender error should be returned");

        assert!(error.to_string().contains("send failed"));
    }
}
