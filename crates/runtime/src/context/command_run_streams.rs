//! Render command_run tool results into CLI-style stdout/stderr: turn
//! structured JSON input back into a readable shell command, and render
//! structured stdout (results/matches/outline/…) as `path:line:content`
//! CLI lines, with diagnostics/errors extracted.
//!
//! Pure rendering layer carved out of `context_management.rs`; exposes only
//! `command_run_display_command` and `command_run_llm_streams`.

use super::text_truncate::{formatted_truncate_text, COMMAND_RUN_RESULT_OUTPUT_MAX_TOKENS};

pub(super) fn command_run_display_command(command: &str, command_line: &str) -> String {
    if command_line.trim().is_empty() {
        return command.to_string();
    }
    if normalized_command_run_subcommand(command) == "apply_patch"
        && command_line.trim_start().starts_with("*** Begin Patch")
    {
        return format!("apply_patch <<'PATCH'\n{}\nPATCH", command_line.trim_end());
    }
    if let Some(cli) = structured_command_line_as_cli(command, command_line) {
        return cli;
    }
    if is_structured_code_read_command(command) {
        return format!("{command} {}", command_line.trim());
    }
    command_line.trim().to_string()
}

pub(super) fn command_run_llm_streams(command: &str, stdout: &str) -> (String, String) {
    if let Some(streams) = verify_stdout_as_cli_streams(stdout) {
        return streams;
    }
    structured_stdout_as_cli_streams(command, stdout)
        .unwrap_or_else(|| (stdout.trim_end().to_string(), String::new()))
}

fn verify_stdout_as_cli_streams(stdout: &str) -> Option<(String, String)> {
    let value = serde_json::from_str::<serde_json::Value>(stdout).ok()?;
    let returncodes = value
        .get("returncodes")
        .and_then(serde_json::Value::as_object)?;
    let stdout_map = value.get("stdout").and_then(serde_json::Value::as_object);
    let stderr_map = value.get("stderr").and_then(serde_json::Value::as_object);
    if stdout_map.is_none() && stderr_map.is_none() {
        return None;
    }

    let ok = value
        .get("ok")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or_else(|| {
            returncodes
                .values()
                .all(|code| code.as_i64().unwrap_or(1) == 0)
        });
    let mut names = returncodes.keys().cloned().collect::<Vec<_>>();
    names.sort();

    let mut output_lines = vec![format!("verify.ps1 ok: {ok}")];
    output_lines.push(format!(
        "returncodes: {}",
        names
            .iter()
            .map(|name| format!(
                "{}={}",
                name,
                returncodes
                    .get(name)
                    .and_then(serde_json::Value::as_i64)
                    .unwrap_or(0)
            ))
            .collect::<Vec<_>>()
            .join(", ")
    ));

    if ok {
        return Some((output_lines.join("\n"), String::new()));
    }

    let mut failure_blocks = Vec::new();
    for name in names {
        let code = returncodes
            .get(&name)
            .and_then(serde_json::Value::as_i64)
            .unwrap_or(0);
        if code == 0 {
            output_lines.push(format!("{name}: passed"));
            continue;
        }
        for (label, map) in [("stdout", stdout_map), ("stderr", stderr_map)] {
            let Some(text) = map
                .and_then(|map| map.get(&name))
                .and_then(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|text| !text.is_empty())
            else {
                continue;
            };
            failure_blocks.push(format!(
                "{name} {label}:\n{}",
                formatted_truncate_text(text, COMMAND_RUN_RESULT_OUTPUT_MAX_TOKENS)
            ));
        }
    }

    Some((output_lines.join("\n"), failure_blocks.join("\n\n")))
}

fn structured_command_line_as_cli(command: &str, command_line: &str) -> Option<String> {
    let trimmed = command_line.trim();
    if !(trimmed.starts_with('{') || trimmed.starts_with('[')) {
        return None;
    }
    let value = serde_json::from_str::<serde_json::Value>(trimmed).ok()?;
    let item = match value {
        serde_json::Value::Array(items) => items.into_iter().next()?,
        other => other,
    };
    let command = normalized_command_run_subcommand(command);
    let path = json_string_field(&item, &["path", "file_path", "filePath"]);
    match command.as_str() {
        "read_line" | "cat" => {
            let path = path?;
            let start =
                json_usize_field(&item, "start_line").or_else(|| json_usize_field(&item, "line"));
            let end = json_usize_field(&item, "end_line").or(start);
            match (start, end) {
                (Some(start), Some(end)) if start != 1 || end != usize::MAX => Some(format!(
                    "sed -n '{}{}p' {}",
                    start,
                    if start == end {
                        String::new()
                    } else {
                        format!(",{end}")
                    },
                    shell_quote(&path)
                )),
                _ => Some(format!("cat {}", shell_quote(&path))),
            }
        }
        "read_block" | "sed" => {
            let path = path?;
            let start = json_usize_field(&item, "start_line")
                .or_else(|| json_usize_field(&item, "line"))
                .unwrap_or(1);
            let end = json_usize_field(&item, "end_line").unwrap_or(start);
            Some(format!(
                "sed -n '{}{}p' {}",
                start,
                if start == end {
                    String::new()
                } else {
                    format!(",{end}")
                },
                shell_quote(&path)
            ))
        }
        "rg" | "grep" => {
            let query = json_string_field(&item, &["query", "pattern"]).unwrap_or_default();
            let directory =
                json_string_field(&item, &["directory", "path"]).unwrap_or_else(|| ".".to_string());
            let mut parts = vec![if command == "grep" {
                "grep".to_string()
            } else {
                "rg".to_string()
            }];
            if command == "rg" {
                parts.push("-n".to_string());
            } else {
                parts.push("-R".to_string());
            }
            if !json_bool_field(&item, "case_sensitive").unwrap_or(false) {
                parts.push("-i".to_string());
            }
            if command == "rg" && !json_bool_field(&item, "use_regex").unwrap_or(false) {
                parts.push("--fixed-strings".to_string());
            }
            if let Some(glob) = json_string_field(&item, &["file_glob", "glob"]) {
                if command == "rg" {
                    parts.push("-g".to_string());
                    parts.push(shell_quote(&glob));
                } else {
                    parts.push("--include".to_string());
                    parts.push(shell_quote(&glob));
                }
            }
            parts.push(shell_quote(&query));
            parts.push(shell_quote(&directory));
            Some(parts.join(" "))
        }
        "glob" | "find" => {
            let directory =
                json_string_field(&item, &["directory", "path"]).unwrap_or_else(|| ".".to_string());
            let pattern = json_string_field(&item, &["pattern", "glob"])
                .unwrap_or_else(|| "**/*".to_string());
            let file_type = if json_bool_field(&item, "include_directories").unwrap_or(false) {
                String::new()
            } else {
                " -type f".to_string()
            };
            Some(format!(
                "find {}{} -path {}",
                shell_quote(&directory),
                file_type,
                shell_quote(&pattern)
            ))
        }
        "write_file" => path.map(|path| format!("cat > {}", shell_quote(&path))),
        _ => None,
    }
}

fn is_structured_code_read_command(command: &str) -> bool {
    matches!(
        command,
        "cat"
            | "sed"
            | "read_line"
            | "read_block"
            | "rg"
            | "grep"
            | "find"
            | "glob"
            | "get_file_outline"
            | "find_definition"
            | "find_references"
    )
}

fn structured_stdout_as_cli_streams(command: &str, stdout: &str) -> Option<(String, String)> {
    let value = serde_json::from_str::<serde_json::Value>(stdout).ok()?;
    let results = value
        .get("results")
        .and_then(|results| results.as_array())?;
    let command = normalized_command_run_subcommand(command);
    let mut blocks = Vec::new();
    let mut stderr = command_run_structured_diagnostics(&value);
    for result in results {
        stderr.extend(command_run_result_diagnostics(result));
        match command.as_str() {
            "read_line" | "read_block" | "cat" | "sed" => {
                if let Some(content) = result.get("content").and_then(serde_json::Value::as_str) {
                    blocks.push(content.trim_end().to_string());
                }
            }
            "rg" | "grep" | "find_definition" | "find_references" => {
                if let Some(matches) = result.get("matches").and_then(serde_json::Value::as_array) {
                    let lines = matches
                        .iter()
                        .filter_map(command_run_match_as_cli_line)
                        .collect::<Vec<_>>();
                    if !lines.is_empty() {
                        blocks.push(lines.join("\n"));
                    }
                }
            }
            "glob" | "find" => {
                if let Some(paths) = result
                    .get("matched_paths")
                    .and_then(serde_json::Value::as_array)
                {
                    let lines = paths
                        .iter()
                        .filter_map(|path| path.as_str().map(str::to_string))
                        .collect::<Vec<_>>();
                    if !lines.is_empty() {
                        blocks.push(lines.join("\n"));
                    }
                }
            }
            "get_file_outline" => {
                if let Some(outline) = result.get("outline").and_then(serde_json::Value::as_array) {
                    let path = result
                        .get("path")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or_default();
                    let lines = outline
                        .iter()
                        .filter_map(|item| command_run_outline_as_cli_line(path, item))
                        .collect::<Vec<_>>();
                    if !lines.is_empty() {
                        blocks.push(lines.join("\n"));
                    }
                }
            }
            "apply_patch" | "apply_diff" | "write_file" | "delete_file" => {
                if let Some(summary) = result
                    .get("summary_markdown")
                    .and_then(serde_json::Value::as_str)
                {
                    blocks.push(summary.trim_end().to_string());
                } else if let Some(line) = command_run_mutation_result_as_cli_line(result) {
                    blocks.push(line);
                }
            }
            _ => {
                if let Some(summary) = result
                    .get("summary_markdown")
                    .and_then(serde_json::Value::as_str)
                {
                    blocks.push(summary.trim_end().to_string());
                } else if let Some(content) =
                    result.get("content").and_then(serde_json::Value::as_str)
                {
                    blocks.push(content.trim_end().to_string());
                }
            }
        }
    }
    if blocks.is_empty() && stderr.is_empty() {
        return None;
    }
    Some((blocks.join("\n\n"), stderr.join("\n")))
}

fn normalized_command_run_subcommand(command: &str) -> String {
    let command = command
        .trim()
        .rsplit([':', '/'])
        .next()
        .unwrap_or(command)
        .trim()
        .to_ascii_lowercase()
        .replace('-', "_");
    match command.as_str() {
        "type" | "get_content" => "read_line".to_string(),
        "cat" => "cat".to_string(),
        "sed" => "sed".to_string(),
        "read_line" => "read_line".to_string(),
        "read_block" => "read_block".to_string(),
        "ripgrep" => "rg".to_string(),
        "grep" => "grep".to_string(),
        "rg" => "rg".to_string(),
        "find" => "find".to_string(),
        "glob" => "glob".to_string(),
        "outline" | "symbols" => "get_file_outline".to_string(),
        "definition" => "find_definition".to_string(),
        "references" => "find_references".to_string(),
        "patch" | "applypatch" => "apply_patch".to_string(),
        other => other.to_string(),
    }
}

fn command_run_match_as_cli_line(value: &serde_json::Value) -> Option<String> {
    let path = value.get("path").and_then(serde_json::Value::as_str)?;
    let content = value
        .get("content")
        .or_else(|| value.get("line"))
        .or_else(|| value.get("text"))
        .and_then(serde_json::Value::as_str);
    let line = value
        .get("line")
        .or_else(|| value.get("line_number"))
        .and_then(serde_json::Value::as_u64);
    match (line, content) {
        (Some(line), Some(content)) => Some(format!("{path}:{line}:{}", content.trim_end())),
        (_, Some(content)) => Some(format!("{path}:{}", content.trim_end())),
        _ => Some(path.to_string()),
    }
}

fn command_run_outline_as_cli_line(path: &str, value: &serde_json::Value) -> Option<String> {
    let name = value.get("name").and_then(serde_json::Value::as_str)?;
    let kind = value
        .get("kind")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("symbol");
    let line = value
        .get("line")
        .or_else(|| value.get("line_number"))
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    if line > 0 {
        Some(format!("{path}:{line}:{kind} {name}"))
    } else {
        Some(format!("{path}:{kind} {name}"))
    }
}

fn command_run_mutation_result_as_cli_line(value: &serde_json::Value) -> Option<String> {
    let path = value
        .get("path")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    if let Some(error) = value
        .get("error")
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.is_empty())
    {
        return Some(if path.is_empty() {
            error.to_string()
        } else {
            format!("{path}: {error}")
        });
    }
    if value.get("applied").and_then(serde_json::Value::as_bool) == Some(true) {
        return Some(if path.is_empty() {
            "Applied patch.".to_string()
        } else {
            format!("{path}: applied")
        });
    }
    if value.get("success").and_then(serde_json::Value::as_bool) == Some(true) {
        return Some(if path.is_empty() {
            "Wrote file.".to_string()
        } else {
            format!("{path}: wrote file")
        });
    }
    if value.get("deleted").and_then(serde_json::Value::as_bool) == Some(true) {
        return Some(if path.is_empty() {
            "Deleted file.".to_string()
        } else {
            format!("{path}: deleted")
        });
    }
    None
}

fn command_run_structured_diagnostics(value: &serde_json::Value) -> Vec<String> {
    ["errors", "warnings"]
        .into_iter()
        .filter_map(|field| value.get(field).and_then(serde_json::Value::as_array))
        .flat_map(|items| items.iter().filter_map(command_run_diagnostic_line))
        .collect()
}

fn command_run_result_diagnostics(value: &serde_json::Value) -> Vec<String> {
    let mut lines = command_run_structured_diagnostics(value);
    if let Some(error) = value
        .get("error")
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.is_empty())
    {
        lines.push(error.to_string());
    }
    lines
}

fn command_run_diagnostic_line(value: &serde_json::Value) -> Option<String> {
    if let Some(text) = value.as_str() {
        return Some(text.to_string());
    }
    let message = value.get("message").and_then(serde_json::Value::as_str)?;
    let path = value
        .get("path")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    let code = value
        .get("code")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    Some(match (path.is_empty(), code.is_empty()) {
        (true, true) => message.to_string(),
        (false, true) => format!("{path}: {message}"),
        (true, false) => format!("{code}: {message}"),
        (false, false) => format!("{path}: {code}: {message}"),
    })
}

fn json_string_field(value: &serde_json::Value, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(serde_json::Value::as_str))
        .map(str::to_string)
}

fn json_bool_field(value: &serde_json::Value, key: &str) -> Option<bool> {
    value.get(key).and_then(serde_json::Value::as_bool)
}

fn json_usize_field(value: &serde_json::Value, key: &str) -> Option<usize> {
    value
        .get(key)
        .and_then(serde_json::Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())
}

fn shell_quote(value: &str) -> String {
    if value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '/' | '\\' | '_' | '-' | ':'))
    {
        return value.to_string();
    }
    format!("'{}'", value.replace('\'', "'\\''"))
}
