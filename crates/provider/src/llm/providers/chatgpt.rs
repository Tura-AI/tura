//! `chatgpt` sub-branch of the openapi **Responses** tier: standard OpenAI
//! Responses API driven by an API key (as opposed to the `codex` OAuth route).
//! Used for the non-OAuth `openai` / `openai-api` providers.

use serde_json::Value;

use crate::llm::openapi;
use crate::tura_llm::{CallOptions, ProviderResponse, ProviderStreamEventSink, TuraError};

pub async fn embed(
    base_url: &str,
    model: &str,
    api_key: &str,
    text: &str,
) -> Result<Vec<f32>, TuraError> {
    openapi::embed(base_url, model, api_key, text).await
}

pub async fn call_with_stream_events(
    base_url: &str,
    model: &str,
    api_key: &str,
    messages: &[Value],
    options: &CallOptions,
    stream_events: Option<ProviderStreamEventSink>,
) -> Result<ProviderResponse, TuraError> {
    openapi::responses_api_key_call(
        "chatgpt",
        base_url,
        model,
        api_key,
        messages,
        options,
        stream_events,
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::super::{parameter_policy, ProviderApiStyle};

    #[test]
    fn chatgpt_uses_responses_api_with_openapi_metrics() {
        let policy = parameter_policy("chatgpt");
        assert_eq!(policy.api_style, ProviderApiStyle::CodexResponses);
        assert_eq!(policy.metrics_style, ProviderApiStyle::OpenApi);
        assert!(policy.supports_forced_tool_choice);
        assert!(policy.supports_reasoning_effort);
    }
}
