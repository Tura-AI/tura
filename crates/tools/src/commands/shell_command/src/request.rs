use serde_json::Value;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub(super) struct ShellRequest {
    pub(super) command: String,
    pub(super) cwd: PathBuf,
    pub(super) timeout_secs: u64,
}

pub(super) fn parse_shell_request(
    command_line: &str,
    session_dir: &Path,
    default_timeout_secs: u64,
) -> ShellRequest {
    let text = command_line.trim();
    if text.starts_with('{') || text.starts_with('"') || text.starts_with('\'') {
        if let Some(value) = parse_shell_request_json(text) {
            if let Some(command) = value
                .get("command")
                .or_else(|| value.get("cmd"))
                .and_then(Value::as_str)
            {
                let timeout_secs = value
                    .get("timeout_secs")
                    .and_then(Value::as_u64)
                    .or_else(|| {
                        value
                            .get("timeout_ms")
                            .and_then(Value::as_u64)
                            .map(|ms| ms.div_ceil(1000).max(1))
                    })
                    .unwrap_or(default_timeout_secs);
                let cwd = value
                    .get("workdir")
                    .or_else(|| value.get("cwd"))
                    .and_then(Value::as_str)
                    .map(PathBuf::from)
                    .map(|path| {
                        if path.is_absolute() {
                            path
                        } else {
                            session_dir.join(path)
                        }
                    })
                    .unwrap_or_else(|| session_dir.to_path_buf());
                return ShellRequest {
                    command: normalize_shell_command_text(command),
                    cwd,
                    timeout_secs,
                };
            }
        }
    }
    ShellRequest {
        command: normalize_shell_command_text(command_line),
        cwd: session_dir.to_path_buf(),
        timeout_secs: default_timeout_secs,
    }
}

fn normalize_shell_command_text(command: &str) -> String {
    let trimmed = command.trim_start();
    for prefix in ["command:", "cmd:", "shell:", "bash:"] {
        if trimmed
            .get(..prefix.len())
            .is_some_and(|head| head.eq_ignore_ascii_case(prefix))
        {
            return trimmed[prefix.len()..].trim_start().to_string();
        }
    }
    let normalized_lines = command
        .lines()
        .map(|line| {
            let line_trimmed = line.trim_start();
            for prefix in ["command:", "cmd:", "shell:", "bash:"] {
                if line_trimmed
                    .get(..prefix.len())
                    .is_some_and(|head| head.eq_ignore_ascii_case(prefix))
                {
                    let leading_len = line.len().saturating_sub(line_trimmed.len());
                    return format!(
                        "{}{}",
                        &line[..leading_len],
                        line_trimmed[prefix.len()..].trim_start()
                    );
                }
            }
            line.to_string()
        })
        .collect::<Vec<_>>()
        .join("\n");
    if command.ends_with('\n') {
        format!("{normalized_lines}\n")
    } else {
        normalized_lines
    }
}
pub(super) fn embedded_apply_patch_text(command: &str) -> Option<String> {
    let begin = command.find("*** Begin Patch")?;
    let after_begin = &command[begin..];
    let end_relative = after_begin.find("*** End Patch")?;
    let end = begin + end_relative + "*** End Patch".len();
    let patch = &command[begin..end];
    if command[..begin].contains("cat ")
        || command[..begin].contains("Get-Content")
        || command[..begin].contains("grep ")
        || command[..begin].contains("rg ")
    {
        return None;
    }
    Some(patch.trim().to_string())
}

fn parse_shell_request_json(text: &str) -> Option<Value> {
    fn parse_candidate(candidate: &str, depth: usize) -> Option<Value> {
        if depth > 3 {
            return None;
        }
        match serde_json::from_str::<Value>(candidate).ok()? {
            Value::String(inner) => parse_candidate(inner.trim(), depth + 1),
            value => Some(value),
        }
    }

    let trimmed = text.trim();
    parse_candidate(trimmed, 0)
        .or_else(|| parse_candidate(&format!("\"{trimmed}\""), 0))
        .or_else(|| parse_loose_shell_request_object(trimmed))
        .or_else(|| {
            trimmed
                .strip_prefix('\'')
                .and_then(|value| value.strip_suffix('\''))
                .and_then(|inner| parse_candidate(inner.trim(), 0))
        })
        .or_else(|| {
            if trimmed.contains("\\\"") {
                parse_candidate(&trimmed.replace("\\\"", "\""), 0)
            } else {
                None
            }
        })
}

fn parse_loose_shell_request_object(text: &str) -> Option<Value> {
    let trimmed = text.trim();
    if !trimmed.starts_with('{') || !trimmed.ends_with('}') {
        return None;
    }

    let command = loose_json_string_field(trimmed, "command")
        .or_else(|| loose_json_string_field(trimmed, "cmd"))?;
    let mut object = serde_json::Map::new();
    object.insert("command".to_string(), Value::String(command));
    if let Some(workdir) = loose_json_string_field(trimmed, "workdir") {
        object.insert("workdir".to_string(), Value::String(workdir));
    }
    if let Some(timeout_ms) = loose_json_number_field(trimmed, "timeout_ms") {
        object.insert("timeout_ms".to_string(), Value::Number(timeout_ms.into()));
    }
    if let Some(timeout_secs) = loose_json_number_field(trimmed, "timeout_secs") {
        object.insert(
            "timeout_secs".to_string(),
            Value::Number(timeout_secs.into()),
        );
    }
    Some(Value::Object(object))
}

fn loose_json_string_field(text: &str, field: &str) -> Option<String> {
    let marker = format!("\"{field}\":\"");
    let start = text.find(&marker)? + marker.len();
    let raw = loose_json_string_field_raw(&text[start..])?;
    decode_loose_json_string(raw)
}

fn loose_json_string_field_raw(rest: &str) -> Option<&str> {
    let bytes = rest.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'"' {
            let mut slash_count = 0;
            let mut cursor = index;
            while cursor > 0 && bytes[cursor - 1] == b'\\' {
                slash_count += 1;
                cursor -= 1;
            }
            if slash_count % 2 == 0 {
                let after = &rest[index + 1..];
                if after.trim_start().starts_with(',')
                    || after.trim_start().starts_with('}')
                    || after.trim_start().is_empty()
                {
                    return Some(&rest[..index]);
                }
            }
        }
        index += 1;
    }
    None
}

fn loose_json_number_field(text: &str, field: &str) -> Option<u64> {
    let marker = format!("\"{field}\":");
    let start = text.find(&marker)? + marker.len();
    let rest = text[start..].trim_start();
    let digits = rest
        .chars()
        .take_while(|ch| ch.is_ascii_digit())
        .collect::<String>();
    (!digits.is_empty())
        .then(|| digits.parse::<u64>().ok())
        .flatten()
}

fn decode_loose_json_string(raw: &str) -> Option<String> {
    let mut decoded = String::with_capacity(raw.len());
    let mut chars = raw.chars();
    while let Some(ch) = chars.next() {
        if ch != '\\' {
            decoded.push(ch);
            continue;
        }
        let Some(next) = chars.next() else {
            decoded.push('\\');
            break;
        };
        match next {
            '"' => decoded.push('"'),
            '\\' => decoded.push('\\'),
            '/' => decoded.push('/'),
            'n' => decoded.push('\n'),
            'r' => decoded.push('\r'),
            't' => decoded.push('\t'),
            'b' => decoded.push('\u{0008}'),
            'f' => decoded.push('\u{000c}'),
            'u' => {
                let digits = chars.by_ref().take(4).collect::<String>();
                if let Ok(code) = u16::from_str_radix(&digits, 16) {
                    if let Some(value) = char::from_u32(code as u32) {
                        decoded.push(value);
                    }
                }
            }
            other => {
                decoded.push('\\');
                decoded.push(other);
            }
        }
    }
    Some(decoded)
}
