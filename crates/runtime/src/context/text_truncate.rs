//! Context-text truncation helpers: token budget, section-/query-/ripgrep-
//! grouped truncation, and character-boundary safe slicing.
//!
//! Pure text-processing layer carved out of `context_management.rs`; no
//! external state. Exposed only inside `context::*` via `pub(super)`.

pub(super) const CONTEXT_OUTPUT_MAX_TOKENS: usize = 2_500;
pub(super) const COMMAND_RUN_RESULT_OUTPUT_MAX_TOKENS: usize = 2_500;
pub(super) const APPROX_CHARS_PER_TOKEN: usize = 4;

pub(super) fn truncate_text_to_token_budget(text: &str, max_tokens: usize) -> String {
    let max_chars = max_tokens.saturating_mul(APPROX_CHARS_PER_TOKEN);
    if text.len() <= max_chars {
        return text.to_string();
    }
    let mut out = text.chars().take(max_chars).collect::<String>();
    out.push_str("\n\n[context checkpoint truncated to about 20,000 tokens]");
    out
}

pub(super) fn environment_context_message(cwd: &std::path::Path) -> String {
    format!(
        "<environment_context>\n  <cwd>{}</cwd>\n  <shell>{}</shell>\n  <current_date>{}</current_date>\n  <timezone>{}</timezone>\n</environment_context>",
        cwd.display(),
        context_shell_name(),
        chrono::Local::now().format("%Y-%m-%d"),
        std::env::var("TZ").unwrap_or_else(|_| "Europe/Paris".to_string())
    )
}

fn context_shell_name() -> &'static str {
    match std::env::var("TURA_COMMAND_RUN_SHELL")
        .ok()
        .map(|value| value.trim().to_ascii_lowercase())
        .as_deref()
    {
        Some("bash") => "bash",
        Some("shell") | Some("shell_command") | Some("shll") | Some("shall") => {
            if cfg!(windows) {
                "powershell"
            } else {
                "bash"
            }
        }
        _ if cfg!(windows) => "powershell",
        _ => "bash",
    }
}

pub(super) fn context_output_byte_budget() -> usize {
    CONTEXT_OUTPUT_MAX_TOKENS * APPROX_CHARS_PER_TOKEN
}

pub(super) fn formatted_truncate_text(content: &str, max_tokens: usize) -> String {
    if content.len() <= max_tokens * APPROX_CHARS_PER_TOKEN {
        return content.to_string();
    }
    let total_lines = content.lines().count();
    let truncated = truncate_middle_with_token_budget(content, max_tokens);
    format!("Total output lines: {total_lines}\n\n{truncated}")
}

pub(super) fn command_run_truncate_text(
    content: &str,
    max_tokens: usize,
    command_line: Option<&str>,
) -> String {
    let effective_max_tokens = command_run_effective_max_tokens(max_tokens, command_line);
    if content.len() <= effective_max_tokens * APPROX_CHARS_PER_TOKEN {
        return content.to_string();
    }
    truncate_marker_sections_for_command_run(content, effective_max_tokens, command_line)
        .or_else(|| {
            truncate_query_sections_for_command_run(content, effective_max_tokens, command_line)
        })
        .or_else(|| truncate_ripgrep_file_sections_for_command_run(content, effective_max_tokens))
        .unwrap_or_else(|| formatted_truncate_text(content, effective_max_tokens))
}

fn command_run_effective_max_tokens(max_tokens: usize, command_line: Option<&str>) -> usize {
    let Some(command_line) = command_line else {
        return max_tokens;
    };
    if extract_read_targets(command_line).len() == 1 {
        max_tokens.saturating_mul(3)
    } else {
        max_tokens
    }
}

fn truncate_marker_sections_for_command_run(
    content: &str,
    max_tokens: usize,
    command_line: Option<&str>,
) -> Option<String> {
    let mut preamble = String::new();
    let mut sections = Vec::<String>::new();
    let mut current = String::new();
    let mut bare_file_marker_index = 0usize;
    let mut saw_bare_file_marker = false;
    let read_targets = command_line.map(extract_read_targets).unwrap_or_default();

    for chunk in content.split_inclusive('\n') {
        if is_command_run_section_marker(chunk) {
            if chunk.trim_end_matches(['\r', '\n']) == "---FILE---" {
                saw_bare_file_marker = true;
            }
            let chunk = rewrite_bare_file_marker(chunk, &read_targets, &mut bare_file_marker_index);
            if current.is_empty() {
                current.push_str(&chunk);
            } else {
                sections.push(std::mem::take(&mut current));
                current.push_str(&chunk);
            }
            continue;
        }

        if current.is_empty() {
            preamble.push_str(chunk);
        } else {
            current.push_str(chunk);
        }
    }

    if !current.is_empty() {
        sections.push(current);
    }

    if saw_bare_file_marker && !read_targets.is_empty() {
        split_first_bare_file_section(&mut preamble, &mut sections, &read_targets[0]);
    }

    if sections.is_empty() {
        return None;
    }

    let mut output = String::new();
    if !preamble.is_empty() {
        output.push_str(&formatted_truncate_text(&preamble, max_tokens));
        if !output.ends_with('\n') {
            output.push('\n');
        }
    }

    for section in sections {
        output.push_str(&formatted_truncate_text(&section, max_tokens));
        if !output.ends_with('\n') {
            output.push('\n');
        }
    }

    Some(output)
}

fn split_first_bare_file_section(
    preamble: &mut String,
    sections: &mut Vec<String>,
    first_path: &str,
) {
    let Some(output_marker) = preamble.rfind("Output:\n") else {
        return;
    };
    let file_body_start = output_marker + "Output:\n".len();
    if file_body_start >= preamble.len() {
        return;
    }
    let header = preamble[..file_body_start].to_string();
    let first_body = preamble[file_body_start..].to_string();
    *preamble = header;
    sections.insert(0, format!("---FILE--- {first_path}\n{first_body}"));
}

fn rewrite_bare_file_marker(
    line: &str,
    read_targets: &[String],
    bare_file_marker_index: &mut usize,
) -> String {
    let trimmed = line.trim_end_matches(['\r', '\n']);
    if trimmed != "---FILE---" {
        return line.to_string();
    }
    let target_index = bare_file_marker_index.saturating_add(1);
    *bare_file_marker_index = bare_file_marker_index.saturating_add(1);
    let Some(path) = read_targets.get(target_index) else {
        return line.to_string();
    };
    let newline = if line.ends_with("\r\n") {
        "\r\n"
    } else if line.ends_with('\n') {
        "\n"
    } else {
        ""
    };
    format!("---FILE--- {path}{newline}")
}

fn is_command_run_section_marker(line: &str) -> bool {
    let trimmed = line.trim_end_matches(['\r', '\n']);
    if let Some(path) = trimmed.strip_prefix("---FILE--- ") {
        return !path.trim().is_empty();
    }
    if trimmed == "---FILE---" {
        return true;
    }
    if !trimmed.starts_with("---") {
        return false;
    }
    let Some(rest) = trimmed.strip_prefix("---") else {
        return false;
    };
    let Some(label_end) = rest.find("---") else {
        return false;
    };
    if label_end == 0 {
        return false;
    }
    let label = &rest[..label_end];
    if !label
        .chars()
        .all(|ch| ch.is_ascii_uppercase() || ch.is_ascii_digit() || ch == '_' || ch == ' ')
    {
        return false;
    }
    rest[label_end + 3..]
        .chars()
        .next()
        .is_some_and(char::is_whitespace)
}

fn extract_read_targets(command_line: &str) -> Vec<String> {
    let mut targets = Vec::new();
    let command_text = shell_command_text_for_read_targets(command_line)
        .unwrap_or_else(|| command_line.trim().to_string());
    let tokens = shell_like_tokens(&command_text);
    let mut index = 0usize;
    while index < tokens.len() {
        let token = tokens[index].to_ascii_lowercase();
        if (token == "get-content" || token == "gc" || token == "cat" || token == "type")
            && index + 1 < tokens.len()
        {
            let mut next = index + 1;
            while next < tokens.len() && tokens[next].starts_with('-') {
                next += 1;
            }
            if let Some(path) = tokens
                .get(next)
                .and_then(|value| normalize_read_target(value))
            {
                if !targets.iter().any(|existing| existing == &path) {
                    targets.push(path);
                }
            }
            index = next;
        }
        index += 1;
    }
    targets
}

fn shell_command_text_for_read_targets(command_line: &str) -> Option<String> {
    fn parse_candidate(candidate: &str, depth: usize) -> Option<String> {
        if depth > 3 {
            return None;
        }
        let value = serde_json::from_str::<serde_json::Value>(candidate).ok()?;
        match value {
            serde_json::Value::String(inner) => {
                parse_candidate(inner.trim(), depth + 1).or_else(|| Some(inner.trim().to_string()))
            }
            serde_json::Value::Object(object) => object
                .get("command")
                .or_else(|| object.get("cmd"))
                .or_else(|| object.get("command_line"))
                .and_then(serde_json::Value::as_str)
                .map(|value| value.trim().to_string()),
            _ => None,
        }
    }

    let trimmed = command_line.trim();
    parse_candidate(trimmed, 0).or_else(|| {
        if trimmed.contains("\\\"") {
            parse_candidate(&trimmed.replace("\\\"", "\""), 0)
        } else {
            None
        }
    })
}

fn normalize_read_target(value: &str) -> Option<String> {
    let trimmed = value
        .trim()
        .trim_matches(';')
        .trim_matches(',')
        .trim_matches('"')
        .trim_matches('\'');
    if trimmed.is_empty() || trimmed.starts_with('$') || trimmed.starts_with('|') {
        return None;
    }
    if !(trimmed.contains('/') || trimmed.contains('\\') || trimmed.contains('.')) {
        return None;
    }
    Some(trimmed.replace('\\', "/"))
}

fn shell_like_tokens(value: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut quote = None::<char>;
    let mut escaped = false;
    for ch in value.chars() {
        if escaped {
            current.push(ch);
            escaped = false;
            continue;
        }
        if ch == '\\' && quote.is_some() {
            escaped = true;
            continue;
        }
        if let Some(active) = quote {
            if ch == active {
                quote = None;
            } else {
                current.push(ch);
            }
            continue;
        }
        if ch == '"' || ch == '\'' {
            quote = Some(ch);
        } else if ch.is_whitespace() || ch == ';' {
            if !current.is_empty() {
                tokens.push(std::mem::take(&mut current));
            }
        } else {
            current.push(ch);
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    tokens
}

fn truncate_query_sections_for_command_run(
    content: &str,
    max_tokens: usize,
    command_line: Option<&str>,
) -> Option<String> {
    let terms = extract_query_terms(command_line?);
    if terms.len() < 2 {
        return None;
    }

    let mut preamble = String::new();
    let mut sections = terms
        .iter()
        .map(|term| {
            (
                term.to_string(),
                format!("---QUERY--- {term}\n"),
                term.to_ascii_lowercase(),
            )
        })
        .collect::<Vec<_>>();

    for line in content.split_inclusive('\n') {
        let lower = line.to_ascii_lowercase();
        if let Some((_, section, _)) = sections
            .iter_mut()
            .find(|(_, _, term)| lower.contains(term.as_str()))
        {
            section.push_str(line);
        } else {
            preamble.push_str(line);
        }
    }

    if sections
        .iter()
        .all(|(_, section, _)| section.lines().count() <= 1)
    {
        return None;
    }

    let mut output = String::new();
    if !preamble.trim().is_empty() {
        output.push_str(&formatted_truncate_text(&preamble, max_tokens));
        if !output.ends_with('\n') {
            output.push('\n');
        }
    }
    for (_, section, _) in sections {
        output.push_str(&formatted_truncate_text(&section, max_tokens));
        if !output.ends_with('\n') {
            output.push('\n');
        }
    }
    Some(output)
}

fn extract_query_terms(command_line: &str) -> Vec<String> {
    let lower = command_line.to_ascii_lowercase();
    if !(lower.contains("rg ") || lower.contains("ripgrep") || lower.contains("select-string")) {
        return Vec::new();
    }

    let mut terms = Vec::new();
    for quoted in quoted_fragments(command_line) {
        let candidates = if quoted.contains('|') {
            quoted.split('|').collect::<Vec<_>>()
        } else if should_split_space_separated_query(&quoted) {
            quoted.split_whitespace().collect::<Vec<_>>()
        } else {
            vec![quoted.as_str()]
        };
        for candidate in candidates {
            let term = normalize_query_term(candidate);
            if is_query_term(&term) && !terms.iter().any(|existing| existing == &term) {
                terms.push(term);
            }
        }
    }
    terms
}

fn should_split_space_separated_query(value: &str) -> bool {
    let parts = value.split_whitespace().collect::<Vec<_>>();
    parts.len() >= 2
        && parts
            .iter()
            .all(|part| is_query_term(&normalize_query_term(part)))
}

fn quoted_fragments(value: &str) -> Vec<String> {
    let mut fragments = Vec::new();
    let mut current = String::new();
    let mut quote = None::<char>;
    let mut escaped = false;
    for ch in value.chars() {
        if escaped {
            current.push(ch);
            escaped = false;
            continue;
        }
        if ch == '\\' {
            if quote.is_some() {
                escaped = true;
            }
            continue;
        }
        if let Some(active) = quote {
            if ch == active {
                fragments.push(std::mem::take(&mut current));
                quote = None;
            } else {
                current.push(ch);
            }
        } else if ch == '"' || ch == '\'' {
            quote = Some(ch);
        }
    }
    fragments
}

fn normalize_query_term(value: &str) -> String {
    value
        .trim()
        .trim_matches('(')
        .trim_matches(')')
        .replace("\\b", "")
        .replace("\\s+", " ")
        .replace(".*", "")
        .trim()
        .to_string()
}

fn is_query_term(value: &str) -> bool {
    let len = value.chars().count();
    (1..=80).contains(&len)
        && value
            .chars()
            .any(|ch| ch.is_ascii_alphanumeric() || ch == '_')
        && !value.contains("**/")
        && !value.contains("*.")
}

fn truncate_ripgrep_file_sections_for_command_run(
    content: &str,
    max_tokens: usize,
) -> Option<String> {
    let mut preamble = String::new();
    let mut sections = Vec::<(String, String)>::new();

    for line in content.split_inclusive('\n') {
        if let Some(path) = ripgrep_result_path(line) {
            if let Some((_, section)) = sections.iter_mut().find(|(existing, _)| existing == &path)
            {
                section.push_str(line);
            } else {
                sections.push((path.clone(), format!("---MATCHES--- {path}\n{line}")));
            }
        } else {
            preamble.push_str(line);
        }
    }

    if sections.len() < 2 {
        return None;
    }

    let mut output = String::new();
    if !preamble.trim().is_empty() {
        output.push_str(&formatted_truncate_text(&preamble, max_tokens));
        if !output.ends_with('\n') {
            output.push('\n');
        }
    }
    for (_, section) in sections {
        output.push_str(&formatted_truncate_text(&section, max_tokens));
        if !output.ends_with('\n') {
            output.push('\n');
        }
    }
    Some(output)
}

fn ripgrep_result_path(line: &str) -> Option<String> {
    let trimmed = line.trim_end_matches(['\r', '\n']);
    let (path, rest) = trimmed.split_once(':')?;
    if path.is_empty() || !path.contains('.') {
        return None;
    }
    let line_number = rest.split_once(':').map(|(line, _)| line).unwrap_or(rest);
    if line_number.is_empty() || !line_number.chars().all(|ch| ch.is_ascii_digit()) {
        return None;
    }
    Some(path.replace('\\', "/"))
}

fn truncate_middle_with_token_budget(content: &str, max_tokens: usize) -> String {
    let max_chars = max_tokens.saturating_mul(APPROX_CHARS_PER_TOKEN);
    if content.len() <= max_chars {
        return content.to_string();
    }
    if max_chars == 0 {
        return format!("…{} tokens truncated…", approx_token_count(content.len()));
    }

    let marker_budget = 32usize;
    let visible_budget = max_chars.saturating_sub(marker_budget).max(2);
    let head_budget = visible_budget / 2;
    let tail_budget = visible_budget.saturating_sub(head_budget);
    let head_end = byte_floor_char_boundary(content, head_budget);
    let tail_start = byte_ceil_char_boundary(content, content.len().saturating_sub(tail_budget));
    let removed = tail_start.saturating_sub(head_end);
    let removed_tokens = approx_token_count(removed);
    format!(
        "{}…{} tokens truncated…{}",
        &content[..head_end],
        removed_tokens,
        &content[tail_start..]
    )
}

fn approx_token_count(byte_count: usize) -> usize {
    byte_count.div_ceil(APPROX_CHARS_PER_TOKEN)
}

fn byte_floor_char_boundary(text: &str, target: usize) -> usize {
    if target >= text.len() {
        return text.len();
    }
    let mut index = target;
    while index > 0 && !text.is_char_boundary(index) {
        index -= 1;
    }
    index
}

fn byte_ceil_char_boundary(text: &str, target: usize) -> usize {
    if target >= text.len() {
        return text.len();
    }
    let mut index = target;
    while index < text.len() && !text.is_char_boundary(index) {
        index += 1;
    }
    index
}
