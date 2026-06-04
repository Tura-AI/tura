use crate::commands::CommandResponse;
use serde_json::Value;

pub(super) fn blocked_command_response(command: &str, reason: &str) -> CommandResponse {
    let message =
        format!("Blocked by command interceptor: {reason}\nCommand was not executed: {command}");
    CommandResponse {
        success: false,
        exit_code: 126,
        stdout: String::new(),
        stderr: message.clone(),
        output: Value::String(message),
        changes: Vec::new(),
    }
}

pub(super) fn failed_async_response(message: &str, exit_code: i32) -> CommandResponse {
    CommandResponse {
        success: false,
        exit_code,
        stdout: String::new(),
        stderr: message.to_string(),
        output: Value::String(message.to_string()),
        changes: Vec::new(),
    }
}

pub(super) fn shell_output_value(response: CommandResponse) -> Value {
    json_like_output(
        response.exit_code,
        response.stdout,
        response.stderr,
        response.output,
        response.changes,
    )
}

pub(crate) fn json_like_output(
    exit_code: i32,
    stdout: String,
    stderr: String,
    output: Value,
    changes: Vec<Value>,
) -> Value {
    let transcript = output.as_str().unwrap_or_default().to_string();
    let mut object = serde_json::Map::new();
    object.insert("exit_code".to_string(), Value::Number(exit_code.into()));
    object.insert("stdout".to_string(), Value::String(stdout));
    object.insert("stderr".to_string(), Value::String(stderr));
    object.insert("output".to_string(), output);
    object.insert("transcript".to_string(), Value::String(transcript));
    if !changes.is_empty() {
        object.insert("changes".to_string(), Value::Array(changes));
    }
    Value::Object(object)
}
