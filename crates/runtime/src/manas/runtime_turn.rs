use crate::prompt_style::{agent_identity, compact_context, self_reflection, PromptBuilder};
use crate::provider_flow::call::{call_runtime_with_writer, CallRuntimeInput};
use crate::runtime::create_runtime::{
    create_runtime, generate_runtime_id, runtime_provider_config_from_tura, CreateRuntimeInput,
};
use crate::runtime::types::ToolCallData;
use crate::runtime_event_writer::RuntimeEventWriter;
use crate::state_machine::agent_management::AgentManagement;
use lifecycle::RuntimeId;
use lifecycle::SessionManagement;
#[cfg(test)]
use lifecycle::DEFAULT_CONTEXT_TOKEN_LIMIT;
use lifecycle::{RuntimeAggregate, UsageReport};

use super::agent_prompts::{active_persona_display_name, load_agent_system_prompt_messages};
use super::constants::{COMMAND_RUN_TOOL, PLANNING_TOOL};
use super::prompt_messages::messages_for_turn_with_context_limit;
use super::tool_catalog::{
    command_run_commands_for_agent, extend_command_run_commands_with_capabilities,
    filter_tools_for_turn, load_agent_capabilities_with_commands, planning_tool_disabled,
    tool_schema_name,
};

const FORCE_COMPACT_CONTEXT_TOKEN_CAP: u64 = 260_000;
const PROMPT_INJECTION_CONTEXT_TOKEN_CAP: u64 = 240_000;

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
    runtime_id: Option<RuntimeId>,
    fallback_from_id: Option<RuntimeId>,
    mut runtime_event_writer: Option<&mut RuntimeEventWriter>,
) -> Result<(RuntimeAggregate, Vec<ToolCallData>), String> {
    let agent = agents
        .first()
        .ok_or_else(|| "no agent available".to_string())?;

    let mut agent_commands = command_run_commands_for_agent(agent);
    extend_command_run_commands_with_capabilities(
        &mut agent_commands,
        session.session_capabilities.iter().map(String::as_str),
    );
    let planning_enabled = agent_commands.contains(PLANNING_TOOL);
    let disable_tool_invocation = is_final_turn || force_no_tools;
    let require_startup_task_state = session.task_type.is_empty();
    let mut tools = load_agent_capabilities_with_commands(agent, session, &agent_commands)?;
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
            force_compact_context_limit_tokens(settings.as_ref(), &runtime_provider_config);
        let prompt_injection_limit_tokens =
            compact_prompt_injection_limit_tokens(settings.as_ref(), &runtime_provider_config);
        let language = session_language();
        let user_name = session_user_name();
        let identity = turn_identity(
            agent,
            &user_name,
            &runtime_provider_config.model_name,
            &runtime_provider_config.llm_provider_name,
            compact_limit_tokens,
            &language,
        );
        let mut runtime_messages = vec![serde_json::json!({
            "role": "system",
            "content": identity,
        })];
        runtime_messages.extend(load_agent_system_prompt_messages(agent)?);
        let turn = messages_for_turn_with_context_limit(
            current_messages,
            session,
            original_user_task,
            compact_limit_tokens,
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
        if !disable_tool_invocation
            && should_force_compact_prompt(session, prompt_injection_limit_tokens)
        {
            crate::prompt_style::tail_injection::append_tail_prompt(
                &mut runtime_messages,
                crate::prompt_style::tail_injection::TailPrompt::developer(
                    compact_context_required_message(prompt_injection_limit_tokens),
                ),
            );
        }
        append_self_reflection_tail_prompt(&mut runtime_messages, agent, session);
        session.context_tokens.input = provider_context_input_tokens(session).unwrap_or(0);
        session.context_tokens.limit = compact_limit_tokens;
        let (mut runtime, queue_item) = create_runtime(CreateRuntimeInput {
            runtime_id: runtime_id.unwrap_or_else(generate_runtime_id),
            session_id: session.session_id.clone(),
            fallback_from_id,
            agent_id: agent.agent_id.clone(),
            messages: runtime_messages,
            tools,
            provider_config: agent.provider.clone(),
            tura_settings: std::sync::Arc::clone(&settings),
            thinking: false,
            context_tokens: session.context_tokens,
        })
        .await?;
        if let Some(writer) = runtime_event_writer.as_deref_mut() {
            writer.flush(&mut runtime)?;
        }

        let config = std::sync::Arc::new(tura_llm_rust::TuraConfig::default());

        let runtime = call_runtime_with_writer(
            CallRuntimeInput {
                runtime,
                messages: queue_item.messages,
                tools: queue_item.tools,
                provider_name: queue_item.provider_name,
                stream: agent.provider.stream,
                max_tokens: agent.provider.max_tokens,
                tool_choice: tool_choice_for_turn(),
                session_directory: session.session_directory.clone(),
                allowed_command_run_commands: Some(agent_commands),
                require_startup_task_state,
            },
            settings,
            config,
            runtime_event_writer.as_deref_mut(),
        )
        .await?;
        sync_context_tokens_from_provider_usage(session, &runtime, compact_limit_tokens);
        Ok::<RuntimeAggregate, String>(runtime)
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

fn force_compact_context_limit_tokens(
    settings: &tura_llm_rust::Settings,
    runtime_provider_config: &lifecycle::RuntimeProviderConfig,
) -> u64 {
    dynamic_context_limit_tokens(
        settings,
        runtime_provider_config,
        95,
        FORCE_COMPACT_CONTEXT_TOKEN_CAP,
    )
}

fn compact_prompt_injection_limit_tokens(
    settings: &tura_llm_rust::Settings,
    runtime_provider_config: &lifecycle::RuntimeProviderConfig,
) -> u64 {
    dynamic_context_limit_tokens(
        settings,
        runtime_provider_config,
        80,
        PROMPT_INJECTION_CONTEXT_TOKEN_CAP,
    )
}

fn dynamic_context_limit_tokens(
    settings: &tura_llm_rust::Settings,
    runtime_provider_config: &lifecycle::RuntimeProviderConfig,
    model_context_percent: u64,
    cap_tokens: u64,
) -> u64 {
    if let Some(limit) = fixed_context_limit_tokens_from_env() {
        return limit;
    }
    let Some(model_context) = catalog_model_context_tokens(
        settings,
        &runtime_provider_config.llm_provider_name,
        &runtime_provider_config.model_name,
    ) else {
        return cap_tokens;
    };
    cap_tokens.min(model_context.saturating_mul(model_context_percent) / 100)
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

#[cfg(test)]
fn model_context_window_tokens(
    settings: &tura_llm_rust::Settings,
    runtime_provider_config: &lifecycle::RuntimeProviderConfig,
) -> u64 {
    catalog_model_context_tokens(
        settings,
        &runtime_provider_config.llm_provider_name,
        &runtime_provider_config.model_name,
    )
    .unwrap_or(DEFAULT_CONTEXT_TOKEN_LIMIT)
}

fn should_force_compact_prompt(session: &SessionManagement, context_limit_tokens: u64) -> bool {
    provider_context_input_tokens(session)
        .is_some_and(|input_tokens| input_tokens >= context_limit_tokens)
}

fn previous_provider_input_tokens(session: &SessionManagement) -> Option<u64> {
    session
        .runtime_usage
        .get("input_tokens")
        .and_then(serde_json::Value::as_u64)
}

fn provider_context_input_tokens(session: &SessionManagement) -> Option<u64> {
    previous_provider_input_tokens(session)
        .or_else(|| (session.context_tokens.input > 0).then_some(session.context_tokens.input))
}

fn sync_context_tokens_from_provider_usage(
    session: &mut SessionManagement,
    runtime: &RuntimeAggregate,
    active_context_limit_tokens: u64,
) {
    if let Some(input_tokens) = runtime_input_tokens(runtime.usage.as_ref()) {
        session.context_tokens.input = input_tokens;
    }
    session.context_tokens.limit = active_context_limit_tokens;
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

fn tool_choice_for_turn() -> Option<serde_json::Value> {
    Some(serde_json::json!("auto"))
}

fn turn_identity(
    agent: &AgentManagement,
    user_name: &str,
    model_name: &str,
    llm_provider_name: &str,
    active_context_limit_tokens: u64,
    language: &str,
) -> String {
    let persona_or_agent_name =
        active_persona_display_name(agent).unwrap_or_else(|| agent.agent_name.clone());
    PromptBuilder::new()
        .part(agent_identity::agent_identity(
            &persona_or_agent_name,
            user_name,
            model_name,
            llm_provider_name,
            active_context_limit_tokens,
            language,
        ))
        .render()
}

fn append_self_reflection_tail_prompt(
    messages: &mut Vec<serde_json::Value>,
    agent: &AgentManagement,
    session: &SessionManagement,
) {
    if !agent.self_reflection && !session.goal_mode {
        return;
    }

    crate::prompt_style::tail_injection::append_tail_prompt(
        messages,
        crate::prompt_style::tail_injection::TailPrompt::developer(
            self_reflection::self_reflection_tail_prompt(session),
        ),
    );
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
    use lifecycle::RuntimeProviderConfig;
    use lifecycle::{ProviderConfig, ToolChoice};
    use std::collections::HashMap;
    use std::ffi::OsString;

    #[test]
    fn turns_use_auto_tool_choice_for_prompt_cache_stability() {
        assert_eq!(tool_choice_for_turn(), Some(serde_json::json!("auto")));
    }

    #[test]
    fn identity_uses_active_persona_display_name_instead_of_agent_name() {
        let _guard = crate::manas::TEST_ENV_LOCK
            .lock()
            .unwrap_or_else(|err| err.into_inner());
        let previous_persona = std::env::var_os("TURA_SESSION_PERSONA");
        let previous_root = std::env::var_os("TURA_PROJECT_ROOT");
        let previous_frontend_source = std::env::var_os("TURA_FRONTEND_SOURCE");
        let run_id = format!(
            "tura-runtime-identity-persona-test-{}",
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default()
        );
        let root = std::env::temp_dir().join(run_id);
        let agent_dir = root.join("agents").join("src").join("balanced");
        let prompt_dir = root
            .join("personas")
            .join("src")
            .join("guide")
            .join("prompt");
        std::fs::create_dir_all(&agent_dir).expect("agent dir should be created");
        std::fs::create_dir_all(&prompt_dir).expect("persona prompt dir should be created");
        std::fs::write(prompt_dir.join("persona.md"), "persona prompt")
            .expect("persona prompt should be written");
        std::fs::write(
            root.join("personas")
                .join("src")
                .join("guide")
                .join("persona_config.json"),
            serde_json::to_string_pretty(&serde_json::json!({
                "persona_name": "guide",
                "display_name": "Guide",
                "description": "Guide persona",
                "short_description": "Guide",
                "default_config": true,
                "persona_directory": "personas/src/guide",
                "prompt_directory": "personas/src/guide/prompt",
                "media": null,
                "metadata": {}
            }))
            .expect("persona config should encode"),
        )
        .expect("persona config should be written");

        std::env::set_var("TURA_SESSION_PERSONA", "guide");
        std::env::set_var("TURA_PROJECT_ROOT", &root);
        std::env::remove_var("TURA_FRONTEND_SOURCE");

        let agent = AgentManagement::new(
            "agent-id".to_string(),
            "balanced".to_string(),
            agent_dir,
            None,
            true,
            false,
            false,
            false,
            ProviderConfig {
                tura_llm_name: "fast".to_string(),
                default_model_tier: None,
                current_model: None,
                stream: true,
                temperature: 0.0,
                max_tokens: 0,
                tool_choice: ToolChoice::Auto,
                time_out_ms: 30_000,
            },
            crate::state_machine::agent_management::ValidatorConfig {
                need_validator: false,
                validator_name: None,
            },
            chrono::Utc::now(),
        );

        let identity = turn_identity(&agent, "Local User", "gpt-5.5", "codex", 255_000, "en");

        assert!(
            identity.starts_with("You are Guide, an agent."),
            "{identity}"
        );
        assert!(!identity.starts_with("You are balanced"), "{identity}");

        restore_env("TURA_SESSION_PERSONA", previous_persona);
        restore_env("TURA_PROJECT_ROOT", previous_root);
        restore_env("TURA_FRONTEND_SOURCE", previous_frontend_source);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn self_reflection_tail_prompt_appends_for_reflective_agent_as_last_message() {
        let mut session = session_for_tail_prompt();
        session.task_type =
            crate::prompt_style::runtime_prompt_manual::normalize_task_type_ids(["debug"]);
        let mut agent = agent_for_tail_prompt(false);
        agent.self_reflection = true;
        let mut messages = vec![serde_json::json!({"role": "user", "content": "work"})];

        append_self_reflection_tail_prompt(&mut messages, &agent, &session);

        let tail = messages.last().expect("tail prompt should be appended");
        assert_eq!(tail["role"], "developer");
        let content = tail["content"].as_str().expect("tail content");
        assert!(content.contains("Debug Operation Manual"), "{content}");
        assert!(content.contains("complete the required `Self Reflection`"));
    }

    #[test]
    fn self_reflection_tail_prompt_appends_for_goal_mode_without_agent_flag() {
        let mut session = session_for_tail_prompt();
        session.goal_mode = true;
        session.task_type =
            crate::prompt_style::runtime_prompt_manual::normalize_task_type_ids(["frontend"]);
        let agent = agent_for_tail_prompt(false);
        let mut messages = vec![serde_json::json!({"role": "user", "content": "work"})];

        append_self_reflection_tail_prompt(&mut messages, &agent, &session);

        let tail = messages.last().expect("tail prompt should be appended");
        assert_eq!(tail["role"], "developer");
        let content = tail["content"].as_str().expect("tail content");
        assert!(
            content.contains("Visual Operation Manual, Frontend Operation Manual"),
            "{content}"
        );
    }

    #[test]
    fn self_reflection_tail_prompt_skips_when_disabled_and_not_goal_mode() {
        let session = session_for_tail_prompt();
        let agent = agent_for_tail_prompt(false);
        let mut messages = vec![serde_json::json!({"role": "user", "content": "work"})];

        append_self_reflection_tail_prompt(&mut messages, &agent, &session);

        assert_eq!(messages.len(), 1);
    }

    #[test]
    fn compact_limits_use_distinct_model_percentages_when_smaller_than_caps() {
        let _guard = crate::manas::TEST_ENV_LOCK
            .lock()
            .unwrap_or_else(|err| err.into_inner());
        let _env = clean_context_limit_env();
        let settings = settings_with_model_context("openai", "gpt-small", 128_000);
        let provider = runtime_provider("openai", "gpt-small");

        assert_eq!(
            compact_prompt_injection_limit_tokens(&settings, &provider),
            102_400
        );
        assert_eq!(
            force_compact_context_limit_tokens(&settings, &provider),
            121_600
        );
    }

    #[test]
    fn compact_limits_use_requested_caps_for_large_models() {
        let _guard = crate::manas::TEST_ENV_LOCK
            .lock()
            .unwrap_or_else(|err| err.into_inner());
        let _env = clean_context_limit_env();
        let settings = settings_with_model_context("openai", "gpt-large", 1_000_000);
        let provider = runtime_provider("openai", "gpt-large");

        assert_eq!(
            compact_prompt_injection_limit_tokens(&settings, &provider),
            240_000
        );
        assert_eq!(
            force_compact_context_limit_tokens(&settings, &provider),
            260_000
        );
    }

    #[test]
    fn compact_limits_honor_fixed_context_env_override() {
        let _guard = crate::manas::TEST_ENV_LOCK
            .lock()
            .unwrap_or_else(|err| err.into_inner());
        let _env = clean_context_limit_env();
        std::env::set_var("COMMAND_RUN_AGENT_FIXED_CONTEXT_TOKENS", "4096");
        let settings = settings_with_model_context("openai", "gpt-large", 1_000_000);
        let provider = runtime_provider("openai", "gpt-large");

        assert_eq!(
            compact_prompt_injection_limit_tokens(&settings, &provider),
            4096
        );
        assert_eq!(
            force_compact_context_limit_tokens(&settings, &provider),
            4096
        );
    }

    #[test]
    fn force_compact_prompt_uses_previous_provider_usage() {
        let mut session = SessionManagement::new(
            "sess-provider-usage-compact".to_string(),
            "provider usage compact".to_string(),
            std::path::PathBuf::from("C:/workspace"),
            false,
            "coding".to_string(),
            lifecycle::SessionInput {
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
            "input_tokens": 200_012_u64,
        });
        assert!(should_force_compact_prompt(&session, 200_000));
    }

    #[test]
    fn force_compact_prompt_uses_provider_usage_without_scanning_prompt_text() {
        let mut session = SessionManagement::new(
            "sess-provider-usage-compact-existing".to_string(),
            "provider usage compact existing".to_string(),
            std::path::PathBuf::from("C:/workspace"),
            false,
            "coding".to_string(),
            lifecycle::SessionInput {
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
            "input_tokens": 200_012_u64,
        });
        assert!(should_force_compact_prompt(&session, 200_000));
    }

    #[test]
    fn provider_usage_replaces_context_tokens_for_display() {
        let mut session = SessionManagement::new(
            "sess-provider-usage-display".to_string(),
            "provider usage display".to_string(),
            std::path::PathBuf::from("C:/workspace"),
            false,
            "coding".to_string(),
            lifecycle::SessionInput {
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
        session.context_tokens.limit = 200_000;

        let provider = runtime_provider("codex", "gpt-5.5");
        let mut runtime = RuntimeAggregate::new(
            "runtime-provider-usage-display".to_string(),
            session.session_id.clone(),
            "agent-test".to_string(),
            provider,
            chrono::Utc::now(),
        );
        runtime
            .update_usage(Some(UsageReport {
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
            }))
            .expect("fixture usage should apply");

        sync_context_tokens_from_provider_usage(&mut session, &runtime, 200_000);

        assert_eq!(session.context_tokens.input, 1_353_553);
        assert_eq!(session.context_tokens.limit, 200_000);
    }

    #[test]
    fn model_context_window_uses_catalog_without_compact_cap() {
        let _guard = crate::manas::TEST_ENV_LOCK
            .lock()
            .unwrap_or_else(|err| err.into_inner());
        let _env = clean_context_limit_env();
        let settings = settings_with_model_context("codex", "gpt-5.5", 1_050_000);
        let provider = runtime_provider("codex", "gpt-5.5");

        assert_eq!(model_context_window_tokens(&settings, &provider), 1_050_000);
        assert_eq!(
            compact_prompt_injection_limit_tokens(&settings, &provider),
            240_000
        );
        assert_eq!(
            force_compact_context_limit_tokens(&settings, &provider),
            260_000
        );
    }

    fn restore_env(key: &str, value: Option<OsString>) {
        if let Some(value) = value {
            std::env::set_var(key, value);
        } else {
            std::env::remove_var(key);
        }
    }

    struct ContextLimitEnvGuard {
        fixed: Option<OsString>,
        tura: Option<OsString>,
    }

    impl Drop for ContextLimitEnvGuard {
        fn drop(&mut self) {
            restore_env("COMMAND_RUN_AGENT_FIXED_CONTEXT_TOKENS", self.fixed.take());
            restore_env("TURA_CONTEXT_LIMIT_TOKENS", self.tura.take());
        }
    }

    fn clean_context_limit_env() -> ContextLimitEnvGuard {
        let guard = ContextLimitEnvGuard {
            fixed: std::env::var_os("COMMAND_RUN_AGENT_FIXED_CONTEXT_TOKENS"),
            tura: std::env::var_os("TURA_CONTEXT_LIMIT_TOKENS"),
        };
        std::env::remove_var("COMMAND_RUN_AGENT_FIXED_CONTEXT_TOKENS");
        std::env::remove_var("TURA_CONTEXT_LIMIT_TOKENS");
        guard
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

    fn agent_for_tail_prompt(self_reflection: bool) -> AgentManagement {
        AgentManagement::new(
            "agent-tail".to_string(),
            "tail".to_string(),
            std::path::PathBuf::from("agents/tail"),
            None,
            true,
            false,
            false,
            self_reflection,
            ProviderConfig {
                tura_llm_name: "fast".to_string(),
                default_model_tier: None,
                current_model: None,
                stream: true,
                temperature: 0.0,
                max_tokens: 0,
                tool_choice: ToolChoice::Auto,
                time_out_ms: 30_000,
            },
            crate::state_machine::agent_management::ValidatorConfig {
                need_validator: false,
                validator_name: None,
            },
            chrono::Utc::now(),
        )
    }

    fn session_for_tail_prompt() -> SessionManagement {
        SessionManagement::new(
            "session-tail".to_string(),
            "tail".to_string(),
            std::path::PathBuf::from("C:/workspace"),
            false,
            "coding".to_string(),
            lifecycle::SessionInput {
                user_input: "work".to_string(),
                file_input: vec![],
                agent: None,
                runtime_context: None,
                planning_mode_override: None,
            },
            "work".to_string(),
            chrono::Utc::now(),
        )
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
