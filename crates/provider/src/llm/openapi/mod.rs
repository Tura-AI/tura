//! OpenAI-compatible provider family.
//!
//! Historically this lived in one oversized `openapi.rs`. It is now split into
//! three focused submodules:
//!
//! * [`common`] — option-normalization + content-flattening helpers shared by
//!   both tiers.
//! * [`response`] — the **Responses API** tier (`/responses`, SSE): codex
//!   (OAuth) plus the API-key sub-providers `chatgpt`, `grok`, `qwen`.
//! * [`chat`] — the **Chat Completions** tier (`/chat/completions`): the
//!   default route for every other OpenAI-compatible provider.
//!
//! This module re-exports the small public surface the rest of the crate uses,
//! so external callers keep importing `crate::llm::openapi::{…}` unchanged.

mod chat;
mod common;
mod response;

pub(crate) use chat::force_search;
pub use chat::{call, call_with_stream_events, embed, embed_for_provider};
pub(crate) use response::{codex_oauth_call, responses_api_key_call};

#[cfg(test)]
pub(crate) use chat::process_chat_stream_line_for_test;
#[cfg(test)]
pub(crate) use chat::{
    build_chat_payload, emit_completed_tool_call, last_complete_minimax_invoke,
    normalize_messages_for_provider, StreamingToolCall,
};
#[cfg(test)]
pub(crate) use common::should_pass_service_tier;
#[cfg(test)]
pub(crate) use response::{
    append_codex_stream_text, build_codex_oauth_payload, build_responses_payload_for_provider,
    codex_event_tool_calls, complete_codex_tool_calls, normalize_codex_response_content,
    ready_streaming_tool_call, CodexCommandRunCommandCollector, CodexToolCallStreamCollector,
};

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
