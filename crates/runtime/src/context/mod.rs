mod context_management;
pub mod docker_snapshot;
pub mod process_snapshot;

pub use context_management::{
    accumulate_message, accumulate_tool_result, accumulate_tool_result_with_feedback,
    build_context, build_messages_from_session, messages_with_runtime_context,
    ContextInput, ContextOutput,
};

pub mod types {
    use crate::state_machine::agent_management::AgentId;
    use crate::state_machine::session_management::SessionId;
    use chrono::{DateTime, Utc};
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct ContextItem {
        pub session_id: SessionId,
        pub agent_id: Option<AgentId>,
        pub context_type: ContextType,
        pub content: serde_json::Value,
        pub created_at: DateTime<Utc>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    #[serde(tag = "type")]
    pub enum ContextType {
        UserInput,
        AgentOutput,
        ToolResult,
        Reasoning,
        System,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct ContextState {
        pub session_id: SessionId,
        pub messages: Vec<serde_json::Value>,
        pub tool_results: Vec<serde_json::Value>,
        pub last_tool_call_response: Option<serde_json::Value>,
        pub reasoning_history: Vec<String>,
    }
}
