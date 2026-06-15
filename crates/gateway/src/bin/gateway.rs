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

    let desired_port = std::env::var("PORT")
        .ok()
        .and_then(|value| value.trim().parse::<u16>().ok())
        .unwrap_or_else(default_port_for_exe);

    let gateway_lock = match gateway::process_lock::ProcessLock::acquire(
        &tura_path::instance_home(),
        "gateway",
        mode_for_exe(),
        None,
    ) {
        Ok(lock) => lock,
        Err(error) => {
            if gateway_root_on_port(desired_port).as_deref() == Some(my_root().as_str()) {
                println!("✅ Gateway for this home is already running on http://127.0.0.1:{desired_port}");
                return;
            }
            eprintln!("gateway ownership lock refused startup: {error:#}");
            std::process::exit(1);
        }
    };

    let port = match select_listen_port(desired_port) {
        PortDecision::Bind(port) => port,
        PortDecision::AlreadyOwned(port) => {
            println!("✅ Gateway for this directory is already running on http://127.0.0.1:{port}");
            return;
        }
        PortDecision::Unavailable(port) => {
            eprintln!("gateway port {port} is occupied by a foreign process; set PORT to an explicit free port or stop the foreign process");
            drop(gateway_lock);
            std::process::exit(1);
        }
    };
    // Keep the resolved port visible to children (runtime workers, callbacks).
    std::env::set_var("PORT", port.to_string());

    if let Err(error) = gateway::router_process::start_global_router_process() {
        eprintln!("gateway failed to start persistent router: {error:#}");
        drop(gateway_lock);
        std::process::exit(1);
    }
    start_router_front_heartbeat();
    install_stdin_eof_shutdown_watcher();
    let server_result = runtime.block_on(gateway::web::server::run_server(port));
    drop(gateway_lock);
    if let Err(error) = server_result {
        eprintln!("gateway server stopped with error: {error}");
        std::process::exit(1);
    }
}

/// Default gateway port for a bare invocation, derived from the compile-time
/// build-kind so the two independent routes never collide:
///   - `dev`     (the repo-local `bin/` dev package) → 4126
///   - `release` (the portable `release/` package)   → 4156
///
/// dev/release are distinguished by build-kind (`TURA_BUILD_KIND`) and isolated
/// by instance_home — never by the executable's file name. Launchers and the
/// TUI/GUI spawners always pass an explicit `PORT`, so this is only the fallback.
const DEV_GATEWAY_PORT: u16 = 4126;
const RELEASE_GATEWAY_PORT: u16 = 4156;

fn default_port_for_exe() -> u16 {
    if mode_for_exe() == "dev" {
        DEV_GATEWAY_PORT
    } else {
        RELEASE_GATEWAY_PORT
    }
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
fn select_listen_port(desired_port: u16) -> PortDecision {
    if port_is_free(desired_port) {
        return PortDecision::Bind(desired_port);
    }
    if gateway_root_on_port(desired_port).as_deref() == Some(my_root().as_str()) {
        return PortDecision::AlreadyOwned(desired_port);
    }
    PortDecision::Unavailable(desired_port)
}

fn port_is_free(port: u16) -> bool {
    TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, port))).is_ok()
}

fn my_root() -> String {
    // Single source of truth for project-root resolution (TURA_PROJECT_ROOT or
    // cwd, canonicalized + verbatim-stripped).
    tura_path::canonical_root().to_string_lossy().to_string()
}

/// Probe `/global/health` on a loopback port and return the gateway's reported
/// `root`, or `None` if the port is not a healthy Tura gateway.
fn gateway_root_on_port(port: u16) -> Option<String> {
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
    value
        .get("root")
        .and_then(serde_json::Value::as_str)
        .filter(|root| !root.is_empty())
        .map(str::to_string)
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

fn install_stdin_eof_shutdown_watcher() {
    if std::env::var("TURA_GATEWAY_SHUTDOWN_ON_STDIN_EOF")
        .ok()
        .as_deref()
        != Some("1")
    {
        return;
    }
    std::thread::spawn(|| {
        let mut stdin = std::io::stdin();
        let mut buffer = [0_u8; 1];
        loop {
            match stdin.read(&mut buffer) {
                Ok(0) => break,
                Ok(_) => continue,
                Err(_) => break,
            }
        }
        std::process::exit(0);
    });
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
        .find(|candidate| {
            candidate.join("agents").join("src").is_dir()
                || candidate.join("personas").join("src").is_dir()
                || candidate
                    .join("config")
                    .join("provider_config.json")
                    .exists()
        })
        .map(Path::to_path_buf)
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
