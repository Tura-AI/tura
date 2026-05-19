use std::path::Path;

use crate::state_machine::session_management::SessionManagement;

pub(super) fn load_persisted_gateway_session(
    directory: &Path,
    session_id: &str,
) -> Option<SessionManagement> {
    let path = directory
        .join(".tura")
        .join("sessions")
        .join(format!("{session_id}.json"));
    let content = std::fs::read_to_string(path).ok()?;
    let value = serde_json::from_str::<serde_json::Value>(&content).ok()?;
    let management = value.get("info")?.get("management")?.clone();
    serde_json::from_value(management).ok()
}

pub(crate) fn persist_gateway_session(session: &SessionManagement) -> Result<(), String> {
    let dir = session.session_directory.join(".tura").join("sessions");
    std::fs::create_dir_all(&dir).map_err(|err| err.to_string())?;
    let path = dir.join(format!("{}.json", session.session_id));
    let content = serde_json::to_string_pretty(&persisted_record(session))
        .map_err(|err| err.to_string())?;
    std::fs::write(path, content).map_err(|err| err.to_string())
}

fn persisted_record(session: &SessionManagement) -> serde_json::Value {
    serde_json::json!({
        "info": {
            "id": session.session_id,
            "name": session.session_name,
            "created_at": session.session_created_at.timestamp_millis(),
            "updated_at": session.session_last_update_at.timestamp_millis(),
            "directory": session.session_directory.to_string_lossy(),
            "model": null,
            "agent": session.input.agent,
            "session_type": session.session_topic,
            "lsp": {
                "mode": "auto",
                "enabled": [],
                "disabled": [],
            },
            "kill_processes_on_start": false,
            "validator_enabled": false,
            "force_planning": false,
            "model_variant": null,
            "model_acceleration_enabled": false,
            "disable_permission_restrictions": session.disable_permission_restrictions,
            "use_last_tool_call_response": session.use_last_tool_call_response,
            "status": session_status(session),
            "message_count": session.session_current_turn,
            "management": session,
        },
        "parent_id": null,
        "messages": [],
        "todos": [],
    })
}

fn session_status(session: &SessionManagement) -> &'static str {
    use crate::state_machine::session_management::SessionState;

    match session.state {
        SessionState::Created | SessionState::Completed => "idle",
        SessionState::Running | SessionState::Paused => "busy",
        SessionState::Failed | SessionState::Cancelled => "error",
    }
}
