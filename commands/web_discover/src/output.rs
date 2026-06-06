use serde_json::Value;

use super::util::truncate_chars;

pub(super) fn summarize_records(records: &[Value], downloaded: &[Value]) -> String {
    let mut lines = Vec::new();
    for (index, record) in records.iter().enumerate().take(10) {
        if let Some(text) = record.as_str() {
            lines.push(format!(
                "{}. {}",
                index + 1,
                truncate_chars(&text.replace('\n', " "), 220)
            ));
            continue;
        }
        let title = record
            .get("title")
            .and_then(Value::as_str)
            .unwrap_or("Untitled");
        let url = record
            .get("url")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let path = record
            .get("local_path")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if path.is_empty() {
            lines.push(format!("{}. [{}]({})", index + 1, title, url));
        } else {
            lines.push(format!("{}. [{}]({}) -> {}", index + 1, title, url, path));
        }
    }
    if !downloaded.is_empty() {
        lines.push("downloaded:".to_string());
        for item in downloaded {
            let path = item.get("path").and_then(Value::as_str).unwrap_or_default();
            let size = item.get("size").and_then(Value::as_u64).unwrap_or(0);
            lines.push(format!("- {path} ({size} bytes)"));
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
