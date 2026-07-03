mod execution;
mod process;
mod read_batch;
mod readonly;
mod request;
mod response;
mod shell;

pub use process::{
    current_shell_process_scope_strategy, terminate_retained_shell_process_scopes,
    ShellProcessScopeStrategy,
};

use crate::commands::{apply_patch, command_safety, CommandResponse};
use crate::runtime::tool::ToolContext;
use std::path::Path;
use std::process::Command;

const BACKGROUND_PROCESS_KIND_ENV: &str = "TURA_BACKGROUND_PROCESS_KIND";
const RUNTIME_SHELL_BACKGROUND_PROCESS_KIND: &str = "runtime_shell";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ShellKind {
    ShellCommand,
    Bash,
    Zsh,
}

impl ShellKind {
    fn id(self) -> &'static str {
        match self {
            Self::ShellCommand => "shell_command",
            Self::Bash => "bash",
            Self::Zsh => "zsh",
        }
    }
}

pub fn execute(
    command_line: &str,
    session_dir: &Path,
    timeout_secs: u64,
    shell_kind: ShellKind,
) -> CommandResponse {
    let request = request::parse_shell_request(command_line, session_dir, timeout_secs);
    if let Some(patch_text) = request::embedded_apply_patch_text(&request.command) {
        return apply_patch::execute(&patch_text, session_dir);
    }
    if let Some(reason) = command_safety::is_dangerous_command_with_workspace(
        &request.command,
        &request.cwd,
        session_dir,
    ) {
        return response::blocked_command_response(&request.command, &reason);
    }
    let shell_kind = shell_kind.id();
    let use_zsh = shell_kind == "zsh";
    let use_bash = shell_kind == "bash"
        || (cfg!(windows) && shell::looks_posix_shell_script(&request.command));
    let use_posix = use_zsh || use_bash || !cfg!(windows);
    let command_text = read_batch::space_batched_read_command(&request.command, use_posix)
        .unwrap_or_else(|| request.command.clone());
    let mut command = if use_zsh {
        let Some(zsh) = shell::zsh_executable() else {
            return response::failed_async_response(
                "zsh executable was not found. Install zsh, set TURA_ZSH_PATH to a valid zsh binary, or use TURA_COMMAND_RUN_SHELL=bash.",
                127,
            );
        };
        let mut command = Command::new(zsh);
        command
            .arg("-lc")
            .arg(shell::normalize_bash_command(&command_text));
        command
    } else if use_bash {
        let bash = shell::bash_executable();
        let mut command = Command::new(bash);
        command
            .arg("-lc")
            .arg(shell::normalize_bash_command(&command_text));
        command
    } else if cfg!(windows) {
        let mut command = Command::new(shell::powershell_executable());
        command.arg("-NoProfile").arg("-Command").arg(&command_text);
        command
    } else {
        let (executable, kind) = shell::default_posix_shell_executable();
        let mut command = Command::new(executable);
        command
            .arg(if kind == "sh" { "-c" } else { "-lc" })
            .arg(&command_text);
        command
    };
    command.current_dir(&request.cwd);
    command.env(
        BACKGROUND_PROCESS_KIND_ENV,
        RUNTIME_SHELL_BACKGROUND_PROCESS_KIND,
    );

    execution::run_command_with_timeout(command, request.timeout_secs)
}

pub async fn execute_async(
    command_line: &str,
    session_dir: &Path,
    timeout_secs: u64,
    shell_kind: ShellKind,
    ctx: &ToolContext,
) -> CommandResponse {
    let request = request::parse_shell_request(command_line, session_dir, timeout_secs);
    if let Some(patch_text) = request::embedded_apply_patch_text(&request.command) {
        return apply_patch::execute(&patch_text, session_dir);
    }
    if let Some(reason) = command_safety::is_dangerous_command_with_workspace(
        &request.command,
        &request.cwd,
        session_dir,
    ) {
        return response::blocked_command_response(&request.command, &reason);
    }
    if ctx.cancellation.is_cancelled() {
        return response::failed_async_response("tool task aborted", -1);
    }
    let shell_kind = shell_kind.id();
    let use_zsh = shell_kind == "zsh";
    let use_bash = shell_kind == "bash"
        || (cfg!(windows) && shell::looks_posix_shell_script(&request.command));
    let use_posix = use_zsh || use_bash || !cfg!(windows);
    let command_text = read_batch::space_batched_read_command(&request.command, use_posix)
        .unwrap_or_else(|| request.command.clone());
    let mut command = if use_zsh {
        let Some(zsh) = shell::zsh_executable() else {
            return response::failed_async_response(
                "zsh executable was not found. Install zsh, set TURA_ZSH_PATH to a valid zsh binary, or use TURA_COMMAND_RUN_SHELL=bash.",
                127,
            );
        };
        let mut command = tokio::process::Command::new(zsh);
        command
            .arg("-lc")
            .arg(shell::normalize_bash_command(&command_text));
        command
    } else if use_bash {
        let bash = shell::bash_executable();
        let mut command = tokio::process::Command::new(bash);
        command
            .arg("-lc")
            .arg(shell::normalize_bash_command(&command_text));
        command
    } else if cfg!(windows) {
        let mut command = tokio::process::Command::new(shell::powershell_executable());
        command
            .arg("-NoProfile")
            .arg("-Command")
            .arg(shell::prefix_powershell_script_with_utf8(&command_text));
        command
    } else {
        let (executable, kind) = shell::default_posix_shell_executable();
        let mut command = tokio::process::Command::new(executable);
        command
            .arg(if kind == "sh" { "-c" } else { "-lc" })
            .arg(&command_text);
        command
    };
    command.current_dir(&request.cwd);
    command.env(
        BACKGROUND_PROCESS_KIND_ENV,
        RUNTIME_SHELL_BACKGROUND_PROCESS_KIND,
    );
    execution::run_tokio_command_with_timeout(command, request.timeout_secs, ctx).await
}

pub fn looks_read_only(command_line: &str) -> bool {
    readonly::looks_read_only(command_line)
}

pub fn looks_read_only_with_root(command_line: &str, root: &Path) -> bool {
    readonly::looks_read_only_with_root(command_line, root)
}

pub fn display_command(command_line: &str, session_dir: &Path, timeout_secs: u64) -> String {
    request::parse_shell_request(command_line, session_dir, timeout_secs).command
}

pub fn shell_output_value(response: CommandResponse) -> serde_json::Value {
    response::shell_output_value(response)
}

pub(crate) fn json_like_output(
    exit_code: i32,
    stdout: String,
    stderr: String,
    output: serde_json::Value,
    changes: Vec<serde_json::Value>,
) -> serde_json::Value {
    response::json_like_output(exit_code, stdout, stderr, output, changes)
}

#[cfg(test)]
mod tests {
    use super::{execute, execute_async, read_batch, request, shell, ShellKind};
    use crate::runtime::tool::ToolContext;
    use read_batch::space_batched_read_command;
    use request::{embedded_apply_patch_text, parse_shell_request};
    use shell::{looks_posix_shell_script, normalize_bash_command};
    use std::ffi::OsString;
    use std::path::Path;

    fn restore_env(name: &str, value: Option<OsString>) {
        if let Some(value) = value {
            std::env::set_var(name, value);
        } else {
            std::env::remove_var(name);
        }
    }

    #[test]
    fn parses_json_shell_request_with_escaped_quotes() {
        let request = parse_shell_request(
            r#"{\"command\":\"Write-Output ok\",\"workdir\":\"subdir\",\"timeout_ms\":1500}"#,
            Path::new("C:/workspace"),
            120,
        );

        assert_eq!(request.command, "Write-Output ok");
        assert!(request.cwd.ends_with("subdir"));
        assert_eq!(request.timeout_secs, 2);
    }

    #[test]
    fn strips_current_style_shell_text_prefixes() {
        let request = parse_shell_request(
            r#"{"command":"command:rg -n symbol src","workdir":"subdir","timeout_ms":1500}"#,
            Path::new("C:/workspace"),
            120,
        );

        assert_eq!(request.command, "rg -n symbol src");
        assert!(request.cwd.ends_with("subdir"));
    }

    #[test]
    fn strips_current_style_shell_text_prefixes_inside_multiline_scripts() {
        let request = parse_shell_request(
            "echo before\ncommand:for i in 1 2; do echo $i; done\n",
            Path::new("C:/workspace"),
            120,
        );

        assert_eq!(
            request.command,
            "echo before\nfor i in 1 2; do echo $i; done\n"
        );
    }

    #[test]
    fn extracts_apply_patch_embedded_in_shell_wrapper() {
        let patch = embedded_apply_patch_text(
            "@'\n*** Begin Patch\n*** Update File: src/app.txt\n@@\n-old\n+new\n*** End Patch\n'@ | apply_patch",
        )
        .expect("patch should be extracted");

        assert_eq!(
            patch,
            "*** Begin Patch\n*** Update File: src/app.txt\n@@\n-old\n+new\n*** End Patch"
        );
    }

    #[test]
    fn does_not_extract_patch_from_read_only_text_output() {
        assert!(
            embedded_apply_patch_text("cat <<'EOF'\n*** Begin Patch\n*** End Patch\nEOF").is_none()
        );
    }

    #[test]
    fn parses_escaped_json_shell_request_with_inner_command_quotes() {
        let request = parse_shell_request(
            r#"{\"command\":\"rg -n \\\"def close_month|score_policy\\\" src/retail_core\",\"workdir\":\"subdir\",\"timeout_ms\":120000}"#,
            Path::new("C:/workspace"),
            120,
        );

        assert_eq!(
            request.command,
            r#"rg -n "def close_month|score_policy" src/retail_core"#
        );
        assert!(request.cwd.ends_with("subdir"));
        assert_eq!(request.timeout_secs, 120);
    }

    #[test]
    fn parses_json_shell_request_wrapped_as_json_string() {
        let request = parse_shell_request(
            r#""{\"command\":\"Write-Output ok\",\"workdir\":\"subdir\",\"timeout_ms\":1500}""#,
            Path::new("C:/workspace"),
            120,
        );

        assert_eq!(request.command, "Write-Output ok");
        assert!(request.cwd.ends_with("subdir"));
        assert_eq!(request.timeout_secs, 2);
    }

    #[test]
    fn parses_escaped_json_request_with_here_string_command() {
        let request = parse_shell_request(
            r#"{\"command\":\"@'\\nprint(1)\\n'@ | python -\",\"workdir\":\"subdir\",\"timeout_ms\":10000}"#,
            Path::new("C:/workspace"),
            120,
        );

        assert!(request.command.starts_with("@'"));
        assert!(request.command.contains("python -"));
        assert!(request.cwd.ends_with("subdir"));
        assert_eq!(request.timeout_secs, 10);
    }

    #[test]
    fn parses_loose_json_request_with_raw_multiline_command() {
        let request = parse_shell_request(
            "{\"command\":\"@'\nprint(\\\"ok\\\")\n'@ | python -\",\"workdir\":\"subdir\",\"timeout_ms\":10000}",
            Path::new("C:/workspace"),
            120,
        );

        assert_eq!(request.command, "@'\nprint(\"ok\")\n'@ | python -");
        assert!(request.cwd.ends_with("subdir"));
        assert_eq!(request.timeout_secs, 10);
    }

    #[test]
    fn parses_loose_json_request_with_regex_backslashes() {
        let request = parse_shell_request(
            r#"{"command":"rg -n \"toFixed\(1\)|count \+ 2\" frontend/src/views","workdir":"subdir","timeout_ms":10000}"#,
            Path::new("C:/workspace"),
            120,
        );

        assert_eq!(
            request.command,
            r#"rg -n "toFixed\(1\)|count \+ 2" frontend/src/views"#
        );
        assert!(request.cwd.ends_with("subdir"));
        assert_eq!(request.timeout_secs, 10);
    }

    #[test]
    fn accepts_codex_command_run_cmd_alias() {
        let request = parse_shell_request(
            r#"{\"cmd\":\"Write-Output ok\",\"workdir\":\"subdir\",\"timeout_ms\":1500}"#,
            Path::new("C:/workspace"),
            120,
        );

        assert_eq!(request.command, "Write-Output ok");
        assert!(request.cwd.ends_with("subdir"));
        assert_eq!(request.timeout_secs, 2);
    }

    #[test]
    fn raw_shell_text_stays_raw() {
        let request = parse_shell_request("rg -n needle src", Path::new("C:/workspace"), 120);

        assert_eq!(request.command, "rg -n needle src");
        assert_eq!(request.timeout_secs, 120);
    }

    #[test]
    fn spaces_simple_powershell_batch_reads_without_file_markers() {
        let command = "Get-Content tests/a.py; Get-Content -Raw \"src/b.py\"; gc -Path src/c.py";

        let spaced =
            space_batched_read_command(command, false).expect("simple read batch should be spaced");

        assert!(!spaced.contains("---FILE---"));
        assert!(spaced.contains("Get-Content 'tests/a.py'"));
        assert!(spaced.contains("Write-Output ''"));
        assert!(spaced.contains("Get-Content -Raw 'src/b.py'"));
        assert!(spaced.contains("gc -Path 'src/c.py'"));
    }

    #[test]
    fn spaces_simple_bash_batch_reads_without_file_markers() {
        let command = "cat src/a.py; cat -- 'src/b.py'";

        let spaced = space_batched_read_command(command, true)
            .expect("simple bash read batch should be spaced");

        assert!(!spaced.contains("---FILE---"));
        assert!(spaced.contains("cat 'src/a.py'"));
        assert!(spaced.contains("printf '\\n'"));
        assert!(spaced.contains("cat -- 'src/b.py'"));
    }

    #[test]
    fn spaces_multi_target_read_commands() {
        let powershell = space_batched_read_command(
            "Get-Content -Path src/a.py,src/b.py; type .\\src\\c.py",
            false,
        )
        .expect("multi-target powershell reads should be spaced");

        assert!(powershell.contains("Get-Content -Path 'src/a.py'"));
        assert!(powershell.contains("Write-Output ''"));
        assert!(powershell.contains("Get-Content -Path 'src/b.py'"));
        assert!(powershell.contains("type '.\\src\\c.py'"));
        assert!(!powershell.contains("---FILE---"));

        let bash = space_batched_read_command("cat src/a.py src/b.py", true)
            .expect("multi-target bash reads should be spaced");
        assert!(bash.contains("cat 'src/a.py'"));
        assert!(bash.contains("printf '\\n'"));
        assert!(bash.contains("cat 'src/b.py'"));
        assert!(!bash.contains("---FILE---"));
    }

    #[test]
    fn preserves_safe_read_options_when_spacing() {
        let spaced =
            space_batched_read_command("Get-Content -TotalCount 40 -Path src/a.py,src/b.py", false)
                .expect("safe read options should be preserved");

        assert!(spaced.contains("Get-Content -TotalCount 40 -Path 'src/a.py'"));
        assert!(spaced.contains("Write-Output ''"));
        assert!(spaced.contains("Get-Content -TotalCount 40 -Path 'src/b.py'"));
    }

    #[test]
    fn does_not_space_complex_or_single_read_commands() {
        assert!(space_batched_read_command("Get-Content src/a.py", false).is_none());
        assert!(space_batched_read_command(
            "Get-Content src/a.py | Select-String needle; Get-Content src/b.py",
            false
        )
        .is_none());
        assert!(space_batched_read_command(
            "$files=@('src/a.py','src/b.py'); foreach ($f in $files) { Get-Content $f }",
            false
        )
        .is_none());
    }

    #[test]
    fn windows_bash_command_normalizes_wsl_mount_paths() {
        let command = "cd /mnt/c/Users/example/project && python - <<'PY'\nprint('ok')\nPY";

        let normalized = normalize_bash_command(command);

        if cfg!(windows) {
            assert!(normalized.starts_with("cd C:/Users/example/project"));
        } else {
            assert_eq!(normalized, command);
        }
    }

    #[test]
    fn detects_posix_shell_scripts_sent_to_shell_command() {
        assert!(looks_posix_shell_script(
            "for f in src/*.py; do sed -n '1,20p' \"$f\"; done"
        ));
        assert!(looks_posix_shell_script(
            "PYTHONPATH=src python - <<'PY'\nprint('ok')\nPY"
        ));
        assert!(!looks_posix_shell_script(
            "Get-Content -Raw src/app.txt; Write-Output ok"
        ));
        assert!(!looks_posix_shell_script(
            "$env:PYTHONPATH='src'; python -c \"print('ok')\""
        ));
        assert!(!looks_posix_shell_script(
            "\"C:\\Program Files\\PowerShell\\7\\pwsh.exe\" -Command 'for f in *.py; do echo $f; done'"
        ));
    }

    #[test]
    fn shell_executor_marks_spawned_processes_as_runtime_shell_background_processes() {
        let response = execute(
            &background_process_kind_command(),
            Path::new("."),
            10,
            ShellKind::ShellCommand,
        );

        assert!(response.success, "{response:?}");
        assert_eq!(
            response.stdout.trim(),
            super::RUNTIME_SHELL_BACKGROUND_PROCESS_KIND
        );
    }

    #[tokio::test]
    async fn async_shell_executor_marks_spawned_processes_as_runtime_shell_background_processes() {
        let context = ToolContext::new(std::env::current_dir().expect("current dir"));
        let response = execute_async(
            &background_process_kind_command(),
            Path::new("."),
            10,
            ShellKind::ShellCommand,
            &context,
        )
        .await;

        assert!(response.success, "{response:?}");
        assert_eq!(
            response.stdout.trim(),
            super::RUNTIME_SHELL_BACKGROUND_PROCESS_KIND
        );
    }

    fn background_process_kind_command() -> String {
        if cfg!(windows) {
            format!(
                "[Environment]::GetEnvironmentVariable('{}', 'Process')",
                super::BACKGROUND_PROCESS_KIND_ENV
            )
        } else {
            format!("printf '%s' \"${}\"", super::BACKGROUND_PROCESS_KIND_ENV)
        }
    }

    #[test]
    fn zsh_surface_returns_clear_failure_when_configured_binary_is_missing() {
        let previous = std::env::var_os("TURA_ZSH_PATH");
        std::env::set_var(
            "TURA_ZSH_PATH",
            if cfg!(windows) {
                r"C:\definitely\missing\tura-zsh.exe"
            } else {
                "/definitely/missing/tura-zsh"
            },
        );

        let response = super::execute(
            r#"{"command":"print -r -- zsh-ok","timeout_ms":1000}"#,
            Path::new("."),
            120,
            super::ShellKind::Zsh,
        );

        restore_env("TURA_ZSH_PATH", previous);
        assert!(!response.success);
        assert_eq!(response.exit_code, 127);
        assert!(response.stderr.contains("zsh executable was not found"));
    }

    #[test]
    fn supported_posix_shell_kind_recognizes_zsh_bash_and_sh() {
        assert_eq!(
            shell::supported_posix_shell_kind(Path::new("/bin/zsh")),
            Some("zsh")
        );
        assert_eq!(
            shell::supported_posix_shell_kind(Path::new("/bin/bash")),
            Some("bash")
        );
        assert_eq!(
            shell::supported_posix_shell_kind(Path::new("/bin/sh")),
            Some("sh")
        );
        assert_eq!(
            shell::supported_posix_shell_kind(Path::new("/bin/fish")),
            None
        );
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn macos_zsh_candidates_assert_system_zsh_first() {
        assert_eq!(
            shell::zsh_candidate_paths().first().copied(),
            Some("/bin/zsh")
        );
        assert!(
            Path::new("/bin/zsh").exists(),
            "macOS should provide /bin/zsh for the default zsh fallback"
        );
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod business_tests;
