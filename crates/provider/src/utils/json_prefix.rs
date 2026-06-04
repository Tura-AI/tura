pub fn json_prefix(text: &str) -> Option<&str> {
    let trimmed = text.trim_start();
    let first = trimmed.chars().next()?;
    if !matches!(first, '[' | '{' | '"') {
        return None;
    }
    let mut stack = Vec::new();
    let mut in_string = false;
    let mut escaped = false;
    let string_started_as_root = first == '"';

    for (index, ch) in trimmed.char_indices() {
        if in_string {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
                if string_started_as_root && stack.is_empty() {
                    return Some(&trimmed[..index + ch.len_utf8()]);
                }
            }
            continue;
        }

        match ch {
            '"' => in_string = true,
            '[' => stack.push(']'),
            '{' => stack.push('}'),
            ']' | '}' => {
                if stack.pop() != Some(ch) {
                    return None;
                }
                if stack.is_empty() {
                    return Some(&trimmed[..index + ch.len_utf8()]);
                }
            }
            _ => {}
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::json_prefix;

    #[test]
    fn returns_complete_json_prefix_before_trailing_text() {
        assert_eq!(json_prefix(r#"[{"a":1}] trailing"#), Some(r#"[{"a":1}]"#));
        assert_eq!(json_prefix(r#"{"a":"}"} trailing"#), Some(r#"{"a":"}"}"#));
    }
}
