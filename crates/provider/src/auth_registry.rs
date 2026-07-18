use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthMethodKind {
    ApiKey,
    #[serde(rename = "oauth_pkce")]
    OAuthPkce,
    BrowserToken,
    LocalCliToken,
    DeviceCode,
    AwsCredentials,
    None,
}

impl AuthMethodKind {
    pub const fn login_value(self) -> &'static str {
        match self {
            Self::ApiKey => "api",
            Self::OAuthPkce => "oauth",
            Self::BrowserToken => "browser",
            Self::LocalCliToken => "local",
            Self::DeviceCode => "device",
            Self::AwsCredentials => "aws",
            Self::None => "none",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthState {
    Unknown,
    NotConfigured,
    ApiKeyConfigured,
    OAuthStarting,
    OAuthWaitingForBrowser,
    OAuthWaitingForCallback,
    BrowserTokenRequired,
    LocalTokenDiscovered,
    Authenticated,
    Refreshing,
    Expired,
    Revoking,
    Revoked,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderRuntimeState {
    Unknown,
    Disabled,
    Configured,
    MissingAuth,
    Ready,
    Degraded,
    RateLimited,
    Paused,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OAuthAuthorizeKind {
    OpenAiPkce,
    AnthropicPkce,
    GooglePkce,
    GithubDevice,
    BrowserTokenPaste,
    Unsupported,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthMethodDescriptor {
    pub kind: AuthMethodKind,
    pub login: &'static str,
    pub label: &'static str,
}

impl AuthMethodDescriptor {
    pub const fn new(kind: AuthMethodKind, label: &'static str) -> Self {
        Self {
            kind,
            login: kind.login_value(),
            label,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct ProviderCapabilityFlags {
    pub supports_streaming: bool,
    pub supports_tool_call_streaming: bool,
    pub supports_cache_usage: bool,
    pub supports_reasoning_usage: bool,
    pub supports_subscription: bool,
    pub supports_api_key: bool,
    pub supports_oauth_refresh: bool,
    pub supports_model_validation: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProviderAuthRegistryEntry {
    pub provider_id: &'static str,
    pub runtime_provider_id: &'static str,
    pub display_name: &'static str,
    pub base_url_config_key: &'static str,
    pub default_base_url: &'static str,
    pub supported_models: &'static [&'static str],
    pub auth_methods: &'static [AuthMethodDescriptor],
    pub token_env: Option<&'static str>,
    pub login_env: Option<&'static str>,
    pub refresh_env: Option<&'static str>,
    pub expires_env: Option<&'static str>,
    pub account_env: Option<&'static str>,
    pub endpoint_env: Option<&'static str>,
    pub local_auth_discovery: Option<&'static str>,
    pub oauth_authorize_kind: Option<OAuthAuthorizeKind>,
    pub oauth_callback_kind: Option<OAuthAuthorizeKind>,
    pub capabilities: ProviderCapabilityFlags,
    pub disabled_reason: Option<&'static str>,
}

const OPENAI_OAUTH_METHODS: &[AuthMethodDescriptor] = &[AuthMethodDescriptor::new(
    AuthMethodKind::OAuthPkce,
    "ChatGPT Pro/Plus (browser)",
)];
const API_KEY_METHODS: &[AuthMethodDescriptor] =
    &[AuthMethodDescriptor::new(AuthMethodKind::ApiKey, "API Key")];
const CLAUDE_CODE_METHODS: &[AuthMethodDescriptor] = &[AuthMethodDescriptor::new(
    AuthMethodKind::LocalCliToken,
    "Claude Code OAuth",
)];
const GOOGLE_METHODS: &[AuthMethodDescriptor] = &[
    AuthMethodDescriptor::new(AuthMethodKind::OAuthPkce, "Google OAuth"),
    AuthMethodDescriptor::new(AuthMethodKind::ApiKey, "API Key"),
];
const AWS_METHODS: &[AuthMethodDescriptor] = &[AuthMethodDescriptor::new(
    AuthMethodKind::AwsCredentials,
    "AWS Credentials",
)];
const GITHUB_COPILOT_METHODS: &[AuthMethodDescriptor] = &[AuthMethodDescriptor::new(
    AuthMethodKind::DeviceCode,
    "GitHub Copilot OAuth",
)];

const OPENAI_MODELS: &[&str] = &[
    "gpt-5.6-sol",
    "gpt-5.6-terra",
    "gpt-5.6-luna",
    "gpt-5.5",
    "gpt-5.3-codex-spark",
    "gpt-5.4-mini",
    "gpt-5.4-nano",
    "text-embedding-3-large",
    "text-embedding-3-small",
];
const OPENAI_API_MODELS: &[&str] = &[
    "gpt-5.6-sol",
    "gpt-5.6-terra",
    "gpt-5.6-luna",
    "gpt-5.5-pro",
    "gpt-5.5",
    "gpt-5.4-mini",
    "gpt-5.4-nano",
    "text-embedding-3-large",
    "text-embedding-3-small",
];
const ANTHROPIC_MODELS: &[&str] = &["claude-opus-4-8", "claude-opus-4-7"];
const GOOGLE_MODELS: &[&str] = &[
    "gemini-3.5-flash",
    "gemini-3.1-pro-preview",
    "gemini-3.1-flash-lite",
    "gemini-embedding-2",
];
const ANTIGRAVITY_MODELS: &[&str] = &["antigravity-browser"];
const ANTIGRAVITY_API_MODELS: &[&str] = &[
    "gemini-3.5-flash",
    "gemini-3.1-pro-preview",
    "gemini-3.1-flash-lite",
    "gemini-embedding-2",
];
const DEEPSEEK_MODELS: &[&str] = &["deepseek-v4-pro", "deepseek-v4-flash"];
const MOONSHOT_MODELS: &[&str] = &["kimi-k2.6"];
const QWEN_MODELS: &[&str] = &["qwen3.7-max", "qwen3.6-flash", "text-embedding-v4"];
const XAI_MODELS: &[&str] = &["grok-4.3"];
const MISTRAL_MODELS: &[&str] = &[
    "mistral-medium-3.5",
    "mistral-small-2603",
    "codestral-embed-2505",
];
const COHERE_MODELS: &[&str] = &["embed-v4.0"];
const TOGETHER_MODELS: &[&str] = &[
    "BAAI/bge-large-en-v1.5",
    "togethercomputer/m2-bert-80M-8k-retrieval",
];
const HUGGINGFACE_MODELS: &[&str] = &["jinaai/jina-embeddings-v5-text"];
const OPENROUTER_MODELS: &[&str] = &[
    "gpt-5.5-pro",
    "claude-opus-4-7",
    "gemini-3.5-flash",
    "gemini-3.1-pro-preview",
    "deepseek-v4-pro",
    "kimi-k2.6",
    "mistral-medium-3.5",
    "grok-4.3",
    "qwen3.7-max",
    "gpt-5.4-mini",
    "deepseek-v4-flash",
    "qwen3.6-flash",
    "gpt-5.4-nano",
    "gemini-3.1-flash-lite",
    "text-embedding-3-large",
    "text-embedding-3-small",
];
const OPENCODE_ZEN_MODELS: &[&str] = &[
    "gpt-5.6-sol",
    "gpt-5.6-terra",
    "gpt-5.6-luna",
    "gpt-5.5",
    "gpt-5.5-pro",
    "gpt-5.4-mini",
    "gpt-5.4-nano",
    "claude-opus-4-8",
    "claude-sonnet-5",
    "gemini-3.5-flash",
    "gemini-3.1-pro",
    "deepseek-v4-pro",
    "deepseek-v4-flash",
    "kimi-k2.6",
    "qwen3.7-max",
];
const OPENCODE_GO_MODELS: &[&str] = &[
    "deepseek-v4-pro",
    "deepseek-v4-flash",
    "kimi-k2.6",
    "kimi-k2.5",
    "glm-5.2",
    "glm-5",
    "qwen3.7-max",
    "qwen3.7-plus",
    "minimax-m2.5",
    "mimo-v2.5",
];
const CLINEPASS_MODELS: &[&str] = &[
    "cline-pass/deepseek-v4-pro",
    "cline-pass/deepseek-v4-flash",
    "cline-pass/qwen3.7-max",
    "cline-pass/qwen3.7-plus",
    "cline-pass/kimi-k2.6",
    "cline-pass/kimi-k2.7-code",
    "cline-pass/glm-5.2",
    "cline-pass/minimax-m3",
    "cline-pass/mimo-v2.5",
    "cline-pass/mimo-v2.5-pro",
];
const EMPTY_MODELS: &[&str] = &[];

const fn openai_subscription_capabilities() -> ProviderCapabilityFlags {
    ProviderCapabilityFlags {
        supports_streaming: true,
        supports_tool_call_streaming: true,
        supports_cache_usage: true,
        supports_reasoning_usage: true,
        supports_subscription: true,
        supports_api_key: false,
        supports_oauth_refresh: true,
        supports_model_validation: true,
    }
}

const fn openai_compatible_api_capabilities() -> ProviderCapabilityFlags {
    ProviderCapabilityFlags {
        supports_streaming: true,
        supports_tool_call_streaming: true,
        supports_cache_usage: true,
        supports_reasoning_usage: true,
        supports_subscription: false,
        supports_api_key: true,
        supports_oauth_refresh: false,
        supports_model_validation: true,
    }
}

const fn disabled_subscription_capabilities() -> ProviderCapabilityFlags {
    ProviderCapabilityFlags {
        supports_streaming: false,
        supports_tool_call_streaming: false,
        supports_cache_usage: false,
        supports_reasoning_usage: false,
        supports_subscription: true,
        supports_api_key: false,
        supports_oauth_refresh: false,
        supports_model_validation: false,
    }
}

const fn google_capabilities() -> ProviderCapabilityFlags {
    ProviderCapabilityFlags {
        supports_streaming: true,
        supports_tool_call_streaming: false,
        supports_cache_usage: true,
        supports_reasoning_usage: false,
        supports_subscription: true,
        supports_api_key: true,
        supports_oauth_refresh: true,
        supports_model_validation: true,
    }
}

const fn aws_capabilities() -> ProviderCapabilityFlags {
    ProviderCapabilityFlags {
        supports_streaming: false,
        supports_tool_call_streaming: false,
        supports_cache_usage: false,
        supports_reasoning_usage: false,
        supports_subscription: false,
        supports_api_key: false,
        supports_oauth_refresh: false,
        supports_model_validation: true,
    }
}

pub const PROVIDER_AUTH_REGISTRY: &[ProviderAuthRegistryEntry] = &[
    ProviderAuthRegistryEntry {
        provider_id: "codex",
        runtime_provider_id: "codex",
        display_name: "Codex Subscription",
        base_url_config_key: "codex",
        default_base_url: "https://chatgpt.com/backend-api/codex/responses",
        supported_models: OPENAI_MODELS,
        auth_methods: OPENAI_OAUTH_METHODS,
        token_env: Some("OPENAI_API_KEY"),
        login_env: Some("OPENAI_LOGIN"),
        refresh_env: Some("OPENAI_REFRESH_TOKEN"),
        expires_env: Some("OPENAI_TOKEN_EXPIRES"),
        account_env: Some("OPENAI_ACCOUNT_ID"),
        endpoint_env: Some("OPENAI_CODEX_ENDPOINT"),
        local_auth_discovery: Some("codex_auth_json"),
        oauth_authorize_kind: Some(OAuthAuthorizeKind::OpenAiPkce),
        oauth_callback_kind: Some(OAuthAuthorizeKind::OpenAiPkce),
        capabilities: openai_subscription_capabilities(),
        disabled_reason: None,
    },
    ProviderAuthRegistryEntry {
        provider_id: "openai",
        runtime_provider_id: "openai",
        display_name: "OpenAI API",
        base_url_config_key: "openai",
        default_base_url: "https://api.openai.com/v1",
        supported_models: OPENAI_API_MODELS,
        auth_methods: API_KEY_METHODS,
        token_env: Some("OPENAI_API_KEY"),
        login_env: Some("OPENAI_LOGIN"),
        refresh_env: None,
        expires_env: None,
        account_env: None,
        endpoint_env: None,
        local_auth_discovery: None,
        oauth_authorize_kind: None,
        oauth_callback_kind: None,
        capabilities: openai_compatible_api_capabilities(),
        disabled_reason: None,
    },
    ProviderAuthRegistryEntry {
        provider_id: "anthropic",
        runtime_provider_id: "anthropic",
        display_name: "Anthropic API",
        base_url_config_key: "anthropic",
        default_base_url: "https://api.anthropic.com/v1",
        supported_models: ANTHROPIC_MODELS,
        auth_methods: API_KEY_METHODS,
        token_env: Some("ANTHROPIC_API_KEY"),
        login_env: Some("ANTHROPIC_LOGIN"),
        refresh_env: None,
        expires_env: None,
        account_env: None,
        endpoint_env: None,
        local_auth_discovery: None,
        oauth_authorize_kind: None,
        oauth_callback_kind: None,
        capabilities: openai_compatible_api_capabilities(),
        disabled_reason: None,
    },
    ProviderAuthRegistryEntry {
        provider_id: "claude-code",
        runtime_provider_id: "anthropic",
        display_name: "Claude Code",
        base_url_config_key: "anthropic",
        default_base_url: "https://api.anthropic.com/v1",
        supported_models: ANTHROPIC_MODELS,
        auth_methods: CLAUDE_CODE_METHODS,
        token_env: Some("CLAUDE_CODE_OAUTH_TOKEN"),
        login_env: Some("ANTHROPIC_LOGIN"),
        refresh_env: Some("CLAUDE_CODE_REFRESH_TOKEN"),
        expires_env: Some("CLAUDE_CODE_TOKEN_EXPIRES"),
        account_env: None,
        endpoint_env: None,
        local_auth_discovery: Some("claude_code_local_auth"),
        oauth_authorize_kind: Some(OAuthAuthorizeKind::AnthropicPkce),
        oauth_callback_kind: Some(OAuthAuthorizeKind::AnthropicPkce),
        capabilities: ProviderCapabilityFlags {
            supports_oauth_refresh: true,
            ..disabled_subscription_capabilities()
        },
        disabled_reason: None,
    },
    ProviderAuthRegistryEntry {
        provider_id: "google",
        runtime_provider_id: "google",
        display_name: "Google Gemini",
        base_url_config_key: "google",
        default_base_url: "https://generativelanguage.googleapis.com/v1beta",
        supported_models: GOOGLE_MODELS,
        auth_methods: GOOGLE_METHODS,
        token_env: Some("GOOGLE_API_KEY"),
        login_env: Some("GOOGLE_LOGIN"),
        refresh_env: Some("GOOGLE_REFRESH_TOKEN"),
        expires_env: Some("GOOGLE_TOKEN_EXPIRES"),
        account_env: Some("GOOGLE_ACCOUNT_ID"),
        endpoint_env: None,
        local_auth_discovery: None,
        oauth_authorize_kind: Some(OAuthAuthorizeKind::GooglePkce),
        oauth_callback_kind: Some(OAuthAuthorizeKind::GooglePkce),
        capabilities: google_capabilities(),
        disabled_reason: None,
    },
    ProviderAuthRegistryEntry {
        provider_id: "google-api",
        runtime_provider_id: "google",
        display_name: "Google Gemini API",
        base_url_config_key: "google",
        default_base_url: "https://generativelanguage.googleapis.com/v1beta",
        supported_models: GOOGLE_MODELS,
        auth_methods: API_KEY_METHODS,
        token_env: Some("GOOGLE_API_KEY"),
        login_env: Some("GOOGLE_LOGIN"),
        refresh_env: None,
        expires_env: None,
        account_env: None,
        endpoint_env: None,
        local_auth_discovery: None,
        oauth_authorize_kind: None,
        oauth_callback_kind: None,
        capabilities: google_capabilities(),
        disabled_reason: None,
    },
    ProviderAuthRegistryEntry {
        provider_id: "gemini",
        runtime_provider_id: "google",
        display_name: "Gemini",
        base_url_config_key: "google",
        default_base_url: "https://generativelanguage.googleapis.com/v1beta",
        supported_models: GOOGLE_MODELS,
        auth_methods: GOOGLE_METHODS,
        token_env: Some("GEMINI_API_KEY"),
        login_env: Some("GEMINI_LOGIN"),
        refresh_env: Some("GOOGLE_REFRESH_TOKEN"),
        expires_env: Some("GOOGLE_TOKEN_EXPIRES"),
        account_env: Some("GOOGLE_ACCOUNT_ID"),
        endpoint_env: None,
        local_auth_discovery: None,
        oauth_authorize_kind: Some(OAuthAuthorizeKind::GooglePkce),
        oauth_callback_kind: Some(OAuthAuthorizeKind::GooglePkce),
        capabilities: google_capabilities(),
        disabled_reason: None,
    },
    ProviderAuthRegistryEntry {
        provider_id: "gemini-api",
        runtime_provider_id: "google",
        display_name: "Gemini API",
        base_url_config_key: "google",
        default_base_url: "https://generativelanguage.googleapis.com/v1beta",
        supported_models: GOOGLE_MODELS,
        auth_methods: API_KEY_METHODS,
        token_env: Some("GEMINI_API_KEY"),
        login_env: Some("GEMINI_LOGIN"),
        refresh_env: None,
        expires_env: None,
        account_env: None,
        endpoint_env: None,
        local_auth_discovery: None,
        oauth_authorize_kind: None,
        oauth_callback_kind: None,
        capabilities: google_capabilities(),
        disabled_reason: None,
    },
    ProviderAuthRegistryEntry {
        provider_id: "antigravity",
        ..PROVIDER_AUTH_REGISTRY_ANTIGRAVITY_BROWSER
    },
    ProviderAuthRegistryEntry {
        provider_id: "antigravity-api",
        runtime_provider_id: "antigravity",
        display_name: "Antigravity API",
        base_url_config_key: "antigravity",
        default_base_url: "https://antigravity.google.com/v1",
        supported_models: ANTIGRAVITY_API_MODELS,
        auth_methods: API_KEY_METHODS,
        token_env: Some("ANTIGRAVITY_API_KEY"),
        login_env: Some("ANTIGRAVITY_LOGIN"),
        refresh_env: None,
        expires_env: None,
        account_env: None,
        endpoint_env: None,
        local_auth_discovery: None,
        oauth_authorize_kind: None,
        oauth_callback_kind: None,
        capabilities: openai_compatible_api_capabilities(),
        disabled_reason: Some("No verified Antigravity provider endpoint is implemented"),
    },
    simple_openai_compatible(
        "openrouter",
        "OpenRouter",
        "OPENROUTER_API_KEY",
        OPENROUTER_MODELS,
        "https://openrouter.ai/api/v1",
    ),
    simple_openai_compatible(
        "deepseek",
        "DeepSeek",
        "DEEPSEEK_API_KEY",
        DEEPSEEK_MODELS,
        "https://api.deepseek.com",
    ),
    simple_openai_compatible(
        "moonshotai",
        "Moonshot AI",
        "MOONSHOTAI_API_KEY",
        MOONSHOT_MODELS,
        "https://api.moonshot.ai/v1",
    ),
    // International DashScope endpoint (Alibaba international station key).
    simple_openai_compatible(
        "qwen",
        "Qwen",
        "QWEN_API_KEY",
        QWEN_MODELS,
        "https://dashscope-intl.aliyuncs.com/compatible-mode/v1",
    ),
    simple_openai_compatible(
        "qwen_cn",
        "Qwen China",
        "QWEN_CN_API_KEY",
        QWEN_MODELS,
        "https://dashscope.aliyuncs.com/compatible-mode/v1",
    ),
    simple_openai_compatible(
        "xai",
        "xAI",
        "XAI_API_KEY",
        XAI_MODELS,
        "https://api.x.ai/v1",
    ),
    simple_openai_compatible(
        "mistral",
        "Mistral AI",
        "MISTRAL_API_KEY",
        MISTRAL_MODELS,
        "https://api.mistral.ai/v1",
    ),
    simple_openai_compatible(
        "huggingface",
        "Hugging Face Inference Providers",
        "HUGGINGFACE_API_KEY",
        HUGGINGFACE_MODELS,
        "https://router.huggingface.co/v1",
    ),
    simple_openai_compatible(
        "azure",
        "Azure AI Foundry",
        "AZURE_OPENAI_API_KEY",
        OPENAI_API_MODELS,
        "https://{resource}.openai.azure.com/openai/v1",
    ),
    simple_openai_compatible(
        "replicate",
        "Replicate",
        "REPLICATE_API_TOKEN",
        EMPTY_MODELS,
        "https://api.replicate.com/v1",
    ),
    simple_openai_compatible(
        "cohere",
        "Cohere",
        "COHERE_API_KEY",
        COHERE_MODELS,
        "https://api.cohere.com/v2",
    ),
    simple_openai_compatible(
        "together",
        "Together AI",
        "TOGETHER_API_KEY",
        TOGETHER_MODELS,
        "https://api.together.xyz/v1",
    ),
    simple_openai_compatible(
        "opencode-zen",
        "OpenCode Zen",
        "OPENCODE_API_KEY",
        OPENCODE_ZEN_MODELS,
        "https://opencode.ai/zen/v1",
    ),
    simple_openai_compatible(
        "opencode-go",
        "OpenCode Go",
        "OPENCODE_API_KEY",
        OPENCODE_GO_MODELS,
        "https://opencode.ai/zen/go/v1",
    ),
    simple_openai_compatible(
        "clinepass",
        "ClinePass",
        "CLINE_API_KEY",
        CLINEPASS_MODELS,
        "https://api.cline.bot/api/v1",
    ),
    ProviderAuthRegistryEntry {
        provider_id: "github-copilot",
        runtime_provider_id: "github-copilot",
        display_name: "GitHub Copilot",
        base_url_config_key: "github-copilot",
        default_base_url: "https://api.githubcopilot.com",
        supported_models: EMPTY_MODELS,
        auth_methods: GITHUB_COPILOT_METHODS,
        token_env: Some("COPILOT_GITHUB_TOKEN"),
        login_env: Some("COPILOT_LOGIN"),
        refresh_env: None,
        expires_env: Some("COPILOT_GITHUB_TOKEN_EXPIRES"),
        account_env: Some("COPILOT_GITHUB_ACCOUNT"),
        endpoint_env: Some("COPILOT_API_URL"),
        local_auth_discovery: Some("copilot_cli_credentials"),
        oauth_authorize_kind: Some(OAuthAuthorizeKind::GithubDevice),
        oauth_callback_kind: Some(OAuthAuthorizeKind::GithubDevice),
        capabilities: ProviderCapabilityFlags {
            supports_subscription: true,
            supports_api_key: true,
            supports_oauth_refresh: false,
            supports_model_validation: true,
            ..openai_compatible_api_capabilities()
        },
        disabled_reason: None,
    },
    ProviderAuthRegistryEntry {
        provider_id: "bedrock",
        runtime_provider_id: "bedrock",
        display_name: "Bedrock",
        base_url_config_key: "bedrock",
        default_base_url: "us-east-1",
        supported_models: EMPTY_MODELS,
        auth_methods: AWS_METHODS,
        token_env: None,
        login_env: None,
        refresh_env: None,
        expires_env: None,
        account_env: Some("AWS_PROFILE"),
        endpoint_env: Some("AWS_REGION"),
        local_auth_discovery: Some("aws_sdk_default_chain"),
        oauth_authorize_kind: None,
        oauth_callback_kind: None,
        capabilities: aws_capabilities(),
        disabled_reason: None,
    },
];

const PROVIDER_AUTH_REGISTRY_ANTIGRAVITY_BROWSER: ProviderAuthRegistryEntry =
    ProviderAuthRegistryEntry {
        provider_id: "antigravity",
        runtime_provider_id: "antigravity",
        display_name: "Antigravity OAuth",
        base_url_config_key: "antigravity",
        default_base_url: "https://antigravity.google.com/v1",
        supported_models: ANTIGRAVITY_MODELS,
        auth_methods: GOOGLE_METHODS,
        token_env: Some("ANTIGRAVITY_API_KEY"),
        login_env: Some("ANTIGRAVITY_LOGIN"),
        refresh_env: Some("ANTIGRAVITY_REFRESH_TOKEN"),
        expires_env: Some("ANTIGRAVITY_TOKEN_EXPIRES"),
        account_env: Some("ANTIGRAVITY_ACCOUNT_ID"),
        endpoint_env: None,
        local_auth_discovery: None,
        oauth_authorize_kind: Some(OAuthAuthorizeKind::GooglePkce),
        oauth_callback_kind: Some(OAuthAuthorizeKind::GooglePkce),
        capabilities: google_capabilities(),
        disabled_reason: None,
    };

const fn simple_openai_compatible(
    provider_id: &'static str,
    display_name: &'static str,
    token_env: &'static str,
    models: &'static [&'static str],
    default_base_url: &'static str,
) -> ProviderAuthRegistryEntry {
    ProviderAuthRegistryEntry {
        provider_id,
        runtime_provider_id: provider_id,
        display_name,
        base_url_config_key: provider_id,
        default_base_url,
        supported_models: models,
        auth_methods: API_KEY_METHODS,
        token_env: Some(token_env),
        login_env: None,
        refresh_env: None,
        expires_env: None,
        account_env: None,
        endpoint_env: None,
        local_auth_discovery: None,
        oauth_authorize_kind: None,
        oauth_callback_kind: None,
        capabilities: openai_compatible_api_capabilities(),
        disabled_reason: None,
    }
}

pub fn provider_auth_registry() -> &'static [ProviderAuthRegistryEntry] {
    PROVIDER_AUTH_REGISTRY
}

pub fn provider_auth_registry_entry(
    provider_id: &str,
) -> Option<&'static ProviderAuthRegistryEntry> {
    if let Some(entry) = provider_auth_registry()
        .iter()
        .find(|entry| entry.provider_id.eq_ignore_ascii_case(provider_id))
    {
        return Some(entry);
    }
    // Compatibility aliases: an `-api` suffixed id with no dedicated entry
    // resolves to its canonical base provider (e.g. `openai-api` -> `openai`).
    if let Some(base) = provider_id
        .strip_suffix("-api")
        .or_else(|| provider_id.strip_suffix("-API"))
    {
        return provider_auth_registry()
            .iter()
            .find(|entry| entry.provider_id.eq_ignore_ascii_case(base));
    }
    None
}

pub fn runtime_provider_id(provider_id: &str) -> &str {
    provider_auth_registry_entry(provider_id)
        .map(|entry| entry.runtime_provider_id)
        .unwrap_or(provider_id)
}

pub fn provider_token_env(provider_id: &str) -> Option<&'static str> {
    provider_auth_registry_entry(provider_id).and_then(|entry| entry.token_env)
}

pub fn provider_login_env(provider_id: &str) -> Option<&'static str> {
    provider_auth_registry_entry(provider_id).and_then(|entry| entry.login_env)
}

pub fn provider_default_auth_method(provider_id: &str) -> Option<&'static AuthMethodDescriptor> {
    provider_auth_registry_entry(provider_id).and_then(|entry| entry.auth_methods.first())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_contains_required_provider_ids() {
        for provider_id in [
            "openai",
            "codex",
            "anthropic",
            "claude-code",
            "google",
            "google-api",
            "gemini",
            "gemini-api",
            "antigravity",
            "antigravity-api",
            "openrouter",
            "deepseek",
            "moonshotai",
            "qwen",
            "qwen_cn",
            "xai",
            "mistral",
            "huggingface",
            "azure",
            "replicate",
            "github-copilot",
            "bedrock",
            "opencode-zen",
            "opencode-go",
            "clinepass",
        ] {
            assert!(
                provider_auth_registry_entry(provider_id).is_some(),
                "missing registry entry for {provider_id}"
            );
        }
    }

    #[test]
    fn compatibility_ids_keep_runtime_mapping() {
        assert_eq!(runtime_provider_id("anthropic-api"), "anthropic");
        assert_eq!(runtime_provider_id("google-api"), "google");
        assert_eq!(runtime_provider_id("gemini-api"), "google");
    }
}
