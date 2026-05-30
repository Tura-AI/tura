use tura_llm_rust::{AuthMethodKind, OAuthAuthorizeKind};

use super::oauth_support::{
    anthropic_oauth_token_url, google_oauth_token_url, openai_oauth_token_url,
};

pub(super) fn legacy_auth_method_type(kind: AuthMethodKind) -> &'static str {
    match kind {
        AuthMethodKind::ApiKey => "api",
        AuthMethodKind::OAuthPkce | AuthMethodKind::DeviceCode => "oauth",
        AuthMethodKind::BrowserToken => "token",
        AuthMethodKind::LocalCliToken => "oauth",
        AuthMethodKind::AwsCredentials => "aws",
        AuthMethodKind::None => "none",
    }
}

pub(super) fn oauth_authorize_endpoint(kind: Option<OAuthAuthorizeKind>) -> Option<String> {
    match kind {
        Some(OAuthAuthorizeKind::OpenAiPkce) => {
            Some("https://auth.openai.com/oauth/authorize".to_string())
        }
        Some(OAuthAuthorizeKind::AnthropicPkce) => {
            Some("https://claude.ai/oauth/authorize".to_string())
        }
        Some(OAuthAuthorizeKind::GooglePkce) => {
            Some("https://accounts.google.com/o/oauth2/v2/auth".to_string())
        }
        Some(OAuthAuthorizeKind::GithubDevice) => {
            Some("https://github.com/login/device".to_string())
        }
        Some(OAuthAuthorizeKind::BrowserTokenPaste)
        | Some(OAuthAuthorizeKind::Unsupported)
        | None => None,
    }
}

pub(super) fn oauth_token_endpoint(kind: Option<OAuthAuthorizeKind>) -> Option<String> {
    match kind {
        Some(OAuthAuthorizeKind::OpenAiPkce) => Some(openai_oauth_token_url()),
        Some(OAuthAuthorizeKind::AnthropicPkce) => Some(anthropic_oauth_token_url()),
        Some(OAuthAuthorizeKind::GooglePkce) => Some(google_oauth_token_url()),
        Some(OAuthAuthorizeKind::GithubDevice) => {
            Some("https://github.com/login/oauth/access_token".to_string())
        }
        Some(OAuthAuthorizeKind::BrowserTokenPaste)
        | Some(OAuthAuthorizeKind::Unsupported)
        | None => None,
    }
}

pub(super) fn provider_api_key_url(provider_id: &str) -> Option<String> {
    let url = match tura_llm_rust::runtime_provider_id(provider_id) {
        "openai" => "https://platform.openai.com/api-keys",
        "anthropic" => "https://console.anthropic.com/settings/keys",
        "google" => "https://aistudio.google.com/app/apikey",
        "openrouter" => "https://openrouter.ai/settings/keys",
        "deepseek" => "https://platform.deepseek.com/api_keys",
        "moonshotai" => "https://platform.moonshot.ai/console/api-keys",
        "qwen" | "qwen_cn" => "https://bailian.console.aliyun.com/?tab=model#/api-key",
        "xai" => "https://console.x.ai/team/default/api-keys",
        "mistral" => "https://console.mistral.ai/api-keys/",
        "huggingface" => "https://huggingface.co/settings/tokens",
        "azure" => "https://portal.azure.com/",
        "replicate" => "https://replicate.com/account/api-tokens",
        "github-copilot" => "https://github.com/settings/personal-access-tokens",
        "bedrock" => "https://console.aws.amazon.com/iam/home#/security_credentials",
        _ => return None,
    };
    Some(url.to_string())
}

pub(super) fn provider_auth_docs_url(provider_id: &str) -> Option<String> {
    let url = match tura_llm_rust::runtime_provider_id(provider_id) {
        "openai" => "https://platform.openai.com/docs/api-reference/authentication",
        "anthropic" => "https://docs.anthropic.com/en/api/overview",
        "claude-code" => "https://code.claude.com/docs/en/iam",
        "google" => "https://ai.google.dev/gemini-api/docs/oauth",
        "openrouter" => "https://openrouter.ai/docs/api-keys",
        "mistral" => "https://docs.mistral.ai/admin/security-access/api-keys",
        "github-copilot" => {
            "https://docs.github.com/en/copilot/how-tos/copilot-cli/set-up-copilot-cli/authenticate-copilot-cli"
        }
        "bedrock" => "https://docs.aws.amazon.com/bedrock/latest/userguide/security-iam.html",
        _ => return None,
    };
    Some(url.to_string())
}
