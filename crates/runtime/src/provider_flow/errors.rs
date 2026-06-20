//! Provider failure helpers.

use chrono::{DateTime, Utc};
use std::time::Duration;

use crate::state_machine::runtime_management::{
    RuntimeCallResultStatus, RuntimeError, RuntimeManagement, UsageReport,
};

pub(crate) fn runtime_timeout(runtime: &RuntimeManagement) -> std::time::Duration {
    std::time::Duration::from_millis(runtime.provider.base.time_out_ms.max(1_000))
}

pub(crate) fn finish_runtime_failure(
    runtime: &mut RuntimeManagement,
    finished_at: DateTime<Utc>,
    error_code: &str,
    error_text: String,
    status: RuntimeCallResultStatus,
) -> Result<(), String> {
    finish_runtime_failure_with_usage(runtime, finished_at, error_code, error_text, status, None)
}

pub(crate) fn finish_runtime_failure_with_usage(
    runtime: &mut RuntimeManagement,
    finished_at: DateTime<Utc>,
    error_code: &str,
    error_text: String,
    status: RuntimeCallResultStatus,
    usage: Option<UsageReport>,
) -> Result<(), String> {
    let err = RuntimeError {
        error_code: Some(error_code.to_string()),
        error_text: Some(error_text),
        retry_allowed: true,
        fallback_allowed: true,
        fallback_to_id: None,
    };
    runtime
        .finish_failure(finished_at, err, status, usage)
        .map_err(|e| format!("failed to finish runtime failure: {e}"))
}

pub(crate) fn provider_timeout_retry_wait(retry_count: u8) -> Option<Duration> {
    if let Some(duration) = provider_retry_wait_override(retry_count) {
        return Some(duration);
    }
    match retry_count {
        0 => Some(Duration::from_secs(5)),
        1 => Some(Duration::from_secs(15)),
        2 => Some(Duration::from_secs(45)),
        _ => None,
    }
}

fn provider_retry_wait_override(retry_count: u8) -> Option<Duration> {
    let raw = std::env::var("TURA_PROVIDER_RETRY_BACKOFF_MS").ok()?;
    let values = raw
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .filter_map(|value| value.parse::<u64>().ok())
        .collect::<Vec<_>>();
    values
        .get(usize::from(retry_count))
        .copied()
        .map(Duration::from_millis)
}

pub(crate) fn runtime_failure_allows_retry(runtime: &RuntimeManagement) -> bool {
    runtime.call_result_status == RuntimeCallResultStatus::Failed
        && runtime
            .error
            .as_ref()
            .map(|error| error.retry_allowed)
            .unwrap_or(false)
}

pub(crate) fn runtime_failure_text(runtime: &RuntimeManagement) -> Option<String> {
    runtime
        .error
        .as_ref()
        .and_then(|error| error.error_text.clone())
        .or_else(|| {
            runtime
                .output
                .as_ref()
                .and_then(|output| output.get("error"))
                .and_then(serde_json::Value::as_str)
                .map(ToString::to_string)
        })
}

#[cfg(test)]
mod tests {
    use crate::state_machine::agent_management::{ProviderConfig, ToolChoice};
    use crate::state_machine::runtime_management::{
        RuntimeCallResultStatus, RuntimeError, RuntimeManagement, RuntimeProviderConfig,
    };
    use chrono::Utc;
    use serde_json::json;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn provider_timeout_retry_waits_use_three_step_backoff() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|error| error.into_inner());
        let previous = std::env::var_os("TURA_PROVIDER_RETRY_BACKOFF_MS");
        std::env::remove_var("TURA_PROVIDER_RETRY_BACKOFF_MS");
        assert_eq!(
            super::provider_timeout_retry_wait(0),
            Some(std::time::Duration::from_secs(5))
        );
        assert_eq!(
            super::provider_timeout_retry_wait(1),
            Some(std::time::Duration::from_secs(15))
        );
        assert_eq!(
            super::provider_timeout_retry_wait(2),
            Some(std::time::Duration::from_secs(45))
        );
        assert_eq!(super::provider_timeout_retry_wait(3), None);
        restore_env("TURA_PROVIDER_RETRY_BACKOFF_MS", previous);
    }

    #[test]
    fn provider_timeout_retry_wait_allows_fast_business_test_override() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|error| error.into_inner());
        let previous = std::env::var_os("TURA_PROVIDER_RETRY_BACKOFF_MS");
        std::env::set_var("TURA_PROVIDER_RETRY_BACKOFF_MS", "0,1,2");

        assert_eq!(
            super::provider_timeout_retry_wait(0),
            Some(std::time::Duration::from_millis(0))
        );
        assert_eq!(
            super::provider_timeout_retry_wait(1),
            Some(std::time::Duration::from_millis(1))
        );
        assert_eq!(
            super::provider_timeout_retry_wait(2),
            Some(std::time::Duration::from_millis(2))
        );
        assert_eq!(super::provider_timeout_retry_wait(3), None);

        restore_env("TURA_PROVIDER_RETRY_BACKOFF_MS", previous);
    }

    #[test]
    fn provider_schema_error_removes_rejected_media_content_type() {
        let error = "http status 400: Invalid value: 'input_file'. Supported values are: 'input_text', 'input_image'";
        assert_eq!(
            tura_llm_rust::provider_unsupported_content_type(error),
            Some("input_file")
        );

        let mut messages = vec![json!({
            "type": "function_call_output",
            "call_id": "call_1",
            "output": [
                { "type": "input_text", "text": "kept" },
                { "type": "input_file", "filename": "tone.mp3", "file_data": "data:audio/mpeg;base64,QUJD" },
                { "type": "input_image", "image_url": "data:image/jpeg;base64,AAA" }
            ]
        })];

        let removed = tura_llm_rust::replace_unsupported_content_type_in_messages(
            &mut messages,
            "input_file",
        );
        assert_eq!(removed, 1);
        let serialized = serde_json::to_string(&messages).expect("serialize");
        assert!(serialized.contains("Unsupported media omitted"));
        assert!(serialized.contains("input_image"));
        assert!(!serialized.contains("file_data"));
        assert!(!serialized.contains("tone.mp3"));
    }

    #[test]
    fn retry_allowed_failed_runtime_uses_provider_retry_path() {
        let mut runtime = runtime_for_retry_test("retryable-runtime");
        runtime.call_result_status = RuntimeCallResultStatus::Failed;
        runtime.error = Some(RuntimeError {
            error_code: Some("CALL_FAILED".to_string()),
            error_text: Some(
                "all providers failed: openai:gpt-5.1 => network error: error decoding response body"
                    .to_string(),
            ),
            retry_allowed: true,
            fallback_allowed: true,
            fallback_to_id: None,
        });

        assert!(super::runtime_failure_allows_retry(&runtime));
        assert_eq!(
            super::runtime_failure_text(&runtime).as_deref(),
            Some("all providers failed: openai:gpt-5.1 => network error: error decoding response body")
        );
    }

    #[test]
    fn non_retryable_failed_runtime_does_not_use_provider_retry_path() {
        let mut runtime = runtime_for_retry_test("non-retryable-runtime");
        runtime.call_result_status = RuntimeCallResultStatus::Failed;
        runtime.error = Some(RuntimeError {
            error_code: Some("CALL_FAILED".to_string()),
            error_text: Some("provider rejected invalid request".to_string()),
            retry_allowed: false,
            fallback_allowed: false,
            fallback_to_id: None,
        });

        assert!(!super::runtime_failure_allows_retry(&runtime));
        assert_eq!(
            super::runtime_failure_text(&runtime).as_deref(),
            Some("provider rejected invalid request")
        );
    }

    fn runtime_for_retry_test(runtime_id: &str) -> RuntimeManagement {
        let now = Utc::now();
        let provider = "openai".to_string();
        RuntimeManagement::new(
            runtime_id.to_string(),
            "session-for-retry-test".to_string(),
            "session-for-retry-test".to_string(),
            RuntimeProviderConfig {
                base: ProviderConfig {
                    tura_llm_name: provider.clone(),
                    default_model_tier: None,
                    current_model: None,
                    stream: true,
                    temperature: 0.0,
                    max_tokens: 0,
                    tool_choice: ToolChoice::Auto,
                    time_out_ms: 120_000,
                },
                thinking: false,
                provider_name: provider.clone(),
                model_name: "gpt-5.1".to_string(),
                provider_url_name: provider.clone(),
                llm_provider_name: provider,
            },
            now,
        )
    }

    fn restore_env(key: &str, previous: Option<std::ffi::OsString>) {
        if let Some(previous) = previous {
            std::env::set_var(key, previous);
        } else {
            std::env::remove_var(key);
        }
    }
}
