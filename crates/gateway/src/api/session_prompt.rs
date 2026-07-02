use super::*;

pub async fn prompt_async(
    Path(session_id): Path<String>,
    Json(payload): Json<serde_json::Value>,
) -> impl IntoResponse {
    session_store().clear_cancelled(&session_id);
    let content = prompt_text(&payload).unwrap_or_else(|| "Prompt submitted".to_string());
    let session = session_store().get_session(&session_id);
    if session.is_none() {
        return StatusCode::NOT_FOUND;
    }
    if session
        .as_ref()
        .is_some_and(|session| matches!(session.status, SessionStatus::Busy))
    {
        let metadata = serde_json::json!({
            "kind": "user_new_command",
        });
        let parts = prompt_message_parts(&payload);
        let _ = session_store().add_message_with_parts(
            &session_id,
            SessionMessageRole::User,
            parts,
            prompt_message_id(&payload),
            Some(metadata),
        );
        append_user_command_for_runtime(&session_id, content);
        return StatusCode::NO_CONTENT;
    }
    let user_message = session_store().add_message_with_parts(
        &session_id,
        SessionMessageRole::User,
        prompt_message_parts(&payload),
        prompt_message_id(&payload),
        None,
    );
    session_store().update_session_status(&session_id, SessionStatusMano::Busy);
    session_store().set_todos(
        &session_id,
        vec![serde_json::json!({
            "id": format!("{session_id}:planning"),
            "content": "规划执行步骤",
            "status": "in_progress",
            "priority": "medium",
        })],
    );
    watch_direct_mano_messages(
        session_id.clone(),
        session_store().get_messages(&session_id).len(),
    );
    let session_id_for_task = session_id;
    let payload_for_task = user_message
        .as_ref()
        .map(|message| prompt_payload_with_frontend_ids(payload.clone(), message))
        .unwrap_or(payload);
    tokio::task::spawn_blocking(move || {
        if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            run_mano_for_prompt(session_id_for_task.clone(), payload_for_task);
        }))
        .is_err()
        {
            tracing::error!(session_id = %session_id_for_task, "MANO prompt task panicked");
            session_store().update_session_status(&session_id_for_task, SessionStatusMano::Error);
            session_store().finish_todos(&session_id_for_task, false);
            add_agent_fallback_message(
                &session_id_for_task,
                "MANO failed while processing this prompt: background task panicked before completion.".to_string(),
            );
        }
    });
    StatusCode::NO_CONTENT
}

pub fn start_task_scheduler() {
    tokio::spawn(async {
        let mut interval = tokio::time::interval(Duration::from_secs(15));
        loop {
            interval.tick().await;
            run_due_task_scheduler_tick();
        }
    });
}

fn run_due_task_scheduler_tick() {
    run_due_task_scheduler_tick_with_launcher(|run, prompt| {
        std::thread::spawn(move || {
            if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                run_mano_for_prompt(run.session_id.clone(), prompt);
                if matches!(run.start_condition, StartCondition::PollingTask) {
                    reset_polling_task_after_run(&run.session_id);
                }
            }))
            .is_err()
            {
                tracing::error!(session_id = %run.session_id, "scheduled task panicked");
                session_store().update_session_status(&run.session_id, SessionStatusMano::Error);
                session_store().finish_todos(&run.session_id, false);
                add_agent_fallback_message(
                    &run.session_id,
                    "Scheduled task failed before completion.".to_string(),
                );
            }
        });
    });
}

fn run_due_task_scheduler_tick_with_launcher(
    mut launch: impl FnMut(crate::session::store::ScheduledTaskRun, serde_json::Value),
) {
    run_due_task_scheduler_tick_for_store_with_launcher(session_store(), true, |run, prompt| {
        launch(run, prompt);
    });
}

fn run_due_task_scheduler_tick_for_store_with_launcher(
    store: &crate::session::SessionStore,
    watch_messages: bool,
    mut launch: impl FnMut(crate::session::store::ScheduledTaskRun, serde_json::Value),
) {
    for run in store.claim_due_task_runs(chrono::Utc::now()) {
        let prompt = scheduler_prompt_payload(&run.task_summary, run.start_condition);
        let content = prompt_text(&prompt).unwrap_or_else(|| run.task_summary.clone());
        let initial_count = store.get_messages(&run.session_id).len();
        let _ = store.add_message_with_metadata(
            &run.session_id,
            SessionMessageRole::User,
            content,
            Some(serde_json::json!({
                "kind": "task_scheduler",
                "start_condition": run.start_condition,
            })),
        );
        store.set_todos(
            &run.session_id,
            vec![serde_json::json!({
                "id": format!("{}:scheduled-task", run.session_id),
                "content": run.task_summary,
                "status": "in_progress",
                "priority": "medium",
            })],
        );
        if watch_messages {
            watch_direct_mano_messages(run.session_id.clone(), initial_count);
        }
        launch(run, prompt);
    }
}

#[cfg(any(feature = "business-tests", feature = "os-tests"))]
pub fn run_due_task_scheduler_tick_for_business_test() {
    run_due_task_scheduler_tick_with_launcher(|_, _| {});
}

#[cfg(any(feature = "business-tests", feature = "os-tests"))]
pub fn run_due_task_scheduler_tick_for_store_business_test(store: &crate::session::SessionStore) {
    run_due_task_scheduler_tick_for_store_with_launcher(store, false, |_, _| {});
}

fn scheduler_prompt_payload(
    task_summary: &str,
    start_condition: StartCondition,
) -> serde_json::Value {
    let trigger = match start_condition {
        StartCondition::SessionIdle => "session became idle",
        StartCondition::ScheduledTask => "scheduled start time arrived",
        StartCondition::PollingTask => "polling interval became due",
        StartCondition::UserAction => "user action",
    };
    serde_json::json!({
        "parts": [{
            "id": format!("part_scheduler_{}", uuid::Uuid::new_v4()),
            "type": "text",
            "text": format!("Continue the pending task because the {trigger}: {task_summary}")
        }],
        "source": "task_scheduler"
    })
}

fn reset_polling_task_after_run(session_id: &str) {
    let Some(session) = session_store().get_session(session_id) else {
        return;
    };
    let current_status = session
        .task_management
        .get("status")
        .and_then(serde_json::Value::as_str);
    if matches!(current_status, Some("done" | "archived")) {
        return;
    }
    let _ = session_store().update_session(
        session_id,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        Some(serde_json::json!({ "status": "todo" })),
    );
}

fn watch_direct_mano_messages(session_id: String, initial_count: usize) {
    std::thread::spawn(move || {
        let mut seen = initial_count;
        for _ in 0..1200 {
            std::thread::sleep(std::time::Duration::from_millis(250));
            let messages = session_store().get_messages(&session_id);
            if messages.len() > seen {
                for message in messages.iter().skip(seen).cloned() {
                    session_store().push_event(GlobalEvent::MessageUpdated {
                        properties: MessageUpdatedProperties {
                            session_id: session_id.clone(),
                            info: api_message_from_store(message),
                        },
                    });
                }
                seen = messages.len();
            }
        }
    });
}

pub(super) fn prompt_text(payload: &serde_json::Value) -> Option<String> {
    let parts = payload.get("parts")?.as_array()?;
    let text = parts
        .iter()
        .filter_map(|part| {
            if part.get("type")?.as_str()? != "text" {
                return None;
            }
            part.get("text")?.as_str()
        })
        .collect::<Vec<_>>()
        .join("");
    (!text.is_empty()).then_some(text)
}

fn prompt_message_parts(payload: &serde_json::Value) -> Vec<crate::session::MessagePart> {
    payload
        .get("parts")
        .and_then(serde_json::Value::as_array)
        .map(|parts| {
            parts
                .iter()
                .filter_map(prompt_message_part)
                .collect::<Vec<_>>()
        })
        .filter(|parts| !parts.is_empty())
        .unwrap_or_else(|| {
            vec![crate::session::MessagePart {
                id: prompt_fallback_part_id(),
                part_type: "text".to_string(),
                content: Some("Prompt submitted".to_string()),
                text: Some("Prompt submitted".to_string()),
                metadata: None,
                call_id: None,
                tool: None,
                state: None,
            }]
        })
}

fn prompt_message_part(part: &serde_json::Value) -> Option<crate::session::MessagePart> {
    let part_type = part
        .get("type")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("text");
    if !is_queue_prompt_part_type(part_type) {
        return None;
    }
    Some(crate::session::MessagePart {
        id: part
            .get("id")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToString::to_string)
            .unwrap_or_else(prompt_fallback_part_id),
        part_type: part_type.to_string(),
        content: part
            .get("content")
            .and_then(serde_json::Value::as_str)
            .map(ToString::to_string),
        text: part
            .get("text")
            .and_then(serde_json::Value::as_str)
            .map(ToString::to_string),
        metadata: part.get("metadata").cloned(),
        call_id: part
            .get("callID")
            .or_else(|| part.get("call_id"))
            .and_then(serde_json::Value::as_str)
            .map(ToString::to_string),
        tool: part
            .get("tool")
            .and_then(serde_json::Value::as_str)
            .map(ToString::to_string),
        state: part.get("state").cloned(),
    })
}

fn is_queue_prompt_part_type(part_type: &str) -> bool {
    matches!(
        part_type.trim().to_ascii_lowercase().as_str(),
        "text" | "message" | "voice" | "audio" | "speech" | "input_audio" | "audio_url"
    )
}

fn prompt_fallback_part_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

pub(super) fn prompt_message_id(payload: &serde_json::Value) -> Option<String> {
    payload
        .get("messageID")
        .or_else(|| payload.get("message_id"))
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

pub(super) fn first_prompt_part_id(payload: &serde_json::Value) -> Option<String> {
    payload
        .get("parts")?
        .as_array()?
        .iter()
        .find(|part| {
            part.get("type")
                .and_then(|value| value.as_str())
                .is_none_or(|part_type| part_type == "text")
        })?
        .get("id")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn prompt_payload_with_frontend_ids(
    mut payload: serde_json::Value,
    message: &crate::session::Message,
) -> serde_json::Value {
    let Some(object) = payload.as_object_mut() else {
        return payload;
    };
    object
        .entry("messageID".to_string())
        .or_insert_with(|| serde_json::Value::String(message.id.clone()));

    if first_prompt_part_id(&serde_json::Value::Object(object.clone())).is_some() {
        return payload;
    }

    let Some(part_id) = message
        .parts
        .iter()
        .find(|part| part.part_type == "text")
        .map(|part| part.id.clone())
    else {
        return payload;
    };
    if let Some(parts) = object
        .get_mut("parts")
        .and_then(serde_json::Value::as_array_mut)
    {
        if let Some(part) = parts.iter_mut().find(|part| {
            part.get("type")
                .and_then(serde_json::Value::as_str)
                .is_none_or(|part_type| part_type == "text")
        }) {
            if let Some(part_object) = part.as_object_mut() {
                part_object
                    .entry("id".to_string())
                    .or_insert_with(|| serde_json::Value::String(part_id));
            }
        }
    }
    payload
}

fn prompt_runtime_context(payload: &serde_json::Value) -> Option<String> {
    payload
        .get("system")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

/// Forward one prompt through the router; the gateway only handles handoff and
/// final state bookkeeping. Runtime work runs in router-managed worker
/// subprocesses and reports events through the existing callback channel.
pub(super) fn run_mano_for_prompt(session_id: String, payload: serde_json::Value) {
    let content = prompt_text(&payload).unwrap_or_else(|| "Prompt submitted".to_string());
    let before_count = session_store().get_messages(&session_id).len();
    let session = session_store().get_session(&session_id);
    let directory = session
        .as_ref()
        .and_then(|session| {
            session
                .directory
                .clone()
                .map(|directory| directory.trim().to_string())
                .filter(|directory| !directory.is_empty())
        })
        .or_else(|| global_store().get_current_directory());
    let session_config = directory.as_deref().and_then(load_prompt_session_config);

    let agent = prompt_agent_for_run(
        &payload,
        session.as_ref().and_then(|session| session.agent.clone()),
        session_config.as_ref(),
    );
    let runtime_context = prompt_runtime_context(&payload);
    let force_planning = session
        .as_ref()
        .map(|session| session.force_planning)
        .unwrap_or(false);
    let agent_runtime_settings = agent
        .as_deref()
        .and_then(|agent| agent_runtime_settings(agent, directory.as_deref()))
        .unwrap_or_default();
    let model_override = prompt_runtime_model_override(
        &payload,
        session.as_ref().and_then(|session| session.model.clone()),
        session_config.as_ref(),
    );
    let reasoning_effort = prompt_model_variant(&payload)
        .or_else(|| {
            session_config.as_ref().and_then(|config| {
                config.string("model_variant", |config| config.model_variant.clone())
            })
        })
        .or(agent_runtime_settings.reasoning_effort)
        .or_else(|| {
            session
                .as_ref()
                .and_then(|session| session.model_variant.clone())
                .filter(|value| !value.trim().is_empty())
        });
    let acceleration_enabled = prompt_model_acceleration(&payload)
        .or_else(|| {
            session_config.as_ref().and_then(|config| {
                config.bool("model_acceleration_enabled", |config| {
                    config.model_acceleration_enabled
                })
            })
        })
        .or_else(|| {
            session
                .as_ref()
                .map(|session| session.model_acceleration_enabled)
        })
        .unwrap_or(false);
    let command_run_stall_guard = session_config
        .as_ref()
        .map(|config| config.config.command_run_stall_guard())
        .unwrap_or_else(|| TuraSessionConfig::default().command_run_stall_guard());
    let command_run_shell = prompt_command_run_shell(&payload);
    let language = session_config
        .as_ref()
        .and_then(|config| config.string("language", |config| config.language.clone()))
        .or_else(|| global_store().get_config().language)
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let persona = session_config
        .as_ref()
        .and_then(|config| config.string("active_persona", |config| config.active_persona.clone()))
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());

    // Worker env contract: router injects these values into the runtime worker.
    let mut worker_env: std::collections::HashMap<String, String> =
        std::collections::HashMap::new();
    if let Some(message_id) = prompt_message_id(&payload) {
        worker_env.insert("TURA_FRONTEND_MESSAGE_ID".to_string(), message_id);
    }
    if let Some(part_id) = first_prompt_part_id(&payload) {
        worker_env.insert("TURA_FRONTEND_PART_ID".to_string(), part_id);
    }
    if let Some(reasoning) = reasoning_effort
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty() && !value.eq_ignore_ascii_case("default"))
    {
        worker_env.insert(
            "TURA_SESSION_REASONING_EFFORT".to_string(),
            reasoning.to_string(),
        );
    }
    if acceleration_enabled {
        worker_env.insert(
            "TURA_SESSION_ACCELERATION_ENABLED".to_string(),
            "1".to_string(),
        );
    }
    if let Some(shell) = command_run_shell {
        worker_env.insert("TURA_COMMAND_RUN_SHELL".to_string(), shell);
    }
    if let Some(language) = language {
        worker_env.insert("TURA_SESSION_LANGUAGE".to_string(), language);
    }
    if prompt_source_is_cli(&payload) {
        worker_env.insert("TURA_FRONTEND_SOURCE".to_string(), "cli".to_string());
    }
    if let Some(persona) = persona {
        worker_env.insert("TURA_SESSION_PERSONA".to_string(), persona);
    }
    let user_name = current_user_snapshot().name.trim().to_string();
    if !user_name.is_empty() {
        worker_env.insert("TURA_SESSION_USER_NAME".to_string(), user_name);
    }
    worker_env.insert(
        "TURA_COMMAND_RUN_STALL_CHECK_SECS".to_string(),
        command_run_stall_guard.check_secs.to_string(),
    );
    worker_env.insert(
        "TURA_COMMAND_RUN_STALL_IDENTICAL_CHECKS".to_string(),
        command_run_stall_guard.identical_checks.to_string(),
    );
    let turn_id =
        first_prompt_part_id(&payload).unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let body = serde_json::json!({
        "session_id": session_id,
        "directory": directory,
        "model": model_override,
        "agent": agent,
        "prompt": content,
        "runtime_context": runtime_context,
        "planning_mode_override": force_planning.then_some(true),
        "worker_env": worker_env,
    });

    let result = forward_run_agent_to_router(&turn_id, &session_id, &body);

    if session_store().is_cancelled(&session_id) {
        session_store().finish_todos(&session_id, false);
        session_store().update_session_status(&session_id, SessionStatusMano::Idle);
        return;
    }

    match result {
        Ok(()) => {
            session_store().update_session_status(&session_id, SessionStatusMano::Idle);
            session_store().finish_todos(&session_id, true);
            if let Some(message) = final_agent_message(&session_id, before_count) {
                session_store().push_event(GlobalEvent::MessageUpdated {
                    properties: MessageUpdatedProperties {
                        session_id: session_id.clone(),
                        info: api_message_from_store(message),
                    },
                });
            }
        }
        Err(error) => {
            session_store().update_session_status(&session_id, SessionStatusMano::Error);
            session_store().finish_todos(&session_id, false);
            add_agent_fallback_message(
                &session_id,
                format!("MANO failed while processing this prompt: {error}"),
            );
        }
    }
}

/// Submit through the gateway-owned persistent router instead of spawning a
/// runtime worker directly.
fn forward_run_agent_to_router(
    turn_id: &str,
    session_id: &str,
    body: &serde_json::Value,
) -> Result<(), String> {
    let value = crate::router_client::RouterClient::global()
        .enqueue_turn(crate::router_client::EnqueueTurnRequest {
            turn_id: turn_id.to_string(),
            session_id: session_id.to_string(),
            payload: body.clone(),
        })
        .map_err(|error| {
            format!("failed to enqueue turn {turn_id} for session {session_id}: {error}")
        })?;
    if value
        .get("ok")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(true)
    {
        Ok(())
    } else {
        let error = value
            .get("error")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("gateway worker returned failure")
            .to_string();
        Err(format!(
            "router rejected turn {turn_id} for session {session_id}: {error}"
        ))
    }
}

fn prompt_model_override(payload: &serde_json::Value) -> Option<String> {
    let model = payload.get("model")?;
    if let Some(value) = model.as_str() {
        return Some(value.to_string());
    }
    let provider = model
        .get("providerID")
        .or_else(|| model.get("provider_id"))
        .and_then(|value| value.as_str())?;
    let model_id = model
        .get("modelID")
        .or_else(|| model.get("model_id"))
        .and_then(|value| value.as_str())?;
    Some(format!("{provider}/{model_id}"))
}

fn prompt_runtime_model_override(
    payload: &serde_json::Value,
    session_model: Option<String>,
    session_config: Option<&PromptSessionConfig>,
) -> Option<String> {
    prompt_model_override(payload)
        .or_else(|| session_config.and_then(explicit_config_model_override))
        .or(session_model)
        .and_then(normalize_model_override)
}

pub(super) fn config_model_override(config: &TuraSessionConfig) -> Option<String> {
    if let Some(model) = non_empty_string(config.model.clone()).filter(|model| model.contains('/'))
    {
        return Some(model);
    }
    let provider = non_empty_string(config.active_provider.clone())?;
    let model = non_empty_string(config.active_model.clone())?;
    Some(format!("{provider}/{model}"))
}

struct PromptSessionConfig {
    config: TuraSessionConfig,
    keys: std::collections::BTreeSet<String>,
}

impl PromptSessionConfig {
    fn has(&self, key: &str) -> bool {
        self.keys.contains(key)
    }

    fn string(
        &self,
        key: &str,
        getter: impl FnOnce(&TuraSessionConfig) -> Option<String>,
    ) -> Option<String> {
        self.has(key).then(|| getter(&self.config)).flatten()
    }

    fn bool(
        &self,
        key: &str,
        getter: impl FnOnce(&TuraSessionConfig) -> Option<bool>,
    ) -> Option<bool> {
        self.has(key).then(|| getter(&self.config)).flatten()
    }
}

fn load_prompt_session_config(directory: &str) -> Option<PromptSessionConfig> {
    let content = std::fs::read_to_string(crate::session::config::config_path(directory)).ok()?;
    Some(PromptSessionConfig {
        config: load_config(directory),
        keys: config_keys(&content),
    })
}

fn config_keys(content: &str) -> std::collections::BTreeSet<String> {
    content
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                return None;
            }
            line.split_once('=')
                .map(|(key, _)| key.trim().to_string())
                .filter(|key| !key.is_empty())
        })
        .collect()
}

fn explicit_config_model_override(config: &PromptSessionConfig) -> Option<String> {
    if config.has("model") {
        if let Some(model) =
            non_empty_string(config.config.model.clone()).filter(|model| model.contains('/'))
        {
            return Some(model);
        }
    }
    if !(config.has("active_provider") || config.has("active_model")) {
        return None;
    }
    config_model_override(&config.config)
}

pub(super) fn prompt_model_variant(payload: &serde_json::Value) -> Option<String> {
    payload
        .get("variant")
        .or_else(|| payload.get("model_variant"))
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty() && !value.eq_ignore_ascii_case("default"))
        .map(ToString::to_string)
}

fn non_empty_string(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

#[derive(Default)]
struct AgentRuntimeSettings {
    reasoning_effort: Option<String>,
}

fn agent_runtime_settings(agent_id: &str, directory: Option<&str>) -> Option<AgentRuntimeSettings> {
    let mut roots = Vec::new();
    if let Some(directory) = directory {
        roots.push(std::path::PathBuf::from(directory));
    }
    let env_root = tura_agents::store::project_root_from_env_or_cwd();
    if !roots.iter().any(|root| root == &env_root) {
        roots.push(env_root);
    }
    let agent = roots
        .iter()
        .find_map(|root| tura_agents::store::load_agent(root, agent_id))?;
    let provider = agent.config.provider.as_object()?;
    Some(AgentRuntimeSettings {
        reasoning_effort: provider_string(
            provider,
            &[
                "model_reasoning_effort",
                "reasoning_effort",
                "model_variant",
            ],
        ),
    })
}

fn provider_string(
    provider: &serde_json::Map<String, serde_json::Value>,
    keys: &[&str],
) -> Option<String> {
    keys.iter()
        .filter_map(|key| provider.get(*key))
        .filter_map(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty() && !value.eq_ignore_ascii_case("default"))
        .map(ToString::to_string)
        .next()
}

fn prompt_agent_override(payload: &serde_json::Value) -> Option<String> {
    payload
        .get("agent")
        .or_else(|| payload.get("agent_id"))
        .or_else(|| payload.get("agentID"))
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn prompt_agent_for_run(
    payload: &serde_json::Value,
    session_agent: Option<String>,
    session_config: Option<&PromptSessionConfig>,
) -> Option<String> {
    prompt_agent_override(payload)
        .or_else(|| non_empty_string(session_agent))
        .or_else(|| {
            session_config.and_then(|config| {
                config.string("active_agent", |config| config.active_agent.clone())
            })
        })
}

pub(super) fn prompt_model_acceleration(payload: &serde_json::Value) -> Option<bool> {
    payload
        .get("model_acceleration_enabled")
        .or_else(|| payload.get("modelAccelerationEnabled"))
        .or_else(|| payload.get("accelerated"))
        .and_then(|value| value.as_bool())
}

pub(super) fn prompt_command_run_shell(payload: &serde_json::Value) -> Option<String> {
    let value = payload
        .get("command_run_shell")
        .or_else(|| payload.get("commandRunShell"))
        .and_then(|value| value.as_str())?
        .trim();
    match value {
        "bash" => Some("bash".to_string()),
        "zsh" => Some("zsh".to_string()),
        "shll" | "shell_command" => Some("shell_command".to_string()),
        _ => None,
    }
}

fn prompt_source_is_cli(payload: &serde_json::Value) -> bool {
    payload
        .get("source")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .is_some_and(|value| value.eq_ignore_ascii_case("cli"))
}

fn normalize_model_override(value: String) -> Option<String> {
    let trimmed = value.trim();
    let (provider, model) = trimmed.split_once('/')?;
    let provider = provider.trim();
    let model = model.trim();
    if provider.is_empty() || model.is_empty() {
        return None;
    }
    let provider = match provider {
        "openai-api" => "openai",
        "anthropic-api" => "anthropic",
        "antigravity-api" => "antigravity",
        other => other,
    };
    Some(format!("{provider}/{model}"))
}

pub(super) fn final_agent_message(
    session_id: &str,
    before_count: usize,
) -> Option<crate::session::store::Message> {
    session_store()
        .get_messages(session_id)
        .into_iter()
        .skip(before_count)
        .find(|message| {
            message.role == SessionMessageRole::Assistant
                && message
                    .parts
                    .iter()
                    .filter_map(|part| part.text.as_deref().or(part.content.as_deref()))
                    .any(is_meaningful_final_message)
        })
}

fn is_meaningful_final_message(text: &str) -> bool {
    let trimmed = text.trim_start();
    if trimmed.starts_with("Step summary:")
        || trimmed.starts_with("MANO failed while processing this prompt:")
    {
        return false;
    }
    if looks_like_tool_payload(trimmed) {
        return false;
    }

    !strip_runtime_markup(
        trimmed
            .replace("<think>", "")
            .replace("</think>", "")
            .trim(),
    )
    .trim()
    .is_empty()
}

fn strip_runtime_markup(text: &str) -> String {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .filter(|line| !is_runtime_markup_line(line))
        .collect::<Vec<_>>()
        .join("\n")
}

fn is_runtime_markup_line(line: &str) -> bool {
    let lower = line.to_ascii_lowercase();
    if matches!(
        lower.as_str(),
        "<invoke>" | "</invoke>" | "<tool_call>" | "</tool_call>" | "<tool>" | "</tool>"
    ) {
        return true;
    }

    if lower.starts_with('<') && lower.ends_with('>') {
        return true;
    }

    if lower.starts_with("command_run:") && (lower.contains('{') || lower.contains('[')) {
        return true;
    }

    (lower.starts_with("<invoke") && lower.ends_with('>'))
        || (lower.starts_with("</invoke") && lower.ends_with('>'))
        || (lower.starts_with("<tool_call") && lower.ends_with('>'))
        || (lower.starts_with("</tool_call") && lower.ends_with('>'))
}

pub(super) fn frontend_safe_reply_message(text: &str) -> String {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    if let Some(reply_message) = extract_reply_message_from_json(trimmed) {
        return reply_message;
    }
    if looks_like_tool_payload(trimmed) {
        return String::new();
    }
    trimmed.to_string()
}

fn extract_reply_message_from_json(text: &str) -> Option<String> {
    serde_json::from_str::<serde_json::Value>(text)
        .ok()
        .and_then(|value| find_reply_message(&value))
}

fn find_reply_message(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::Object(object) => {
            if let Some(message) = object
                .get("reply_message")
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
            {
                return Some(message.to_string());
            }
            object.values().find_map(find_reply_message)
        }
        serde_json::Value::Array(items) => items.iter().find_map(find_reply_message),
        _ => None,
    }
}

fn looks_like_tool_payload(text: &str) -> bool {
    let trimmed = text.trim_start();
    if !trimmed.starts_with('{') {
        return false;
    }

    if let Ok(value) = serde_json::from_str::<serde_json::Value>(trimmed) {
        return json_looks_like_tool_payload(&value);
    }

    trimmed.contains("\"reply_message\"")
        || trimmed.contains("\"new_learning\"")
        || trimmed.contains("\"tool_calls\"")
        || trimmed.contains("\"input\"")
        || trimmed.contains("\"last_tool_call_status\"")
        || trimmed.contains("\"last_tool_call_summary\"")
        || trimmed.contains("\"step_summary\"")
}

fn json_looks_like_tool_payload(value: &serde_json::Value) -> bool {
    match value {
        serde_json::Value::Object(object) => {
            let has_reporting_fields = object.contains_key("last_tool_call_status")
                || object.contains_key("last_tool_call_summary")
                || object.contains_key("step_summary");
            let has_tool_shape = object.contains_key("requests")
                || object.contains_key("reply_message")
                || object.contains_key("new_learning")
                || object.contains_key("tool_calls")
                || object.contains_key("input")
                || object.contains_key("command_code")
                || object.contains_key("environment");

            (has_reporting_fields && has_tool_shape)
                || object.contains_key("tool_calls")
                || object.values().any(json_looks_like_tool_payload)
        }
        serde_json::Value::Array(items) => items.iter().any(json_looks_like_tool_payload),
        _ => false,
    }
}

fn add_agent_fallback_message(session_id: &str, content: String) {
    if let Some(message) =
        session_store().add_message(session_id, SessionMessageRole::Assistant, content)
    {
        session_store().push_event(GlobalEvent::MessageUpdated {
            properties: MessageUpdatedProperties {
                session_id: session_id.to_string(),
                info: api_message_from_store(message),
            },
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn prompt_config(active_agent: &str) -> PromptSessionConfig {
        PromptSessionConfig {
            config: TuraSessionConfig {
                active_agent: Some(active_agent.to_string()),
                ..TuraSessionConfig::default()
            },
            keys: ["active_agent".to_string()].into_iter().collect(),
        }
    }

    #[test]
    fn prompt_agent_prefers_prompt_then_session_then_workspace_config() {
        let config = prompt_config("workspace-agent");

        assert_eq!(
            prompt_agent_for_run(
                &serde_json::json!({"agent": "prompt-agent"}),
                Some("session-agent".to_string()),
                Some(&config),
            )
            .as_deref(),
            Some("prompt-agent")
        );
        assert_eq!(
            prompt_agent_for_run(
                &serde_json::json!({}),
                Some("session-agent".to_string()),
                Some(&config),
            )
            .as_deref(),
            Some("session-agent")
        );
        assert_eq!(
            prompt_agent_for_run(&serde_json::json!({}), None, Some(&config)).as_deref(),
            Some("workspace-agent")
        );
    }

    #[test]
    fn dynamic_agent_runtime_settings_ignore_agent_priority_fields() {
        let temp = tempfile::tempdir().expect("temp dir");
        let mut config = tura_agents::store::default_agent_config(temp.path(), "runtime-agent")
            .expect("default agent config");
        config.provider["model_reasoning_effort"] = serde_json::json!("high");
        config.provider["model_acceleration_enabled"] = serde_json::json!(true);
        config.provider["service_tier"] = serde_json::json!("priority");
        tura_agents::store::save_dynamic_agent(temp.path(), &config, Some("runtime agent"))
            .expect("save dynamic agent");

        let settings = agent_runtime_settings("runtime-agent", temp.path().to_str())
            .expect("agent runtime settings");

        assert_eq!(settings.reasoning_effort.as_deref(), Some("high"));
    }

    #[test]
    fn runtime_model_override_uses_gateway_config_before_stale_session_model() {
        let config = PromptSessionConfig {
            config: TuraSessionConfig {
                model: Some("openrouter/qwen/qwen3.7-max".to_string()),
                active_provider: Some("openrouter".to_string()),
                active_model: Some("qwen/qwen3.7-max".to_string()),
                ..TuraSessionConfig::default()
            },
            keys: [
                "model".to_string(),
                "active_provider".to_string(),
                "active_model".to_string(),
            ]
            .into_iter()
            .collect(),
        };

        let runtime_model = prompt_runtime_model_override(
            &serde_json::json!({}),
            Some("codex/gpt-5.5".to_string()),
            Some(&config),
        );

        assert_eq!(
            runtime_model.as_deref(),
            Some("openrouter/qwen/qwen3.7-max")
        );
        assert_eq!(runtime_model, config_model_override(&config.config));
    }

    #[test]
    fn runtime_model_override_matches_frontend_posted_active_model() {
        let config = PromptSessionConfig {
            config: TuraSessionConfig {
                model: Some("openrouter/qwen/qwen3.7-max".to_string()),
                active_provider: Some("openrouter".to_string()),
                active_model: Some("qwen/qwen3.7-max".to_string()),
                ..TuraSessionConfig::default()
            },
            keys: [
                "model".to_string(),
                "active_provider".to_string(),
                "active_model".to_string(),
            ]
            .into_iter()
            .collect(),
        };

        assert_eq!(
            prompt_runtime_model_override(
                &serde_json::json!({ "model": "anthropic/claude-opus-4.5" }),
                Some("codex/gpt-5.5".to_string()),
                Some(&config),
            )
            .as_deref(),
            Some("anthropic/claude-opus-4.5")
        );
    }

    #[test]
    fn prompt_source_is_cli_matches_cli_source_only() {
        assert!(super::prompt_source_is_cli(
            &serde_json::json!({"source": "cli"})
        ));
        assert!(super::prompt_source_is_cli(
            &serde_json::json!({"source": " CLI "})
        ));
        assert!(!super::prompt_source_is_cli(
            &serde_json::json!({"source": "tui"})
        ));
        assert!(!super::prompt_source_is_cli(&serde_json::json!({})));
    }
}
