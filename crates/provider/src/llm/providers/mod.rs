pub mod bedrock;
pub mod chatgpt;
pub mod claude_code;
pub mod codex;
pub mod google;
pub mod minimax;
pub mod openai;
pub mod qwen;
pub mod xai;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ProviderApiStyle {
    OpenApi,
    CodexResponses,
    Google,
    Bedrock,
    AnthropicMessages,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ProviderParameterPolicy {
    pub api_style: ProviderApiStyle,
    pub metrics_style: ProviderApiStyle,
    pub supports_forced_tool_choice: bool,
    pub supports_stream_usage: bool,
    pub supports_reasoning_effort: bool,
    pub supports_service_tier: bool,
    pub supports_prompt_cache_key: bool,
    pub ignored_parameters: &'static [&'static str],
}

pub(crate) fn parameter_policy(provider: &str) -> ProviderParameterPolicy {
    match provider.to_ascii_lowercase().as_str() {
        "codex" => ProviderParameterPolicy {
            api_style: ProviderApiStyle::CodexResponses,
            metrics_style: ProviderApiStyle::OpenApi,
            supports_forced_tool_choice: true,
            supports_stream_usage: true,
            supports_reasoning_effort: true,
            supports_service_tier: true,
            supports_prompt_cache_key: true,
            ignored_parameters: &[],
        },
        "claude-code" => ProviderParameterPolicy {
            api_style: ProviderApiStyle::AnthropicMessages,
            metrics_style: ProviderApiStyle::AnthropicMessages,
            supports_forced_tool_choice: true,
            supports_stream_usage: false,
            supports_reasoning_effort: true,
            supports_service_tier: false,
            supports_prompt_cache_key: false,
            ignored_parameters: &[
                "stream_options",
                "service_tier",
                "prompt_cache_key",
                "parallel_tool_calls",
            ],
        },
        // The non-codex Responses tier: standard OpenAI (chatgpt), xAI (grok),
        // and Alibaba DashScope (qwen). They share the codex Responses request
        // shape but are driven by an API key. `metrics_style` stays OpenApi
        // because the streamed usage block is OpenAI-shaped.
        "openai" | "openai-api" | "chatgpt" => ProviderParameterPolicy {
            api_style: ProviderApiStyle::CodexResponses,
            metrics_style: ProviderApiStyle::OpenApi,
            supports_forced_tool_choice: true,
            supports_stream_usage: true,
            supports_reasoning_effort: true,
            supports_service_tier: true,
            supports_prompt_cache_key: true,
            ignored_parameters: &[],
        },
        "xai" | "grok" => ProviderParameterPolicy {
            api_style: ProviderApiStyle::CodexResponses,
            metrics_style: ProviderApiStyle::OpenApi,
            supports_forced_tool_choice: true,
            supports_stream_usage: true,
            supports_reasoning_effort: true,
            supports_service_tier: false,
            supports_prompt_cache_key: false,
            ignored_parameters: &["service_tier", "prompt_cache_key"],
        },
        "qwen" | "qwen_cn" | "qwen-cn" => ProviderParameterPolicy {
            api_style: ProviderApiStyle::CodexResponses,
            metrics_style: ProviderApiStyle::OpenApi,
            supports_forced_tool_choice: true,
            supports_stream_usage: true,
            supports_reasoning_effort: true,
            supports_service_tier: false,
            supports_prompt_cache_key: false,
            ignored_parameters: &["service_tier", "prompt_cache_key"],
        },
        "google" => ProviderParameterPolicy {
            api_style: ProviderApiStyle::Google,
            metrics_style: ProviderApiStyle::Google,
            // Forced/constrained tool choice is supported via Gemini
            // `toolConfig.functionCallingConfig`.
            supports_forced_tool_choice: true,
            supports_stream_usage: false,
            supports_reasoning_effort: false,
            supports_service_tier: false,
            supports_prompt_cache_key: false,
            ignored_parameters: &[
                "stream_options",
                "reasoning_effort",
                "service_tier",
                "prompt_cache_key",
            ],
        },
        "bedrock" => ProviderParameterPolicy {
            api_style: ProviderApiStyle::Bedrock,
            metrics_style: ProviderApiStyle::Bedrock,
            supports_forced_tool_choice: false,
            supports_stream_usage: false,
            supports_reasoning_effort: false,
            supports_service_tier: false,
            supports_prompt_cache_key: false,
            ignored_parameters: &[
                "tool_choice",
                "stream_options",
                "reasoning_effort",
                "service_tier",
                "prompt_cache_key",
            ],
        },
        _ => ProviderParameterPolicy {
            api_style: ProviderApiStyle::OpenApi,
            metrics_style: ProviderApiStyle::OpenApi,
            supports_forced_tool_choice: true,
            supports_stream_usage: true,
            supports_reasoning_effort: true,
            supports_service_tier: false,
            supports_prompt_cache_key: true,
            ignored_parameters: &["service_tier"],
        },
    }
}

#[cfg(test)]
mod tests {
    use super::{parameter_policy, ProviderApiStyle};

    #[test]
    fn all_configured_provider_families_have_parameter_policies() {
        let providers = [
            "codex",
            "openai",
            "google",
            "bedrock",
            "minimax",
            "deepseek",
            "moonshotai",
            "openrouter",
            "qwen",
            "anthropic",
        ];

        for provider in providers {
            let policy = parameter_policy(provider);
            assert!(
                !policy.ignored_parameters.contains(&""),
                "{provider} policy should not contain empty ignored parameters"
            );
        }
    }

    #[test]
    fn openapi_compatible_providers_share_openapi_metrics_and_ignore_service_tier() {
        for provider in ["minimax", "deepseek", "moonshotai", "openrouter", "anthropic"] {
            let policy = parameter_policy(provider);
            assert_eq!(policy.api_style, ProviderApiStyle::OpenApi);
            assert_eq!(policy.metrics_style, ProviderApiStyle::OpenApi);
            assert!(policy.supports_stream_usage);
            assert!(policy.supports_forced_tool_choice);
            assert!(policy.ignored_parameters.contains(&"service_tier"));
        }
    }

    #[test]
    fn codex_uses_responses_api_but_openapi_usage_shape() {
        let policy = parameter_policy("codex");
        assert_eq!(policy.api_style, ProviderApiStyle::CodexResponses);
        assert_eq!(policy.metrics_style, ProviderApiStyle::OpenApi);
        assert!(policy.supports_reasoning_effort);
        assert!(policy.supports_prompt_cache_key);
        assert!(policy.supports_service_tier);
    }

    #[test]
    fn google_and_bedrock_explicitly_ignore_openapi_only_parameters() {
        for provider in ["google", "bedrock"] {
            let policy = parameter_policy(provider);
            assert!(!policy.supports_stream_usage);
            assert!(!policy.supports_reasoning_effort);
            assert!(!policy.supports_prompt_cache_key);
            assert!(policy.ignored_parameters.contains(&"stream_options"));
            assert!(policy.ignored_parameters.contains(&"reasoning_effort"));
        }
    }
}
