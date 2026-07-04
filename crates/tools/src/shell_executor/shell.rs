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
    let raw = path.to_string_lossy().replace('\\', "/");
    let name = raw.rsplit('/').next()?.to_ascii_lowercase();
    match name.strip_suffix(".exe").unwrap_or(&name) {
        "zsh" => Some("zsh"),
        "bash" => Some("bash"),
        "sh" => Some("sh"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        default_posix_shell_executable, looks_posix_shell_script, normalize_bash_command,
        powershell_executable, prefix_powershell_script_with_utf8, supported_posix_shell_kind,
        zsh_candidate_paths,
    };
    use std::path::{Path, PathBuf};

    #[test]
    fn utf8_prefix_is_idempotent() {
        let script = "Write-Output ok";
        let prefixed = prefix_powershell_script_with_utf8(script);
        assert!(prefixed.contains("[Console]::InputEncoding"));
        assert!(prefixed.ends_with(script));

        let already_prefixed = prefix_powershell_script_with_utf8(&prefixed);
        assert_eq!(already_prefixed, prefixed);
    }

    #[test]
    fn default_shell_executables_have_stable_cross_platform_fallbacks() {
        let (posix_path, kind) = default_posix_shell_executable();
        assert!(matches!(kind, "bash" | "zsh" | "sh"));
        assert!(!posix_path.as_os_str().is_empty());

        let powershell = powershell_executable();
        assert!(!powershell.as_os_str().is_empty());
        if cfg!(windows) {
            let name = powershell
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("");
            assert!(
                matches!(
                    name.to_ascii_lowercase().as_str(),
                    "pwsh" | "pwsh.exe" | "powershell" | "powershell.exe"
                ) || powershell.is_absolute()
            );
        } else {
            assert_eq!(powershell, PathBuf::from("pwsh"));
        }
    }

    #[test]
    fn zsh_candidates_are_platform_specific_and_ordered() {
        let candidates = zsh_candidate_paths();
        assert!(!candidates.is_empty());
        if cfg!(target_os = "macos") {
            assert_eq!(candidates[0], "/bin/zsh");
        } else if cfg!(windows) {
            assert!(candidates
                .iter()
                .all(|candidate| candidate.ends_with(".exe")));
        } else {
            assert!(candidates
                .iter()
                .all(|candidate| candidate.contains("/zsh")));
        }
    }

    #[test]
    fn supported_posix_shell_kind_normalizes_case_and_exe_suffix() {
        assert_eq!(
            supported_posix_shell_kind(Path::new(r"C:\Tools\ZSH.EXE")),
            Some("zsh")
        );
        assert_eq!(
            supported_posix_shell_kind(Path::new("/usr/local/bin/Bash")),
            Some("bash")
        );
        assert_eq!(supported_posix_shell_kind(Path::new("sh")), Some("sh"));
        assert_eq!(supported_posix_shell_kind(Path::new("fish.exe")), None);
        assert_eq!(supported_posix_shell_kind(Path::new("")), None);
    }

    #[test]
    fn posix_script_detection_accepts_common_bash_shapes_and_rejects_powershell() {
        for script in [
            "python - <<'PY'\nprint('ok')\nPY",
            "python3 - <<'PY'\nprint('ok')\nPY",
            "for file in src/*.rs; do cat \"$file\"; done",
            "cd src && sed -n '1,20p' lib.rs",
            "echo $(basename src/lib.rs)",
            "#!/usr/bin/env bash\nset -e",
            "#!/bin/bash\nset -e",
        ] {
            assert!(looks_posix_shell_script(script), "{script}");
        }

        for script in [
            "PowerShell -Command \"Write-Output ok\"",
            "pwsh -NoProfile -Command \"Write-Output ok\"",
            "\"C:\\Program Files\\PowerShell\\7\\pwsh.exe\" -Command \"Write-Output ok\"",
            "Get-Content src/lib.rs; Write-Output done",
            "$env:PYTHONPATH='src'; python -c \"print('ok')\"",
        ] {
            assert!(!looks_posix_shell_script(script), "{script}");
        }
    }

    #[test]
    fn normalize_bash_command_rewrites_only_wsl_mount_prefixes_on_windows() {
        let command = "/mnt/c/Users/me/project && echo /mnt/not-drive && echo /mnt/z";
        let normalized = normalize_bash_command(command);

        if cfg!(windows) {
            assert!(normalized.starts_with("C:/Users/me/project"));
            assert!(normalized.contains("/mnt/not-drive"));
            assert!(normalized.ends_with("/mnt/z"));
        } else {
            assert_eq!(normalized, command);
        }
    }
}
