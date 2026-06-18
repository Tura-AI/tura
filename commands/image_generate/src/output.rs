use serde_json::Value;

pub(super) fn summarize_output(value: &Value) -> String {
    let provider = value
        .get("provider")
        .and_then(Value::as_str)
        .unwrap_or("image_generate");
    let mut lines = vec![format!("generated with {provider}")];
    if let Some(images) = value.get("images").and_then(Value::as_array) {
        for image in images {
            let path = image
                .get("path")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let size = image.get("size").and_then(Value::as_u64).unwrap_or(0);
            lines.push(format!("- {path} ({size} bytes)"));
        }
    }
    if let Some(attempts) = value.get("attempts").and_then(Value::as_array) {
        let failed = attempts
            .iter()
            .filter(|attempt| attempt.get("success").and_then(Value::as_bool) == Some(false))
            .count();
        if failed > 0 {
            lines.push(format!("fallback attempts failed before success: {failed}"));
        }
    }
    lines.join("\n")
}

pub(super) fn summary_text(value: &Value) -> String {
    value
        .get("summary_markdown")
        .and_then(Value::as_str)
        .map(ToString::to_string)
        .unwrap_or_else(|| summarize_output(value))
}
