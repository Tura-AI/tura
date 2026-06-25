#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

use serde::Serialize;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};
use url::Url;

const GATEWAY_HEALTH_TIMEOUT: Duration = Duration::from_secs(20);
const GATEWAY_HEALTH_POLL_INTERVAL: Duration = Duration::from_millis(500);
const GATEWAY_BUILD_KIND: &str = "release";

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct StartGatewayResponse {
    ok: bool,
    status: &'static str,
    gateway_path: Option<String>,
    gateway_url: Option<String>,
}

static OWNED_GATEWAY: OnceLock<Mutex<Option<Child>>> = OnceLock::new();

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![start_gateway, open_external_url])
        .run(tauri::generate_context!())
        .expect("failed to run tura_gui");
}

#[tauri::command]
fn open_external_url(url: String) -> Result<(), String> {
    let parsed = parse_external_url(&url)?;
    open_url_in_default_browser(parsed.as_str())
}

#[tauri::command]
fn start_gateway(gateway_url: String) -> Result<StartGatewayResponse, String> {
    let endpoint = GatewayEndpoint::parse(&gateway_url);
    let my_root = current_runtime_root();
    let instance_home = instance_home_for_runtime_root(&my_root);
    // Only reuse a reachable gateway if it belongs to *this* package directory.
    // A gateway from another route (dev bin / release) on the same port must not
    // be hijacked — we start our own on a free port instead.
    if let Some(root) = gateway_identity(&endpoint) {
        if root.is_empty() || same_root(&root, &my_root) {
            return Ok(StartGatewayResponse {
                ok: true,
                status: "connected",
                gateway_path: None,
                gateway_url: Some(endpoint.url()),
            });
        }
    }
    if !gateway_port_available(&endpoint) {
        terminate_gateway_from_lock(&instance_home, &endpoint)?;
    }
    if !gateway_port_available(&endpoint) {
        return Err(format!(
            "gateway port {} is occupied by a foreign process",
            endpoint.port
        ));
    }

    let gateway = gateway_binary_path().ok_or_else(|| "gateway binary not found".to_string())?;
    let runtime_root = runtime_root_for_gateway(&gateway);
    let instance_home = instance_home_for_runtime_root(&runtime_root);

    for attempt in 0..2 {
        let child = spawn_gateway_child(&gateway, &runtime_root, &instance_home, &endpoint)?;
        *owned_gateway()
            .lock()
            .map_err(|_| "gateway child lock poisoned".to_string())? = Some(child);
        if wait_for_gateway_health(&endpoint, GATEWAY_HEALTH_TIMEOUT) {
            return Ok(StartGatewayResponse {
                ok: true,
                status: "connected",
                gateway_path: Some(gateway.display().to_string()),
                gateway_url: Some(endpoint.url()),
            });
        }
        let killed_by_lock = terminate_gateway_from_lock(&instance_home, &endpoint)?;
        let killed_owned_child = force_kill_owned_gateway();
        if !killed_by_lock && !killed_owned_child {
            return Err(format!(
                "gateway did not become healthy after {} seconds and could not be killed",
                GATEWAY_HEALTH_TIMEOUT.as_secs()
            ));
        }
        if attempt == 0 {
            continue;
        }
    }

    Err(format!(
        "gateway did not become healthy after {} seconds",
        GATEWAY_HEALTH_TIMEOUT.as_secs()
    ))
}

fn spawn_gateway_child(
    gateway: &Path,
    runtime_root: &Path,
    instance_home: &Path,
    endpoint: &GatewayEndpoint,
) -> Result<Child, String> {
    let mut command = Command::new(gateway);
    command
        .current_dir(runtime_root)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .env("TURA_HOME", instance_home)
        .env("TURA_PROJECT_ROOT", runtime_root)
        .env("TURA_GATEWAY_SHUTDOWN_ON_STDIN_EOF", "1")
        .env(
            "TURA_PROVIDER_CONFIG",
            provider_config_path_for_runtime_root(runtime_root),
        )
        .env("TURA_ENV_PATH", runtime_root.join(".env"))
        .env("PORT", endpoint.port.to_string());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    command
        .spawn()
        .map_err(|err| format!("failed to start gateway {}: {err}", gateway.display()))
}

fn open_url_in_default_browser(url: &str) -> Result<(), String> {
    let mut command = default_browser_command(url);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    command
        .spawn()
        .map(|_| ())
        .map_err(|err| format!("failed to open url in default browser: {err}"))
}

fn parse_external_url(url: &str) -> Result<Url, String> {
    let parsed = Url::parse(url.trim()).map_err(|err| format!("invalid url: {err}"))?;
    if !matches!(parsed.scheme(), "http" | "https" | "file") {
        return Err("only http, https, and file urls can be opened externally".to_string());
    }
    Ok(parsed)
}

fn default_browser_command(url: &str) -> Command {
    #[cfg(windows)]
    {
        let mut command = Command::new("rundll32.exe");
        command.args(["url.dll,FileProtocolHandler", url]);
        command
    }
    #[cfg(target_os = "macos")]
    {
        let mut command = Command::new("open");
        command.arg(url);
        command
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let mut command = Command::new("xdg-open");
        command.arg(url);
        command
    }
}

fn owned_gateway() -> &'static Mutex<Option<Child>> {
    OWNED_GATEWAY.get_or_init(|| Mutex::new(None))
}

fn force_kill_owned_gateway() -> bool {
    let child = owned_gateway()
        .lock()
        .ok()
        .and_then(|mut guard| guard.take());
    let Some(mut child) = child else {
        return false;
    };
    match child.try_wait() {
        Ok(Some(_)) => true,
        Ok(None) => {
            let killed = child.kill().is_ok();
            let _ = child.wait();
            killed
        }
        Err(_) => false,
    }
}

fn gateway_binary_path() -> Option<PathBuf> {
    let gateway_name = if cfg!(windows) {
        "tura_gateway.exe"
    } else {
        "tura_gateway"
    };
    let exe = std::env::current_exe().ok()?;
    let exe_dir = exe.parent()?;
    let parent = exe_dir.parent().unwrap_or(exe_dir);
    let mut candidates = vec![
        exe_dir.join(gateway_name),
        exe_dir.join("bin").join(gateway_name),
        parent.join("bin").join(gateway_name),
        parent.join("release").join("bin").join(gateway_name),
    ];
    if let Some(root) = exe_dir
        .ancestors()
        .find(|candidate| is_runtime_root(candidate))
    {
        candidates.push(root.join("bin").join(gateway_name));
        candidates.push(root.join("target").join("release").join(gateway_name));
    }
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

fn instance_home_for_runtime_root(runtime_root: &Path) -> PathBuf {
    std::env::var_os("TURA_HOME")
        .map(PathBuf::from)
        .filter(|path| !path.as_os_str().is_empty())
        .map(|path| normalize_path(&path))
        .unwrap_or_else(|| normalize_path(runtime_root))
}

/// Runtime root the running GUI belongs to (its own package directory).
fn current_runtime_root() -> PathBuf {
    let exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("."));
    let start = exe.parent().unwrap_or_else(|| Path::new("."));
    start
        .ancestors()
        .find(|candidate| is_runtime_root(candidate))
        .map(Path::to_path_buf)
        .unwrap_or_else(|| start.to_path_buf())
}

fn same_root(left: &str, right: &Path) -> bool {
    fn canonical(path: &Path) -> String {
        comparable_path(path)
    }
    canonical(Path::new(left)) == canonical(right)
}

fn normalize_path(path: &Path) -> PathBuf {
    let resolved = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    PathBuf::from(strip_verbatim(&resolved.to_string_lossy()))
}

fn comparable_path(path: &Path) -> String {
    let text = normalize_path(path).to_string_lossy().to_string();
    let text = text.trim_end_matches(['\\', '/']).to_string();
    if cfg!(windows) {
        text.to_lowercase()
    } else {
        text
    }
}

fn strip_verbatim(path: &str) -> String {
    if let Some(rest) = path.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{rest}")
    } else if let Some(rest) = path.strip_prefix(r"\\?\") {
        rest.to_string()
    } else {
        path.to_string()
    }
}

fn gateway_port_available(endpoint: &GatewayEndpoint) -> bool {
    endpoint
        .bind_addrs()
        .into_iter()
        .any(|addr| TcpListener::bind(addr).is_ok())
}

#[cfg_attr(not(test), allow(dead_code))]
fn gateway_health_reachable(endpoint: &GatewayEndpoint) -> bool {
    gateway_identity(endpoint).is_some()
}

fn wait_for_gateway_health(endpoint: &GatewayEndpoint, timeout: Duration) -> bool {
    let started = Instant::now();
    while started.elapsed() < timeout {
        if gateway_health_reachable(endpoint) {
            return true;
        }
        std::thread::sleep(GATEWAY_HEALTH_POLL_INTERVAL);
    }
    false
}

/// Probe `/global/health`; on a healthy gateway return its reported `root`,
/// otherwise `None`.
fn gateway_identity(endpoint: &GatewayEndpoint) -> Option<String> {
    endpoint.socket_addrs().into_iter().find_map(|addr| {
        let mut stream = TcpStream::connect_timeout(&addr, Duration::from_millis(350)).ok()?;
        let _ = stream.set_read_timeout(Some(Duration::from_millis(900)));
        let _ = stream.set_write_timeout(Some(Duration::from_millis(900)));
        let request = format!(
            "GET /global/health HTTP/1.1\r\nHost: {}:{}\r\nConnection: close\r\n\r\n",
            endpoint.host, endpoint.port
        );
        stream.write_all(request.as_bytes()).ok()?;
        let mut response = String::new();
        stream.read_to_string(&mut response).ok()?;
        if !response.starts_with("HTTP/1.1 200") || !response.contains("\"healthy\":true") {
            return None;
        }
        let root = response
            .split("\r\n\r\n")
            .nth(1)
            .and_then(|body| serde_json::from_str::<serde_json::Value>(body.trim()).ok())
            .and_then(|value| {
                value
                    .get("root")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_string)
            })
            .unwrap_or_default();
        Some(root)
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
            // The packaged Tauri GUI is the release route, so the default port is
            // the release gateway's 4156 (the dev GUI runs via the vite dev server,
            // which targets 4126 explicitly).
            port: explicit_port.unwrap_or(4156),
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
}

#[derive(Debug, Default)]
struct GatewayLockRecord {
    pid: Option<u32>,
    process_start_time: Option<u64>,
    kind: Option<String>,
    mode: Option<String>,
    port: Option<u16>,
    root: Option<String>,
}

fn terminate_gateway_from_lock(
    instance_home: &Path,
    endpoint: &GatewayEndpoint,
) -> Result<bool, String> {
    let Some(record) = read_gateway_lock(instance_home) else {
        return Ok(false);
    };
    if record.kind.as_deref() != Some("gateway")
        || record.mode.as_deref() != Some(GATEWAY_BUILD_KIND)
        || record.port != Some(endpoint.port)
        || !record
            .root
            .as_deref()
            .is_some_and(|root| comparable_path(Path::new(root)) == comparable_path(instance_home))
    {
        return Ok(false);
    }
    let Some(pid) = record.pid else {
        return Ok(false);
    };
    if pid == std::process::id() {
        return Ok(false);
    }
    let Some(expected_start) = record.process_start_time else {
        return Ok(false);
    };
    if current_process_start_time(pid) != Some(expected_start) {
        return Ok(false);
    }
    let mut system = sysinfo::System::new_all();
    system.refresh_processes();
    let Some(process) = system.process(sysinfo::Pid::from_u32(pid)) else {
        return Ok(false);
    };
    Ok(process.kill())
}

fn read_gateway_lock(instance_home: &Path) -> Option<GatewayLockRecord> {
    let path = instance_home
        .join(".tura")
        .join("locks")
        .join(format!("gateway-{GATEWAY_BUILD_KIND}.lock"));
    let raw = std::fs::read_to_string(path).ok()?;
    let mut record = GatewayLockRecord::default();
    for line in raw.lines() {
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        match key.trim() {
            "pid" => record.pid = value.trim().parse().ok(),
            "process_start_time" => record.process_start_time = value.trim().parse().ok(),
            "kind" => record.kind = Some(value.trim().to_string()),
            "mode" => record.mode = Some(value.trim().to_string()),
            "port" => record.port = value.trim().parse().ok(),
            "root" => record.root = Some(value.trim().to_string()),
            _ => {}
        }
    }
    Some(record)
}

fn current_process_start_time(pid: u32) -> Option<u64> {
    let mut system = sysinfo::System::new_all();
    system.refresh_processes();
    system
        .process(sysinfo::Pid::from_u32(pid))
        .map(sysinfo::Process::start_time)
}
impl Default for GatewayEndpoint {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 4156,
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
    fn parse_external_url_accepts_http_and_https_urls() {
        assert_eq!(
            parse_external_url(" https://example.com/oauth?code=abc ")
                .expect("https url")
                .as_str(),
            "https://example.com/oauth?code=abc"
        );
        assert_eq!(
            parse_external_url("http://localhost:3000/callback")
                .expect("http url")
                .as_str(),
            "http://localhost:3000/callback"
        );
    }

    #[test]
    fn parse_external_url_rejects_non_web_urls() {
        assert!(parse_external_url("javascript:alert(1)").is_err());
        assert!(parse_external_url("not a url").is_err());
    }

    #[test]
    fn parse_external_url_accepts_file_urls_for_local_links() {
        assert_eq!(
            parse_external_url(" file:///C:/Users/liuliu/Documents/tura/Cargo.toml ")
                .expect("file url")
                .as_str(),
            "file:///C:/Users/liuliu/Documents/tura/Cargo.toml"
        );
    }

    #[cfg(windows)]
    #[test]
    fn default_browser_command_on_windows_uses_system_url_handler() {
        let command = default_browser_command("https://example.com/oauth?a=1&b=2");

        assert_eq!(command.get_program(), "rundll32.exe");
        assert_eq!(
            command
                .get_args()
                .map(|arg| arg.to_string_lossy().into_owned())
                .collect::<Vec<_>>(),
            vec![
                "url.dll,FileProtocolHandler".to_string(),
                "https://example.com/oauth?a=1&b=2".to_string(),
            ]
        );
    }

    #[test]
    fn parses_gateway_endpoint_with_default_port() {
        assert_eq!(
            GatewayEndpoint::parse("http://127.0.0.1"),
            GatewayEndpoint {
                host: "127.0.0.1".to_string(),
                port: 4156,
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
    fn occupied_unhealthy_port_is_not_silently_remapped() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind occupied listener");
        let port = listener.local_addr().expect("local addr").port();
        let endpoint = GatewayEndpoint {
            host: "127.0.0.1".to_string(),
            port,
            explicit_port: Some(port),
        };

        assert!(!gateway_port_available(&endpoint));
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
            "tura_gateway.exe"
        } else {
            "tura_gateway"
        });
        fs::write(&gateway, "").expect("write gateway");

        assert_eq!(runtime_root_for_gateway(&gateway), bin);
        let _ = fs::remove_dir_all(temp);
    }

    #[test]
    fn runtime_root_walks_from_target_release_to_workspace_root() {
        let temp = test_temp_dir("target-release-layout");
        let target_release = temp.join("target").join("release");
        fs::create_dir_all(&target_release).expect("create target release");
        fs::write(temp.join("Cargo.toml"), "[workspace]\n").expect("write Cargo.toml");
        fs::create_dir_all(temp.join("crates").join("gateway")).expect("create gateway crate");
        let gateway = target_release.join(if cfg!(windows) {
            "tura_gateway.exe"
        } else {
            "tura_gateway"
        });
        fs::write(&gateway, "").expect("write gateway");

        assert_eq!(runtime_root_for_gateway(&gateway), temp);
        let _ = fs::remove_dir_all(test_temp_dir("target-release-layout"));
    }

    #[test]
    fn gateway_lock_must_match_release_home_port_and_start_time() {
        let temp = test_temp_dir("gateway-lock-scope");
        let foreign_home = test_temp_dir("gateway-lock-foreign");
        let endpoint = GatewayEndpoint {
            host: "127.0.0.1".to_string(),
            port: 4156,
            explicit_port: Some(4156),
        };
        let pid = std::process::id();
        let start_time = current_process_start_time(pid).expect("current start time");
        let lock_dir = temp.join(".tura").join("locks");
        fs::create_dir_all(&lock_dir).expect("lock dir");
        let lock_path = lock_dir.join("gateway-release.lock");

        fs::write(
            &lock_path,
            format!(
                "pid={pid}\nprocess_start_time={start_time}\nkind=gateway\nmode=dev\nport=4156\nroot={}\n",
                temp.display()
            ),
        )
        .expect("dev lock");
        assert!(!terminate_gateway_from_lock(&temp, &endpoint).expect("dev lock should not kill"));

        fs::write(
            &lock_path,
            format!(
                "pid={pid}\nprocess_start_time={start_time}\nkind=gateway\nmode=release\nport=4156\nroot={}\n",
                foreign_home.display()
            ),
        )
        .expect("foreign home lock");
        assert!(
            !terminate_gateway_from_lock(&temp, &endpoint).expect("foreign lock should not kill")
        );

        fs::write(
            &lock_path,
            format!(
                "pid={pid}\nprocess_start_time={}\nkind=gateway\nmode=release\nport=4156\nroot={}\n",
                start_time.saturating_sub(1),
                temp.display()
            ),
        )
        .expect("stale start lock");
        assert!(!terminate_gateway_from_lock(&temp, &endpoint).expect("stale lock should not kill"));

        let _ = fs::remove_dir_all(temp);
        let _ = fs::remove_dir_all(foreign_home);
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
