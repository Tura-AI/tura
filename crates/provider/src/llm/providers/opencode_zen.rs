//! OpenCode Zen routes GPT-family models through the Responses API and other
//! catalog models through OpenAI-compatible chat completions.

use serde_json::Value;

use crate::llm::openapi;
use crate::tura_llm::{CallOptions, ProviderResponse, ProviderStreamEventSink, TuraError};

pub(crate) fn uses_responses_api(model: &str) -> bool {
    let model = model.trim();
    model.starts_with("gpt-") || model.starts_with("gpt_")
}

pub async fn call_with_stream_events(
    base_url: &str,
    model: &str,
    api_key: &str,
    messages: &[Value],
    options: &CallOptions,
    stream_events: Option<ProviderStreamEventSink>,
) -> Result<ProviderResponse, TuraError> {
    if uses_responses_api(model) {
        return crate::llm::providers::chatgpt::call_with_stream_events(
            base_url,
            model,
            api_key,
            messages,
            options,
            stream_events,
        )
        .await;
    }

    openapi::call_with_stream_events(
        base_url,
        model,
        "opencode-zen",
        api_key,
        messages,
        options,
        stream_events,
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::uses_responses_api;

    #[test]
    fn gpt_models_use_responses_api() {
        assert!(uses_responses_api("gpt-5.6-sol"));
        assert!(uses_responses_api("gpt-5.4-mini"));
    }

    #[test]
    fn non_gpt_models_use_chat_completions() {
        assert!(!uses_responses_api("claude-opus-4-8"));
        assert!(!uses_responses_api("deepseek-v4-pro"));
        assert!(!uses_responses_api("gemini-3.5-flash"));
    }
}
