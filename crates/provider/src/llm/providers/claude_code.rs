use serde_json::Value;

use crate::llm::claude_code;
use crate::tura_llm::{CallOptions, ProviderResponse, ProviderStreamEventSink, TuraError};

pub async fn call_with_stream_events(
    base_url: &str,
    model: &str,
    access_token: &str,
    messages: &[Value],
    options: &CallOptions,
    stream_events: Option<ProviderStreamEventSink>,
) -> Result<ProviderResponse, TuraError> {
    claude_code::call_with_stream_events(
        base_url,
        model,
        access_token,
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
    fn claude_code_provider_uses_anthropic_native_policy() {
        let policy = parameter_policy("claude-code");
        assert_eq!(policy.api_style, ProviderApiStyle::AnthropicMessages);
        assert_eq!(policy.metrics_style, ProviderApiStyle::AnthropicMessages);
        assert!(policy.supports_forced_tool_choice);
        assert!(!policy.supports_stream_usage);
        assert!(policy.ignored_parameters.contains(&"stream_options"));
    }
}
