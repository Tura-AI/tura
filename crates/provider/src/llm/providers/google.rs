use serde_json::Value;

use crate::llm::google;
use crate::tura_llm::{CallOptions, ProviderResponse, TuraError};

pub async fn embed(
    base_url: &str,
    model: &str,
    api_key: &str,
    text: &str,
) -> Result<Vec<f32>, TuraError> {
    google::embed(base_url, model, api_key, text).await
}

pub async fn call(
    base_url: &str,
    model: &str,
    api_key: &str,
    messages: &[Value],
    options: &CallOptions,
) -> Result<ProviderResponse, TuraError> {
    google::call(base_url, model, api_key, messages, options).await
}

#[cfg(test)]
mod tests {
    use super::super::{parameter_policy, ProviderApiStyle};

    #[test]
    fn google_provider_uses_google_api_policy_and_ignores_openapi_only_parameters() {
        let policy = parameter_policy("google");
        assert_eq!(policy.api_style, ProviderApiStyle::Google);
        assert_eq!(policy.metrics_style, ProviderApiStyle::Google);
        assert!(!policy.supports_stream_usage);
        assert!(policy.ignored_parameters.contains(&"tool_choice"));
        assert!(policy.ignored_parameters.contains(&"reasoning_effort"));
    }
}
