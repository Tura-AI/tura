//! Fallback handling when the provider rejects a media content type: detect
//! the provider's error phrasing and decide whether the request can be retried
//! after replacing the corresponding content blocks (tagged in the canonical
//! OpenAI Responses shape: `input_image` / `input_audio` / `input_file`) or
//! whether the selected model simply does not support the required media.
//!
//! Binding rule: every provider error-string / format probe lives in the
//! provider crate. The runtime only consumes the normalized fallback
//! decision plus the replacement count.

use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderMediaFallback {
    RetryWithoutContent { content_type: &'static str },
    UnsupportedRequiredContent { content_type: &'static str },
}

impl ProviderMediaFallback {
    pub fn content_type(self) -> &'static str {
        match self {
            Self::RetryWithoutContent { content_type }
            | Self::UnsupportedRequiredContent { content_type } => content_type,
        }
    }

    pub fn retry_content_type(self) -> Option<&'static str> {
        match self {
            Self::RetryWithoutContent { content_type } => Some(content_type),
            Self::UnsupportedRequiredContent { .. } => None,
        }
    }
}

/// Parse the provider's error text and identify the media fallback action.
pub fn provider_media_fallback(error_text: &str) -> Option<ProviderMediaFallback> {
    let normalized = error_text.to_ascii_lowercase();
    if normalized.contains("invalid file data") || normalized.contains("unsupported mime type") {
        return Some(ProviderMediaFallback::RetryWithoutContent {
            content_type: "input_file",
        });
    }
    if normalized.contains("no endpoints found") && normalized.contains("image input") {
        return Some(ProviderMediaFallback::UnsupportedRequiredContent {
            content_type: "input_image",
        });
    }
    if normalized.contains("no endpoints found") && normalized.contains("audio input") {
        return Some(ProviderMediaFallback::UnsupportedRequiredContent {
            content_type: "input_audio",
        });
    }
    for content_type in ["input_file", "input_image", "input_audio"] {
        let quoted = format!("'{content_type}'");
        let double_quoted = format!("\"{content_type}\"");
        if normalized.contains("invalid value")
            && (normalized.contains(&quoted) || normalized.contains(&double_quoted))
        {
            return Some(ProviderMediaFallback::RetryWithoutContent { content_type });
        }
    }
    None
}

/// Parse the provider's error text and identify which content type needs a
/// fallback; returns the normalized content-type string (`"input_file"` /
/// `"input_image"` / `"input_audio"`) or `None`.
pub fn provider_unsupported_content_type(error_text: &str) -> Option<&'static str> {
    provider_media_fallback(error_text).and_then(ProviderMediaFallback::retry_content_type)
}

/// Replace every content block in the message array whose `type` matches
/// `content_type` with a placeholder `input_text`; returns the replacement count.
pub fn replace_unsupported_content_type_in_messages(
    messages: &mut [Value],
    content_type: &'static str,
) -> usize {
    messages
        .iter_mut()
        .map(|message| replace_unsupported_content_type(message, content_type))
        .sum()
}

fn replace_unsupported_content_type(value: &mut Value, content_type: &'static str) -> usize {
    match value {
        Value::Object(object) => {
            if object.get("type").and_then(Value::as_str) == Some(content_type) {
                *value = serde_json::json!({
                    "type": "input_text",
                    "text": format!(
                        "[Unsupported media omitted: provider rejected `{content_type}` content.]"
                    )
                });
                return 1;
            }
            object
                .values_mut()
                .map(|child| replace_unsupported_content_type(child, content_type))
                .sum()
        }
        Value::Array(items) => items
            .iter_mut()
            .map(|child| replace_unsupported_content_type(child, content_type))
            .sum(),
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        provider_media_fallback, provider_unsupported_content_type, ProviderMediaFallback,
    };

    #[test]
    fn detects_openrouter_image_input_endpoint_rejection_as_required_media() {
        let error = r#"http status 404: {"error":{"message":"No endpoints found that support image input","code":404}}"#;

        assert_eq!(
            provider_media_fallback(error),
            Some(ProviderMediaFallback::UnsupportedRequiredContent {
                content_type: "input_image"
            })
        );
        assert_eq!(provider_unsupported_content_type(error), None);
    }

    #[test]
    fn detects_retryable_invalid_file_content() {
        let error = "http status 400: Invalid value: 'input_file'. Supported values are: 'input_text', 'input_image'";

        assert_eq!(
            provider_media_fallback(error),
            Some(ProviderMediaFallback::RetryWithoutContent {
                content_type: "input_file"
            })
        );
        assert_eq!(provider_unsupported_content_type(error), Some("input_file"));
    }
}
