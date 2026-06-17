use crate::prompt_style::{agent_identity, PromptBuilder};
use crate::runtime::call_runtime::{call_runtime, CallRuntimeInput};
use crate::runtime::create_runtime::{
    create_runtime, runtime_provider_config_from_tura, CreateRuntimeInput,
};
use crate::runtime::types::ToolCallData;
use crate::state_machine::agent_management::AgentManagement;
use crate::state_machine::runtime_management::RuntimeManagement;
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
        let context_limit_tokens =
            dynamic_context_limit_tokens(settings.as_ref(), &runtime_provider_config);
        let persona_names = agent
            .agent_persona
            .iter()
            .map(|persona| persona.persona_name.clone())
            .collect::<Vec<_>>();
        let language = session_language();
        let user_name = session_user_name();
        let identity = PromptBuilder::new()
            .part(agent_identity::agent_identity(
                &agent.agent_name,
                &user_name,
                &persona_names,
                &runtime_provider_config.model_name,
                &runtime_provider_config.llm_provider_name,
                context_limit_tokens,
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
            context_limit_tokens,
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
        session.context_tokens.input = approximate_message_tokens(&runtime_messages);
        session.context_tokens.limit = context_limit_tokens;
        let (runtime, queue_item) = create_runtime(CreateRuntimeInput {
            session_id: session.session_id.clone(),
            agent_id: agent.agent_id.clone(),
            messages: runtime_messages,
            tools,
            provider_config: agent.provider.clone(),
            tura_settings: std::sync::Arc::clone(&settings),
            thinking: false,
        })
        .await?;

        let config = std::sync::Arc::new(tura_llm_rust::TuraConfig::default());

        call_runtime(
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
        .await
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
    let Some(model_context) = catalog_model_context_tokens(
        settings,
        &runtime_provider_config.llm_provider_name,
        &runtime_provider_config.model_name,
    ) else {
        return DEFAULT_CONTEXT_TOKEN_LIMIT;
    };
    DEFAULT_CONTEXT_TOKEN_LIMIT.min(model_context.saturating_mul(60) / 100)
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

    fn runtime_provider(provider_id: &str, model_id: &str) -> RuntimeProviderConfig {
        RuntimeProviderConfig {
            base: ProviderConfig {
                tura_llm_name: "fast".to_string(),
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
