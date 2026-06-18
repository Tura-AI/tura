use super::args::parse_provider_order;
use super::types::{ImageProvider, DEFAULT_OUTPUT_DIR, DEFAULT_PROVIDER_ORDER};
use base64::{engine::general_purpose, Engine as _};
use dotenvy::from_path_iter;
use serde_json::Value;
use std::path::PathBuf;
use tura_llm_rust::TuraConfig;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum OpenAiAuth {
    CodexOAuth {
        token: String,
        account_id: Option<String>,
    },
    ApiKey(String),
}

impl OpenAiAuth {
    pub(super) fn token(&self) -> &str {
        match self {
            Self::CodexOAuth { token, .. } => token,
            Self::ApiKey(token) => token,
        }
    }

    pub(super) fn is_codex_oauth(&self) -> bool {
        matches!(self, Self::CodexOAuth { .. })
    }
}

pub(super) fn configured_provider_order() -> Vec<ImageProvider> {
    first_config_value(&[
        "TURA_IMAGE_GENERATE_PROVIDER_ORDER",
        "IMAGE_GENERATE_PROVIDER_ORDER",
        "TURA_COMMAND_IMAGE_GENERATE_PROVIDER_ORDER",
    ])
    .map(|value| vec![value])
    .and_then(|values| parse_provider_order(&values).ok())
    .filter(|order| !order.is_empty())
    .unwrap_or_else(|| DEFAULT_PROVIDER_ORDER.to_vec())
}

pub(super) fn configured_output_dir() -> String {
    first_config_value(&[
        "TURA_IMAGE_GENERATE_OUTPUT_DIRECTORY",
        "TURA_IMAGE_GENERATE_OUTPUT_DIR",
        "IMAGE_GENERATE_OUTPUT_DIRECTORY",
        "IMAGE_GENERATE_OUTPUT_DIR",
        "TURA_COMMAND_IMAGE_GENERATE_OUTPUT_DIRECTORY",
    ])
    .unwrap_or_else(|| DEFAULT_OUTPUT_DIR.to_string())
}

pub(super) fn provider_model(provider: ImageProvider) -> String {
    let key = match provider {
        ImageProvider::ChatGptImage2 => "TURA_IMAGE_GENERATE_OPENAI_MODEL",
        ImageProvider::ReplicateZImageTurbo => "TURA_IMAGE_GENERATE_REPLICATE_MODEL",
        ImageProvider::Gemini31Flash => "TURA_IMAGE_GENERATE_GEMINI_MODEL",
        ImageProvider::Grok3 => "TURA_IMAGE_GENERATE_GROK_MODEL",
    };
    env_value(key).unwrap_or_else(|| match provider {
        ImageProvider::ChatGptImage2 => "gpt-image-2".to_string(),
        ImageProvider::ReplicateZImageTurbo => "prunaai/z-image-turbo".to_string(),
        ImageProvider::Gemini31Flash => "gemini-3.1-flash-image".to_string(),
        ImageProvider::Grok3 => "grok-imagine-image-quality".to_string(),
    })
}

pub(super) fn provider_endpoint(provider: ImageProvider, edit: bool) -> String {
    let provider_key = match provider {
        ImageProvider::ChatGptImage2 => "OPENAI",
        ImageProvider::ReplicateZImageTurbo => "REPLICATE",
        ImageProvider::Gemini31Flash => "GEMINI",
        ImageProvider::Grok3 => "GROK",
    };
    let specific = if edit { "EDIT_ENDPOINT" } else { "ENDPOINT" };
    env_value(&format!("TURA_IMAGE_GENERATE_{provider_key}_{specific}"))
        .or_else(|| env_value(&format!("TURA_{provider_key}_IMAGE_{specific}")))
        .unwrap_or_else(|| match provider {
            ImageProvider::ChatGptImage2 if edit => {
                "https://api.openai.com/v1/images/edits".to_string()
            }
            ImageProvider::ChatGptImage2 => {
                "https://api.openai.com/v1/images/generations".to_string()
            }
            ImageProvider::ReplicateZImageTurbo => {
                "https://api.replicate.com/v1/models/prunaai/z-image-turbo/predictions".to_string()
            }
            ImageProvider::Gemini31Flash => format!(
                "https://generativelanguage.googleapis.com/v1/models/{}:generateContent",
                provider_model(provider)
            ),
            ImageProvider::Grok3 if edit => "https://api.x.ai/v1/images/edits".to_string(),
            ImageProvider::Grok3 => "https://api.x.ai/v1/images/generations".to_string(),
        })
}

pub(super) fn provider_key(provider: ImageProvider) -> Result<String, String> {
    if provider == ImageProvider::ChatGptImage2 {
        return openai_auth_candidates().map(|candidates| candidates[0].token().to_string());
    }

    let keys: &[&str] = match provider {
        ImageProvider::ChatGptImage2 => unreachable!("handled above"),
        ImageProvider::ReplicateZImageTurbo => &["REPLICATE_API_TOKEN", "REPLICATE_API_KEY"],
        ImageProvider::Gemini31Flash => &["GEMINI_API_KEY", "GOOGLE_API_KEY"],
        ImageProvider::Grok3 => &["XAI_API_KEY", "GROK_API_KEY"],
    };
    keys.iter()
        .find_map(|key| env_value(key))
        .ok_or_else(|| format!("{} key unavailable", provider.id()))
}

fn first_config_value(names: &[&str]) -> Option<String> {
    names.iter().find_map(|name| env_value(name))
}

pub(super) fn env_value(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .or_else(|| {
            TuraConfig::default()
                .get(name)
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
        })
        .or_else(|| current_dir_dotenv_value(name))
}

fn current_dir_dotenv_value(name: &str) -> Option<String> {
    let upper = name.to_ascii_uppercase();
    let mut dir = std::env::current_dir().ok()?;
    loop {
        let env_path = dir.join(".env");
        if env_path.exists() {
            if let Ok(entries) = from_path_iter(&env_path) {
                for (key, value) in entries.flatten() {
                    if key.to_ascii_uppercase() == upper {
                        let value = value.trim().to_string();
                        if !value.is_empty() {
                            return Some(value);
                        }
                    }
                }
            }
        }
        if !dir.pop() {
            return None;
        }
    }
}

pub(super) fn openai_auth_candidates() -> Result<Vec<OpenAiAuth>, String> {
    let mut candidates = Vec::new();
    let mut seen = Vec::<String>::new();
    let account_id = openai_account_id();

    for key in ["CODEX_OPENAI_OAUTH_TOKEN", "CODEX_OAUTH_TOKEN"] {
        let Some(token) = env_value(key) else {
            continue;
        };
        if !looks_like_oauth_token(&token) || seen.contains(&token) {
            continue;
        }
        seen.push(token.clone());
        candidates.push(OpenAiAuth::CodexOAuth {
            account_id: account_id
                .clone()
                .or_else(|| account_id_from_oauth_token(&token)),
            token,
        });
    }

    if let Some((token, codex_account_id)) = load_codex_auth_json() {
        if !seen.contains(&token) {
            seen.push(token.clone());
            candidates.push(OpenAiAuth::CodexOAuth {
                account_id: account_id
                    .clone()
                    .or(codex_account_id)
                    .or_else(|| account_id_from_oauth_token(&token)),
                token,
            });
        }
    }

    for key in [
        "OPENAI_OAUTH_TOKEN",
        "CHATGPT_OAUTH_TOKEN",
        "OPENAI_API_KEY",
        "CHATGPT_API_KEY",
    ] {
        let Some(token) = env_value(key) else {
            continue;
        };
        if !looks_like_oauth_token(&token) || seen.contains(&token) {
            continue;
        }
        seen.push(token.clone());
        candidates.push(OpenAiAuth::CodexOAuth {
            account_id: account_id
                .clone()
                .or_else(|| account_id_from_oauth_token(&token)),
            token,
        });
    }

    for key in [
        "OPENAI_OPENAPI_KEY",
        "OPENAI_API_KEY_OPENAPI",
        "OPENAI_API_KEY",
        "CHATGPT_API_KEY",
    ] {
        let Some(token) = env_value(key) else {
            continue;
        };
        if looks_like_oauth_token(&token) || seen.contains(&token) {
            continue;
        }
        seen.push(token.clone());
        candidates.push(OpenAiAuth::ApiKey(token));
    }

    if candidates.is_empty() {
        Err(
            "chatgpt_image_2 key unavailable; set a Codex OAuth token or OpenAI API key"
                .to_string(),
        )
    } else {
        Ok(candidates)
    }
}

fn openai_account_id() -> Option<String> {
    first_config_value(&[
        "OPENAI_ACCOUNT_ID",
        "CHATGPT_ACCOUNT_ID",
        "CODEX_OPENAI_ACCOUNT_ID",
        "CODEX_CHATGPT_ACCOUNT_ID",
    ])
}

fn looks_like_oauth_token(value: &str) -> bool {
    value.starts_with("eyJ") && value.split('.').count() >= 2
}

fn account_id_from_oauth_token(token: &str) -> Option<String> {
    let payload = token.split('.').nth(1)?;
    let bytes = general_purpose::URL_SAFE_NO_PAD.decode(payload).ok()?;
    let claims = serde_json::from_slice::<Value>(&bytes).ok()?;
    claims
        .get("https://api.openai.com/auth")
        .and_then(|auth| auth.get("chatgpt_account_id"))
        .or_else(|| claims.get("chatgpt_account_id"))
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn load_codex_auth_json() -> Option<(String, Option<String>)> {
    let path = codex_auth_json_path()?;
    let value = serde_json::from_str::<Value>(&std::fs::read_to_string(path).ok()?).ok()?;
    let tokens = value.get("tokens")?;
    let access_token = tokens
        .get("access_token")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())?
        .to_string();
    let account_id = tokens
        .get("account_id")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string);
    Some((access_token, account_id))
}

fn codex_auth_json_path() -> Option<PathBuf> {
    if let Some(home) = std::env::var_os("CODEX_HOME").filter(|value| !value.is_empty()) {
        return Some(PathBuf::from(home).join("auth.json"));
    }
    let home = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME"))?;
    Some(PathBuf::from(home).join(".codex").join("auth.json"))
}
