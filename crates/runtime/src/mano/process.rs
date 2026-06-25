use chrono::Utc;
use tracing::{error, info};

use crate::agent_router::{activate_agents_by_session_type, initialize_agent_state_machine};
use crate::checkpoint::session_snapshot::persist_session_snapshot;
use crate::manas::{process_manas_internal, ManasInput};
use crate::mano::{ManoOverrides, ManoProcessResult};
use crate::session_bootstrap::{
    bootstrap_orchestration_session, create_session_with_topic, initial_messages_for_session,
};
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

    let mut session = bootstrap_orchestration_session(
        input,
        config.session_directory.clone(),
        gateway_session_id,
        now,
    )
    .map_err(|e| {
        error!(error = %e, "failed to bootstrap session");
        format!("failed to bootstrap session: {e}")
    })?;

    info!(
        session_id = %session.session_id,
        task_type = ?session.task_type,
        "session created"
    );

    let mut agents = match activate_agents_by_session_type(&session) {
        Ok(a) => a,
        Err(e) => {
            error!(error = %e, "failed to activate agents");
            return Err(format!("failed to activate agents: {e}"));
        }
    };
    apply_planning_capability_override(&mut agents, &session);
    session.planning_enabled = agents.first().is_some_and(agent_has_planning_capability);

    if let Err(e) = initialize_agent_state_machine(&mut agents, &session) {
        error!(error = %e, "failed to initialize agent state machine");
        return Err(format!("failed to initialize agent state machine: {e}"));
    }

    info!(
        session_id = %session.session_id,
        agent_count = agents.len(),
        "agents activated"
    );

    let initial_messages = initial_messages_for_session(&mut session)?;
    persist_session_snapshot(&session)
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
                return Err(format!("manas processing failed: {e}"));
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
        final_error: manas_result.final_error,
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

    Ok(ManoProcessResult {
        session,
        agents,
        final_error: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::{build_messages_from_session, USER_AGENT_CONTEXT_ROLE};
    use crate::state_machine::session_management::{SessionInput, SessionManagement};
    use chrono::Utc;
    use std::fs;

    #[test]
    fn planning_override_removes_planning_from_started_agent_state() {
        let input = SessionInput {
            user_input: "inspect".to_string(),
            file_input: Vec::new(),
            agent: Some("thoughtful".to_string()),
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
        assert!(!messages.iter().any(|message| {
            message.get("role").and_then(serde_json::Value::as_str) == Some("developer")
        }));
        assert!(messages.iter().any(|message| {
            message.get("role").and_then(serde_json::Value::as_str) == Some("user")
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
            user_input: "[Image 1: screen.png]\n[MEDIA:data:image/png;base64,AAA:MEDIA]"
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
                    .is_some()
        }));

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
        assert_eq!(initial_snapshot["role"], USER_AGENT_CONTEXT_ROLE);
        let replayed_snapshot = replayed
            .iter()
            .find(|message| {
                message["content"]
                    .as_str()
                    .is_some_and(|content| content.contains("<WORKSPACE_SNAPSHOT>"))
            })
            .expect("replayed context should include workspace snapshot");
        assert_eq!(replayed_snapshot["role"], "user");
        assert_eq!(replayed_snapshot["content"], initial_snapshot["content"]);

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn initial_runtime_context_uses_user_agent_storage_and_user_replay() {
        let root = std::env::temp_dir().join(format!(
            "tura-runtime-context-tag-{}",
            Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ));
        fs::create_dir_all(&root).expect("test workspace should be created");
        let input = SessionInput {
            user_input: "inspect this workspace".to_string(),
            file_input: Vec::new(),
            agent: None,
            runtime_context: Some("client runtime context".to_string()),
            planning_mode_override: None,
        };
        let mut session = SessionManagement::new(
            "runtime-context-tag-session".to_string(),
            "runtime context tag".to_string(),
            root.clone(),
            false,
            "coding".to_string(),
            input,
            "inspect this workspace".to_string(),
            Utc::now(),
        );

        let initial =
            initial_messages_for_session(&mut session).expect("initial messages should build");
        let initial_context = initial
            .iter()
            .find(|message| message["content"] == "client runtime context")
            .expect("initial messages should include runtime context");
        assert_eq!(initial_context["role"], USER_AGENT_CONTEXT_ROLE);

        let stored_context = session
            .session_log
            .iter()
            .filter_map(|entry| serde_json::from_str::<serde_json::Value>(entry).ok())
            .find(|entry| entry["content"] == "client runtime context")
            .expect("runtime context should be stored");
        assert_eq!(stored_context["role"], USER_AGENT_CONTEXT_ROLE);

        let replayed = build_messages_from_session(&session);
        let replayed_context = replayed
            .iter()
            .find(|message| message["content"] == "client runtime context")
            .expect("runtime context should replay into provider context");
        assert_eq!(replayed_context["role"], "user");

        let _ = fs::remove_dir_all(root);
    }
}
