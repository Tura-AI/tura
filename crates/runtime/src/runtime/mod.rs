pub mod call_runtime;
pub mod create_runtime;
pub mod runtime_receive;

pub use call_runtime::call_runtime;
pub use create_runtime::create_runtime;
pub use runtime_receive::runtime_receive;

pub mod types {
    pub use crate::state_machine::agent_management::AgentId;
    pub use crate::state_machine::runtime_management::{
        RuntimeError, RuntimeManagement, RuntimeProviderConfig, ToolCallRecord, UsageReport,
    };

    use chrono::{DateTime, Utc};
    use lifecycle::{RuntimeId, SessionId};
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct RuntimeQueueItem {
        pub runtime_id: RuntimeId,
        pub session_id: SessionId,
        pub agent_id: AgentId,
        pub messages: Vec<serde_json::Value>,
        pub tools: Vec<serde_json::Value>,
        pub provider_name: String,
        pub created_at: DateTime<Utc>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct StreamChunk {
        pub runtime_id: RuntimeId,
        pub session_id: SessionId,
        pub chunk_type: StreamChunkType,
        pub content: String,
        pub tool_call: Option<ToolCallData>,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(tag = "type")]
    pub enum StreamChunkType {
        Text,
        ToolCall,
        Reasoning,
        Done,
        Error,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct ToolCallData {
        pub tool_name: String,
        pub arguments: serde_json::Value,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub provider_metadata: Option<serde_json::Value>,
    }
}
