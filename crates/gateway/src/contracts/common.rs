use serde::{Deserialize, Serialize};

pub use crate::types::{
    AttachmentKind, GatewayAttachment, InboundMessage, OutboundAction, OutboundMediaType,
    ProcessedInboundMessage,
};

// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BadRequestError {
    pub error: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpgradeRequest {
    pub target: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpgradeResponse {
    pub success: bool,
    pub version: Option<String>,
    pub error: Option<String>,
}
