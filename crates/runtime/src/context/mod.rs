mod context_management;
pub mod docker_snapshot;
pub mod process_snapshot;
mod workspace_snapshot;

pub use context_management::{
    accumulate_message, accumulate_tool_result, accumulate_tool_result_with_feedback,
    accumulate_tool_result_with_provider_metadata, build_context, build_messages_from_session,
    messages_with_runtime_context, ContextInput, ContextOutput,
};
pub(crate) use workspace_snapshot::WorkspaceSnapshot;

pub trait ContextualUserFragment {
    const ROLE: &'static str;
    const START_MARKER: &'static str;
    const END_MARKER: &'static str;

    fn body(&self) -> String;

    fn matches_text(text: &str) -> bool
    where
        Self: Sized,
    {
        if Self::START_MARKER.is_empty() || Self::END_MARKER.is_empty() {
            return false;
        }

        let trimmed = text.trim_start();
        let starts_with_marker = trimmed
            .get(..Self::START_MARKER.len())
            .is_some_and(|candidate| candidate.eq_ignore_ascii_case(Self::START_MARKER));
        let trimmed = trimmed.trim_end();
        let ends_with_marker = trimmed
            .get(trimmed.len().saturating_sub(Self::END_MARKER.len())..)
            .is_some_and(|candidate| candidate.eq_ignore_ascii_case(Self::END_MARKER));
        starts_with_marker && ends_with_marker
    }

    fn render(&self) -> String {
        if Self::START_MARKER.is_empty() && Self::END_MARKER.is_empty() {
            return self.body();
        }

        format!("{}{}{}", Self::START_MARKER, self.body(), Self::END_MARKER)
    }
}

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
