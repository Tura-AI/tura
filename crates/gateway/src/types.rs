use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ChannelKind {
    Telegram,
    Other(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum AttachmentKind {
    Image,
    Video,
    VoiceAudio,
    AudioFile,
    Document,
    StickerInfo,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayAttachment {
    pub kind: AttachmentKind,
    pub path: Option<PathBuf>,
    pub file_name: Option<String>,
    pub mime_type: Option<String>,
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InboundMessage {
    pub channel: ChannelKind,
    pub conversation_id: String,
    pub message_id: String,
    pub sender_id: String,
    pub text: String,
    pub attachments: Vec<GatewayAttachment>,
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessedInboundMessage {
    pub raw: InboundMessage,
    pub user_content: Vec<Value>,
    pub history_text_entry: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OutboundAction {
    Typing,
    SendText {
        text: String,
        reply_to_message_id: Option<String>,
    },
    EditText {
        message_id: String,
        text: String,
    },
    SendReaction {
        target_message_id: String,
        emoji: String,
    },
    SendSticker {
        file_id: String,
    },
    SendMedia {
        media_type: OutboundMediaType,
        file_path: PathBuf,
        caption: Option<String>,
        reply_to_message_id: Option<String>,
    },
    Status {
        text: String,
        emoji: Option<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum OutboundMediaType {
    Photo,
    Video,
    Voice,
    Audio,
    Document,
}
