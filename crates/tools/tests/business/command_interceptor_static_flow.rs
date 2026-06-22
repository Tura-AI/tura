use code_tools::commands::command_safety::{
    is_dangerous_command, is_dangerous_command_with_workspace,
};
use std::path::Path;

fn assert_blocked(surface: &str, command: &str) {
    let reason = is_dangerous_command(command);
    assert!(
        reason.is_some(),
        "{surface} command should be blocked before any shell is spawned: {command}"
    );
}

fn assert_allowed(surface: &str, command: &str) {
    let reason = is_dangerous_command(command);
    assert!(
        reason.is_none(),
        "{surface} command should remain allowed, got {reason:?}: {command}"
    );
}

fn assert_blocked_with_workspace(surface: &str, command: &str, cwd: &str, workspace: &str) {
    let reason = is_dangerous_command_with_workspace(command, Path::new(cwd), Path::new(workspace));
    assert!(
        reason.is_some(),
        "{surface} command should be blocked by the static interceptor without execution, got allowed: {command}"
    );
}

fn assert_allowed_with_workspace(surface: &str, command: &str, cwd: &str, workspace: &str) {
    let reason = is_dangerous_command_with_workspace(command, Path::new(cwd), Path::new(workspace));
    assert!(
        reason.is_none(),
        "{surface} command should be allowed by the static interceptor without execution, got {reason:?}: {command}"
    );
}

#[test]
fn static_interceptor_blocks_variable_indirection_across_shell_surfaces_without_execution() {
    let cases = [
        ("bash", "X=rm; $X -rf workspace-cache"),
        ("bash", "X='rm -rf workspace-cache'; $X"),
        ("zsh", "X=rm; ${X} -rf workspace-cache"),
        ("sh", "X=rm; ${X} -rf workspace-cache"),
        (
            "shell_command",
            "export X=rm; command $X -rf workspace-cache",
        ),
        ("bash-flag", "F=-rf; rm $F workspace-cache"),
        ("zsh-flag", "F=-rf; rm ${F} workspace-cache"),
        ("sh-command-and-flag", "X=rm; F=-rf; $X $F workspace-cache"),
        (
            "shell_command-quoted-flag",
            "F='-rf workspace-cache'; rm $F",
        ),
        ("bash-nested", "X=rm; bash -c \"$X -rf workspace-cache\""),
        (
            "bash-nested-flag",
            "F=-rf; bash -c \"rm $F workspace-cache\"",
        ),
        ("zsh-nested", "X=rm; zsh -c \"$X -rf workspace-cache\""),
        ("zsh-nested-flag", "F=-rf; zsh -c \"rm $F workspace-cache\""),
        ("sh-eval", "X=rm; eval \"$X -rf workspace-cache\""),
        (
            "shell_command-eval-flag",
            "F=-rf; eval \"rm $F workspace-cache\"",
        ),
    ];

    for (surface, command) in cases {
        assert_blocked(surface, command);
    }
}

#[test]
fn static_interceptor_blocks_local_decoder_to_shell_cradles_without_execution() {
    let encoded_rm = "cm0gLXJmIHdvcmtzcGFjZS1jYWNoZQo=";
    let cases = [
        ("bash", format!("printf {encoded_rm} | base64 -d | bash")),
        ("zsh", format!("echo {encoded_rm} | base64 --decode | zsh")),
        ("sh", format!("echo {encoded_rm} | base64 -d | /bin/sh")),
        (
            "shell_command",
            format!("echo {encoded_rm} | openssl enc -d -base64 | sh"),
        ),
    ];

    for (surface, command) in cases {
        assert_blocked(surface, &command);
    }
}

#[test]
fn static_interceptor_blocks_windows_and_nested_shell_shapes_without_execution() {
    let cases = [
        (
            "powershell",
            "Remove-Item -Recurse -Force C:\\workspace\\cache",
        ),
        ("powershell-alias", "ri -rec -force C:\\workspace\\cache"),
        ("cmd", "rd /s /q C:\\workspace\\cache"),
        (
            "cmd-nested-powershell",
            "cmd /c powershell -NoProfile -Command \"Remove-Item -Recurse -Force C:\\workspace\\cache\"",
        ),
        (
            "python-smuggle",
            "python -c \"import os; os.system('rm -rf workspace-cache')\"",
        ),
        (
            "node-smuggle",
            "node -e \"require('child_process').execSync('rm -rf workspace-cache')\"",
        ),
    ];

    for (surface, command) in cases {
        assert_blocked(surface, command);
    }
}

#[test]
fn static_interceptor_allows_workspace_deletes_without_execution() {
    let cases = [
        ("bash-rm-rf", "rm -rf cache", "/workspace/project", "/workspace/project"),
        (
            "bash-batch-rm",
            "rm -f cache/a.txt cache/b.txt",
            "/workspace/project",
            "/workspace/project",
        ),
        (
            "powershell-file-list",
            "Remove-Item -Force cache\\a.txt,cache\\b.txt -ErrorAction SilentlyContinue",
            "C:\\workspace\\project",
            "C:\\workspace\\project",
        ),
        (
            "powershell-recursive",
            "Remove-Item -Recurse -Force 'C:\\workspace\\project\\cache'",
            "C:\\workspace\\project",
            "C:\\workspace\\project",
        ),
        (
            "cmd-rmdir",
            "rd /s /q cache",
            "C:\\workspace\\project",
            "C:\\workspace\\project",
        ),
        (
            "cmd-del-force",
            "del /f cache\\scratch.txt",
            "C:\\workspace\\project",
            "C:\\workspace\\project",
        ),
    ];

    for (surface, command, cwd, workspace) in cases {
        assert_allowed_with_workspace(surface, command, cwd, workspace);
    }
}

#[test]
fn static_interceptor_blocks_outside_workspace_and_system_deletes_without_execution() {
    let cases = [
        ("bash-outside-rm", "rm -rf ../outside", "/workspace/project", "/workspace/project"),
        (
            "bash-outside-batch",
            "rm -f /tmp/outside-a /tmp/outside-b",
            "/workspace/project",
            "/workspace/project",
        ),
        (
            "powershell-outside-batch",
            "Remove-Item -Force 'C:\\outside\\a.txt','C:\\outside\\b.txt'",
            "C:\\workspace\\project",
            "C:\\workspace\\project",
        ),
        (
            "powershell-system",
            "Remove-Item -Recurse -Force 'C:\\Windows\\System32'",
            "C:\\workspace\\project",
            "C:\\workspace\\project",
        ),
        (
            "cmd-outside-rmdir",
            "rd /s /q C:\\outside\\cache",
            "C:\\workspace\\project",
            "C:\\workspace\\project",
        ),
        (
            "cmd-drive-format",
            "format C:",
            "C:\\workspace\\project",
            "C:\\workspace\\project",
        ),
        (
            "powershell-clear-disk",
            "Clear-Disk -Number 1",
            "C:\\workspace\\project",
            "C:\\workspace\\project",
        ),
    ];

    for (surface, command, cwd, workspace) in cases {
        assert_blocked_with_workspace(surface, command, cwd, workspace);
    }
}

#[test]
fn static_interceptor_blocks_assignment_forms_and_argument_flags_without_execution() {
    let cases = [
        (
            "export-multi",
            "export X=rm F=-rf; command $X $F workspace-cache",
        ),
        (
            "readonly-multi",
            "readonly X=rm F=-rf; $X $F workspace-cache",
        ),
        (
            "declare-multi",
            "declare X=rm F=-rf; bash -lc \"$X $F workspace-cache\"",
        ),
        (
            "typeset-multi",
            "typeset X=rm F=-rf; zsh -lc \"$X $F workspace-cache\"",
        ),
        (
            "local-multi",
            "local X=rm F=-rf; sh -c \"$X $F workspace-cache\"",
        ),
        ("flag-only-eval", "F=-rf; eval \"rm $F workspace-cache\""),
        (
            "flag-only-command-wrapper",
            "F=-rf; command rm $F workspace-cache",
        ),
        (
            "safe-looking-command-with-dangerous-arg-var",
            "TARGET='-rf workspace-cache'; rm $TARGET",
        ),
    ];

    for (surface, command) in cases {
        assert_blocked(surface, command);
    }
}

#[test]
fn static_interceptor_blocks_decoder_and_windows_shell_variants_without_execution() {
    let encoded_rm = "cm0gLXJmIHdvcmtzcGFjZS1jYWNoZQo=";
    let cases = [
        (
            "base64-to-powershell",
            format!("echo {encoded_rm} | base64 -decode | powershell"),
        ),
        (
            "base64-to-pwsh",
            format!("echo {encoded_rm} | base64 -d | pwsh"),
        ),
        (
            "certutil-to-cmd",
            "certutil -decode payload.txt decoded.bat | cmd".to_string(),
        ),
        (
            "cmd-k-nested",
            "cmd /k powershell -Command \"Remove-Item -Recurse -Force C:\\workspace\\cache\""
                .to_string(),
        ),
        (
            "powershell-slash-command",
            "pwsh /Command \"Remove-Item -Recurse -Force C:\\workspace\\cache\"".to_string(),
        ),
        (
            "powershell-short-command",
            "powershell -c \"rd /s /q C:\\workspace\\cache\"".to_string(),
        ),
    ];

    for (surface, command) in cases {
        assert_blocked(surface, &command);
    }
}

#[test]
fn static_interceptor_keeps_safe_variables_and_decoders_available() {
    let cases = [
        ("bash", "X=echo; $X safe"),
        ("zsh", "X=printf; ${X} safe"),
        ("shell_command", "TARGET=workspace-cache; rm $TARGET"),
        ("sh", "echo c2FmZQo= | base64 -d > decoded.txt"),
        ("shell_command", "echo c2FmZQo= | base64 -d | grep safe"),
        ("powershell", "Write-Output safe"),
        ("cmd", "dir"),
    ];

    for (surface, command) in cases {
        assert_allowed(surface, command);
    }
}
