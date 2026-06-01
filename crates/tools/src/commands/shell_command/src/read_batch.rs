pub(super) fn space_batched_read_command(command: &str, use_bash: bool) -> Option<String> {
    if command.contains('\n') {
        return None;
    }
    let parts = split_shell_sequence(command)?;
    let parsed = parts
        .iter()
        .map(|part| simple_read_command(part, use_bash))
        .collect::<Option<Vec<_>>>()?;
    let target_count = parsed
        .iter()
        .map(|command| command.targets.len())
        .sum::<usize>();
    if target_count < 2 {
        return None;
    }

    let mut spaced = Vec::with_capacity(target_count * 2);
    for command in parsed {
        for target in &command.targets {
            if !spaced.is_empty() {
                spaced.push(blank_line_command(use_bash).to_string());
            }
            spaced.push(command.command_for_target(target, use_bash));
        }
    }

    Some(spaced.join("; "))
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct SimpleReadCommand {
    prefix: Vec<String>,
    targets: Vec<String>,
}

fn split_shell_sequence(command: &str) -> Option<Vec<&str>> {
    let mut parts = Vec::new();
    let mut start = 0usize;
    let mut single_quoted = false;
    let mut double_quoted = false;
    let mut escaped = false;

    for (index, ch) in command.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if ch == '\\' && !single_quoted {
            escaped = true;
            continue;
        }
        match ch {
            '\'' if !double_quoted => single_quoted = !single_quoted,
            '"' if !single_quoted => double_quoted = !double_quoted,
            ';' if !single_quoted && !double_quoted => {
                let part = command[start..index].trim();
                if part.is_empty() {
                    return None;
                }
                parts.push(part);
                start = index + ch.len_utf8();
            }
            _ => {}
        }
    }

    if single_quoted || double_quoted {
        return None;
    }
    let tail = command[start..].trim();
    if tail.is_empty() {
        return None;
    }
    parts.push(tail);
    Some(parts)
}

fn simple_read_command(command: &str, use_bash: bool) -> Option<SimpleReadCommand> {
    if command
        .chars()
        .any(|ch| matches!(ch, '|' | '>' | '<' | '&' | '`' | '{' | '}'))
        || command.contains("$(")
    {
        return None;
    }
    let tokens = shell_words(command, use_bash)?;
    let cmd = tokens.first()?.to_ascii_lowercase();
    if !matches!(cmd.as_str(), "get-content" | "gc" | "cat" | "type") {
        return None;
    }

    let mut prefix = vec![tokens[0].clone()];
    let mut targets = Vec::new();
    let mut index = 1usize;
    while index < tokens.len() {
        let token = tokens[index].as_str();
        if token == "--" {
            prefix.push(token.to_string());
            index += 1;
            continue;
        }
        if use_bash && token.starts_with('-') {
            prefix.push(token.to_string());
            index += 1;
            continue;
        }
        if !use_bash && token.starts_with('-') {
            let option = token.to_ascii_lowercase();
            prefix.push(token.to_string());
            index += 1;
            if powershell_option_takes_value(&option) && index < tokens.len() {
                if powershell_path_option(&option) {
                    collect_read_targets(&tokens[index], &mut targets)?;
                } else {
                    prefix.push(tokens[index].clone());
                }
                index += 1;
            }
            continue;
        }
        collect_read_targets(token, &mut targets)?;
        index += 1;
    }

    (!targets.is_empty()).then_some(SimpleReadCommand { prefix, targets })
}

impl SimpleReadCommand {
    fn command_for_target(&self, target: &str, use_bash: bool) -> String {
        let mut tokens = self.prefix.clone();
        tokens.push(shell_quote_for_runtime(target, use_bash));
        tokens.join(" ")
    }
}

fn powershell_option_takes_value(option: &str) -> bool {
    matches!(
        option,
        "-path"
            | "-literalpath"
            | "-filepath"
            | "-totalcount"
            | "-head"
            | "-first"
            | "-tail"
            | "-last"
            | "-encoding"
            | "-readcount"
            | "-delimiter"
            | "-filter"
            | "-include"
            | "-exclude"
    )
}

fn powershell_path_option(option: &str) -> bool {
    matches!(option, "-path" | "-literalpath" | "-filepath")
}

fn collect_read_targets(token: &str, targets: &mut Vec<String>) -> Option<()> {
    for part in token.split(',') {
        let target = normalize_read_target_for_marker(part)?;
        targets.push(target);
    }
    Some(())
}

fn shell_words(command: &str, escape_backslash: bool) -> Option<Vec<String>> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut single_quoted = false;
    let mut double_quoted = false;
    let mut escaped = false;

    for ch in command.chars() {
        if escaped {
            current.push(ch);
            escaped = false;
            continue;
        }
        if escape_backslash && ch == '\\' && !single_quoted {
            escaped = true;
            continue;
        }
        match ch {
            '\'' if !double_quoted => single_quoted = !single_quoted,
            '"' if !single_quoted => double_quoted = !double_quoted,
            ch if ch.is_whitespace() && !single_quoted && !double_quoted => {
                if !current.is_empty() {
                    tokens.push(std::mem::take(&mut current));
                }
            }
            _ => current.push(ch),
        }
    }

    if escaped {
        current.push('\\');
    }
    if single_quoted || double_quoted {
        return None;
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    Some(tokens)
}

fn normalize_read_target_for_marker(value: &str) -> Option<String> {
    let target = value.trim().trim_matches(';').trim_matches(',');
    if target.is_empty() || target.starts_with('$') || target.starts_with('|') {
        return None;
    }
    if !(target.contains('/') || target.contains('\\') || target.contains('.')) {
        return None;
    }
    Some(target.to_string())
}

fn shell_quote_for_runtime(value: &str, use_bash: bool) -> String {
    if use_bash {
        format!("'{}'", sh_single_quote(value))
    } else {
        format!("'{}'", powershell_single_quote(value))
    }
}

fn blank_line_command(use_bash: bool) -> &'static str {
    if use_bash {
        "printf '\\n'"
    } else {
        "Write-Output ''"
    }
}

fn powershell_single_quote(value: &str) -> String {
    value.replace('\'', "''")
}

fn sh_single_quote(value: &str) -> String {
    value.replace('\'', "'\"'\"'")
}
