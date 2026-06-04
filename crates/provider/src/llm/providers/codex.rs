use serde_json::Value;

use crate::llm::openapi;
use crate::tura_llm::{CallOptions, ProviderResponse, ProviderStreamEventSink, TuraError};

pub async fn call_with_stream_events(
    _base_url: &str,
    model: &str,
    access_token: &str,
    messages: &[Value],
    options: &CallOptions,
    stream_events: Option<ProviderStreamEventSink>,
) -> Result<ProviderResponse, TuraError> {
    openapi::codex_oauth_call(model, access_token, messages, options, stream_events).await
}

#[cfg(test)]
mod tests {
    use super::super::{parameter_policy, ProviderApiStyle};

    #[test]
    fn codex_provider_uses_responses_api_with_openapi_metrics() {
        let policy = parameter_policy("codex");
        assert_eq!(policy.api_style, ProviderApiStyle::CodexResponses);
        assert_eq!(policy.metrics_style, ProviderApiStyle::OpenApi);
        assert!(policy.supports_stream_usage);
        assert!(policy.supports_forced_tool_choice);
    }
}
