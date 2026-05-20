use chrono::Utc;
use tracing::{error, info};

use crate::agent_router::{activate_agents_by_session_type, initialize_agent_state_machine};
use crate::context::{
    accumulate_message, build_messages_from_session, ContextualUserFragment, WorkspaceSnapshot,
};
use crate::manas::{process_manas_internal, ManasInput};
use crate::mano::gateway_session::{load_persisted_gateway_session, persist_gateway_session};
use crate::mano::session_bootstrap::create_session_with_topic;
use crate::mano::{ManoOverrides, ManoProcessResult};
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
            "content": session.input.user_input,
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
            serde_json::Value::String(session.input.user_input.clone()),
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
        "<environment_context>\n  <cwd>{}</cwd>\n  <shell>{}</shell>\n  <current_date>{}</current_date>\n  <timezone>{}</timezone>\n</environment_context>",
        cwd.display(),
        context_shell_name(),
        chrono::Local::now().format("%Y-%m-%d"),
        std::env::var("TZ").unwrap_or_else(|_| "Europe/Paris".to_string())
    )
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
    let input = session.input.user_input.trim();
    session.session_log.iter().any(|entry| {
        serde_json::from_str::<serde_json::Value>(entry)
            .ok()
            .is_some_and(|value| {
                value.get("role").and_then(serde_json::Value::as_str) == Some("user")
                    && value
                        .get("content")
                        .and_then(serde_json::Value::as_str)
                        .is_some_and(|content| content.trim() == input)
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
    let session = match overrides.session_factory {
        Some(session_factory) => session_factory(input)?,
        None => create_session_with_topic(input, None)?,
    };

    let agents = match overrides.manas_entry {
        Some(manas_entry) => manas_entry(&session)?,
        None => {
            let mut agts = activate_agents_by_session_type(&session)?;
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
    fn gateway_bootstrap_loads_persisted_session_before_creating_new_session() {
        let root = std::env::temp_dir().join(format!(
            "tura-gateway-bootstrap-{}",
            Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ));
        let session_id = "sess-existing".to_string();
        let sessions_dir = root.join(".tura").join("sessions");
        fs::create_dir_all(&sessions_dir).expect("test session dir");

        let old_input = SessionInput {
            user_input: "old prompt".to_string(),
            file_input: Vec::new(),
            agent: None,
            runtime_context: None,
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

        let record = serde_json::json!({
            "info": {
                "management": persisted,
            },
            "messages": [],
            "todos": [],
        });
        fs::write(
            sessions_dir.join(format!("{session_id}.json")),
            serde_json::to_string_pretty(&record).expect("record json"),
        )
        .expect("write persisted session");

        let next_input = SessionInput {
            user_input: "fix bug in the existing workspace".to_string(),
            file_input: Vec::new(),
            agent: None,
            runtime_context: None,
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

        let _ = fs::remove_dir_all(session.session_directory);
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
