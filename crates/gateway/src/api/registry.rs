use std::path::PathBuf;
use std::process::{Command as ProcessCommand, Stdio};

pub(crate) fn project_root() -> PathBuf {
    project_root_for_router_cli()
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
}

pub(crate) async fn run_router_cli<T>(
    command: &'static str,
    args: &[String],
    payload: Option<serde_json::Value>,
) -> Result<T, String>
where
    T: serde::de::DeserializeOwned + Send + 'static,
{
    let args = args.to_vec();
    tokio::task::spawn_blocking(move || run_router_cli_blocking(command, &args, payload))
        .await
        .map_err(|err| format!("router registry task failed: {err}"))?
}

pub(crate) fn router_binary_path() -> PathBuf {
    let file_name = if cfg!(windows) {
        "tura_router.exe"
    } else {
        "tura_router"
    };
    let mut candidates = Vec::new();
    if let Ok(current_exe) = std::env::current_exe() {
        if let Some(parent) = current_exe.parent() {
            candidates.push(parent.join(file_name));
        }
    }
    if let Ok(current_dir) = std::env::current_dir() {
        candidates.push(current_dir.join("target").join("release").join(file_name));
        candidates.push(current_dir.join("target").join("debug").join(file_name));
    }
    candidates
        .into_iter()
        .find(|path| path.exists())
        .unwrap_or_else(|| PathBuf::from(file_name))
}

fn run_router_cli_blocking<T>(
    command: &str,
    args: &[String],
    payload: Option<serde_json::Value>,
) -> Result<T, String>
where
    T: serde::de::DeserializeOwned,
{
    let router = router_binary_path();
    let mut process = ProcessCommand::new(&router);
    process.arg(command).args(args);
    if payload.is_some() {
        process.stdin(Stdio::piped());
    }
    process.stdout(Stdio::piped()).stderr(Stdio::piped());
    if let Some(root) = project_root_for_router_cli() {
        process.env("TURA_PROJECT_ROOT", root);
    }
    let mut child = process
        .spawn()
        .map_err(|err| format!("failed to start router CLI {}: {err}", router.display()))?;
    if let Some(payload) = payload {
        if let Some(mut stdin) = child.stdin.take() {
            use std::io::Write;
            let encoded = serde_json::to_vec(&payload)
                .map_err(|err| format!("failed to encode router payload: {err}"))?;
            stdin
                .write_all(&encoded)
                .map_err(|err| format!("failed to write router payload: {err}"))?;
        }
    }
    let output = child
        .wait_with_output()
        .map_err(|err| format!("router CLI failed: {err}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        return Err(if stderr.is_empty() { stdout } else { stderr });
    }
    serde_json::from_slice(&output.stdout).map_err(|err| {
        format!(
            "failed to parse router CLI output: {err}; output={}",
            String::from_utf8_lossy(&output.stdout)
        )
    })
}

fn project_root_for_router_cli() -> Option<String> {
    std::env::current_dir().ok().and_then(|current| {
        current
            .ancestors()
            .find(|candidate| {
                candidate.join("Cargo.toml").exists() && candidate.join("crates").exists()
            })
            .map(|path| path.display().to_string())
    })
}
