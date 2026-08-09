use serde_json::Value;
use std::collections::BTreeMap;

const PLACEHOLDER_OPEN: &str = "#@#${";
const PLACEHOLDER_CLOSE: &str = "#@#$";

#[derive(Clone, Debug, Default)]
pub struct CommandRunOutputBindings {
    values: BTreeMap<String, Value>,
}

impl CommandRunOutputBindings {
    pub fn insert(&mut self, command_type: &str, binding_id: Option<&str>, output: &Value) {
        let projection = binding_projection(output);
        self.values
            .insert(command_type.to_string(), projection.clone());
        if let Some(binding_id) = binding_id.filter(|value| !value.trim().is_empty()) {
            self.values.insert(binding_id.to_string(), projection);
        }
    }

    pub fn interpolate_command_line(&self, input: &str) -> Result<String, String> {
        if !input.contains(PLACEHOLDER_OPEN) {
            return Ok(input.to_string());
        }

        if let Some(mut json) = parse_json_document(input) {
            self.interpolate_value(&mut json)?;
            return Ok(json.to_string());
        }

        self.interpolate_text(input)
    }

    pub fn interpolate_value(&self, value: &mut Value) -> Result<(), String> {
        match value {
            Value::String(text) => {
                if let Some(expression) = exact_placeholder_expression(text) {
                    *value = self.resolve(&expression)?;
                } else if text.contains(PLACEHOLDER_OPEN) {
                    *text = self.interpolate_text(text)?;
                }
            }
            Value::Array(items) => {
                for item in items {
                    self.interpolate_value(item)?;
                }
            }
            Value::Object(object) => {
                for child in object.values_mut() {
                    self.interpolate_value(child)?;
                }
            }
            Value::Null | Value::Bool(_) | Value::Number(_) => {}
        }
        Ok(())
    }

    fn interpolate_text(&self, input: &str) -> Result<String, String> {
        let mut output = String::with_capacity(input.len());
        let mut remaining = input;
        while let Some(start) = remaining.find(PLACEHOLDER_OPEN) {
            output.push_str(&remaining[..start]);
            let expression_start = start + PLACEHOLDER_OPEN.len();
            let after_open = &remaining[expression_start..];
            let Some(end) = after_open.find(PLACEHOLDER_CLOSE) else {
                return Err(format!(
                    "unterminated command_run output placeholder starting at `{}`",
                    &remaining[start..]
                ));
            };
            let expression = normalize_expression(&after_open[..end])?;
            output.push_str(&render_embedded_value(&self.resolve(&expression)?));
            remaining = &after_open[end + PLACEHOLDER_CLOSE.len()..];
        }
        output.push_str(remaining);
        Ok(output)
    }

    fn resolve(&self, expression: &str) -> Result<Value, String> {
        let segments = expression.split('.').collect::<Vec<_>>();
        let Some(binding_name) = segments.first().copied() else {
            return Err("empty command_run output placeholder".to_string());
        };
        let Some(root) = self.values.get(binding_name) else {
            let available = self.values.keys().cloned().collect::<Vec<_>>().join(", ");
            return Err(format!(
                "command_run output placeholder `{expression}` references unavailable `{binding_name}`; available previous-step bindings: [{available}]"
            ));
        };
        let path = &segments[1..];
        resolve_compatible_path(root, path).ok_or_else(|| {
            format!(
                "command_run output placeholder `{expression}` did not match the previous-step JSON output"
            )
        })
    }
}

fn binding_projection(value: &Value) -> Value {
    match value {
        Value::String(text) => {
            parse_json_compatible(text).unwrap_or_else(|| Value::String(text.clone()))
        }
        Value::Object(source) => {
            let mut object = source.clone();
            if !object.contains_key("structuredContent")
                && let Some(structured) = structured_content_from_mcp_text(&object)
            {
                object.insert("structuredContent".to_string(), structured);
            }

            for source_key in ["stdout", "output", "result", "data"] {
                let Some(parsed) = object
                    .get(source_key)
                    .and_then(Value::as_str)
                    .and_then(parse_json_compatible)
                else {
                    continue;
                };
                if let Value::Object(fields) = &parsed {
                    for (key, value) in fields {
                        object.entry(key.clone()).or_insert_with(|| value.clone());
                    }
                }
            }
            Value::Object(object)
        }
        other => other.clone(),
    }
}

fn structured_content_from_mcp_text(object: &serde_json::Map<String, Value>) -> Option<Value> {
    object
        .get("content")?
        .as_array()?
        .iter()
        .filter_map(Value::as_object)
        .filter(|item| item.get("type").and_then(Value::as_str) == Some("text"))
        .filter_map(|item| item.get("text").and_then(Value::as_str))
        .find_map(parse_json_compatible)
}

fn exact_placeholder_expression(input: &str) -> Option<String> {
    let trimmed = input.trim();
    let body = trimmed.strip_prefix(PLACEHOLDER_OPEN)?;
    let body = body.strip_suffix(PLACEHOLDER_CLOSE)?;
    normalize_expression(body).ok()
}

fn normalize_expression(input: &str) -> Result<String, String> {
    let normalized = input
        .trim()
        .trim_end_matches('}')
        .trim_end_matches(',')
        .replace("},{", ".")
        .replace(',', ".")
        .replace(['{', '}'], "");
    let segments = normalized
        .split('.')
        .map(str::trim)
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>();
    if segments.is_empty()
        || segments.iter().any(|segment| {
            !segment
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | ':'))
        })
    {
        return Err(format!(
            "invalid command_run output placeholder expression `{input}`"
        ));
    }
    Ok(segments.join("."))
}

fn resolve_compatible_path(root: &Value, path: &[&str]) -> Option<Value> {
    if let Some(value) = resolve_direct_path(root, path) {
        return Some(value);
    }
    if matches!(path.first().copied(), Some("output" | "result" | "data"))
        && let Some(value) = resolve_direct_path(root, &path[1..])
    {
        return Some(value);
    }
    let object = root.as_object()?;
    for key in ["structuredContent", "output", "result", "data"] {
        if let Some(candidate) = object.get(key)
            && let Some(value) = resolve_direct_path(candidate, path)
        {
            return Some(value);
        }
    }
    if let Some(stdout) = object.get("stdout").and_then(Value::as_str)
        && let Some(parsed) = parse_json_compatible(stdout)
        && let Some(value) = resolve_direct_path(&parsed, path)
    {
        return Some(value);
    }
    structured_content_from_mcp_text(object)
        .and_then(|candidate| resolve_direct_path(&candidate, path))
}

fn resolve_direct_path(root: &Value, path: &[&str]) -> Option<Value> {
    let mut current = root.clone();
    for segment in path {
        if let Value::String(text) = &current
            && let Some(parsed) = parse_json_compatible(text)
        {
            current = parsed;
        }
        current = match current {
            Value::Object(object) => object.get(*segment)?.clone(),
            Value::Array(items) => items.get(segment.parse::<usize>().ok()?)?.clone(),
            Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => return None,
        };
    }
    Some(current)
}

fn parse_json_document(input: &str) -> Option<Value> {
    let trimmed = input.trim();
    if let Some(unfenced) = strip_code_fence(trimmed) {
        return serde_json::from_str(unfenced.trim()).ok();
    }
    serde_json::from_str(trimmed).ok()
}

fn parse_json_compatible(input: &str) -> Option<Value> {
    let mut text = input.trim().to_string();
    for _ in 0..3 {
        let parsed = parse_json_document(&text).or_else(|| {
            extract_balanced_json(&text).and_then(|json| serde_json::from_str(json).ok())
        })?;
        match parsed {
            Value::String(nested) if nested.trim() != text => text = nested,
            value => return Some(value),
        }
    }
    None
}

fn strip_code_fence(input: &str) -> Option<&str> {
    let stripped = input.strip_prefix("```")?;
    let newline = stripped.find('\n')?;
    let body = &stripped[newline + 1..];
    let end = body.rfind("```")?;
    Some(&body[..end])
}

fn extract_balanced_json(input: &str) -> Option<&str> {
    for (start, opening) in input.char_indices() {
        let closing = match opening {
            '{' => '}',
            '[' => ']',
            _ => continue,
        };
        let mut stack = vec![closing];
        let mut in_string = false;
        let mut escaped = false;
        for (offset, ch) in input[start + opening.len_utf8()..].char_indices() {
            if in_string {
                if escaped {
                    escaped = false;
                } else if ch == '\\' {
                    escaped = true;
                } else if ch == '"' {
                    in_string = false;
                }
                continue;
            }
            match ch {
                '"' => in_string = true,
                '{' => stack.push('}'),
                '[' => stack.push(']'),
                '}' | ']' if stack.last().copied() == Some(ch) => {
                    stack.pop();
                    if stack.is_empty() {
                        let end = start + opening.len_utf8() + offset + ch.len_utf8();
                        return Some(&input[start..end]);
                    }
                }
                _ => {}
            }
        }
    }
    None
}

fn render_embedded_value(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Null => "null".to_string(),
        Value::Bool(value) => value.to_string(),
        Value::Number(value) => value.to_string(),
        Value::Array(_) | Value::Object(_) => value.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::{binding_projection, CommandRunOutputBindings};
    use serde_json::{json, Value};

    #[test]
    fn builds_internal_json_projections_without_mutating_original_outputs() {
        let direct = Value::String("```json\n{\"id\":7}\n```".to_string());
        assert_eq!(binding_projection(&direct), json!({"id": 7}));
        assert!(direct.is_string());

        let shell = json!({
            "exit_code": 0,
            "stdout": "created: {\"filename\":\"draft.txt\"}\n",
            "stderr": ""
        });
        let projected_shell = binding_projection(&shell);
        assert_eq!(projected_shell["filename"], "draft.txt");
        assert_eq!(shell["stdout"], "created: {\"filename\":\"draft.txt\"}\n");
        assert!(shell.get("filename").is_none());
        assert!(projected_shell.get("parsed_stdout").is_none());

        let mcp = json!({
            "content": [{"type": "text", "text": "{\"document_id\":\"doc-1\"}"}],
            "isError": false
        });
        let projected_mcp = binding_projection(&mcp);
        assert_eq!(projected_mcp["structuredContent"]["document_id"], "doc-1");
        assert!(mcp.get("structuredContent").is_none());
        assert!(mcp["content"][0]["text"].is_string());
    }

    #[test]
    fn interpolates_generic_binding_paths_and_preserves_exact_value_types() {
        let mut bindings = CommandRunOutputBindings::default();
        bindings.insert(
            "workspace_tool",
            Some("create_file"),
            &json!({"filename": "draft.txt", "metadata": {"revision": 3}}),
        );

        let line = bindings
            .interpolate_command_line(
                r##"{"path":"#@#${create_file.filename}#@#$","revision":"#@#${create_file.metadata.revision}#@#$"}"##,
            )
            .expect("interpolate JSON command line");
        let line: Value = serde_json::from_str(&line).expect("resolved JSON command line");
        assert_eq!(line["path"], "draft.txt");
        assert_eq!(line["revision"], 3);

        let mut inline = json!({"revision": "#@#${workspace_tool,metadata,revision}#@#$"});
        bindings
            .interpolate_value(&mut inline)
            .expect("interpolate comma-compatible path");
        assert_eq!(inline["revision"], 3);

        let mut wrapped = json!({
            "filename": "#@#${create_file.output.filename}#@#$"
        });
        bindings
            .interpolate_value(&mut wrapped)
            .expect("accept the full command result output wrapper");
        assert_eq!(wrapped["filename"], "draft.txt");
    }

    #[test]
    fn rejects_missing_future_or_same_step_bindings() {
        let bindings = CommandRunOutputBindings::default();
        let error = bindings
            .interpolate_command_line("#@#${future.value}#@#$")
            .expect_err("missing binding must fail before dispatch");
        assert!(error.contains("unavailable `future`"), "{error}");
    }
}
