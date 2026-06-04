use chrono::Utc;
use tracing::{error, info};

use crate::agent_router::{activate_agents_by_session_type, initialize_agent_state_machine};
use crate::context::{
    accumulate_message, build_messages_from_session, user_input_content_matches,
    user_input_content_value, ContextualUserFragment, WorkspaceSnapshot,
};
use crate::manas::{process_manas_internal, ManasInput};
use crate::mano::gateway_session::{load_persisted_gateway_session, persist_gateway_session};
use crate::mano::session_bootstrap::create_session_with_topic;
use crate::mano::{ManoOverrides, ManoProcessResult};
use crate::state_machine::agent_management::{AgentCapabilityItem, AgentManagement};
use crate::state_machine::session_management::{SessionInput, SessionManagement};
use std::path::PathBuf;

pub struct OrchestrationConfig {
    pub redis_url: String,
    pub session_directory: Option<PathBuf>,
}

impl Default for OrchestrationConfig {
    fn default() -> Self {
        Self {
            redis_url: "redis://localhost:6379".to_string(),
            session_directory: None,
        }
    }
}

pub fn orchestrate(input: SessionInput) -> Result<ManoProcessResult, String> {
    orchestrate_with_config(input, OrchestrationConfig::default())
}

pub fn orchestrate_for_session(
    input: SessionInput,
    session_id: String,
) -> Result<ManoProcessResult, String> {
    orchestrate_with_config_and_session(input, OrchestrationConfig::default(), Some(session_id))
}

pub fn orchestrate_for_session_in_directory(
    input: SessionInput,
    session_id: String,
    session_directory: PathBuf,
) -> Result<ManoProcessResult, String> {
    orchestrate_with_config_and_session(
        input,
        OrchestrationConfig {
            session_directory: Some(session_directory),
            ..OrchestrationConfig::default()
        },
        Some(session_id),
    )
}

pub fn orchestrate_with_config(
    input: SessionInput,
    config: OrchestrationConfig,
) -> Result<ManoProcessResult, String> {
    orchestrate_with_config_and_session(input, config, None)
}

fn orchestrate_with_config_and_session(
    input: SessionInput,
    config: OrchestrationConfig,
    gateway_session_id: Option<String>,
) -> Result<ManoProcessResult, String> {
    let now = Utc::now();

    info!(
        user_input = %input.user_input,
        "starting orchestration"
    );

    let mut session =
        bootstrap_orchestration_session(input.clone(), &config, gateway_session_id.clone(), now)?;

    info!(
        session_id = %session.session_id,
        session_topic = %session.session_topic,
        "session created"
    );

    let mut agents = match activate_agents_by_session_type(&session) {
        Ok(a) => a,
        Err(e) => {
            error!(error = %e, "failed to activate agents");
            return Err(format!("failed to activate agents: {}", e));
        }
    };
    apply_planning_capability_override(&mut agents, &session);
    session.planning_enabled = agents.first().is_some_and(agent_has_planning_capability);

    if let Err(e) = initialize_agent_state_machine(&mut agents, &session) {
        error!(error = %e, "failed to initialize agent state machine");
        return Err(format!("failed to initialize agent state machine: {}", e));
    }

    info!(
        session_id = %session.session_id,
        agent_count = agents.len(),
        "agents activated"
    );

    let initial_messages = initial_messages_for_session(&mut session)?;
    persist_gateway_session(&session)
        .map_err(|err| format!("failed to persist initial gateway session: {err}"))?;

    let mut session_clone = session.clone();

    let manas_input = ManasInput {
        agents: &mut agents,
        session: &mut session_clone,
        initial_messages,
        redis_url: &config.redis_url,
    };

    let manas_result =
        match process_manas_internal(manas_input, crate::manas::ManasOverrides::default()) {
            Ok(r) => r,
            Err(e) => {
                error!(error = %e, "manas processing failed");
                return Err(format!("manas processing failed: {}", e));
            }
        };

    info!(
        session_id = %manas_result.session.session_id,
        final_turn = manas_result.session.session_current_turn,
        final_state = ?manas_result.session.state,
        "orchestration completed"
    );

    Ok(ManoProcessResult {
        session: manas_result.session,
        agents: manas_result.agents,
    })
}

fn apply_planning_capability_override(agents: &mut [AgentManagement], session: &SessionManagement) {
    let Some(enabled) = session.input.planning_mode_override else {
        return;
    };
    let Some(agent) = agents.first_mut() else {
        return;
    };
    if enabled {
        if !agent_has_planning_capability(agent) {
            let capability_directory = agent
                .agent_capabilities
                .iter()
                .find(|capability| capability.capability_name == "command_run")
                .or_else(|| agent.agent_capabilities.first())
                .map(|capability| capability.capability_directory.clone())
                .unwrap_or_else(|| {
                    session
                        .session_directory
                        .join("crates")
                        .join("tools")
                        .join("src")
                });
            agent.agent_capabilities.push(AgentCapabilityItem {
                capability_name: "planning".to_string(),
                capability_directory,
            });
        }
    } else {
        agent
            .agent_capabilities
            .retain(|capability| capability.capability_name != "planning");
    }
}

fn agent_has_planning_capability(agent: &AgentManagement) -> bool {
    agent
        .agent_capabilities
        .iter()
        .any(|capability| capability.capability_name == "planning")
}

fn initial_messages_for_session(
    session: &mut SessionManagement,
) -> Result<Vec<serde_json::Value>, String> {
    let permissions_message = serde_json::json!({
        "role": "developer",
        "content": permissions_instructions(),
    });
    if session.session_current_turn == 0 && !session_has_initial_user_message(session) {
        let snapshot_message = serde_json::json!({
            "role": "user",
            "content": workspace_snapshot_message(&session.session_directory),
        });
        let environment_message = serde_json::json!({
            "role": "user",
            "content": environment_context_message(&session.session_directory),
        });
        let user_message = serde_json::json!({
            "role": "user",
            "content": user_input_content_value(&session.input.user_input),
        });

        for message in [
            &permissions_message,
            &snapshot_message,
            &environment_message,
            &user_message,
        ] {
            let role = message
                .get("role")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("system");
            let content = message
                .get("content")
                .cloned()
                .unwrap_or_else(|| serde_json::Value::String(String::new()));
            accumulate_message(session, role, content)?;
        }

        return Ok(vec![
            permissions_message,
            snapshot_message,
            environment_message,
            user_message,
        ]);
    }

    if !session_has_initial_user_message(session) {
        accumulate_message(
            session,
            "user",
            user_input_content_value(&session.input.user_input),
        )?;
    }

    let mut messages = vec![permissions_message];
    messages.extend(build_messages_from_session(session));
    Ok(messages)
}

fn permissions_instructions() -> &'static str {
    "<permissions instructions>\nFilesystem sandboxing defines which files can be read or written. `sandbox_mode` is `danger-full-access`: No filesystem sandboxing - all commands are permitted. Network access is enabled.\nApproval policy is currently never. Do not provide the `sandbox_permissions` for any reason, commands will be rejected.\n</permissions instructions>"
}

fn environment_context_message(cwd: &std::path::Path) -> String {
    format!(
        "<environment_context>\n  <cwd>{}</cwd>\n  <shell>{}</shell>\n  <current_date>{}</current_date>\n  <timezone>{}</timezone>\n  <system_language>{}</system_language>\n</environment_context>",
        cwd.display(),
        context_shell_name(),
        chrono::Local::now().format("%Y-%m-%d"),
        std::env::var("TZ").unwrap_or_else(|_| "Europe/Paris".to_string()),
        session_language()
    )
}

fn session_language() -> String {
    std::env::var("TURA_SESSION_LANGUAGE")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "en".to_string())
}

fn context_shell_name() -> &'static str {
    match std::env::var("TURA_COMMAND_RUN_SHELL")
        .ok()
        .map(|value| value.trim().to_ascii_lowercase())
        .as_deref()
    {
        Some("bash") => "bash",
        Some("shell") | Some("shell_command") | Some("shll") | Some("shall") => {
            if cfg!(windows) {
                "powershell"
            } else {
                "bash"
            }
        }
        _ if cfg!(windows) => "powershell",
        _ => "bash",
    }
}

fn workspace_snapshot_message(cwd: &std::path::Path) -> String {
    WorkspaceSnapshot::from_cwd(cwd)
        .map(|snapshot| snapshot.render())
        .unwrap_or_else(|| "<WORKSPACE_SNAPSHOT>\n\n</WORKSPACE_SNAPSHOT>".to_string())
}

fn session_has_initial_user_message(session: &SessionManagement) -> bool {
    let raw_input = &session.input.user_input;
    session.session_log.iter().any(|entry| {
        serde_json::from_str::<serde_json::Value>(entry)
            .ok()
            .is_some_and(|value| {
                value.get("role").and_then(serde_json::Value::as_str) == Some("user")
                    && value
                        .get("content")
                        .is_some_and(|content| user_input_content_matches(content, raw_input))
            })
    })
}

fn bootstrap_orchestration_session(
    input: SessionInput,
    config: &OrchestrationConfig,
    gateway_session_id: Option<String>,
    now: chrono::DateTime<Utc>,
) -> Result<crate::state_machine::session_management::SessionManagement, String> {
    if let Some(session_id) = gateway_session_id {
        if let Some(mut persisted) = config
            .session_directory
            .as_ref()
            .and_then(|directory| load_persisted_gateway_session(directory, &session_id))
        {
            persisted.prepare_for_new_user_turn(input, now);
            if let Some(directory) = config.session_directory.clone() {
                persisted.session_directory = directory;
            }
            persisted.session_id = session_id;
            return Ok(persisted);
        }

        let mut session = create_session_with_topic(input, config.session_directory.clone())
            .map_err(|e| {
                error!(error = %e, "failed to create session");
                format!("failed to create session: {}", e)
            })?;
        session.session_id = session_id;
        return Ok(session);
    }

    create_session_with_topic(input, config.session_directory.clone()).map_err(|e| {
        error!(error = %e, "failed to create session");
        format!("failed to create session: {}", e)
    })
}

pub fn process_from_user_internal(
    input: SessionInput,
    overrides: ManoOverrides,
) -> Result<ManoProcessResult, String> {
    let mut session = match overrides.session_factory {
        Some(session_factory) => session_factory(input)?,
        None => create_session_with_topic(input, None)?,
    };

    let agents = match overrides.manas_entry {
        Some(manas_entry) => manas_entry(&session)?,
        None => {
            let mut agts = activate_agents_by_session_type(&session)?;
            apply_planning_capability_override(&mut agts, &session);
            session.planning_enabled = agts.first().is_some_and(agent_has_planning_capability);
            initialize_agent_state_machine(&mut agts, &session)?;
            agts
        }
    };

    Ok(ManoProcessResult { session, agents })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::build_messages_from_session;
    use crate::state_machine::session_management::{SessionInput, SessionManagement};
    use chrono::Utc;
    use std::fs;

    #[test]
    fn planning_override_removes_planning_from_started_agent_state() {
        let input = SessionInput {
            user_input: "inspect".to_string(),
            file_input: Vec::new(),
            agent: Some("coding_agent_planning".to_string()),
            runtime_context: None,
            planning_mode_override: Some(false),
        };
        let mut session = SessionManagement::new(
            "planning-off".to_string(),
            "planning-off".to_string(),
            std::env::current_dir().expect("current dir should resolve"),
            false,
            "coding".to_string(),
            input,
            "inspect".to_string(),
            Utc::now(),
        );
        let mut agents = activate_agents_by_session_type(&session).expect("agents should activate");

        apply_planning_capability_override(&mut agents, &session);
        session.planning_enabled = agents.first().is_some_and(agent_has_planning_capability);

        assert!(!session.planning_enabled);
        assert!(!agent_has_planning_capability(&agents[0]));
    }

    #[test]
    fn planning_override_adds_planning_to_started_agent_state() {
        let input = SessionInput {
            user_input: "inspect".to_string(),
            file_input: Vec::new(),
            agent: Some("general".to_string()),
            runtime_context: None,
            planning_mode_override: Some(true),
        };
        let mut session = SessionManagement::new(
            "planning-on".to_string(),
            "planning-on".to_string(),
            std::env::current_dir().expect("current dir should resolve"),
            false,
            "general".to_string(),
            input,
            "inspect".to_string(),
            Utc::now(),
        );
        let mut agents = activate_agents_by_session_type(&session).expect("agents should activate");

        apply_planning_capability_override(&mut agents, &session);
        session.planning_enabled = agents.first().is_some_and(agent_has_planning_capability);

        assert!(session.planning_enabled);
        assert!(agent_has_planning_capability(&agents[0]));
    }

    #[test]
    fn gateway_bootstrap_loads_session_log_session_before_creating_new_session() {
        let root = std::env::temp_dir().join(format!(
            "tura-gateway-bootstrap-{}",
            Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ));
        let session_id = format!(
            "sess-existing-{}",
            Utc::now().timestamp_nanos_opt().unwrap_or_default()
        );

        let old_input = SessionInput {
            user_input: "old prompt".to_string(),
            file_input: Vec::new(),
            agent: None,
            runtime_context: None,
            planning_mode_override: None,
        };
        let mut persisted = SessionManagement::new(
            session_id.clone(),
            "existing".to_string(),
            root.clone(),
            false,
            "coding".to_string(),
            old_input,
            "old prompt".to_string(),
            Utc::now(),
        );
        persisted.push_log("persisted-session-loaded", Utc::now());
        persist_gateway_session(&persisted).expect("persist session_log session");

        let next_input = SessionInput {
            user_input: "fix bug in the existing workspace".to_string(),
            file_input: Vec::new(),
            agent: None,
            runtime_context: None,
            planning_mode_override: None,
        };
        let session = bootstrap_orchestration_session(
            next_input.clone(),
            &OrchestrationConfig {
                redis_url: "redis://localhost:6379".to_string(),
                session_directory: Some(root.clone()),
            },
            Some(session_id.clone()),
            Utc::now(),
        )
        .expect("persisted gateway session should load");

        assert_eq!(session.session_id, session_id);
        assert_eq!(session.session_directory, root);
        assert_eq!(session.input, next_input);
        assert!(session
            .session_log
            .iter()
            .any(|entry| entry == "persisted-session-loaded"));

        let _ = std::fs::remove_dir_all(session.session_directory);
    }

    #[test]
    fn gateway_persistence_writes_session_log_records() {
        let root = std::env::temp_dir().join(format!(
            "tura-session-log-records-{}",
            Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ));
        fs::create_dir_all(&root).expect("test workspace should be created");
        let session_id = format!(
            "persist-records-{}",
            Utc::now().timestamp_nanos_opt().unwrap_or_default()
        );
        let mut session = SessionManagement::new(
            session_id.clone(),
            "persist records".to_string(),
            root,
            false,
            "coding".to_string(),
            SessionInput {
                user_input: "persist records".to_string(),
                file_input: Vec::new(),
                agent: None,
                runtime_context: None,
                planning_mode_override: None,
            },
            "persist records".to_string(),
            Utc::now(),
        );
        accumulate_message(
            &mut session,
            "user",
            serde_json::Value::String("persist records".to_string()),
        )
        .expect("user message should accumulate");
        session.push_log(
            serde_json::json!({
                "type": "tool_result",
                "tool_name": "command_run",
                "input": {"commands": [{"command_type": "shell_command", "command_line": "pwd"}]},
                "output": {"results": [{"command_type": "shell_command", "success": true}]},
                "success": true,
                "timestamp": Utc::now().to_rfc3339(),
            })
            .to_string(),
            Utc::now(),
        );
        session.push_log(
            serde_json::json!({
                "type": "runtime_usage",
                "usage": {"input_tokens": 1, "output_tokens": 2},
                "timestamp": Utc::now().to_rfc3339(),
            })
            .to_string(),
            Utc::now(),
        );

        persist_gateway_session(&session).expect("session should persist");

        let (_page, records) = crate::session_log_client::SessionLogClient::discover()
            .expect("session_log client should be available")
            .list_session_records(session_id, 0, 50)
            .expect("records should load");
        assert_eq!(records.len(), session.session_log.len());
        assert!(records.iter().any(|record| {
            record.role == "tool"
                && record.record["type"] == "tool_result"
                && record.record["tool_name"] == "command_run"
        }));
        assert!(records
            .iter()
            .any(|record| record.role == "runtime" && record.record["type"] == "runtime_usage"));
    }

    #[test]
    fn resumed_session_initial_messages_include_prior_image_tool_context_and_new_user_turn() {
        let root = std::env::temp_dir().join(format!(
            "tura-resume-image-context-{}",
            Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ));
        fs::create_dir_all(&root).expect("test workspace should be created");
        let old_input = SessionInput {
            user_input: "inspect image".to_string(),
            file_input: Vec::new(),
            agent: None,
            runtime_context: None,
            planning_mode_override: None,
        };
        let mut session = SessionManagement::new(
            "resume-image-session".to_string(),
            "resume-image".to_string(),
            root.clone(),
            false,
            "coding".to_string(),
            old_input,
            "inspect image".to_string(),
            Utc::now(),
        );
        session.session_current_turn = 2;
        session.push_log(
            serde_json::json!({
                "type": "tool_result",
                "tool_name": "command_run",
                "context_messages": [
                    {
                        "type": "function_call",
                        "name": "command_run",
                        "call_id": "call_image",
                        "arguments": "{\"commands\":[]}",
                        "status": "completed"
                    },
                    {
                        "type": "function_call_output",
                        "call_id": "call_image",
                        "output": [
                            {"type": "input_text", "text": "image inspected"},
                            {"type": "input_image", "image_url": "data:image/png;base64,AAA"}
                        ]
                    }
                ]
            })
            .to_string(),
            Utc::now(),
        );
        session.prepare_for_new_user_turn(
            SessionInput {
                user_input: "what was in the previous image?".to_string(),
                file_input: Vec::new(),
                agent: None,
                runtime_context: None,
                planning_mode_override: None,
            },
            Utc::now(),
        );

        let messages =
            initial_messages_for_session(&mut session).expect("resume messages should build");
        let serialized = serde_json::to_string(&messages).expect("messages json");

        assert!(serialized.contains("data:image/png;base64,AAA"));
        assert!(serialized.contains("what was in the previous image?"));
        assert!(messages.iter().any(|message| {
            message.get("role").and_then(serde_json::Value::as_str) == Some("developer")
        }));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn initial_user_input_media_markers_become_input_images() {
        let root = std::env::temp_dir().join(format!(
            "tura-inline-input-image-{}",
            Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ));
        fs::create_dir_all(&root).expect("test workspace should be created");
        let input = SessionInput {
            user_input: "先看第一处\n[Image 1: screen.png]\n[MEDIA:data:image/png;base64,AAA:MEDIA]\n再看第二处"
                .to_string(),
            file_input: Vec::new(),
            agent: None,
            runtime_context: None,
                planning_mode_override: None,
        };
        let mut session = SessionManagement::new(
            "inline-image-session".to_string(),
            "inline image".to_string(),
            root.clone(),
            false,
            "coding".to_string(),
            input,
            "inline image".to_string(),
            Utc::now(),
        );

        let messages =
            initial_messages_for_session(&mut session).expect("initial messages should build");
        let user_message = messages
            .iter()
            .rev()
            .find(|message| message.get("role").and_then(serde_json::Value::as_str) == Some("user"))
            .expect("initial user message should exist");
        let content = user_message["content"]
            .as_array()
            .expect("media input should become content array");

        assert!(content.iter().any(|part| {
            part.get("type").and_then(serde_json::Value::as_str) == Some("input_image")
                && part.get("image_url").and_then(serde_json::Value::as_str)
                    == Some("data:image/png;base64,AAA")
        }));
        assert!(content.iter().any(|part| {
            part.get("type").and_then(serde_json::Value::as_str) == Some("input_text")
                && part
                    .get("text")
                    .and_then(serde_json::Value::as_str)
                    .is_some_and(|text| text.contains("screen.png"))
        }));
        assert!(content.iter().any(|part| {
            part.get("type").and_then(serde_json::Value::as_str) == Some("input_text")
                && part
                    .get("text")
                    .and_then(serde_json::Value::as_str)
                    .is_some_and(|text| text.contains("再看第二处"))
        }));
        assert!(session_has_initial_user_message(&session));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn initial_messages_persist_workspace_file_snapshot_for_cache_reuse() {
        let root = std::env::temp_dir().join(format!(
            "tura-initial-snapshot-{}",
            Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ));
        fs::create_dir_all(root.join("src")).expect("test workspace should be created");
        fs::write(root.join("src").join("lib.rs"), "fn main() {}\n").expect("fixture should write");
        let input = SessionInput {
            user_input: "inspect this workspace".to_string(),
            file_input: Vec::new(),
            agent: None,
            runtime_context: None,
            planning_mode_override: None,
        };
        let mut session = SessionManagement::new(
            "snapshot-session".to_string(),
            "snapshot".to_string(),
            root.clone(),
            false,
            "coding".to_string(),
            input,
            "inspect this workspace".to_string(),
            Utc::now(),
        );

        let initial =
            initial_messages_for_session(&mut session).expect("initial messages should build");
        let replayed = build_messages_from_session(&session);

        let initial_snapshot = initial
            .iter()
            .find(|message| {
                message["content"]
                    .as_str()
                    .is_some_and(|content| content.contains("<WORKSPACE_SNAPSHOT>"))
            })
            .expect("initial messages should include workspace snapshot");
        assert!(initial_snapshot["content"]
            .as_str()
            .expect("snapshot content should be text")
            .contains("src/lib.rs"));
        assert!(replayed.iter().any(|message| message == initial_snapshot));

        let _ = fs::remove_dir_all(root);
    }
}
