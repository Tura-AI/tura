use serde_json::Value;

pub(super) fn results_summary(results: &[Value]) -> String {
    let mut lines = Vec::new();
    for result in results {
        let path = result.get("path").and_then(Value::as_str).unwrap_or("");
        let media_type = result
            .get("media_type")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        let preview_count = result
            .get("visual_preview_count")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        let audio_count = result
            .get("audio_preview_count")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        if result.get("success").and_then(Value::as_bool) == Some(true) {
            lines.push(format!(
                "- {path}: {media_type}, {preview_count} visual previews, {audio_count} audio previews"
            ));
        } else {
            let error = result
                .get("error")
                .and_then(Value::as_str)
                .unwrap_or("error");
            lines.push(format!("- {path}: failed: {error}"));
        }
    }
    lines.join("\n")
}

pub(super) fn summary_text(value: &Value) -> String {
    value
        .get("summary_markdown")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}
