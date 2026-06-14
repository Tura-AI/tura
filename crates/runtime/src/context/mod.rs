mod build;
mod command_run_streams;
mod compaction;
mod media;
mod text_truncate;
mod token_budget;
mod tool_results;
mod workspace;

pub(crate) const USER_AGENT_CONTEXT_ROLE: &str = "user-agent";

pub use build::{
    accumulate_message, accumulate_tool_result, accumulate_tool_result_with_provider_metadata,
    build_context, build_messages_from_session, user_input_content_matches,
    user_input_content_value, ContextInput, ContextOutput,
};
pub use compaction::compact_session_context;
pub(crate) use workspace::WorkspaceSnapshot;

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
    use crate::state_machine::session_management::SessionId;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct ContextState {
        pub session_id: SessionId,
        pub messages: Vec<serde_json::Value>,
        pub tool_results: Vec<serde_json::Value>,
        pub last_tool_call_response: Option<serde_json::Value>,
        pub reasoning_history: Vec<String>,
    }
}
