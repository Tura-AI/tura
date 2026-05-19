use std::path::Path;

use anyhow::{anyhow, Context, Result};
use tokio::process::Command;
use tracing::{error, info, warn};

pub async fn run_command(workdir: &Path, command: &str) -> Result<()> {
    let output = if cfg!(windows) {
        Command::new("cmd")
            .arg("/C")
            .arg(command)
            .current_dir(workdir)
            .output()
            .await
            .with_context(|| format!("failed to run command: {command}"))?
    } else {
        Command::new("bash")
            .arg("-lc")
            .arg(command)
            .current_dir(workdir)
            .output()
            .await
            .with_context(|| format!("failed to run command: {command}"))?
    };

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    if output.status.success() {
        if !stdout.trim().is_empty() {
            info!(
                command = command,
                workdir = %workdir.display(),
                stdout = %stdout.trim(),
                "command stdout"
            );
        }
        if !stderr.trim().is_empty() {
            warn!(
                command = command,
                workdir = %workdir.display(),
                stderr = %stderr.trim(),
                "command stderr"
            );
        }
        return Ok(());
    }

    error!(
        command = command,
        workdir = %workdir.display(),
        status = ?output.status.code(),
        stdout = %stdout.trim(),
        stderr = %stderr.trim(),
        "command failed"
    );

    Err(anyhow!("command failed: {command}"))
}
