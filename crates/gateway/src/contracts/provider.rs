use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tura_llm_rust::AuthMethodKind;

// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Provider {
    pub id: String,
    pub name: String,
    pub auth_type: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderListResponse {
    pub all: Vec<SdkProvider>,
    pub default: HashMap<String, String>,
    pub connected: Vec<String>,
    pub enums: tura_llm_rust::ProviderEnumCatalog,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SdkProvider {
    pub id: String,
    pub name: String,
    pub source: String,
    pub env: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    pub options: HashMap<String, serde_json::Value>,
    pub models: HashMap<String, SdkProviderModel>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub npm: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SdkProviderModel {
    pub id: String,
    pub name: String,
    pub family: String,
    pub release_date: String,
    pub attachment: bool,
    pub reasoning: bool,
    pub temperature: bool,
    pub tool_call: bool,
    pub limit: SdkProviderModelLimit,
    pub modalities: SdkProviderModelModalities,
    pub options: HashMap<String, serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SdkProviderModelLimit {
    pub context: u32,
    pub input: u32,
    pub output: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SdkProviderModelModalities {
    pub input: Vec<String>,
    pub output: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderAuth {
    #[serde(rename = "type")]
    pub auth_type: String,
    pub key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub access: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refresh: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires: Option<i64>,
    #[serde(default, rename = "accountId", skip_serializing_if = "Option::is_none")]
    pub account_id: Option<String>,
    #[serde(default)]
    pub metadata: Option<HashMap<String, serde_json::Value>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderAuthResponse {
    pub success: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProviderAuthStatusResponse {
    pub provider_id: String,
    pub display_name: String,
    pub login: Option<String>,
    pub configured: bool,
    pub authenticated: bool,
    pub expired: Option<bool>,
    pub account_id: Option<String>,
    pub token_env: Option<String>,
    pub login_env: Option<String>,
    pub refresh_env: Option<String>,
    pub expires_env: Option<String>,
    pub updated_at: Option<String>,
    pub auth_state: tura_llm_rust::AuthState,
    pub runtime_state: tura_llm_rust::ProviderRuntimeState,
    pub last_error_category: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProviderAuthActionResponse {
    pub ok: bool,
    pub provider_id: String,
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub level: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub details: Vec<ProviderAuthActionDetail>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<ProviderAuthStatusResponse>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProviderAuthActionDetail {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct ProviderAuthQuery {
    pub directory: Option<String>,
    pub workspace: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProviderAuthMethod {
    #[serde(rename = "type")]
    pub method_type: String,
    pub kind: AuthMethodKind,
    pub login: String,
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompts: Option<Vec<serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_env: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub login_env: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authorize_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub docs_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub configured_value: Option<String>,
    pub available: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unavailable_reason: Option<String>,
    pub supports_refresh: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ValidateModelRequest {
    #[serde(rename = "providerID")]
    pub provider_id: String,
    #[serde(rename = "modelID")]
    pub model_id: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ValidateModelResponse {
    pub ok: bool,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct OAuthAuthorizeParams {
    pub directory: Option<String>,
    pub workspace: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OAuthAuthorizePayload {
    pub method: usize,
    pub inputs: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OAuthMethod {
    #[serde(rename = "auto")]
    Auto,
    #[serde(rename = "code")]
    Code,
}

#[derive(Debug, Clone, Serialize)]
pub struct OAuthAuthorizeResponse {
    pub url: String,
    pub method: OAuthMethod,
    pub instructions: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OAuthCallbackParams {
    pub directory: Option<String>,
    pub workspace: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OAuthCallbackPayload {
    pub method: usize,
    pub state: Option<String>,
    pub code: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OAuthRedirectCallbackParams {
    pub code: Option<String>,
    pub state: Option<String>,
    pub error: Option<String>,
}

impl OAuthRedirectCallbackParams {
    pub(crate) fn has_callback_payload(&self) -> bool {
        self.code
            .as_deref()
            .is_some_and(|value| !value.trim().is_empty())
            || self
                .state
                .as_deref()
                .is_some_and(|value| !value.trim().is_empty())
            || self
                .error
                .as_deref()
                .is_some_and(|value| !value.trim().is_empty())
    }
}
