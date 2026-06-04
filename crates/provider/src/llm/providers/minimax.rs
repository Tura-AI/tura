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
    openapi::call_with_stream_events(
        base_url,
        model,
        "minimax",
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
    fn minimax_provider_uses_openapi_policy_and_ignores_service_tier() {
        let policy = parameter_policy("minimax");
        assert_eq!(policy.api_style, ProviderApiStyle::OpenApi);
        assert_eq!(policy.metrics_style, ProviderApiStyle::OpenApi);
        assert!(policy.supports_stream_usage);
        assert!(policy.supports_forced_tool_choice);
        assert!(policy.ignored_parameters.contains(&"service_tier"));
    }
}
