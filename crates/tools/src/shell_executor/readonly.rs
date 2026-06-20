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

#[cfg(test)]
mod tests {
    use super::{contains_shell_write_operator, git_command_line_is_read_only, looks_read_only};

    #[test]
    fn read_only_detector_accepts_common_inspection_commands() {
        for command in [
            "rg needle src",
            "grep -R needle src",
            "find src -name '*.rs'",
            "fd lib src",
            "ls -la",
            "dir",
            "pwd",
            "cat src/lib.rs",
            "type src\\lib.rs",
            "Get-Content src/lib.rs",
            "Select-String needle src/lib.rs",
            "Get-ChildItem -Recurse src",
            "Get-Location",
            "Test-Path src/lib.rs",
            "Where-Object { $_.Name -like '*.rs' }",
        ] {
            assert!(looks_read_only(command), "{command}");
        }
    }

    #[test]
    fn read_only_detector_rejects_write_operators_and_build_commands() {
        for command in [
            "cat src/lib.rs > out.txt",
            "rg needle src >> out.txt",
            "Get-Content src/lib.rs | Set-Content out.txt",
            "New-Item out.txt",
            "Remove-Item out.txt",
            "Move-Item a b",
            "Copy-Item a b",
            "Get-Content a | Tee-Object out.txt",
            "apply_patch < patch.txt",
            "cargo test -p tools",
            "cargo build",
            "cargo check",
        ] {
            assert!(!looks_read_only(command), "{command}");
        }
    }

    #[test]
    fn git_read_only_rules_allow_only_inspection_subcommands() {
        for tokens in [
            vec!["git", "status"],
            vec!["git", "-c", "color.ui=false", "diff"],
            vec!["git", "-C", "repo", "show"],
            vec!["git", "branch", "--show-current"],
            vec!["git", "branch", "--list"],
            vec!["git", "branch", "-a"],
        ] {
            assert!(git_command_line_is_read_only(&tokens), "{tokens:?}");
        }

        for tokens in [
            vec!["git", "checkout", "main"],
            vec!["git", "branch", "new-branch"],
            vec!["git", "commit", "-m", "msg"],
            vec!["git", "reset", "--hard"],
            vec!["git"],
        ] {
            assert!(!git_command_line_is_read_only(&tokens), "{tokens:?}");
        }
    }

    #[test]
    fn git_shell_command_must_not_contain_write_operator() {
        assert!(looks_read_only("git status"));
        assert!(!looks_read_only("git status > status.txt"));
        assert!(!looks_read_only("git diff >> diff.txt"));
    }

    #[test]
    fn contains_shell_write_operator_matches_case_insensitive_input_contract() {
        assert!(contains_shell_write_operator(
            "get-content a | set-content b"
        ));
        assert!(contains_shell_write_operator("cargo test"));
        assert!(!contains_shell_write_operator("git status --short"));
    }
}
