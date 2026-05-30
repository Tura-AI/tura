//! `qwen` sub-branch of the openapi **Responses** tier. Routed through Alibaba
//! DashScope's *international* compatible-mode endpoint (provider id `qwen`,
//! base url `https://dashscope-intl.aliyuncs.com/compatible-mode/v1`). The
//! shared Responses core handles the request shape; the quirk layer omits
//! OpenAI-only fields DashScope rejects.

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
        "qwen",
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
    fn qwen_uses_responses_api_with_openapi_metrics() {
        let policy = parameter_policy("qwen");
        assert_eq!(policy.api_style, ProviderApiStyle::CodexResponses);
        assert_eq!(policy.metrics_style, ProviderApiStyle::OpenApi);
        assert!(policy.supports_forced_tool_choice);
    }
}
