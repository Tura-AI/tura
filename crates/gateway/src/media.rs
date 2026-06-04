//! Media processing for gateway
//! Handles attachment processing and message conversion

use crate::types::{InboundMessage, ProcessedInboundMessage};
use anyhow::Result;
use serde_json::Value;

pub struct GatewayMediaProcessor {
    // Placeholder for future media processing implementation
}

impl GatewayMediaProcessor {
    pub fn new() -> Self {
        Self {}
    }

    pub async fn process_message(&self, msg: InboundMessage) -> Result<ProcessedInboundMessage> {
        let user_content = vec![Value::String(msg.text.clone())];

        let history_text_entry = msg.text.clone();

        Ok(ProcessedInboundMessage {
            raw: msg,
            user_content,
            history_text_entry,
        })
    }
}

impl Default for GatewayMediaProcessor {
    fn default() -> Self {
        Self::new()
    }
}
