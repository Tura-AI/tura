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
    if options.force_search {
        return openapi::force_search(base_url, model, api_key, messages, options).await;
    }

    // Non-OAuth OpenAI now rides the shared Responses core (the `chatgpt`
    // sub-branch of the openapi response tier).
    crate::llm::providers::chatgpt::call_with_stream_events(
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
    fn openai_provider_uses_responses_policy_with_full_parameter_support() {
        let policy = parameter_policy("openai");
        assert_eq!(policy.api_style, ProviderApiStyle::CodexResponses);
        assert_eq!(policy.metrics_style, ProviderApiStyle::OpenApi);
        assert!(policy.supports_forced_tool_choice);
        assert!(policy.supports_stream_usage);
        assert!(policy.supports_reasoning_effort);
        assert!(policy.supports_service_tier);
        assert!(policy.supports_prompt_cache_key);
        assert!(policy.ignored_parameters.is_empty());
    }
}
