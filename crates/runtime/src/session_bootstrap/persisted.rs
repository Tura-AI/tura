use std::path::Path;

use crate::session_log_client::SessionLogClient;
use crate::state_machine::session_management::SessionManagement;

pub(crate) fn load_persisted_gateway_session(
    directory: &Path,
    session_id: &str,
) -> Option<SessionManagement> {
    let snapshot = SessionLogClient::discover()
        .ok()?
        .get_session(session_id.to_string())
        .ok()??;
    if normalize_workspace(&snapshot.workspace) != normalize_workspace(&directory.to_string_lossy())
    {
        return None;
    }
    serde_json::from_value(snapshot.management).ok()
}

fn normalize_workspace(value: &str) -> String {
    let normalized = value.replace('\\', "/");
    let trimmed = normalized.trim_end_matches('/');
    if trimmed.is_empty() && normalized.starts_with('/') {
        "/".to_string()
    } else if trimmed.is_empty() {
        normalized
    } else {
        trimmed.to_string()
    }
}
