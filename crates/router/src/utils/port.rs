use anyhow::{anyhow, Result};
use std::{net::TcpListener, time::Duration};
use tokio::process::Command;
use tokio::time;
use tracing::{info, warn};

const PORT_RELEASE_TIMEOUT: Duration = Duration::from_secs(20);
const PORT_RELEASE_POLL: Duration = Duration::from_millis(250);

pub async fn ensure_port_is_free(port: u16) -> Result<()> {
    let pids = pids_using_port(port).await?;
    if pids.is_empty() {
        info!(port, "port is free before router start");
        return Ok(());
    }

    warn!(
        port,
        ?pids,
        "found processes occupying router port, killing them"
    );
    for pid in pids {
        if pid == 0 {
            continue;
        }
        kill_pid(pid).await?;
    }

    let deadline = time::Instant::now() + PORT_RELEASE_TIMEOUT;
    loop {
        if can_bind_port(port) {
            info!(port, "router port cleanup finished");
            return Ok(());
        }

        let after = pids_using_port(port).await?;
        let mut still_running = Vec::new();
        for &pid in &after {
            if pid != 0 && is_pid_alive(pid).await {
                still_running.push(pid);
            }
        }

        if !still_running.is_empty() && time::Instant::now() >= deadline {
            return Err(anyhow!(
                "port {} still occupied after kill: {:?}",
                port,
                still_running
            ));
        }

        if time::Instant::now() >= deadline {
            return Err(anyhow!("port {} did not become bindable after kill", port));
        }

        time::sleep(PORT_RELEASE_POLL).await;
    }
}

fn can_bind_port(port: u16) -> bool {
    TcpListener::bind(("0.0.0.0", port)).is_ok()
}

async fn is_pid_alive(pid: u32) -> bool {
    #[cfg(windows)]
    {
        let output = Command::new("cmd")
            .arg("/C")
            .arg(format!("tasklist /FI \"PID eq {}\" /NH", pid))
            .output()
            .await;
        match output {
            Ok(o) => {
                let stdout = String::from_utf8_lossy(&o.stdout);
                // If the PID is in the output, it's still alive
                stdout.contains(&pid.to_string())
            }
            Err(_) => false,
        }
    }
    #[cfg(not(windows))]
    {
        kill_pid(pid).await.is_ok()
    }
}

pub async fn port_is_occupied(port: u16) -> Result<bool> {
    Ok(!pids_using_port(port).await?.is_empty())
}

async fn pids_using_port(port: u16) -> Result<Vec<u32>> {
    let cmd = if cfg!(windows) {
        format!("netstat -ano | findstr :{port}")
    } else {
        format!("lsof -nP -iTCP:{port} -sTCP:LISTEN -t")
    };

    let output = shell_output(&cmd).await?;
    if output.trim().is_empty() {
        return Ok(Vec::new());
    }

    if cfg!(windows) {
        let mut pids = Vec::new();
        for line in output.lines() {
            let cols: Vec<&str> = line.split_whitespace().collect();
            if !cols.iter().any(|col| col.eq_ignore_ascii_case("LISTENING")) {
                continue;
            }
            if let Some(last) = cols.last() {
                if let Ok(pid) = last.parse::<u32>() {
                    if pid > 0 {
                        pids.push(pid);
                    }
                }
            }
        }
        pids.sort_unstable();
        pids.dedup();
        Ok(pids)
    } else {
        let mut pids = Vec::new();
        for line in output.lines() {
            if let Ok(pid) = line.trim().parse::<u32>() {
                pids.push(pid);
            }
        }
        pids.sort_unstable();
        pids.dedup();
        Ok(pids)
    }
}

async fn kill_pid(pid: u32) -> Result<()> {
    let cmd = if cfg!(windows) {
        format!("taskkill /PID {pid} /F")
    } else {
        format!("kill -9 {pid}")
    };

    // On Windows, taskkill might fail if the process is already gone (stale netstat data)
    // We ignore the error in this case since the goal is just to ensure the port is free
    if cfg!(windows) {
        let _ = Command::new("cmd").arg("/C").arg(&cmd).output().await;
        info!(
            pid,
            "kill attempted (ignoring result on Windows for stale netstat data)"
        );
        return Ok(());
    }

    let output = shell_output(&cmd).await?;
    info!(pid, output = %output.trim(), "killed port occupant");
    Ok(())
}

async fn shell_output(command: &str) -> Result<String> {
    let output = if cfg!(windows) {
        Command::new("cmd").arg("/C").arg(command).output().await?
    } else {
        Command::new("bash")
            .arg("-lc")
            .arg(command)
            .output()
            .await?
    };

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();

        // On Windows, taskkill failing is often because the process is already gone
        // (stale netstat data). We should continue gracefully.
        if cfg!(windows) {
            // The error message from taskkill contains the PID in the message
            // If the error contains "pid" or "process" (case insensitive),
            // it's likely a "not found" error which is okay
            let combined_lower = format!("{} {}", stdout.trim(), stderr.trim()).to_lowercase();
            if combined_lower.contains("pid") && combined_lower.contains("not")
                || combined_lower.contains("process") && combined_lower.contains("not")
                || combined_lower.contains("找不到")
                || combined_lower.contains("不存在")
            {
                info!("taskkill failed but process likely already gone (stale netstat data)");
                return Ok(String::new());
            }
        }

        if stderr.trim().is_empty() && stdout.trim().is_empty() {
            Ok(String::new())
        } else {
            Err(anyhow!(
                "command failed: {}{}",
                stdout,
                if stderr.is_empty() {
                    String::new()
                } else {
                    format!("\n{stderr}")
                }
            ))
        }
    }
}
