use std::collections::HashSet;
use std::io::Write;
use std::sync::{Mutex, OnceLock};

use crate::state_machine::session_management::SessionManagement;
use crate::turn_loop::retry_policy::env_flag;

pub(crate) fn emit_cli_live_session_checkpoint(session: &SessionManagement, stage: &str) {
    if matches!(stage, "completed") {
        return;
    }
    static EMITTED: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    let emitted = EMITTED.get_or_init(|| Mutex::new(HashSet::new()));
    let Ok(mut emitted) = emitted.lock() else {
        return;
    };
    if !emitted.insert(session.session_id.clone()) {
        return;
    }
    if env_flag("TURA_CLI_PROGRESS") && !env_flag("TURA_CLI_LIVE_JSONL") {
        eprintln!("status: runtime session active ({stage})");
        return;
    }
    if !env_flag("TURA_CLI_LIVE_JSONL") {
        return;
    }
    let event = serde_json::json!({
        "type": "item.completed",
        "item": {
            "id": "item_live_0",
            "type": "agent_message",
            "text": "Runtime session is active; detailed command events will follow."
        }
    });
    println!("{event}");
    let _ = std::io::stdout().flush();
}
