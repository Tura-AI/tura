use serde::Serialize;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;
use url::Url;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct StartGatewayResponse {
    ok: bool,
    status: &'static str,
    gateway_path: Option<String>,
    gateway_url: Option<String>,
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![start_gateway])
        .run(tauri::generate_context!())
        .expect("failed to run Tura GUI");
}

#[tauri::command]
fn start_gateway(gateway_url: String) -> Result<StartGatewayResponse, String> {
    let mut endpoint = GatewayEndpoint::parse(&gateway_url);
    if gateway_tcp_reachable(&endpoint) {
        return Ok(StartGatewayResponse {
            ok: true,
            status: "connected",
            gateway_path: None,
            gateway_url: Some(endpoint.url()),
        });
    }
    endpoint = endpoint_for_gateway_start(endpoint);

    let gateway = gateway_binary_path().ok_or_else(|| "gateway binary not found".to_string())?;
    let runtime_root = runtime_root_for_gateway(&gateway);
    let mut command = Command::new(&gateway);
    command
        .current_dir(&runtime_root)
        .env("TURA_PROJECT_ROOT", &runtime_root)
        .env(
            "TURA_PROVIDER_CONFIG",
            provider_config_path_for_runtime_root(&runtime_root),
        )
        .env("TURA_ENV_PATH", runtime_root.join(".env"));
    command.env("PORT", endpoint.port.to_string());
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
        gateway_url: Some(endpoint.url()),
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
            .join("release")
            .join(file_name),
        exe_dir
            .parent()
            .unwrap_or(exe_dir)
            .join("debug")
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
    gateway_dir
        .ancestors()
        .find(|candidate| is_runtime_root(candidate))
        .unwrap_or(gateway_dir)
        .to_path_buf()
}

fn is_runtime_root(candidate: &Path) -> bool {
    (candidate.join("agents").join("src").is_dir()
        && candidate.join("personas").join("src").is_dir())
        || candidate
            .join("config")
            .join("provider_config.json")
            .exists()
        || (candidate.join("Cargo.toml").exists()
            && candidate.join("crates").join("gateway").is_dir())
}

fn provider_config_path_for_runtime_root(runtime_root: &Path) -> PathBuf {
    let packaged = runtime_root.join("config").join("provider_config.json");
    if packaged.exists() {
        return packaged;
    }
    let workspace = runtime_root
        .join("crates")
        .join("provider")
        .join("config")
        .join("provider_config.json");
    if workspace.exists() {
        return workspace;
    }
    packaged
}

fn gateway_tcp_reachable(endpoint: &GatewayEndpoint) -> bool {
    gateway_health_reachable(endpoint)
}

fn endpoint_for_gateway_start(endpoint: GatewayEndpoint) -> GatewayEndpoint {
    if gateway_port_available(&endpoint) {
        return endpoint;
    }
    available_endpoint_on_same_host(&endpoint).unwrap_or(endpoint)
}

fn gateway_port_available(endpoint: &GatewayEndpoint) -> bool {
    endpoint
        .bind_addrs()
        .into_iter()
        .any(|addr| TcpListener::bind(addr).is_ok())
}

fn available_endpoint_on_same_host(endpoint: &GatewayEndpoint) -> Option<GatewayEndpoint> {
    endpoint.bind_addrs().into_iter().find_map(|addr| {
        TcpListener::bind(SocketAddr::new(addr.ip(), 0))
            .ok()
            .and_then(|listener| listener.local_addr().ok())
            .map(|local| endpoint.with_port(local.port()))
    })
}

fn gateway_health_reachable(endpoint: &GatewayEndpoint) -> bool {
    endpoint.socket_addrs().into_iter().any(|addr| {
        let Ok(mut stream) = TcpStream::connect_timeout(&addr, Duration::from_millis(350)) else {
            return false;
        };
        let _ = stream.set_read_timeout(Some(Duration::from_millis(900)));
        let _ = stream.set_write_timeout(Some(Duration::from_millis(900)));
        let request = format!(
            "GET /global/health HTTP/1.1\r\nHost: {}:{}\r\nConnection: close\r\n\r\n",
            endpoint.host, endpoint.port
        );
        if stream.write_all(request.as_bytes()).is_err() {
            return false;
        }
        let mut response = String::new();
        stream.read_to_string(&mut response).is_ok()
            && response.starts_with("HTTP/1.1 200")
            && response.contains("\"healthy\":true")
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct GatewayEndpoint {
    host: String,
    port: u16,
    explicit_port: Option<u16>,
}

impl GatewayEndpoint {
    fn parse(gateway_url: &str) -> Self {
        let trimmed = gateway_url.trim();
        let parseable = if trimmed.is_empty() {
            "http://127.0.0.1".to_string()
        } else if trimmed.contains("://") {
            trimmed.to_string()
        } else {
            format!("http://{trimmed}")
        };
        let Ok(url) = Url::parse(&parseable) else {
            return Self::default();
        };
        let host = url
            .host_str()
            .unwrap_or("127.0.0.1")
            .trim_start_matches('[')
            .trim_end_matches(']')
            .to_string();
        let explicit_port = url.port();
        Self {
            host,
            port: explicit_port.unwrap_or(4096),
            explicit_port,
        }
    }

    fn socket_addrs(&self) -> Vec<std::net::SocketAddr> {
        use std::net::ToSocketAddrs;
        (self.host.as_str(), self.port)
            .to_socket_addrs()
            .map(|addrs| addrs.collect())
            .unwrap_or_default()
    }

    fn bind_addrs(&self) -> Vec<SocketAddr> {
        let addrs = self.socket_addrs();
        if addrs.is_empty() && self.host == "localhost" {
            return GatewayEndpoint {
                host: "127.0.0.1".to_string(),
                ..self.clone()
            }
            .socket_addrs();
        }
        addrs
    }

    fn url(&self) -> String {
        let host = if self.host.contains(':') {
            format!("[{}]", self.host)
        } else {
            self.host.clone()
        };
        format!("http://{host}:{}", self.port)
    }

    fn with_port(&self, port: u16) -> Self {
        Self {
            host: self.host.clone(),
            port,
            explicit_port: Some(port),
        }
    }
}

impl Default for GatewayEndpoint {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 4096,
            explicit_port: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::net::TcpListener;

    #[test]
    fn parses_gateway_endpoint_with_default_port() {
        assert_eq!(
            GatewayEndpoint::parse("http://127.0.0.1"),
            GatewayEndpoint {
                host: "127.0.0.1".to_string(),
                port: 4096,
                explicit_port: None,
            }
        );
    }

    #[test]
    fn parses_gateway_endpoint_with_explicit_port_path_and_query() {
        assert_eq!(
            GatewayEndpoint::parse("http://localhost:4100/global/health?probe=1"),
            GatewayEndpoint {
                host: "localhost".to_string(),
                port: 4100,
                explicit_port: Some(4100),
            }
        );
    }

    #[test]
    fn parses_bare_host_port_endpoint() {
        assert_eq!(
            GatewayEndpoint::parse("127.0.0.1:4101"),
            GatewayEndpoint {
                host: "127.0.0.1".to_string(),
                port: 4101,
                explicit_port: Some(4101),
            }
        );
    }

    #[test]
    fn parses_ipv6_endpoint() {
        assert_eq!(
            GatewayEndpoint::parse("http://[::1]:4102/global/health"),
            GatewayEndpoint {
                host: "::1".to_string(),
                port: 4102,
                explicit_port: Some(4102),
            }
        );
    }

    #[test]
    fn invalid_endpoint_falls_back_to_local_gateway_default() {
        assert_eq!(
            GatewayEndpoint::parse("http://[::1"),
            GatewayEndpoint::default()
        );
    }

    #[test]
    fn endpoint_url_formats_localhost_and_ipv6() {
        assert_eq!(
            GatewayEndpoint::parse("http://localhost:4100/global/health").url(),
            "http://localhost:4100"
        );
        assert_eq!(
            GatewayEndpoint::parse("http://[::1]:4102/global/health").url(),
            "http://[::1]:4102"
        );
    }

    #[test]
    fn occupied_unhealthy_port_uses_available_fallback_port() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind occupied listener");
        let port = listener.local_addr().expect("local addr").port();
        let endpoint = GatewayEndpoint {
            host: "127.0.0.1".to_string(),
            port,
            explicit_port: Some(port),
        };

        let next = endpoint_for_gateway_start(endpoint.clone());

        assert_ne!(next.port, endpoint.port);
        assert_eq!(next.host, endpoint.host);
        assert!(gateway_port_available(&next));
    }

    #[test]
    fn reachable_requires_gateway_health_response() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind local listener");
        let port = listener.local_addr().expect("local addr").port();
        let endpoint = GatewayEndpoint {
            host: "127.0.0.1".to_string(),
            port,
            explicit_port: Some(port),
        };
        std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept health check");
            let mut buffer = [0_u8; 512];
            let _ = stream.read(&mut buffer);
            stream
                .write_all(
                    b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 16\r\n\r\n{\"healthy\":true}",
                )
                .expect("write health response");
        });
        assert!(gateway_health_reachable(&endpoint));
    }

    #[test]
    fn open_tcp_port_without_health_response_is_not_reachable() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind local listener");
        let port = listener.local_addr().expect("local addr").port();
        let endpoint = GatewayEndpoint {
            host: "127.0.0.1".to_string(),
            port,
            explicit_port: Some(port),
        };
        std::thread::spawn(move || {
            let (_stream, _) = listener.accept().expect("accept probe");
            std::thread::sleep(Duration::from_millis(1_200));
        });
        assert!(!gateway_health_reachable(&endpoint));
    }

    #[test]
    fn start_gateway_returns_connected_when_endpoint_is_reachable() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind local listener");
        let port = listener.local_addr().expect("local addr").port();
        std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept health check");
            let mut buffer = [0_u8; 512];
            let _ = stream.read(&mut buffer);
            stream
                .write_all(
                    b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 16\r\n\r\n{\"healthy\":true}",
                )
                .expect("write health response");
        });

        let response =
            start_gateway(format!("http://127.0.0.1:{port}")).expect("start gateway response");

        assert!(response.ok);
        assert_eq!(response.status, "connected");
        assert_eq!(response.gateway_path, None);
        assert_eq!(
            response.gateway_url,
            Some(format!("http://127.0.0.1:{port}"))
        );
    }

    #[test]
    fn runtime_root_prefers_packaged_bin_layout() {
        let temp = test_temp_dir("packaged-bin-layout");
        let bin = temp.join("bin");
        fs::create_dir_all(bin.join("agents").join("src")).expect("create agents");
        fs::create_dir_all(bin.join("personas").join("src")).expect("create personas");
        let gateway = bin.join(if cfg!(windows) {
            "gateway.exe"
        } else {
            "gateway"
        });
        fs::write(&gateway, "").expect("write gateway");

        assert_eq!(runtime_root_for_gateway(&gateway), bin);
        let _ = fs::remove_dir_all(temp);
    }

    #[test]
    fn runtime_root_walks_from_target_debug_to_workspace_root() {
        let temp = test_temp_dir("target-debug-layout");
        let target_debug = temp.join("target").join("debug");
        fs::create_dir_all(&target_debug).expect("create target debug");
        fs::write(temp.join("Cargo.toml"), "[workspace]\n").expect("write Cargo.toml");
        fs::create_dir_all(temp.join("crates").join("gateway")).expect("create gateway crate");
        let gateway = target_debug.join(if cfg!(windows) {
            "gateway.exe"
        } else {
            "gateway"
        });
        fs::write(&gateway, "").expect("write gateway");

        assert_eq!(runtime_root_for_gateway(&gateway), temp);
        let _ = fs::remove_dir_all(test_temp_dir("target-debug-layout"));
    }

    #[test]
    fn provider_config_prefers_packaged_config_when_present() {
        let temp = test_temp_dir("packaged-config");
        let config = temp.join("config").join("provider_config.json");
        fs::create_dir_all(config.parent().expect("config parent")).expect("create config dir");
        fs::write(&config, "{}").expect("write config");

        assert_eq!(provider_config_path_for_runtime_root(&temp), config);
        let _ = fs::remove_dir_all(temp);
    }

    #[test]
    fn provider_config_falls_back_to_workspace_config_when_present() {
        let temp = test_temp_dir("workspace-config");
        let config = temp
            .join("crates")
            .join("provider")
            .join("config")
            .join("provider_config.json");
        fs::create_dir_all(config.parent().expect("config parent")).expect("create config dir");
        fs::write(&config, "{}").expect("write config");

        assert_eq!(provider_config_path_for_runtime_root(&temp), config);
        let _ = fs::remove_dir_all(temp);
    }

    fn test_temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("tura-tauri-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }
}
