use serde_json::json;
use std::io::{Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::time::Duration;

fn main() {
    tura_path::process_hardening::harden_current_process("gateway");
    configure_release_runtime_env();

    if std::env::args().nth(1).as_deref() == Some("session-log") {
        run_session_log_command();
        return;
    }

    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => {
            eprintln!("gateway tokio runtime failed to start: {error}");
            std::process::exit(1);
        }
    };

    if std::env::var("OPENAI_LOGIN")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .is_none()
    {
        std::env::set_var("OPENAI_LOGIN", "oauth");
    }

    let desired_port = desired_port_for_exe();

    let port = match select_listen_port(desired_port) {
        PortDecision::Bind(port) => port,
        PortDecision::AlreadyOwned(port) => {
            write_active_gateway_url(port);
            println!("✅ Gateway for this directory is already running on http://127.0.0.1:{port}");
            return;
        }
        PortDecision::Unavailable(port) => {
            eprintln!("gateway port {port} is occupied by a foreign process; set PORT to an explicit free port or stop the foreign process");
            std::process::exit(1);
        }
    };

    let gateway_lock = match gateway::process_lock::ProcessLock::acquire(
        &tura_path::instance_home(),
        "gateway",
        mode_for_exe(),
        Some(port),
    ) {
        Ok(lock) => lock,
        Err(error) => {
            if gateway_identity_on_port(port).is_some_and(|identity| identity.matches_instance()) {
                write_active_gateway_url(port);
                println!("✅ Gateway for this home is already running on http://127.0.0.1:{port}");
                return;
            }
            eprintln!("gateway ownership lock refused startup: {error:#}");
            std::process::exit(1);
        }
    };
    // Keep the resolved port visible to children (runtime workers, callbacks).
    std::env::set_var("PORT", port.to_string());
    std::env::set_var(tura_path::TURA_GATEWAY_PORT_ENV, port.to_string());

    if let Err(error) = gateway::router_process::start_global_router_process() {
        eprintln!("gateway failed to start persistent router: {error:#}");
        drop(gateway_lock);
        std::process::exit(1);
    }
    start_router_front_heartbeat();
    let stdin_eof_shutdown = install_stdin_eof_shutdown_watcher();
    let stdin_eof_lifecycle = stdin_eof_shutdown.is_some();
    if !stdin_eof_lifecycle && gateway::tray::tray_enabled() {
        match gateway::tray::GatewayTrayApp::new(port) {
            Ok(tray) => {
                std::thread::spawn(move || {
                    let server_result = run_gateway_server(runtime, port, stdin_eof_shutdown);
                    shutdown_global_router_process("gateway server exit");
                    if let Err(error) = server_result {
                        eprintln!("gateway server stopped with error: {error}");
                        std::process::exit(1);
                    }
                    drop(gateway_lock);
                });
                tray.run();
                shutdown_global_router_process("gateway tray quit");
                return;
            }
            Err(error) => {
                eprintln!("gateway tray unavailable; continuing without tray icon: {error:#}");
            }
        }
    }

    let server_result = run_gateway_server(runtime, port, stdin_eof_shutdown);
    shutdown_global_router_process("gateway server exit");
    drop(gateway_lock);
    if let Err(error) = server_result {
        eprintln!("gateway server stopped with error: {error}");
        std::process::exit(1);
    }
}

fn run_gateway_server(
    runtime: tokio::runtime::Runtime,
    port: u16,
    shutdown: Option<tokio::sync::oneshot::Receiver<()>>,
) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(shutdown) = shutdown {
        return runtime.block_on(gateway::web::server::run_server_until_shutdown(
            port,
            async move {
                let _ = shutdown.await;
            },
        ));
    }
    runtime.block_on(gateway::web::server::run_server(port))
}

fn shutdown_global_router_process(reason: &str) {
    match gateway::router_process::global_router_process().and_then(|router| router.shutdown()) {
        Ok(payload) => eprintln!("gateway shutdown ({reason}): router shutdown {payload}"),
        Err(error) => eprintln!("gateway shutdown ({reason}): router shutdown failed: {error:#}"),
    }
}

/// Default gateway port for a bare invocation, derived from the build kind:
///   - `dev`     → 4125
///   - `release` → 4126
///
/// dev/release are distinguished by build-kind (`TURA_BUILD_KIND`) and isolated
/// by instance_home — never by the executable's file name. Launchers and the
/// TUI/GUI spawners always pass an explicit `PORT`, so this is only the fallback.
fn default_port_for_exe() -> u16 {
    tura_path::default_gateway_port_for_build_kind(mode_for_exe())
}

#[derive(Clone, Copy)]
struct PortPreference {
    port: u16,
    explicit: bool,
}

fn desired_port_for_exe() -> PortPreference {
    if let Some(port) = env_port("PORT") {
        return PortPreference {
            port,
            explicit: true,
        };
    }
    if let Some(port) = env_port(tura_path::TURA_GATEWAY_PORT_ENV) {
        return PortPreference {
            port,
            explicit: false,
        };
    }
    PortPreference {
        port: default_port_for_exe(),
        explicit: false,
    }
}

fn env_port(key: &str) -> Option<u16> {
    std::env::var(key)
        .ok()
        .and_then(|value| value.trim().parse::<u16>().ok())
}

fn mode_for_exe() -> &'static str {
    tura_path::build_kind()
}

enum PortDecision {
    /// Bind and serve on this port.
    Bind(u16),
    /// A gateway for this same directory already owns the port; do nothing.
    AlreadyOwned(u16),
    /// A different process owns the desired port.
    Unavailable(u16),
}

/// Resolve which port this standalone gateway should bind.
///
/// Prefer the fixed (per-package) port. If it is occupied by our own
/// directory's gateway, report it as already-running. If it is occupied by a
/// foreign process, fail instead of silently floating to another port.
fn select_listen_port(desired: PortPreference) -> PortDecision {
    if port_is_free(desired.port) {
        return PortDecision::Bind(desired.port);
    }
    if gateway_identity_on_port(desired.port).is_some_and(|identity| identity.matches_instance()) {
        return PortDecision::AlreadyOwned(desired.port);
    }
    if desired.explicit {
        return PortDecision::Unavailable(desired.port);
    }
    fallback_ports(desired.port)
        .into_iter()
        .find(|port| port_is_free(*port))
        .map(PortDecision::Bind)
        .unwrap_or(PortDecision::Unavailable(desired.port))
}

fn fallback_ports(desired_port: u16) -> Vec<u16> {
    (1..=100)
        .filter_map(|offset| desired_port.checked_add(offset))
        .collect()
}

fn port_is_free(port: u16) -> bool {
    TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, port))).is_ok()
}

fn my_root() -> String {
    // Single source of truth for project-root resolution (TURA_PROJECT_ROOT or
    // cwd, canonicalized + verbatim-stripped).
    tura_path::canonical_root().to_string_lossy().to_string()
}

fn my_home() -> String {
    tura_path::instance_home().to_string_lossy().to_string()
}

fn write_active_gateway_url(port: u16) {
    let url = format!("http://127.0.0.1:{port}");
    if let Err(error) =
        tura_path::write_active_gateway_url_for_home(tura_path::instance_home(), &url)
    {
        eprintln!("gateway failed to write active URL {url}: {error}");
    }
}

#[derive(Debug, Clone, Default)]
struct GatewayIdentity {
    root: String,
    home: String,
}

impl GatewayIdentity {
    fn matches_instance(&self) -> bool {
        if !self.home.trim().is_empty() {
            return same_path(&self.home, &my_home());
        }
        same_path(&self.root, &my_root())
    }
}

fn same_path(left: &str, right: &str) -> bool {
    let left = comparable_path(Path::new(left));
    let right = comparable_path(Path::new(right));
    !left.is_empty() && left == right
}

fn comparable_path(path: &Path) -> String {
    let text = tura_path::normalize_path(path)
        .to_string_lossy()
        .to_string();
    let text = text.trim_end_matches(['\\', '/']).to_string();
    if cfg!(windows) {
        text.to_lowercase()
    } else {
        text
    }
}

/// Probe `/global/health` on a loopback port and return the gateway's reported
/// identity, or `None` if the port is not a healthy tura_gateway.
fn gateway_identity_on_port(port: u16) -> Option<GatewayIdentity> {
    let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
    let mut stream = TcpStream::connect_timeout(&addr, Duration::from_millis(400)).ok()?;
    stream
        .set_read_timeout(Some(Duration::from_millis(900)))
        .ok()?;
    stream
        .set_write_timeout(Some(Duration::from_millis(900)))
        .ok()?;
    let request = format!(
        "GET /global/health HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n\r\n"
    );
    stream.write_all(request.as_bytes()).ok()?;
    let mut response = String::new();
    stream.read_to_string(&mut response).ok()?;
    if !response.starts_with("HTTP/1.1 200") {
        return None;
    }
    let body = response.split("\r\n\r\n").nth(1)?;
    let value: serde_json::Value = serde_json::from_str(body.trim()).ok()?;
    let root = value
        .get("root")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_string();
    let home = value
        .get("home")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_string();
    Some(GatewayIdentity { root, home })
}

fn configure_release_runtime_env() {
    let root = project_root_from_exe();
    if std::env::var_os("TURA_PROJECT_ROOT").is_none() {
        std::env::set_var("TURA_PROJECT_ROOT", &root);
    }
    if std::env::var_os("TURA_PROVIDER_CONFIG").is_none() {
        let provider_config = PathBuf::from(&root)
            .join("config")
            .join("provider_config.json");
        if provider_config.exists() {
            std::env::set_var("TURA_PROVIDER_CONFIG", provider_config);
        }
    }
    if std::env::var_os("TURA_ENV_PATH").is_none() {
        let env_path = PathBuf::from(&root).join(".env");
        if env_path.exists() {
            std::env::set_var("TURA_ENV_PATH", env_path);
        }
    }
}

fn install_stdin_eof_shutdown_watcher() -> Option<tokio::sync::oneshot::Receiver<()>> {
    if std::env::var("TURA_GATEWAY_SHUTDOWN_ON_STDIN_EOF")
        .ok()
        .as_deref()
        != Some("1")
    {
        return None;
    }
    let (tx, rx) = tokio::sync::oneshot::channel();
    std::thread::spawn(move || {
        let mut stdin = std::io::stdin();
        let mut buffer = [0_u8; 1];
        loop {
            match stdin.read(&mut buffer) {
                Ok(0) => break,
                Ok(_) => continue,
                Err(_) => break,
            }
        }
        let _ = tx.send(());
    });
    Some(rx)
}

fn start_router_front_heartbeat() {
    let front_id = format!("gateway-{}-{}", std::process::id(), uuid::Uuid::new_v4());
    let ttl = gateway_router_lease_ttl();
    let interval = ttl.div_f64(3.0).max(Duration::from_secs(1));
    std::thread::spawn(move || loop {
        if let Ok(router_process) = gateway::router_process::global_router_process() {
            let _ = router_process.call(
                "lifecycle.front_heartbeat",
                json!({
                    "front_id": front_id,
                    "kind": "gateway",
                    "ttl_ms": ttl.as_millis() as u64,
                }),
            );
        }
        std::thread::sleep(interval);
    });
}

fn gateway_router_lease_ttl() -> Duration {
    std::env::var("TURA_GATEWAY_ROUTER_LEASE_TTL_SECS")
        .ok()
        .and_then(|value| value.trim().parse::<u64>().ok())
        .filter(|seconds| *seconds > 0)
        .map(Duration::from_secs)
        .unwrap_or_else(|| Duration::from_secs(15))
}

fn project_root_from_exe() -> PathBuf {
    std::env::current_exe()
        .ok()
        .as_deref()
        .and_then(find_release_root_from)
        .or_else(|| {
            std::env::current_dir()
                .ok()
                .as_deref()
                .and_then(find_release_root_from)
        })
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
}

fn find_release_root_from(path: &Path) -> Option<PathBuf> {
    let start = if path.is_dir() {
        path
    } else {
        path.parent().unwrap_or(path)
    };
    start
        .ancestors()
        .find(|candidate| release_root_has_runtime_sources(candidate))
        .map(Path::to_path_buf)
}

fn release_root_has_runtime_sources(candidate: &Path) -> bool {
    candidate
        .join("crates")
        .join("tools")
        .join("src")
        .join("command_run")
        .join("schema.json")
        .is_file()
        && (candidate.join("agents").join("src").is_dir()
            || candidate.join("personas").join("src").is_dir()
            || candidate.join("Cargo.toml").is_file())
}

fn run_session_log_command() {
    let result = {
        use std::io::Read;
        let mut raw = String::new();
        std::io::stdin()
            .read_to_string(&mut raw)
            .map_err(anyhow::Error::from)
            .and_then(|_| serde_json::from_str(raw.trim()).map_err(anyhow::Error::from))
            .and_then(|command| {
                gateway::session_db_client::SessionDbClient::discover()?.call(command)
            })
    };
    match result {
        Ok(response) => println!(
            "{}",
            match serde_json::to_string(&response) {
                Ok(encoded) => encoded,
                Err(error) => {
                    eprintln!("session-log response serialization failed: {error}");
                    std::process::exit(1);
                }
            }
        ),
        Err(error) => {
            eprintln!("session-log session_db command failed: {error:#}");
            std::process::exit(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        desired_port_for_exe, find_release_root_from, select_listen_port, PortDecision,
        PortPreference,
    };
    use std::net::TcpListener;

    #[test]
    fn release_root_skips_target_release_config_without_runtime_tools() {
        let temp = tempfile::tempdir().expect("temp dir");
        let root = temp.path();
        let target_release = root.join("target").join("release");

        std::fs::create_dir_all(root.join("agents").join("src")).expect("agents dir");
        std::fs::create_dir_all(
            root.join("crates")
                .join("tools")
                .join("src")
                .join("command_run"),
        )
        .expect("command_run dir");
        std::fs::write(root.join("Cargo.toml"), "[workspace]\n").expect("cargo toml");
        std::fs::write(
            root.join("crates")
                .join("tools")
                .join("src")
                .join("command_run")
                .join("schema.json"),
            "{}",
        )
        .expect("command_run schema");

        std::fs::create_dir_all(target_release.join("config")).expect("release config dir");
        std::fs::write(
            target_release.join("config").join("provider_config.json"),
            "{}",
        )
        .expect("release provider config");

        assert_eq!(
            find_release_root_from(&target_release.join("tura_gateway.exe")),
            Some(root.to_path_buf())
        );
    }

    #[test]
    fn port_env_prefers_explicit_port_then_non_explicit_gateway_port() {
        let _guard = ENV_LOCK.lock().expect("env lock");
        let env = TestEnv::set([("PORT", "4311"), (tura_path::TURA_GATEWAY_PORT_ENV, "4312")]);
        let desired = desired_port_for_exe();
        assert_eq!(desired.port, 4311);
        assert!(desired.explicit);
        drop(env);

        let env = TestEnv::set([("PORT", ""), (tura_path::TURA_GATEWAY_PORT_ENV, "4312")]);
        let desired = desired_port_for_exe();
        assert_eq!(desired.port, 4312);
        assert!(!desired.explicit);
        drop(env);
    }

    #[test]
    fn non_explicit_occupied_port_falls_back_to_free_loopback_port() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind occupied port");
        let occupied = listener.local_addr().expect("local addr").port();

        let decision = select_listen_port(PortPreference {
            port: occupied,
            explicit: false,
        });

        match decision {
            PortDecision::Bind(port) => assert_ne!(port, occupied),
            _ => panic!("expected fallback bind decision"),
        }
    }

    #[test]
    fn non_explicit_occupied_port_reuses_same_home_gateway_with_different_root() {
        let _guard = ENV_LOCK.lock().expect("env lock");
        let home = tempfile::tempdir().expect("home");
        let project_root = tempfile::tempdir().expect("project root");
        let other_root = tempfile::tempdir().expect("other root");
        let home_text = home.path().to_string_lossy().to_string();
        let project_root_text = project_root.path().to_string_lossy().to_string();
        let env = TestEnv::set([
            ("TURA_HOME", home_text.as_str()),
            ("TURA_PROJECT_ROOT", project_root_text.as_str()),
        ]);
        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind gateway port");
        let occupied = listener.local_addr().expect("local addr").port();
        let other_root_text = other_root.path().to_string_lossy().to_string();
        std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept health probe");
            let mut buffer = [0_u8; 512];
            let _ = std::io::Read::read(&mut stream, &mut buffer);
            std::io::Write::write_all(
                &mut stream,
                format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\r\n{{\"healthy\":true,\"root\":{},\"home\":{}}}",
                    serde_json::to_string(&other_root_text).expect("json root"),
                    serde_json::to_string(&home_text).expect("json home")
                )
                .as_bytes(),
            )
            .expect("write health response");
        });

        let decision = select_listen_port(PortPreference {
            port: occupied,
            explicit: false,
        });

        match decision {
            PortDecision::AlreadyOwned(port) => assert_eq!(port, occupied),
            _ => panic!("expected already-owned decision"),
        }
        drop(env);
    }

    #[test]
    fn explicit_occupied_port_is_unavailable() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind occupied port");
        let occupied = listener.local_addr().expect("local addr").port();

        let decision = select_listen_port(PortPreference {
            port: occupied,
            explicit: true,
        });

        match decision {
            PortDecision::Unavailable(port) => assert_eq!(port, occupied),
            _ => panic!("expected unavailable decision"),
        }
    }

    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    struct TestEnv {
        previous: Vec<(&'static str, Option<std::ffi::OsString>)>,
    }

    impl TestEnv {
        fn set<const N: usize>(values: [(&'static str, &str); N]) -> Self {
            let previous = values
                .iter()
                .map(|(key, _)| (*key, std::env::var_os(key)))
                .collect::<Vec<_>>();
            for (key, value) in values {
                if value.is_empty() {
                    std::env::remove_var(key);
                } else {
                    std::env::set_var(key, value);
                }
            }
            Self { previous }
        }
    }

    impl Drop for TestEnv {
        fn drop(&mut self) {
            for (key, value) in self.previous.drain(..).rev() {
                if let Some(value) = value {
                    std::env::set_var(key, value);
                } else {
                    std::env::remove_var(key);
                }
            }
        }
    }
}
