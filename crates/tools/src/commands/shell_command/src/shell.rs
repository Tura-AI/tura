use std::env;
use std::path::{Path, PathBuf};

pub(super) fn prefix_powershell_script_with_utf8(script: &str) -> String {
    if script.contains("[Console]::OutputEncoding") {
        script.to_string()
    } else {
        format!(
            "[Console]::InputEncoding=[Console]::OutputEncoding=[System.Text.UTF8Encoding]::new(); $OutputEncoding=[Console]::OutputEncoding; {script}"
        )
    }
}

pub(super) fn bash_executable() -> PathBuf {
    if !cfg!(windows) {
        return PathBuf::from("/bin/bash");
    }
    [
        r"C:\msys64\usr\bin\bash.exe",
        r"C:\msys64\ucrt64\bin\bash.exe",
        r"C:\Program Files\Git\bin\bash.exe",
        r"C:\Program Files\Git\usr\bin\bash.exe",
        r"C:\Program Files (x86)\Git\bin\bash.exe",
        "bash",
    ]
    .iter()
    .map(PathBuf::from)
    .find(|path| path == Path::new("bash") || path.exists())
    .unwrap_or_else(|| PathBuf::from("bash"))
}

pub(super) fn zsh_executable() -> Option<PathBuf> {
    if let Some(configured) = configured_executable("TURA_ZSH_PATH") {
        return configured;
    }

    for candidate in zsh_candidate_paths() {
        let path = PathBuf::from(candidate);
        if path.exists() {
            return Some(path);
        }
    }

    find_program_on_path("zsh")
}

pub(super) fn default_posix_shell_executable() -> (PathBuf, &'static str) {
    if cfg!(target_os = "macos") {
        if let Some(shell) = macos_user_posix_shell() {
            return shell;
        }
        if let Some(zsh) = zsh_executable() {
            return (zsh, "zsh");
        }
    }

    for candidate in ["/bin/bash", "/usr/bin/bash"] {
        let path = PathBuf::from(candidate);
        if path.exists() {
            return (path, "bash");
        }
    }
    if let Some(path) = find_program_on_path("bash") {
        return (path, "bash");
    }
    for candidate in ["/bin/sh", "/usr/bin/sh"] {
        let path = PathBuf::from(candidate);
        if path.exists() {
            return (path, "sh");
        }
    }
    (PathBuf::from("sh"), "sh")
}

pub(super) fn powershell_executable() -> PathBuf {
    if !cfg!(windows) {
        return PathBuf::from("pwsh");
    }
    [
        r"C:\Program Files\PowerShell\7\pwsh.exe",
        "pwsh",
        r"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe",
        "powershell",
    ]
    .iter()
    .map(PathBuf::from)
    .find(|path| path == Path::new("pwsh") || path == Path::new("powershell") || path.exists())
    .unwrap_or_else(|| PathBuf::from("powershell"))
}

pub(super) fn normalize_bash_command(command: &str) -> String {
    if !cfg!(windows) {
        return command.to_string();
    }

    let mut normalized = String::with_capacity(command.len());
    let mut rest = command;
    while let Some(index) = rest.find("/mnt/") {
        normalized.push_str(&rest[..index]);
        rest = &rest[index + "/mnt/".len()..];
        let Some(drive) = rest.chars().next() else {
            normalized.push_str("/mnt/");
            break;
        };
        let drive_len = drive.len_utf8();
        let after_drive = &rest[drive_len..];
        if drive.is_ascii_alphabetic() && after_drive.starts_with('/') {
            normalized.push(drive.to_ascii_uppercase());
            normalized.push(':');
            rest = after_drive;
        } else {
            normalized.push_str("/mnt/");
        }
    }
    normalized.push_str(rest);
    normalized
}

pub(super) fn looks_posix_shell_script(command: &str) -> bool {
    let text = command.trim_start();
    let lower = text.to_ascii_lowercase();
    if lower.starts_with("powershell ")
        || lower.starts_with("powershell.exe ")
        || lower.starts_with("pwsh ")
        || lower.starts_with("pwsh.exe ")
        || (lower.starts_with('"')
            && (lower.contains("powershell.exe\"") || lower.contains("pwsh.exe\"")))
    {
        return false;
    }

    lower.starts_with("python - <<")
        || lower.contains(" python - <<")
        || lower.starts_with("python3 - <<")
        || lower.contains(" python3 - <<")
        || lower.starts_with("pythonpath=")
        || lower.contains(" pythonpath=")
        || lower.contains("; do ")
        || lower.contains("; done")
        || (lower.starts_with("for ") && lower.contains(" in ") && lower.contains(" do "))
        || lower.contains(" && sed ")
        || lower.contains(" && cat ")
        || lower.contains("$(basename ")
        || lower.contains("#!/usr/bin/env bash")
        || lower.contains("#!/bin/bash")
}

fn configured_executable(env_name: &str) -> Option<Option<PathBuf>> {
    let value = env::var_os(env_name)?;
    let value = value.to_string_lossy().trim().to_string();
    if value.is_empty() {
        return None;
    }
    Some(resolve_program(&value))
}

fn resolve_program(program: &str) -> Option<PathBuf> {
    let path = PathBuf::from(program);
    if is_explicit_path(program, &path) {
        return path.exists().then_some(path);
    }
    find_program_on_path(program)
}

fn is_explicit_path(program: &str, path: &Path) -> bool {
    path.is_absolute() || program.contains('/') || program.contains('\\')
}

fn find_program_on_path(program: &str) -> Option<PathBuf> {
    let path_var = env::var_os("PATH")?;
    let mut names = vec![program.to_string()];
    if cfg!(windows) && Path::new(program).extension().is_none() {
        names.push(format!("{program}.exe"));
    }
    for dir in env::split_paths(&path_var) {
        for name in &names {
            let candidate = dir.join(name);
            if candidate.exists() {
                return Some(candidate);
            }
        }
    }
    None
}

pub(super) fn zsh_candidate_paths() -> &'static [&'static str] {
    if cfg!(target_os = "macos") {
        &[
            "/bin/zsh",
            "/usr/bin/zsh",
            "/opt/homebrew/bin/zsh",
            "/usr/local/bin/zsh",
        ]
    } else if cfg!(windows) {
        &[
            r"C:\msys64\usr\bin\zsh.exe",
            r"C:\msys64\ucrt64\bin\zsh.exe",
            r"C:\Program Files\Git\usr\bin\zsh.exe",
            r"C:\Program Files\Git\bin\zsh.exe",
        ]
    } else {
        &["/usr/bin/zsh", "/bin/zsh", "/usr/local/bin/zsh"]
    }
}

fn macos_user_posix_shell() -> Option<(PathBuf, &'static str)> {
    if !cfg!(target_os = "macos") {
        return None;
    }
    let shell = env::var("SHELL").ok()?;
    let shell = shell.trim();
    if shell.is_empty() {
        return None;
    }
    let path = resolve_program(shell)?;
    let kind = supported_posix_shell_kind(&path)?;
    Some((path, kind))
}

pub(super) fn supported_posix_shell_kind(path: &Path) -> Option<&'static str> {
    let name = path.file_name()?.to_string_lossy().to_ascii_lowercase();
    match name.strip_suffix(".exe").unwrap_or(&name) {
        "zsh" => Some("zsh"),
        "bash" => Some("bash"),
        "sh" => Some("sh"),
        _ => None,
    }
}
