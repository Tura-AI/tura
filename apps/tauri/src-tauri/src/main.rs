use serde::Serialize;
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

#[derive(Debug, Serialize)]
struct StartGatewayResponse {
    ok: bool,
    status: &'static str,
    gateway_path: Option<String>,
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![start_gateway])
        .run(tauri::generate_context!())
        .expect("failed to run Tura GUI");
}

#[tauri::command]
fn start_gateway(gateway_url: String) -> Result<StartGatewayResponse, String> {
    if gateway_tcp_reachable(&gateway_url) {
        return Ok(StartGatewayResponse {
            ok: true,
            status: "connected",
            gateway_path: None,
        });
    }

    let gateway = gateway_binary_path().ok_or_else(|| "gateway binary not found".to_string())?;
    let runtime_root = runtime_root_for_gateway(&gateway);
    let mut command = Command::new(&gateway);
    command
        .current_dir(&runtime_root)
        .env("TURA_PROJECT_ROOT", &runtime_root)
        .env(
            "TURA_PROVIDER_CONFIG",
            runtime_root.join("config").join("provider_config.json"),
        )
        .env("TURA_ENV_PATH", runtime_root.join(".env"));
    if let Some(port) = gateway_port(&gateway_url) {
        command.env("PORT", port);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    command
        .spawn()
        .map_err(|err| format!("failed to start gateway {}: {err}", gateway.display()))?;

    Ok(StartGatewayResponse {
        ok: true,
        status: "starting",
        gateway_path: Some(gateway.display().to_string()),
    })
}

fn gateway_binary_path() -> Option<PathBuf> {
    let file_name = if cfg!(windows) {
        "gateway.exe"
    } else {
        "gateway"
    };
    let exe = std::env::current_exe().ok()?;
    let exe_dir = exe.parent()?;
    let candidates = [
        exe_dir.join(file_name),
        exe_dir.join("bin").join(file_name),
        exe_dir
            .parent()
            .unwrap_or(exe_dir)
            .join("bin")
            .join(file_name),
        exe_dir
            .parent()
            .unwrap_or(exe_dir)
            .join("target")
            .join("release")
            .join(file_name),
        exe_dir
            .parent()
            .unwrap_or(exe_dir)
            .join("target")
            .join("debug")
            .join(file_name),
    ];
    candidates.into_iter().find(|path| path.exists())
}

fn runtime_root_for_gateway(gateway: &Path) -> PathBuf {
    let gateway_dir = gateway.parent().unwrap_or_else(|| Path::new("."));
    if gateway_dir.join("agents").join("src").is_dir()
        || gateway_dir
            .join("config")
            .join("provider_config.json")
            .exists()
    {
        return gateway_dir.to_path_buf();
    }
    gateway_dir
        .parent()
        .filter(|root| root.join("agents").join("src").is_dir())
        .unwrap_or(gateway_dir)
        .to_path_buf()
}

fn gateway_tcp_reachable(gateway_url: &str) -> bool {
    let host = gateway_host(gateway_url).unwrap_or_else(|| "127.0.0.1".to_string());
    let port = gateway_port(gateway_url).unwrap_or_else(|| "4096".to_string());
    let Ok(port) = port.parse::<u16>() else {
        return false;
    };
    let address = format!("{host}:{port}");
    address
        .parse()
        .ok()
        .and_then(|addr| TcpStream::connect_timeout(&addr, Duration::from_millis(350)).ok())
        .is_some()
}

fn gateway_host(gateway_url: &str) -> Option<String> {
    let without_scheme = gateway_url
        .split_once("://")
        .map(|(_, rest)| rest)
        .unwrap_or(gateway_url);
    let host_port = without_scheme.split('/').next()?.split('?').next()?;
    Some(
        host_port
            .split(':')
            .next()
            .unwrap_or("127.0.0.1")
            .to_string(),
    )
}

fn gateway_port(gateway_url: &str) -> Option<String> {
    let without_scheme = gateway_url
        .split_once("://")
        .map(|(_, rest)| rest)
        .unwrap_or(gateway_url);
    let host_port = without_scheme.split('/').next()?.split('?').next()?;
    host_port.split(':').nth(1).map(str::to_string)
}
