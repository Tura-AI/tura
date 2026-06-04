pub mod execute_tool;
pub mod send_calldata;

pub use execute_tool::{dequeue_tool_call, execute_tool, ToolExecutionResult};
pub use send_calldata::{send_calldata, CallData, CallbackData};

pub mod types {
    use chrono::{DateTime, Utc};
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct ToolRouterQueueItem {
        pub tool_name: String,
        pub arguments: serde_json::Value,
        pub session_id: String,
        pub runtime_id: String,
        pub agent_id: String,
        pub enqueued_at: DateTime<Utc>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct ToolExecutionRequest {
        pub tool_name: String,
        pub arguments: serde_json::Value,
        pub callback: Option<String>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct ToolExecutionResponse {
        pub ok: bool,
        pub result: serde_json::Value,
        pub error: Option<String>,
    }
}
