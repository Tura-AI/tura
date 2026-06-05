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
