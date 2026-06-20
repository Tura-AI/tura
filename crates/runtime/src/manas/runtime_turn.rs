use crate::prompt_style::{agent_identity, compact_context, PromptBuilder};
use crate::runtime::call_runtime::{call_runtime, CallRuntimeInput};
use crate::runtime::create_runtime::{
    create_runtime, runtime_provider_config_from_tura, CreateRuntimeInput,
};
use crate::runtime::types::ToolCallData;
use crate::state_machine::agent_management::AgentManagement;
use crate::state_machine::runtime_management::{RuntimeManagement, UsageReport};
use crate::state_machine::session_management::{SessionManagement, DEFAULT_CONTEXT_TOKEN_LIMIT};

use super::agent_prompts::load_agent_system_prompt_messages;
use super::constants::{COMMAND_RUN_TOOL, PLANNING_TOOL};
use super::prompt_messages::{approximate_message_tokens, messages_for_turn_with_context_limit};
use super::tool_catalog::{
    command_run_commands_for_agent, filter_tools_for_turn, load_agent_capabilities,
    planning_tool_disabled, tool_schema_name,
};

#[allow(clippy::too_many_arguments)]
pub(crate) fn execute_turn(
    agents: &[AgentManagement],
    session: &mut SessionManagement,
    current_messages: &[serde_json::Value],
    original_user_task: &str,
    extra_tail_system_prompt: Option<&'static str>,
    _redis_url: &str,
    _is_first_llm_call: bool,
    is_final_turn: bool,
    force_no_tools: bool,
) -> Result<(RuntimeManagement, Vec<ToolCallData>), String> {
    let agent = agents
        .first()
        .ok_or_else(|| "no agent available".to_string())?;

    let agent_commands = command_run_commands_for_agent(agent);
    let planning_enabled = agent_commands.contains(PLANNING_TOOL);
    let disable_tool_invocation = is_final_turn || force_no_tools;
    let mut tools = load_agent_capabilities(agent)?;
    if planning_tool_disabled() {
        tools.retain(|tool| tool_schema_name(tool) != Some(PLANNING_TOOL));
    }
    tools = filter_tools_for_turn(tools, is_final_turn, force_no_tools)?;
    let mut allowed_tool_names: std::collections::HashSet<String> = tools
        .iter()
        .filter_map(tool_schema_name)
        .map(ToString::to_string)
        .collect();
    if planning_enabled && !planning_tool_disabled() {
        allowed_tool_names.insert(PLANNING_TOOL.to_string());
    }
    let executable_tool_names = if disable_tool_invocation {
        std::collections::HashSet::new()
    } else {
        allowed_tool_names.clone()
    };
    tools = move_command_run_to_end(tools);
    if debug_runtime_enabled() {
        eprintln!(
            "tura runtime debug [{}]: agent={} provider_tools={:?} executable_tools={:?}",
            debug_runtime_timestamp(),
            agent.agent_name,
            tools
                .iter()
                .filter_map(tool_schema_name)
                .collect::<Vec<_>>(),
            executable_tool_names
        );
    }
    let tura_runtime = tokio::runtime::Runtime::new()
        .map_err(|err| format!("failed to create tokio runtime: {err}"))?;

    let runtime = tura_runtime.block_on(async {
        let settings = tura_llm_rust::Settings::default()
            .await
            .map_err(|err| format!("failed to load tura llm settings: {err}"))?;
        let runtime_provider_config =
            runtime_provider_config_from_tura(&agent.provider, settings.as_ref(), false)?;
        let compact_limit_tokens =
            dynamic_context_limit_tokens(settings.as_ref(), &runtime_provider_config);
        let context_window_tokens =
            model_context_window_tokens(settings.as_ref(), &runtime_provider_config);
        let language = session_language();
        let user_name = session_user_name();
        let identity = PromptBuilder::new()
            .part(agent_identity::agent_identity(
                &agent.agent_name,
                &user_name,
                &runtime_provider_config.model_name,
                &runtime_provider_config.llm_provider_name,
                compact_limit_tokens,
                &language,
            ))
            .render();
        let mut runtime_messages = vec![serde_json::json!({
            "role": "system",
            "content": identity,
        })];
        runtime_messages.extend(load_agent_system_prompt_messages(agent)?);
        let fixed_prefix_tokens = approximate_message_tokens(&runtime_messages);
        let turn = messages_for_turn_with_context_limit(
            current_messages,
            session,
            original_user_task,
            compact_limit_tokens,
            fixed_prefix_tokens,
        );
        session.context_tokens = turn.context_tokens;
        let mut turn_messages = turn.messages;
        if let Some(prompt) = extra_tail_system_prompt {
            crate::prompt_style::tail_injection::append_tail_prompt(
                &mut turn_messages,
                crate::prompt_style::tail_injection::TailPrompt::system(prompt),
            );
        }
        runtime_messages.extend(turn_messages);
        let mut final_context_input = approximate_message_tokens(&runtime_messages);
        if !disable_tool_invocation
            && should_force_compact_prompt(
                session,
                &runtime_messages,
                final_context_input,
                compact_limit_tokens,
            )
        {
            crate::prompt_style::tail_injection::append_tail_prompt(
                &mut runtime_messages,
                crate::prompt_style::tail_injection::TailPrompt::user(
                    compact_context_required_message(compact_limit_tokens),
                ),
            );
            final_context_input = approximate_message_tokens(&runtime_messages);
        }
        session.context_tokens.input = previous_provider_input_tokens(session)
            .filter(|value| *value > 0)
            .map(|value| value.max(final_context_input))
            .unwrap_or(final_context_input);
        session.context_tokens.limit = context_window_tokens;
        let (runtime, queue_item) = create_runtime(CreateRuntimeInput {
            session_id: session.session_id.clone(),
            agent_id: agent.agent_id.clone(),
            messages: runtime_messages,
            tools,
            provider_config: agent.provider.clone(),
            tura_settings: std::sync::Arc::clone(&settings),
            thinking: false,
            context_tokens: session.context_tokens,
        })
        .await?;

        let config = std::sync::Arc::new(tura_llm_rust::TuraConfig::default());

        let mut runtime = call_runtime(
            CallRuntimeInput {
                runtime,
                messages: queue_item.messages,
                tools: queue_item.tools,
                provider_name: queue_item.provider_name,
                stream: agent.provider.stream,
                max_tokens: agent.provider.max_tokens,
                tool_choice: tool_choice_for_turn(disable_tool_invocation),
                session_directory: session.session_directory.clone(),
                allowed_command_run_commands: Some(agent_commands),
            },
            settings,
            config,
        )
        .await?;
        sync_context_tokens_from_provider_usage(session, &mut runtime, context_window_tokens);
        Ok::<RuntimeManagement, String>(runtime)
    })?;

    let tool_calls: Vec<ToolCallData> = runtime
        .tool_call
        .iter()
        .filter(|record| executable_tool_names.contains(&record.tool_called_name))
        .map(|record| ToolCallData {
            tool_name: record.tool_called_name.clone(),
            arguments: record.tool_called_input.clone(),
            provider_metadata: record.provider_metadata.clone(),
        })
        .collect();
    if debug_runtime_enabled() {
        eprintln!(
            "tura runtime debug [{}]: state={:?} text_len={} raw_tool_calls={} filtered_tool_calls={}",
            debug_runtime_timestamp(),
            runtime.state,
            runtime.text.len(),
            runtime.tool_call.len(),
            tool_calls.len()
        );
    }

    Ok((runtime, tool_calls))
}

fn session_language() -> String {
    std::env::var("TURA_SESSION_LANGUAGE")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "en".to_string())
}

fn session_user_name() -> String {
    std::env::var("TURA_SESSION_USER_NAME")
        .ok()
        .or_else(|| std::env::var("USERNAME").ok())
        .or_else(|| std::env::var("USER").ok())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "user".to_string())
}

fn dynamic_context_limit_tokens(
    settings: &tura_llm_rust::Settings,
    runtime_provider_config: &crate::state_machine::runtime_management::RuntimeProviderConfig,
) -> u64 {
    if let Some(limit) = fixed_context_limit_tokens_from_env() {
        return limit;
    }
    let Some(model_context) = catalog_model_context_tokens(
        settings,
        &runtime_provider_config.llm_provider_name,
        &runtime_provider_config.model_name,
    ) else {
        return DEFAULT_CONTEXT_TOKEN_LIMIT;
    };
    DEFAULT_CONTEXT_TOKEN_LIMIT.min(model_context.saturating_mul(60) / 100)
}

fn fixed_context_limit_tokens_from_env() -> Option<u64> {
    [
        "COMMAND_RUN_AGENT_FIXED_CONTEXT_TOKENS",
        "TURA_CONTEXT_LIMIT_TOKENS",
    ]
    .iter()
    .find_map(|key| {
        std::env::var(key)
            .ok()
            .and_then(|value| value.trim().parse::<u64>().ok())
            .filter(|value| *value > 0)
    })
}

fn catalog_model_context_tokens(
    settings: &tura_llm_rust::Settings,
    provider_id: &str,
    model_id: &str,
) -> Option<u64> {
    let provider = settings.model_catalog.providers.get(provider_id)?;
    provider
        .models
        .values()
        .flatten()
        .find(|entry| {
            tura_llm_rust::Settings::normalize_model_name(provider_id, entry.id()) == model_id
                || entry.id() == model_id
        })?
        .detail()
        .map(|detail| u64::from(detail.limit.context))
}

fn model_context_window_tokens(
    settings: &tura_llm_rust::Settings,
    runtime_provider_config: &crate::state_machine::runtime_management::RuntimeProviderConfig,
) -> u64 {
    catalog_model_context_tokens(
        settings,
        &runtime_provider_config.llm_provider_name,
        &runtime_provider_config.model_name,
    )
    .unwrap_or(DEFAULT_CONTEXT_TOKEN_LIMIT)
}

fn should_force_compact_prompt(
    session: &SessionManagement,
    messages: &[serde_json::Value],
    estimated_input_tokens: u64,
    context_limit_tokens: u64,
) -> bool {
    if messages_already_request_compact(messages) {
        return false;
    }
    estimated_input_tokens >= context_limit_tokens
        || previous_provider_input_tokens(session)
            .is_some_and(|input_tokens| input_tokens >= context_limit_tokens)
}

fn messages_already_request_compact(messages: &[serde_json::Value]) -> bool {
    messages.iter().any(|message| {
        serde_json::to_string(message).is_ok_and(|text| {
            text.contains("Context checkpoint required")
                || text.contains("compact_context as the final command")
        })
    })
}

fn previous_provider_input_tokens(session: &SessionManagement) -> Option<u64> {
    session
        .runtime_usage
        .get("input_tokens")
        .and_then(serde_json::Value::as_u64)
}

fn sync_context_tokens_from_provider_usage(
    session: &mut SessionManagement,
    runtime: &mut RuntimeManagement,
    context_window_tokens: u64,
) {
    if let Some(input_tokens) = runtime_input_tokens(runtime.usage.as_ref()) {
        session.context_tokens.input = input_tokens;
    }
    session.context_tokens.limit = context_window_tokens;
    runtime.context_tokens = session.context_tokens;
}

fn runtime_input_tokens(usage: Option<&UsageReport>) -> Option<u64> {
    usage
        .map(|usage| usage.input_tokens)
        .filter(|input_tokens| *input_tokens > 0)
}

fn compact_context_required_message(context_limit_tokens: u64) -> String {
    PromptBuilder::new()
        .part(compact_context::compact_context_required(
            context_limit_tokens,
        ))
        .render()
}

fn debug_runtime_enabled() -> bool {
    std::env::var("TURA_DEBUG_RUNTIME")
        .ok()
        .is_some_and(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
}

fn debug_runtime_timestamp() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

fn tool_choice_for_turn(disable_tool_invocation: bool) -> Option<serde_json::Value> {
    let _ = disable_tool_invocation;
    Some(serde_json::json!("auto"))
}

fn move_command_run_to_end(tools: Vec<serde_json::Value>) -> Vec<serde_json::Value> {
    let (mut others, mut command_run): (Vec<_>, Vec<_>) = tools
        .into_iter()
        .partition(|tool| tool_schema_name(tool) != Some(COMMAND_RUN_TOOL));
    others.append(&mut command_run);
    others
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state_machine::agent_management::{ProviderConfig, ToolChoice};
    use crate::state_machine::runtime_management::RuntimeProviderConfig;
    use std::collections::HashMap;
    use std::ffi::OsString;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn non_final_turn_uses_auto_tool_choice() {
        assert_eq!(tool_choice_for_turn(false), Some(serde_json::json!("auto")));
    }

    #[test]
    fn final_turn_keeps_auto_tool_choice_for_prompt_cache() {
        assert_eq!(tool_choice_for_turn(true), Some(serde_json::json!("auto")));
    }

    #[test]
    fn force_no_tools_keeps_auto_tool_choice_without_removing_schema() {
        assert_eq!(tool_choice_for_turn(true), Some(serde_json::json!("auto")));
    }

    #[test]
    fn dynamic_context_limit_uses_sixty_percent_of_model_context_when_smaller_than_cap() {
        let settings = settings_with_model_context("openai", "gpt-small", 128_000);
        let provider = runtime_provider("openai", "gpt-small");

        assert_eq!(dynamic_context_limit_tokens(&settings, &provider), 76_800);
    }

    #[test]
    fn dynamic_context_limit_caps_at_default_limit_for_large_models() {
        let settings = settings_with_model_context("openai", "gpt-large", 1_000_000);
        let provider = runtime_provider("openai", "gpt-large");

        assert_eq!(
            dynamic_context_limit_tokens(&settings, &provider),
            DEFAULT_CONTEXT_TOKEN_LIMIT
        );
    }

    #[test]
    fn dynamic_context_limit_honors_fixed_context_env_override() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|err| err.into_inner());
        let previous_fixed = std::env::var_os("COMMAND_RUN_AGENT_FIXED_CONTEXT_TOKENS");
        let previous_tura = std::env::var_os("TURA_CONTEXT_LIMIT_TOKENS");
        std::env::set_var("COMMAND_RUN_AGENT_FIXED_CONTEXT_TOKENS", "4096");
        std::env::remove_var("TURA_CONTEXT_LIMIT_TOKENS");
        let settings = settings_with_model_context("openai", "gpt-large", 1_000_000);
        let provider = runtime_provider("openai", "gpt-large");

        assert_eq!(dynamic_context_limit_tokens(&settings, &provider), 4096);

        restore_env("COMMAND_RUN_AGENT_FIXED_CONTEXT_TOKENS", previous_fixed);
        restore_env("TURA_CONTEXT_LIMIT_TOKENS", previous_tura);
    }

    #[test]
    fn force_compact_prompt_uses_previous_provider_usage_when_estimate_lags() {
        let mut session = SessionManagement::new(
            "sess-provider-usage-compact".to_string(),
            "provider usage compact".to_string(),
            std::path::PathBuf::from("C:/workspace"),
            false,
            "coding".to_string(),
            crate::state_machine::session_management::SessionInput {
                user_input: "continue".to_string(),
                file_input: vec![],
                agent: None,
                runtime_context: None,
                planning_mode_override: None,
            },
            "continue".to_string(),
            chrono::Utc::now(),
        );
        session.runtime_usage = serde_json::json!({
            "input_tokens": 250_012_u64,
        });
        let messages = vec![serde_json::json!({
            "role": "user",
            "content": "small estimated context"
        })];

        assert!(should_force_compact_prompt(
            &session, &messages, 100, 250_000
        ));
    }

    #[test]
    fn force_compact_prompt_does_not_duplicate_existing_request() {
        let session = SessionManagement::new(
            "sess-provider-usage-compact-existing".to_string(),
            "provider usage compact existing".to_string(),
            std::path::PathBuf::from("C:/workspace"),
            false,
            "coding".to_string(),
            crate::state_machine::session_management::SessionInput {
                user_input: "continue".to_string(),
                file_input: vec![],
                agent: None,
                runtime_context: None,
                planning_mode_override: None,
            },
            "continue".to_string(),
            chrono::Utc::now(),
        );
        let messages = vec![serde_json::json!({
            "role": "user",
            "content": "Context checkpoint required. compact_context as the final command"
        })];

        assert!(!should_force_compact_prompt(
            &session, &messages, 300_000, 250_000
        ));
    }

    #[test]
    fn provider_usage_replaces_estimated_context_tokens_for_display() {
        let mut session = SessionManagement::new(
            "sess-provider-usage-display".to_string(),
            "provider usage display".to_string(),
            std::path::PathBuf::from("C:/workspace"),
            false,
            "coding".to_string(),
            crate::state_machine::session_management::SessionInput {
                user_input: "continue".to_string(),
                file_input: vec![],
                agent: None,
                runtime_context: None,
                planning_mode_override: None,
            },
            "continue".to_string(),
            chrono::Utc::now(),
        );
        session.context_tokens.input = 255_298;
        session.context_tokens.limit = 250_000;

        let provider = runtime_provider("codex", "gpt-5.5");
        let mut runtime = RuntimeManagement::new(
            "runtime-provider-usage-display".to_string(),
            session.session_id.clone(),
            "agent-test".to_string(),
            provider,
            chrono::Utc::now(),
        );
        runtime.usage = Some(UsageReport {
            input_tokens: 1_353_553,
            output_tokens: 1,
            total_tokens: 1_353_554,
            cached_input_tokens: 0,
            cache_write_tokens: 0,
            reasoning_tokens: 0,
            attachment_input_tokens: 0,
            input_cost: 0.0,
            output_cost: 0.0,
            total_cost: 0.0,
            currency: "USD".to_string(),
            pricing_source: "provider".to_string(),
            latency_ms: 2_950,
            time_to_first_token_ms: 2_950,
            token_per_second: 0.33,
        });

        sync_context_tokens_from_provider_usage(&mut session, &mut runtime, 1_050_000);

        assert_eq!(session.context_tokens.input, 1_353_553);
        assert_eq!(session.context_tokens.limit, 1_050_000);
        assert_eq!(runtime.context_tokens, session.context_tokens);
    }

    #[test]
    fn model_context_window_uses_catalog_without_compact_cap() {
        let settings = settings_with_model_context("codex", "gpt-5.5", 1_050_000);
        let provider = runtime_provider("codex", "gpt-5.5");

        assert_eq!(model_context_window_tokens(&settings, &provider), 1_050_000);
        assert_eq!(
            dynamic_context_limit_tokens(&settings, &provider),
            DEFAULT_CONTEXT_TOKEN_LIMIT
        );
    }

    fn restore_env(key: &str, value: Option<OsString>) {
        if let Some(value) = value {
            std::env::set_var(key, value);
        } else {
            std::env::remove_var(key);
        }
    }

    fn runtime_provider(provider_id: &str, model_id: &str) -> RuntimeProviderConfig {
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
            provider_name: "fast".to_string(),
            model_name: model_id.to_string(),
            provider_url_name: provider_id.to_string(),
            llm_provider_name: provider_id.to_string(),
        }
    }

    fn settings_with_model_context(
        provider_id: &str,
        model_id: &str,
        context: u32,
    ) -> tura_llm_rust::Settings {
        let mut providers = HashMap::new();
        let mut models = HashMap::new();
        models.insert(
            provider_id.to_string(),
            vec![tura_llm_rust::CatalogModelConfig::Detailed(
                tura_llm_rust::CatalogModelDetail {
                    id: model_id.to_string(),
                    limit: tura_llm_rust::CatalogModelLimit {
                        context,
                        input: context,
                        output: 16_384,
                    },
                    ..tura_llm_rust::CatalogModelDetail::default()
                },
            )],
        );
        providers.insert(
            provider_id.to_string(),
            tura_llm_rust::ProviderCatalogConfig {
                models,
                ..tura_llm_rust::ProviderCatalogConfig::default()
            },
        );
        tura_llm_rust::Settings {
            provider_base_url: HashMap::new(),
            routes: HashMap::new(),
            model_catalog: tura_llm_rust::ModelCatalog {
                tiers: Vec::new(),
                providers,
            },
            provider_enums: tura_llm_rust::ProviderEnumCatalog::default(),
        }
    }
}
