use chrono::Utc;
use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use tracing::error;

use crate::profile_timings;
use crate::provider_flow::checkpointing;
use crate::provider_flow::errors::{
    finish_provider_call_failure, finish_runtime_failure, runtime_timeout,
};
use crate::provider_flow::provider_response::apply_provider_response;
use crate::provider_flow::provider_streaming::{call_runtime_streaming, RuntimeStreamingInput};
pub use crate::provider_flow::request_options::route_by_name;
use crate::provider_flow::request_options::{
    normalize_provider_messages, parallel_tool_calls_enabled, prompt_cache_key,
    route_for_provider_name, session_max_tokens, session_model_override_route,
    session_reasoning_effort, session_service_tier, stream_options,
};
use crate::provider_flow::usage::usage_report_from_metrics;
use crate::runtime::types::RuntimeQueueItem;
use crate::state_machine::runtime_management::{
    RuntimeCallResultStatus, RuntimeManagement, RuntimeState,
};
use crate::state_machine::session_management::SessionId;

pub struct CallRuntimeInput {
    pub runtime: RuntimeManagement,
    pub messages: Vec<serde_json::Value>,
    pub tools: Vec<serde_json::Value>,
    pub provider_name: String,
    pub stream: bool,
    pub max_tokens: u32,
    pub tool_choice: Option<serde_json::Value>,
    pub session_directory: PathBuf,
    pub allowed_command_run_commands: Option<BTreeSet<String>>,
    pub require_startup_task_state: bool,
}

pub async fn call_runtime(
    input: CallRuntimeInput,
    tura_settings: Arc<tura_llm_rust::Settings>,
    tura_config: Arc<tura_llm_rust::TuraConfig>,
) -> Result<RuntimeManagement, String> {
    let mut runtime = input.runtime;
    tura_config.reload();
    let now = Utc::now();
    let profiling = profile_timings::enabled();
    let normalize_start = Instant::now();
    let provider_messages = normalize_provider_messages(input.messages);
    profile_timings::log_elapsed(
        "call_runtime.normalize_provider_messages",
        normalize_start,
        serde_json::json!({
            "session_id": runtime.session_id,
            "runtime_id": runtime.runtime_id,
            "provider_message_count": provider_messages.len(),
            "provider_messages_bytes": if profiling {
                profile_timings::json_vec_bytes(&provider_messages)
            } else {
                0
            },
        }),
    );
    let clone_input_start = Instant::now();
    let input_messages = provider_messages.clone();
    let input_tools = input.tools.clone();
    profile_timings::log_elapsed(
        "call_runtime.clone_provider_input",
        clone_input_start,
        serde_json::json!({
            "session_id": runtime.session_id,
            "runtime_id": runtime.runtime_id,
            "message_count": input_messages.len(),
            "messages_bytes": if profiling {
                profile_timings::json_vec_bytes(&input_messages)
            } else {
                0
            },
            "tool_count": input_tools.len(),
            "tools_bytes": if profiling {
                profile_timings::json_vec_bytes(&input_tools)
            } else {
                0
            },
        }),
    );

    runtime
        .transition(RuntimeState::Dispatching)
        .map_err(|e| format!("failed to transition runtime to Dispatching: {e}"))?;
    runtime
        .mark_called(now)
        .map_err(|e| format!("failed to mark runtime called: {e}"))?;
    runtime
        .mark_waiting_first_token()
        .map_err(|e| format!("failed to mark runtime waiting for first token: {e}"))?;
    let turn_started_start = Instant::now();
    checkpointing::turn_started(&runtime)?;
    profile_timings::log_elapsed(
        "call_runtime.checkpoint_turn_started",
        turn_started_start,
        serde_json::json!({
            "session_id": runtime.session_id,
            "runtime_id": runtime.runtime_id,
        }),
    );

    let direct_route = route_for_provider_name(tura_settings.as_ref(), &input.provider_name);
    let configured_route = route_by_name(tura_settings.as_ref(), &input.provider_name);
    let route_config_base = direct_route
        .as_ref()
        .or(configured_route)
        .ok_or_else(|| format!("unknown provider route: {}", input.provider_name))?;
    let override_route = session_model_override_route(tura_settings.as_ref(), route_config_base);
    let route_config = override_route.as_ref().unwrap_or(route_config_base);
    let context_window = active_model_context_window(tura_settings.as_ref(), route_config);

    let prompt_cache_key = prompt_cache_key(
        route_config,
        &input.provider_name,
        &runtime.session_id,
        &input_tools,
    );
    let call_options = tura_llm_rust::CallOptions {
        tools: if input.tools.is_empty() {
            None
        } else {
            Some(input.tools)
        },
        stream: Some(input.stream),
        parallel_tool_calls: parallel_tool_calls_enabled(route_config, !input_tools.is_empty()),
        prompt_cache_key,
        stream_options: stream_options(route_config, input.stream),
        reasoning_effort: session_reasoning_effort(),
        service_tier: session_service_tier(),
        max_tokens: session_max_tokens(input.max_tokens),
        store: Some(false),
        tool_choice: input.tool_choice.clone(),
        context_window,
        ..Default::default()
    };
    let set_input_start = Instant::now();
    runtime.set_input(serde_json::json!({
        "messages": input_messages,
        "tools": input_tools,
        "options": {
            "stream": input.stream,
            "parallel_tool_calls": call_options.parallel_tool_calls,
            "prompt_cache_key": call_options.prompt_cache_key.clone(),
            "stream_options": call_options.stream_options.clone(),
            "reasoning_effort": call_options.reasoning_effort.clone(),
            "service_tier": call_options.service_tier.clone(),
            "max_tokens": call_options.max_tokens,
            "store": call_options.store,
            "tool_choice": call_options.tool_choice.clone(),
            "context_window": call_options.context_window,
        }
    }));
    profile_timings::log_elapsed(
        "call_runtime.set_input",
        set_input_start,
        serde_json::json!({
            "session_id": runtime.session_id,
            "runtime_id": runtime.runtime_id,
        }),
    );

    let provider_call_started_start = Instant::now();
    checkpointing::provider_call_started(&runtime)?;
    profile_timings::log_elapsed(
        "call_runtime.checkpoint_provider_call_started",
        provider_call_started_start,
        serde_json::json!({
            "session_id": runtime.session_id,
            "runtime_id": runtime.runtime_id,
        }),
    );

    let call_result = if input.stream || !input_tools.is_empty() {
        call_runtime_streaming(
            &mut runtime,
            route_config,
            &tura_config,
            RuntimeStreamingInput {
                messages: provider_messages,
                options: call_options,
                session_directory: input.session_directory.clone(),
                allowed_command_run_commands: input.allowed_command_run_commands.clone(),
                require_startup_task_state: input.require_startup_task_state,
            },
        )
        .await
    } else {
        call_runtime_non_streaming(
            &mut runtime,
            route_config,
            &tura_config,
            provider_messages,
            call_options,
        )
        .await
    };

    match call_result {
        Ok(()) => {
            checkpointing::provider_call_finished(&runtime)?;
            checkpointing::terminal_turn(&runtime)?;
        }
        Err(error) => {
            checkpointing::best_effort_turn_failed(&runtime);
            return Err(error);
        }
    }

    Ok(runtime)
}

fn active_model_context_window(
    settings: &tura_llm_rust::Settings,
    route_config: &tura_llm_rust::RouteConfig,
) -> Option<u64> {
    let provider = route_config.providers.first()?;
    let catalog = settings.model_catalog.providers.get(&provider.provider)?;
    catalog
        .models
        .values()
        .flatten()
        .find(|entry| {
            tura_llm_rust::Settings::normalize_model_name(&provider.provider, entry.id())
                == provider.model
                || entry.id() == provider.model
        })?
        .detail()
        .map(|detail| u64::from(detail.limit.context))
}

async fn call_runtime_non_streaming(
    runtime: &mut RuntimeManagement,
    route_config: &tura_llm_rust::RouteConfig,
    tura_config: &Arc<tura_llm_rust::TuraConfig>,
    messages: Vec<serde_json::Value>,
    options: tura_llm_rust::CallOptions,
) -> Result<(), String> {
    let started_at = Utc::now();
    let timeout_duration = runtime_timeout(runtime);

    match tokio::time::timeout(
        timeout_duration,
        route_config.run(tura_config.as_ref(), messages, options),
    )
    .await
    {
        Err(_) => {
            let finished_at = Utc::now();
            let message = format!(
                "runtime call timed out after {} ms",
                timeout_duration.as_millis()
            );
            error!(error = %message, "runtime call timed out");
            runtime.set_output(serde_json::json!({
                "error": message
            }));
            finish_runtime_failure(
                runtime,
                finished_at,
                "CALL_TIMED_OUT",
                message,
                RuntimeCallResultStatus::TimedOut,
            )?;
        }
        Ok(Ok(response)) => {
            let finished_at = Utc::now();
            runtime.set_output(response.content.clone());
            apply_provider_response(runtime, &response.content, finished_at);

            runtime
                .mark_first_token(finished_at)
                .map_err(|e| format!("failed to mark first token: {e}"))?;

            let usage = usage_report_from_metrics(
                runtime,
                response.metrics,
                started_at,
                finished_at,
                finished_at,
            );

            runtime
                .finish_success(finished_at, usage)
                .map_err(|e| format!("failed to finish runtime success: {e}"))?;
        }
        Ok(Err(e)) => {
            let finished_at = Utc::now();
            error!(error = %e, "runtime call failed");
            runtime.set_output(serde_json::json!({
                "error": e.to_string()
            }));
            finish_provider_call_failure(
                runtime,
                finished_at,
                &e,
                RuntimeCallResultStatus::Failed,
            )?;
        }
    }

    Ok(())
}

pub async fn dequeue_runtime(
    session_id: &SessionId,
    redis_url: &str,
) -> Result<Option<RuntimeQueueItem>, String> {
    let client = redis::Client::open(redis_url)
        .map_err(|e| format!("failed to create redis client: {e}"))?;

    let mut con = client
        .get_multiplexed_async_connection()
        .await
        .map_err(|e| format!("failed to get redis connection: {e}"))?;

    let queue_key = format!("runtime:queue:{session_id}");

    let result: Option<String> = redis::cmd("LPOP")
        .arg(&queue_key)
        .query_async(&mut con)
        .await
        .map_err(|e| format!("failed to dequeue runtime: {e}"))?;

    match result {
        Some(payload) => {
            let item: RuntimeQueueItem = serde_json::from_str(&payload)
                .map_err(|e| format!("failed to deserialize queue item: {e}"))?;
            Ok(Some(item))
        }
        None => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::{call_runtime, CallRuntimeInput};
    use crate::state_machine::agent_management::{ProviderConfig, ToolChoice};
    use crate::state_machine::runtime_management::{RuntimeManagement, RuntimeProviderConfig};
    use chrono::Utc;
    use serde_json::json;
    use std::collections::{BTreeSet, HashMap};
    use std::sync::Arc;
    use tura_llm_rust::{
        ModelCatalog, ProviderConfig as LlmProviderConfig, ProviderEnumCatalog, RouteConfig,
        Settings, TuraConfig,
    };

    fn runtime() -> RuntimeManagement {
        RuntimeManagement::new(
            "runtime-call-test".to_string(),
            "session-call-test".to_string(),
            "agent-call-test".to_string(),
            RuntimeProviderConfig {
                base: ProviderConfig {
                    tura_llm_name: "fast".to_string(),
                    default_model_tier: None,
                    current_model: None,
                    stream: true,
                    temperature: 0.0,
                    max_tokens: 1024,
                    tool_choice: ToolChoice::Auto,
                    time_out_ms: 30_000,
                },
                thinking: false,
                provider_name: "openai".to_string(),
                model_name: "gpt-test".to_string(),
                provider_url_name: "openai".to_string(),
                llm_provider_name: "openai".to_string(),
            },
            Utc::now(),
        )
    }

    fn missing_key_settings() -> Arc<Settings> {
        Arc::new(Settings {
            provider_base_url: HashMap::new(),
            routes: HashMap::from([(
                "missing-key-route".to_string(),
                RouteConfig {
                    default_temperature: 0.0,
                    providers: vec![LlmProviderConfig {
                        provider: "definitely_missing_provider_for_call_runtime_test".to_string(),
                        base_url: "http://127.0.0.1:9".to_string(),
                        model: "local-test-model".to_string(),
                        temperature: 0.0,
                    }],
                },
            )]),
            model_catalog: ModelCatalog::default(),
            provider_enums: ProviderEnumCatalog::default(),
        })
    }

    #[tokio::test]
    async fn call_runtime_provider_config_failure_finishes_failed_without_network() {
        std::env::remove_var("DEFINITELY_MISSING_PROVIDER_FOR_CALL_RUNTIME_TEST_API_KEY");
        let settings = missing_key_settings();
        let config = Arc::new(TuraConfig::new(".env.missing-for-call-runtime-test"));

        let runtime = call_runtime(
            CallRuntimeInput {
                runtime: runtime(),
                messages: vec![json!({ "role": "user", "content": "hello" })],
                tools: Vec::new(),
                provider_name: "missing-key-route".to_string(),
                stream: false,
                max_tokens: 128,
                tool_choice: None,
                session_directory: std::env::temp_dir(),
                allowed_command_run_commands: Some(BTreeSet::new()),
                require_startup_task_state: false,
            },
            settings,
            config,
        )
        .await
        .expect("provider config failure should be captured on the runtime");

        assert_eq!(
            runtime.state,
            crate::state_machine::runtime_management::RuntimeState::Failed
        );
        assert_eq!(
            runtime.call_result_status,
            crate::state_machine::runtime_management::RuntimeCallResultStatus::Failed
        );
        let output = runtime.output.expect("failure output should be persisted");
        let error = output
            .get("error")
            .and_then(serde_json::Value::as_str)
            .expect("failure output should contain text");
        assert!(
            error.contains("API Key not found"),
            "unexpected failure output: {error}"
        );
        let runtime_error = runtime.error.expect("runtime error should be set");
        assert_eq!(runtime_error.error_code.as_deref(), Some("CALL_FAILED"));
        assert!(!runtime_error.retry_allowed);
        assert!(!runtime_error.fallback_allowed);
        assert!(runtime_error
            .error_text
            .as_deref()
            .unwrap_or_default()
            .contains("API Key not found"));
    }
}
