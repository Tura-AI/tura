use serde_json::Value;

use crate::tura_llm::{CallOptions, ProviderResponse, TuraError};

pub async fn call(
    _base_url: &str,
    model: &str,
    _api_key: &str,
    _messages: &[Value],
    _options: &CallOptions,
) -> Result<ProviderResponse, TuraError> {
    Err(TuraError::ProviderRequest {
        provider: "bedrock".to_string(),
        message: format!(
            "Bedrock provider `{model}` is explicitly unsupported in this build; enable an aws-sdk-bedrockruntime implementation before configuring it."
        ),
    })
}
