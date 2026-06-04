use std::process::Command as ProcessCommand;

pub(crate) fn select_directory(title: Option<&str>) -> anyhow::Result<Option<String>> {
    #[cfg(target_os = "windows")]
    {
        let escaped_title = title.unwrap_or("Select directory").replace('\'', "''");
        let script = format!(
            "Add-Type -AssemblyName System.Windows.Forms; \
             $f = New-Object System.Windows.Forms.Form; \
             $f.TopMost = $true; \
             $f.StartPosition = 'CenterScreen'; \
             $f.ShowInTaskbar = $false; \
             $d = New-Object System.Windows.Forms.FolderBrowserDialog; \
             $d.Description = '{}'; \
             $d.ShowNewFolderButton = $true; \
             if ($d.ShowDialog($f) -eq [System.Windows.Forms.DialogResult]::OK) {{ $d.SelectedPath }}; \
             $f.Dispose()",
            escaped_title,
        );
        let output = ProcessCommand::new("powershell")
            .args([
                "-NoProfile",
                "-STA",
                "-ExecutionPolicy",
                "Bypass",
                "-Command",
                &script,
            ])
            .output()?;
        if !output.status.success() {
            return Ok(None);
        }
        let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
        Ok((!path.is_empty()).then_some(path))
    }

    #[cfg(target_os = "macos")]
    {
        let prompt = applescript_string(title.unwrap_or("Select directory"));
        let script = format!("POSIX path of (choose folder with prompt {prompt})");
        let output = ProcessCommand::new("osascript")
            .args(["-e", &script])
            .output()?;
        if !output.status.success() {
            return Ok(None);
        }
        selected_path_from_stdout(&output.stdout)
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let title = title.unwrap_or("Select directory");
        let home = std::env::var_os("HOME")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| std::path::PathBuf::from("."));
        let attempts: [(&str, Vec<String>); 3] = [
            (
                "zenity",
                vec![
                    "--file-selection".to_string(),
                    "--directory".to_string(),
                    "--title".to_string(),
                    title.to_string(),
                ],
            ),
            (
                "kdialog",
                vec![
                    "--title".to_string(),
                    title.to_string(),
                    "--getexistingdirectory".to_string(),
                    home.to_string_lossy().to_string(),
                ],
            ),
            (
                "yad",
                vec![
                    "--file-selection".to_string(),
                    "--directory".to_string(),
                    "--title".to_string(),
                    title.to_string(),
                ],
            ),
        ];

        let mut saw_picker = false;
        for (command, args) in attempts {
            match ProcessCommand::new(command).args(args).output() {
                Ok(output) => {
                    saw_picker = true;
                    if output.status.success() {
                        return selected_path_from_stdout(&output.stdout);
                    }
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error.into()),
            }
        }

        if saw_picker {
            Ok(None)
        } else {
            Err(anyhow::anyhow!(
                "No Linux directory picker was found. Install zenity, kdialog, or yad."
            ))
        }
    }
}

#[cfg(not(target_os = "windows"))]
fn selected_path_from_stdout(stdout: &[u8]) -> anyhow::Result<Option<String>> {
    let path = String::from_utf8_lossy(stdout).trim().to_string();
    Ok((!path.is_empty()).then_some(path))
}

#[cfg(target_os = "macos")]
fn applescript_string(value: &str) -> String {
    let escaped = value.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{escaped}\"")
}
