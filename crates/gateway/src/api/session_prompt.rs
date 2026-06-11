use super::*;

pub async fn prompt_async(
    Path(session_id): Path<String>,
    Json(payload): Json<serde_json::Value>,
) -> impl IntoResponse {
    session_store().clear_cancelled(&session_id);
    let content = prompt_text(&payload).unwrap_or_else(|| "Prompt submitted".to_string());
    let session = session_store().get_session(&session_id);
    if session
        .as_ref()
        .is_some_and(|session| matches!(session.status, SessionStatus::Busy))
    {
        let _ = session_store().add_message_with_ids(
            &session_id,
            SessionMessageRole::User,
            content.clone(),
            prompt_message_id(&payload),
            first_prompt_part_id(&payload),
            Some(serde_json::json!({
                "kind": "user_new_command",
            })),
        );
        session_store().append_user_command(&session_id, content);
        return StatusCode::NO_CONTENT;
    }
    let _ = session_store().add_message_with_ids(
        &session_id,
        SessionMessageRole::User,
        content,
        prompt_message_id(&payload),
        first_prompt_part_id(&payload),
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
    let payload_for_task = payload;
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
    for run in session_store().claim_due_task_runs(chrono::Utc::now()) {
        let prompt = scheduler_prompt_payload(&run.task_summary, run.start_condition);
        let content = prompt_text(&prompt).unwrap_or_else(|| run.task_summary.clone());
        let initial_count = session_store().get_messages(&run.session_id).len();
        let _ = session_store().add_message_with_metadata(
            &run.session_id,
            SessionMessageRole::User,
            content,
            Some(serde_json::json!({
                "kind": "task_scheduler",
                "start_condition": run.start_condition,
            })),
        );
        session_store().set_todos(
            &run.session_id,
            vec![serde_json::json!({
                "id": format!("{}:scheduled-task", run.session_id),
                "content": run.task_summary,
                "status": "in_progress",
                "priority": "medium",
            })],
        );
        watch_direct_mano_messages(run.session_id.clone(), initial_count);
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
    }
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
        .find(|part| part.get("type").and_then(|value| value.as_str()) == Some("text"))?
        .get("id")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
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

    let agent = prompt_agent_override(&payload)
        .or_else(|| session.as_ref().and_then(|session| session.agent.clone()));
    let runtime_context = prompt_runtime_context(&payload);
    let force_planning = session
        .as_ref()
        .map(|session| session.force_planning)
        .unwrap_or(false);
    let model_override = prompt_model_override(&payload)
        .or_else(|| session.as_ref().and_then(|session| session.model.clone()))
        .and_then(normalize_model_override);
    let agent_runtime_settings = agent
        .as_deref()
        .and_then(agent_runtime_settings)
        .unwrap_or_default();
    let reasoning_effort = prompt_model_variant(&payload)
        .or(agent_runtime_settings.reasoning_effort)
        .or_else(|| {
            session
                .as_ref()
                .and_then(|session| session.model_variant.clone())
                .filter(|value| !value.trim().is_empty())
        });
    let acceleration_enabled = prompt_model_acceleration(&payload)
        .or(agent_runtime_settings.acceleration_enabled)
        .or_else(|| {
            session
                .as_ref()
                .map(|session| session.model_acceleration_enabled)
        })
        .unwrap_or(false);
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
    let command_run_stall_guard = directory
        .as_deref()
        .map(load_config)
        .map(|config| config.command_run_stall_guard())
        .unwrap_or_else(|| TuraSessionConfig::default().command_run_stall_guard());
    let language = directory
        .as_deref()
        .map(load_config)
        .and_then(|config| config.language)
        .or_else(|| global_store().get_config().language)
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());

    // Worker env contract: router injects these values into the runtime worker.
    let mut worker_env: std::collections::HashMap<String, String> =
        std::collections::HashMap::new();
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
    if let Some(language) = language {
        worker_env.insert("TURA_SESSION_LANGUAGE".to_string(), language);
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

    let result = flush_session_to_session_db(&session_id)
        .and_then(|_| forward_run_agent_to_router(&turn_id, &session_id, &body));

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
            } else {
                add_agent_fallback_message(&session_id, user_facing_completion_fallback(&content));
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
        .map_err(|error| error.to_string())?;
    if value
        .get("ok")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(true)
    {
        Ok(())
    } else {
        Err(value
            .get("error")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("gateway worker returned failure")
            .to_string())
    }
}

fn flush_session_to_session_db(session_id: &str) -> Result<(), String> {
    session_store()
        .persist_session_ack(session_id)
        .map_err(|error| format!("session_db ACK failed before enqueue: {error}"))
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

pub(super) fn prompt_model_variant(payload: &serde_json::Value) -> Option<String> {
    payload
        .get("variant")
        .or_else(|| payload.get("model_variant"))
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty() && !value.eq_ignore_ascii_case("default"))
        .map(ToString::to_string)
}

#[derive(Default)]
struct AgentRuntimeSettings {
    reasoning_effort: Option<String>,
    acceleration_enabled: Option<bool>,
}

fn agent_runtime_settings(agent_id: &str) -> Option<AgentRuntimeSettings> {
    let root = repo_root_for_router()?;
    let agent = tura_agents::store::load_agent(&root, agent_id)?;
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
        acceleration_enabled: provider_bool(provider, "model_acceleration_enabled").or_else(|| {
            provider_string(provider, &["service_tier"])
                .map(|value| value.eq_ignore_ascii_case("priority"))
        }),
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

fn provider_bool(provider: &serde_json::Map<String, serde_json::Value>, key: &str) -> Option<bool> {
    provider.get(key).and_then(serde_json::Value::as_bool)
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

pub(super) fn prompt_model_acceleration(payload: &serde_json::Value) -> Option<bool> {
    payload
        .get("model_acceleration_enabled")
        .or_else(|| payload.get("modelAccelerationEnabled"))
        .or_else(|| payload.get("accelerated"))
        .and_then(|value| value.as_bool())
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

pub(super) fn user_facing_completion_fallback(prompt: &str) -> String {
    if let Some(exact) = requested_exact_reply(prompt) {
        return exact;
    }
    let summary = prompt
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or("The request completed.");
    format!("Done: {summary}")
}

fn requested_exact_reply(text: &str) -> Option<String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }
    let lower = trimmed.to_ascii_lowercase();
    for marker in [
        "reply with exactly",
        "reply exactly",
        "respond with exactly",
        "respond exactly",
        "只回复这一行，不要解释：",
        "只回复这一行，不要解释:",
        "只回复：",
        "只回复:",
    ] {
        if let Some(index) = lower.find(marker) {
            let source = &trimmed[index + marker.len()..];
            let candidate = source
                .trim_start_matches(|ch: char| {
                    ch.is_whitespace() || matches!(ch, ':' | '：' | '"' | '\'' | '`')
                })
                .split(" and no extra")
                .next()
                .unwrap_or(source)
                .trim()
                .trim_matches(|ch: char| matches!(ch, '"' | '\'' | '`' | '.' | '。'))
                .trim();
            if !candidate.is_empty() {
                return Some(candidate.to_string());
            }
        }
    }
    None
}
