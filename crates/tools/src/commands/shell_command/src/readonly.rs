use std::path::Path;

use super::request::parse_shell_request;

pub fn looks_read_only(command_line: &str) -> bool {
    looks_read_only_with_root(command_line, Path::new("."))
}

pub(super) fn looks_read_only_with_root(command_line: &str, root: &Path) -> bool {
    let request = parse_shell_request(command_line, root, 120);
    let command = request.command.trim_start();
    let lower = command.to_ascii_lowercase();
    let tokens = lower
        .split_whitespace()
        .map(|token| token.trim_matches(&['"', '\''][..]))
        .collect::<Vec<_>>();
    let first_token = tokens.first().copied().unwrap_or_default();

    if first_token == "git" {
        return git_command_line_is_read_only(&tokens) && !contains_shell_write_operator(&lower);
    }

    matches!(
        first_token,
        "rg" | "grep"
            | "find"
            | "fd"
            | "ls"
            | "dir"
            | "pwd"
            | "cat"
            | "type"
            | "get-content"
            | "select-string"
            | "get-childitem"
            | "get-location"
            | "test-path"
            | "where-object"
    ) && !contains_shell_write_operator(&lower)
}

fn git_command_line_is_read_only(tokens: &[&str]) -> bool {
    let mut index = 1;
    while index < tokens.len() {
        let token = tokens[index];
        match token {
            "-c" | "-C" | "--git-dir" | "--work-tree" => {
                index += 2;
            }
            token if token.starts_with('-') => {
                index += 1;
            }
            "status" | "diff" | "show" | "log" | "ls-files" | "grep" | "rev-parse" | "describe"
            | "blame" => {
                return true;
            }
            "branch" => {
                return tokens
                    .iter()
                    .skip(index + 1)
                    .any(|token| matches!(*token, "--show-current" | "--list" | "-a" | "-r"));
            }
            _ => return false,
        }
    }

    false
}

fn contains_shell_write_operator(command: &str) -> bool {
    command.contains(" >")
        || command.contains(">>")
        || command.contains("set-content")
        || command.contains("out-file")
        || command.contains("new-item")
        || command.contains("remove-item")
        || command.contains("move-item")
        || command.contains("copy-item")
        || command.contains("tee-object")
        || command.contains("apply_patch")
        || command.contains("cargo test")
        || command.contains("cargo build")
        || command.contains("cargo check")
}
