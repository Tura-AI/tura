//! Context-text truncation helpers: section-/query-/ripgrep-grouped truncation.
//!
//! Pure text-processing layer with no external state. Exposed only inside
//! `context::*` via `pub(super)`.

use super::char_budget::formatted_truncate_text;
use crate::prompt_style::context_blocks;

pub(super) fn environment_context_message(cwd: &std::path::Path) -> String {
    let timezone = std::env::var("TZ").unwrap_or_else(|_| "Europe/Paris".to_string());
    let system_language = session_language();
    context_blocks::environment_context(
        cwd,
        context_shell_name(),
        chrono::Local::now().format("%Y-%m-%d"),
        &timezone,
        &system_language,
    )
}

fn session_language() -> String {
    std::env::var("TURA_SESSION_LANGUAGE")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "en".to_string())
}

fn context_shell_name() -> &'static str {
    match std::env::var("TURA_COMMAND_RUN_SHELL")
        .ok()
        .map(|value| value.trim().to_ascii_lowercase())
        .as_deref()
    {
        Some("bash") => "bash",
        Some("zsh") => "zsh",
        Some("shell") | Some("shell_command") | Some("shll") | Some("shall") => {
            if cfg!(windows) {
                "powershell"
            } else if cfg!(target_os = "macos") {
                "zsh"
            } else {
                "bash"
            }
        }
        _ if cfg!(windows) => "powershell",
        _ if cfg!(target_os = "macos") => "zsh",
        _ => "bash",
    }
}

pub(super) fn command_run_truncate_text(
    content: &str,
    max_chars: usize,
    command_line: Option<&str>,
) -> String {
    let effective_max_chars = command_run_effective_max_chars(max_chars, command_line);
    if content.len() <= effective_max_chars {
        return content.to_string();
    }
    truncate_marker_sections_for_command_run(content, effective_max_chars, command_line)
        .or_else(|| {
            truncate_query_sections_for_command_run(content, effective_max_chars, command_line)
        })
        .or_else(|| truncate_ripgrep_file_sections_for_command_run(content, effective_max_chars))
        .unwrap_or_else(|| formatted_truncate_text(content, effective_max_chars))
}

fn command_run_effective_max_chars(max_chars: usize, command_line: Option<&str>) -> usize {
    let Some(command_line) = command_line else {
        return max_chars;
    };
    if extract_read_targets(command_line).len() == 1 {
        max_chars.saturating_mul(3)
    } else {
        max_chars
    }
}

fn truncate_marker_sections_for_command_run(
    content: &str,
    max_chars: usize,
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
        output.push_str(&formatted_truncate_text(&preamble, max_chars));
        if !output.ends_with('\n') {
            output.push('\n');
        }
    }

    for section in sections {
        output.push_str(&formatted_truncate_section_body(&section, max_chars));
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
            while next < tokens.len() {
                if tokens[next] == ";" || tokens[next] == "|" {
                    break;
                }
                if tokens[next].starts_with('-') {
                    next += 1;
                    continue;
                }
                if let Some(path) = normalize_read_target(&tokens[next]) {
                    if !targets.iter().any(|existing| existing == &path) {
                        targets.push(path);
                    }
                }
                next += 1;
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
    for ch in value.chars() {
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
            if ch == ';' {
                tokens.push(";".to_string());
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
    max_chars: usize,
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
        output.push_str(&formatted_truncate_text(&preamble, max_chars));
        if !output.ends_with('\n') {
            output.push('\n');
        }
    }
    for (_, section, _) in sections {
        output.push_str(&formatted_truncate_section_body(&section, max_chars));
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
    max_chars: usize,
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
        output.push_str(&formatted_truncate_text(&preamble, max_chars));
        if !output.ends_with('\n') {
            output.push('\n');
        }
    }
    for (_, section) in sections {
        output.push_str(&formatted_truncate_section_body(&section, max_chars));
        if !output.ends_with('\n') {
            output.push('\n');
        }
    }
    Some(output)
}

fn formatted_truncate_section_body(section: &str, max_chars: usize) -> String {
    let Some((header, body)) = section.split_once('\n') else {
        return formatted_truncate_text(section, max_chars);
    };
    if !is_command_run_section_marker(&format!("{header}\n")) {
        return formatted_truncate_text(section, max_chars);
    }
    if body.is_empty() {
        return section.to_string();
    }
    let body = formatted_truncate_text(body, max_chars);
    format!("{header}\n{body}")
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

#[cfg(test)]
mod tests {
    use super::{
        extract_query_terms, extract_read_targets, ripgrep_result_path, shell_like_tokens,
    };

    #[test]
    fn marker_truncation_rewrites_bare_file_sections_from_read_targets() {
        let content = "Output:\nfirst-file-body-line-one\nfirst-file-body-line-two\n---FILE---\nsecond-file-body-line-one\nsecond-file-body-line-two\n";
        let command_line = r#"cat src/first.rs src/second.rs"#;

        let output =
            super::truncate_marker_sections_for_command_run(content, 20, Some(command_line))
                .expect("marker sections should be recognized");

        assert!(
            output.contains("---FILE--- src/first.rs"),
            "first bare section should be labelled from the first read target: {output}"
        );
        assert!(
            output.contains("---FILE--- src/second.rs"),
            "second bare section should be labelled from the second read target: {output}"
        );
    }

    #[test]
    fn query_truncation_groups_multi_term_search_output() {
        let content =
            "intro\nalpha result line one\nnoise\nbeta result line one\nbeta result line two\n";
        let command_line = r#"rg "alpha beta" crates/runtime"#;

        let output =
            super::truncate_query_sections_for_command_run(content, 12, Some(command_line))
                .expect("query sections should be recognized");

        assert!(output.contains("---QUERY--- alpha"), "{output}");
        assert!(output.contains("---QUERY--- beta"), "{output}");
        assert!(output.contains("intro"), "{output}");
    }

    #[test]
    fn ripgrep_truncation_groups_matches_by_file() {
        let content = "searching\nsrc/a.rs:10:alpha\nsrc/b.rs:20:beta\nsrc/a.rs:11:alpha again\n";

        let output = super::truncate_ripgrep_file_sections_for_command_run(content, 14)
            .expect("ripgrep file sections should be recognized");

        assert!(output.contains("---MATCHES--- src/a.rs"), "{output}");
        assert!(output.contains("---MATCHES--- src/b.rs"), "{output}");
        assert!(output.contains("searching"), "{output}");
    }

    #[test]
    fn read_targets_are_extracted_from_json_wrapped_commands() {
        let command_line = r#"{"command":"Get-Content -LiteralPath 'src\\main.rs'; cat crates/runtime/src/lib.rs"}"#;

        let targets = extract_read_targets(command_line);

        assert_eq!(
            targets,
            vec![
                "src/main.rs".to_string(),
                "crates/runtime/src/lib.rs".to_string()
            ]
        );
    }

    #[test]
    fn shell_like_tokens_preserve_quoted_paths_and_split_semicolon_commands() {
        let tokens = shell_like_tokens(r#"cat "src/main.rs"; rg 'hello world' crates/runtime"#);

        assert_eq!(
            tokens,
            vec![
                "cat".to_string(),
                "src/main.rs".to_string(),
                ";".to_string(),
                "rg".to_string(),
                "hello world".to_string(),
                "crates/runtime".to_string()
            ]
        );
    }

    #[test]
    fn query_terms_reject_globs_and_keep_distinct_terms() {
        let terms = extract_query_terms(r#"rg "alpha|**/*.rs|beta|*.md|alpha" ."#);

        assert_eq!(terms, vec!["alpha".to_string(), "beta".to_string()]);
    }

    #[test]
    fn ripgrep_result_path_accepts_windows_paths_and_rejects_non_matches() {
        assert_eq!(
            ripgrep_result_path(r#"src\main.rs:12:fn main()"#).as_deref(),
            Some("src/main.rs")
        );
        assert_eq!(ripgrep_result_path("not-a-match"), None);
        assert_eq!(ripgrep_result_path("README:abc:text"), None);
    }
}
