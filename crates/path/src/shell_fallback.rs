use std::ffi::OsStr;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WindowsShellKind {
    PowerShell,
    Cmd,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WindowsShell {
    pub kind: WindowsShellKind,
    pub executable: PathBuf,
}

pub fn resolve_windows_shell() -> WindowsShell {
    resolve_windows_shell_from(
        std::env::var_os("PATH").as_deref(),
        windows_pwsh_fallback_paths(),
        windows_powershell_fallback_paths(),
    )
}

fn resolve_windows_shell_from(
    path_var: Option<&OsStr>,
    pwsh_fallbacks: &[&str],
    powershell_fallbacks: &[&str],
) -> WindowsShell {
    resolve_windows_powershell_from(path_var, pwsh_fallbacks, powershell_fallbacks)
        .map(|executable| WindowsShell {
            kind: WindowsShellKind::PowerShell,
            executable,
        })
        .unwrap_or_else(|| WindowsShell {
            kind: WindowsShellKind::Cmd,
            executable: PathBuf::from("cmd.exe"),
        })
}

pub fn resolve_windows_powershell() -> Option<PathBuf> {
    resolve_windows_powershell_from(
        std::env::var_os("PATH").as_deref(),
        windows_pwsh_fallback_paths(),
        windows_powershell_fallback_paths(),
    )
}

fn resolve_windows_powershell_from(
    path_var: Option<&OsStr>,
    pwsh_fallbacks: &[&str],
    powershell_fallbacks: &[&str],
) -> Option<PathBuf> {
    resolve_program_on_path_from("pwsh", path_var)
        .or_else(|| first_existing_file(pwsh_fallbacks))
        .or_else(|| resolve_program_on_path_from("powershell", path_var))
        .or_else(|| first_existing_file(powershell_fallbacks))
}

fn resolve_program_on_path_from(program: &str, path_var: Option<&OsStr>) -> Option<PathBuf> {
    let path_var = path_var?;
    let mut names = vec![program.to_string()];
    if cfg!(windows) && Path::new(program).extension().is_none() {
        names.push(format!("{program}.exe"));
    }

    for dir in std::env::split_paths(path_var) {
        for name in &names {
            let candidate = dir.join(name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

fn first_existing_file(paths: &[&str]) -> Option<PathBuf> {
    paths.iter().map(PathBuf::from).find(|path| path.is_file())
}

fn windows_pwsh_fallback_paths() -> &'static [&'static str] {
    if cfg!(windows) {
        &[r"C:\Program Files\PowerShell\7\pwsh.exe"]
    } else {
        &[]
    }
}

fn windows_powershell_fallback_paths() -> &'static [&'static str] {
    if cfg!(windows) {
        &[r"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe"]
    } else {
        &[]
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::{
        resolve_program_on_path_from, resolve_windows_powershell_from, resolve_windows_shell,
        resolve_windows_shell_from, WindowsShellKind,
    };
    use std::ffi::OsStr;
    use std::fs;
    use std::path::PathBuf;

    #[test]
    fn powershell_fallback_skips_unresolved_bare_pwsh() {
        let temp = tempfile::tempdir().expect("temp shell fallback dir");
        let fallback = temp.path().join("powershell.exe");
        fs::write(&fallback, "").expect("write fake powershell fallback");
        let fallback_text = fallback.to_string_lossy().to_string();

        let resolved =
            resolve_windows_powershell_from(Some(OsStr::new("")), &[], &[&fallback_text])
                .expect("existing powershell fallback should resolve");

        assert_eq!(resolved, fallback);
        assert_ne!(resolved, PathBuf::from("pwsh"));
        assert_ne!(resolved, PathBuf::from("pwsh.exe"));
    }

    #[test]
    fn powershell_resolution_prefers_path_hit_before_fallback() {
        let temp = tempfile::tempdir().expect("temp shell path dir");
        let path_pwsh = temp.path().join("pwsh.exe");
        let fallback_pwsh = temp.path().join("fallback-pwsh.exe");
        fs::write(&path_pwsh, "").expect("write fake path pwsh");
        fs::write(&fallback_pwsh, "").expect("write fake fallback pwsh");
        let path_text = temp.path().as_os_str();
        let fallback_text = fallback_pwsh.to_string_lossy().to_string();

        let resolved = resolve_windows_powershell_from(Some(path_text), &[&fallback_text], &[])
            .expect("PATH pwsh should resolve");

        assert_eq!(resolved, path_pwsh);
    }

    #[test]
    fn windows_shell_falls_back_to_cmd_when_no_powershell_candidate_exists() {
        let shell = resolve_windows_shell_from(Some(OsStr::new("")), &[], &[]);

        assert_eq!(shell.kind, WindowsShellKind::Cmd);
        assert_eq!(shell.executable, PathBuf::from("cmd.exe"));
    }

    #[test]
    fn resolved_windows_shell_never_returns_empty_executable() {
        let shell = resolve_windows_shell();

        assert!(!shell.executable.as_os_str().is_empty());
        assert!(matches!(
            shell.kind,
            WindowsShellKind::PowerShell | WindowsShellKind::Cmd
        ));
    }

    #[test]
    fn program_on_path_requires_existing_file() {
        let temp = tempfile::tempdir().expect("temp shell path dir");
        let resolved = resolve_program_on_path_from("pwsh", Some(temp.path().as_os_str()));

        assert_eq!(resolved, None);
    }
}
