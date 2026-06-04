//! `grok` (xAI) sub-branch of the openapi **Responses** tier. xAI exposes an
//! OpenAI-compatible Responses API; the shared core handles the heavy lifting
//! while [`openapi::responses_api_key_call`]'s per-provider quirk layer skips
//! OpenAI-only knobs (e.g. encrypted reasoning content) that xAI rejects.

use serde_json::Value;

use crate::llm::openapi;
use crate::tura_llm::{CallOptions, ProviderResponse, ProviderStreamEventSink, TuraError};

pub async fn call_with_stream_events(
    base_url: &str,
    model: &str,
    api_key: &str,
    messages: &[Value],
    options: &CallOptions,
    stream_events: Option<ProviderStreamEventSink>,
) -> Result<ProviderResponse, TuraError> {
    openapi::responses_api_key_call(
        "grok",
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
    fn xai_uses_responses_api_with_openapi_metrics() {
        let policy = parameter_policy("xai");
        assert_eq!(policy.api_style, ProviderApiStyle::CodexResponses);
        assert_eq!(policy.metrics_style, ProviderApiStyle::OpenApi);
        assert!(policy.supports_forced_tool_choice);
    }
}
