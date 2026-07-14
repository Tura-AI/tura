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
        Value::Null,
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
    let mut object = serde_json::Map::new();
    object.insert("exit_code".to_string(), Value::Number(exit_code.into()));
    object.insert("stdout".to_string(), Value::String(stdout));
    object.insert("stderr".to_string(), Value::String(stderr));
    if let Value::Object(fields) = output {
        for (key, value) in fields {
            object.entry(key).or_insert(value);
        }
    }
    if !changes.is_empty() {
        object.insert("changes".to_string(), Value::Array(changes));
    }
    Value::Object(object)
}

#[cfg(test)]
mod tests {
    use super::{json_like_output, shell_output_value};
    use crate::commands::CommandResponse;
    use serde_json::{json, Value};

    #[test]
    fn shell_output_value_keeps_flat_stream_fields() {
        let value = shell_output_value(CommandResponse {
            success: true,
            exit_code: 0,
            stdout: "plain stdout\n".to_string(),
            stderr: String::new(),
            output: Value::String("Exit code: 0\nOutput:\nplain stdout\n".to_string()),
            changes: Vec::new(),
        });

        assert_eq!(value["exit_code"], json!(0));
        assert_eq!(value["stdout"], json!("plain stdout\n"));
        assert_eq!(value["stderr"], json!(""));
        assert!(value.get("output").is_none(), "{value}");
        assert!(value.get("cli_output").is_none(), "{value}");
        assert!(value.get("changes").is_none(), "{value}");
    }

    #[test]
    fn json_like_output_flattens_structured_output_and_changes() {
        let changes = vec![json!({"kind":"add","path":"file.txt"})];

        let value = json_like_output(
            1,
            String::new(),
            "ContextMismatch".to_string(),
            json!({"error_type":"ContextMismatch","message":"missing context"}),
            changes,
        );

        assert_eq!(value["stderr"], json!("ContextMismatch"));
        assert_eq!(value["error_type"], json!("ContextMismatch"));
        assert_eq!(value["changes"][0]["path"], json!("file.txt"));
        assert!(value.get("output").is_none(), "{value}");
    }

    #[test]
    fn json_like_output_drops_empty_object_output() {
        let value = json_like_output(
            0,
            "Success. Updated files.".to_string(),
            String::new(),
            json!({}),
            Vec::new(),
        );

        assert_eq!(value["stdout"], json!("Success. Updated files."));
        assert_eq!(value["stderr"], json!(""));
        assert!(value.get("output").is_none(), "{value}");
        assert!(value.get("changes").is_none(), "{value}");
    }
}
