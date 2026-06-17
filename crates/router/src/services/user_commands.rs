use parking_lot::RwLock;
use serde_json::{json, Value};
use std::{collections::HashMap, sync::Arc};

#[derive(Clone, Default)]
pub struct UserCommandService {
    commands: Arc<RwLock<HashMap<String, Vec<String>>>>,
}

impl UserCommandService {
    pub fn append(&self, input: &Value) -> Value {
        let session_id = normalized_session_id(input);
        let command = input
            .get("command")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .trim()
            .to_string();
        let mut commands = self.commands.write();
        let entry = commands.entry(session_id.clone()).or_default();
        if !command.is_empty() {
            entry.push(command);
        }
        json!({
            "ok": true,
            "session_id": session_id,
            "commands": entry.clone(),
        })
    }

    pub fn take(&self, input: &Value) -> Value {
        let session_id = normalized_session_id(input);
        let commands = self
            .commands
            .write()
            .remove(&session_id)
            .unwrap_or_default();
        json!({
            "ok": true,
            "session_id": session_id,
            "commands": commands,
        })
    }
}

fn normalized_session_id(input: &Value) -> String {
    input
        .get("root_session_id")
        .or_else(|| input.get("session_id"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("unknown-session")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::UserCommandService;
    use serde_json::json;

    #[test]
    fn append_and_take_user_commands_use_root_session_id() {
        let service = UserCommandService::default();

        service.append(&json!({
            "session_id": "child",
            "root_session_id": "root",
            "command": " run tests "
        }));
        service.append(&json!({
            "session_id": "child",
            "root_session_id": "root",
            "command": "ship it"
        }));

        let taken = service.take(&json!({ "session_id": "root" }));
        assert_eq!(taken["commands"], json!(["run tests", "ship it"]));
        let empty = service.take(&json!({ "session_id": "root" }));
        assert_eq!(empty["commands"], json!([]));
    }
}
