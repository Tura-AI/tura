use serde_json::Value;

use crate::llm::bedrock;
use crate::tura_llm::{CallOptions, ProviderResponse, TuraError};

pub async fn call(
    base_url: &str,
    model: &str,
    api_key: &str,
    messages: &[Value],
    options: &CallOptions,
) -> Result<ProviderResponse, TuraError> {
    bedrock::call(base_url, model, api_key, messages, options).await
}

#[cfg(test)]
mod tests {
    use super::super::{parameter_policy, ProviderApiStyle};

    #[test]
    fn bedrock_provider_has_explicit_unsupported_parameter_policy() {
        let policy = parameter_policy("bedrock");
        assert_eq!(policy.api_style, ProviderApiStyle::Bedrock);
        assert_eq!(policy.metrics_style, ProviderApiStyle::Bedrock);
        assert!(!policy.supports_forced_tool_choice);
        assert!(policy.ignored_parameters.contains(&"stream_options"));
    }
}
