#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

use base64::{engine::general_purpose, Engine as _};
use serde::Serialize;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;
use sysinfo::{Pid, ProcessRefreshKind, System, UpdateKind};
use tauri::webview::PageLoadEvent;
use tauri::Manager;
use url::Url;

const GATEWAY_BUILD_KIND: &str = "release";
const MAX_NATIVE_INPUT_FILES: usize = 100;
const MAX_NATIVE_INPUT_FILE_BYTES: u64 = 25 * 1024 * 1024;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct StartGatewayResponse {
    ok: bool,
    status: &'static str,
    gateway_path: Option<String>,
    gateway_url: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct NativeInputFile {
    name: String,
    content_base64: String,
    mime_type: Option<&'static str>,
}

static PENDING_MAIN_WINDOW_ARGS: OnceLock<Mutex<Option<Vec<String>>>> = OnceLock::new();

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            remember_gateway_url_from_args(&args);
            restore_main_window_from_args(app, args);
        }))
        .on_page_load(|webview, payload| {
            if webview.label() == "main" && payload.event() == PageLoadEvent::Finished {
                restore_main_window_from_pending_args(webview, payload.url().clone());
            }
        })
        .setup(|app| {
            let args = std::env::args().skip(1).collect::<Vec<_>>();
            remember_gateway_url_from_args(&args);
            remember_active_gateway_url_if_unset();
            queue_main_window_restore(args.clone());
            restore_main_window_from_args(app.handle(), args);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            start_gateway,
            open_external_url,
            read_input_file,
            read_clipboard_image
        ])
        .run(tauri::generate_context!())
        .expect("failed to run tura_gui");
}

fn restore_main_window_from_args(app: &tauri::AppHandle, args: Vec<String>) {
    if let Some(window) = app.get_webview_window("main") {
        if let Ok(base_url) = window.url() {
            if let Some(url) = gui_startup_url_from_args(base_url, args) {
                let _ = window.navigate(url);
            }
        }
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

fn restore_main_window_from_pending_args(webview: &tauri::Webview, base_url: Url) {
    let Some(args) = take_pending_main_window_args_for_base_url(&base_url) else {
        return;
    };
    if let Some(url) = gui_startup_url_from_args(base_url, args) {
        let _ = webview.navigate(url);
    }
    let window = webview.window();
    let _ = window.show();
    let _ = window.unminimize();
    let _ = window.set_focus();
}

fn queue_main_window_restore(args: Vec<String>) -> bool {
    if GuiStartupParams::parse(args.clone()).is_none() {
        return false;
    }
    if let Ok(mut pending) = pending_main_window_args().lock() {
        *pending = Some(args);
        true
    } else {
        false
    }
}

fn take_pending_main_window_args() -> Option<Vec<String>> {
    pending_main_window_args()
        .lock()
        .ok()
        .and_then(|mut pending| pending.take())
}

fn take_pending_main_window_args_for_base_url(base_url: &Url) -> Option<Vec<String>> {
    if !is_gui_startup_base_url(base_url) {
        return None;
    }
    take_pending_main_window_args()
}

fn pending_main_window_args() -> &'static Mutex<Option<Vec<String>>> {
    PENDING_MAIN_WINDOW_ARGS.get_or_init(|| Mutex::new(None))
}

fn gui_startup_url_from_args(mut base_url: Url, args: Vec<String>) -> Option<Url> {
    if !is_gui_startup_base_url(&base_url) {
        return None;
    }
    let params = GuiStartupParams::parse(args)?;
    {
        let mut query = base_url.query_pairs_mut();
        query.clear();
        query.append_pair("gatewayUrl", &params.gateway_url);
        query.append_pair("tab", "conversation");
        if let Some(workspace) = params.workspace.as_deref() {
            query.append_pair("workspace", workspace);
        }
        if let Some(session_id) = params.session_id.as_deref() {
            query.append_pair("sessionId", session_id);
        }
    }
    Some(base_url)
}

fn remember_gateway_url_from_args(args: &[String]) {
    if let Some(params) = GuiStartupParams::parse(args.to_vec()) {
        remember_gateway_url(&params.gateway_url);
    }
}

fn remember_active_gateway_url_if_unset() {
    if std::env::var(tura_path::TURA_GATEWAY_URL_ENV)
        .ok()
        .and_then(|value| non_empty_gateway_url(&value))
        .is_some()
    {
        return;
    }
    let my_root = current_project_root();
    let instance_home = instance_home_for_runtime_root(&my_root);
    if let Some(url) = tura_path::read_active_gateway_url_for_home(&instance_home) {
        remember_gateway_url(&url);
    }
}

fn remember_gateway_url(gateway_url: &str) {
    if let Some(url) = non_empty_gateway_url(gateway_url) {
        std::env::set_var(
            tura_path::TURA_GATEWAY_URL_ENV,
            GatewayEndpoint::parse(&url).url(),
        );
    }
}

fn is_gui_startup_base_url(url: &Url) -> bool {
    matches!(url.scheme(), "http" | "https" | "tauri" | "asset" | "file")
}

#[derive(Debug, PartialEq, Eq)]
struct GuiStartupParams {
    gateway_url: String,
    workspace: Option<String>,
    session_id: Option<String>,
}

impl GuiStartupParams {
    fn parse(args: Vec<String>) -> Option<Self> {
        let mut gateway_url = None;
        let mut workspace = None;
        let mut session_id = None;
        let mut iter = args.into_iter();
        while let Some(arg) = iter.next() {
            match arg.as_str() {
                "--gateway-url" => gateway_url = next_non_empty(&mut iter),
                "--workspace" | "--directory" | "--cwd" => workspace = next_non_empty(&mut iter),
                "--session-id" | "--initial-session" => session_id = next_non_empty(&mut iter),
                _ => {}
            }
        }
        Some(Self {
            gateway_url: gateway_url?,
            workspace,
            session_id,
        })
    }
}

fn next_non_empty(iter: &mut impl Iterator<Item = String>) -> Option<String> {
    iter.next().filter(|value| !value.trim().is_empty())
}

#[tauri::command]
fn open_external_url(url: String) -> Result<(), String> {
    let parsed = parse_external_url(&url)?;
    open_url_in_default_browser(parsed.as_str())
}

#[tauri::command]
fn start_gateway(
    gateway_url: String,
    gateway_url_explicit: Option<bool>,
) -> Result<StartGatewayResponse, String> {
    start_gateway_with_launcher(
        &gateway_url,
        gateway_url_explicit.unwrap_or(false),
        launch_gateway_process,
    )
}

fn start_gateway_with_launcher(
    gateway_url: &str,
    gateway_url_explicit: bool,
    launcher: impl Fn(&GatewayEndpoint, &Path, &Path) -> Result<GatewayEndpoint, String>,
) -> Result<StartGatewayResponse, String> {
    let my_root = current_project_root();
    let instance_home = instance_home_for_runtime_root(&my_root);
    let endpoint =
        select_gateway_endpoint(gateway_url, gateway_url_explicit, &my_root, &instance_home)?;
    if let Some(identity) =
        usable_gateway_identity(&endpoint, gateway_url_explicit, &my_root, &instance_home)
    {
        return connected_gateway_response(&instance_home, &endpoint, "connected", Some(identity));
    }
    if gateway_url_explicit {
        return Err(format!(
            "explicit gateway is not running at {}; start that gateway or remove the explicit URL",
            endpoint.url()
        ));
    }
    match launcher(&endpoint, &my_root, &instance_home) {
        Ok(launched) => {
            if let Some(identity) =
                usable_gateway_identity(&launched, false, &my_root, &instance_home)
            {
                return connected_gateway_response(
                    &instance_home,
                    &launched,
                    "connected",
                    Some(identity),
                );
            }
        }
        Err(error) if !gateway_startup_timeout_error(&error) => return Err(error),
        Err(_) => {}
    }
    if terminate_active_gateway_process(&instance_home) {
        let relaunched = launcher(&endpoint, &my_root, &instance_home)?;
        if let Some(identity) =
            usable_gateway_identity(&relaunched, false, &my_root, &instance_home)
        {
            return connected_gateway_response(
                &instance_home,
                &relaunched,
                "connected",
                Some(identity),
            );
        }
        return Err(format!(
            "gateway did not become healthy for this home at {}",
            relaunched.url()
        ));
    }
    Err(format!(
        "gateway did not become healthy for this home at {}",
        endpoint.url()
    ))
}

fn gateway_startup_timeout_error(error: &str) -> bool {
    error.contains("did not become healthy") || error.contains("exited before becoming healthy")
}

fn connected_gateway_response(
    instance_home: &Path,
    endpoint: &GatewayEndpoint,
    status: &'static str,
    identity: Option<GatewayIdentity>,
) -> Result<StartGatewayResponse, String> {
    if let Some(identity) = identity {
        if let (Some(pid), Some(process_start_time)) = (identity.pid, identity.process_start_time) {
            tura_path::write_active_gateway_process_for_home(
                instance_home,
                &endpoint.url(),
                pid,
                Some(process_start_time),
            )
            .map_err(|error| format!("failed to write active gateway URL: {error}"))?;
        } else {
            write_active_gateway_url(instance_home, endpoint)?;
        }
    } else {
        write_active_gateway_url(instance_home, endpoint)?;
    }
    remember_gateway_url(&endpoint.url());
    Ok(StartGatewayResponse {
        ok: true,
        status,
        gateway_path: None,
        gateway_url: Some(endpoint.url()),
    })
}

fn endpoint_is_usable(
    endpoint: &GatewayEndpoint,
    explicit: bool,
    my_root: &Path,
    instance_home: &Path,
) -> bool {
    usable_gateway_identity(endpoint, explicit, my_root, instance_home).is_some()
}

fn usable_gateway_identity(
    endpoint: &GatewayEndpoint,
    explicit: bool,
    my_root: &Path,
    instance_home: &Path,
) -> Option<GatewayIdentity> {
    let Some(identity) = gateway_identity(endpoint) else {
        return None;
    };
    (explicit || gateway_identity_matches_instance(&identity, my_root, instance_home))
        .then_some(identity)
}

fn launch_gateway_process(
    target: &GatewayEndpoint,
    my_root: &Path,
    instance_home: &Path,
) -> Result<GatewayEndpoint, String> {
    let executable = resolve_gateway_binary(my_root)?;
    let mut command = Command::new(&executable);
    configure_gateway_runtime_command(&mut command, my_root, instance_home, target)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .stdin(Stdio::null());
    tura_path::process_hardening::hide_child_console_window(&mut command);
    let mut child = command
        .spawn()
        .map_err(|err| format!("failed to start gateway {}: {err}", executable.display()))?;
    let deadline = std::time::Instant::now() + Duration::from_secs(20);
    while std::time::Instant::now() < deadline {
        if let Some(status) = child
            .try_wait()
            .map_err(|err| format!("failed to inspect gateway startup: {err}"))?
        {
            return Err(format!(
                "gateway exited before becoming healthy: {}",
                status
            ));
        }
        let candidate = tura_path::read_active_gateway_url_for_home(instance_home)
            .map(|url| GatewayEndpoint::parse(&url))
            .unwrap_or_else(|| target.clone());
        if endpoint_is_usable(&candidate, false, my_root, instance_home) {
            return Ok(candidate);
        }
        std::thread::sleep(Duration::from_millis(500));
    }
    let _ = child.kill();
    Err(format!(
        "gateway did not become healthy after startup at {}",
        target.url()
    ))
}

fn configure_gateway_runtime_command<'a>(
    command: &'a mut Command,
    runtime_root: &Path,
    instance_home: &Path,
    target: &GatewayEndpoint,
) -> &'a mut Command {
    command
        .current_dir(runtime_root)
        .env("TURA_HOME", instance_home)
        .env("TURA_PROJECT_ROOT", runtime_root)
        .env(tura_path::TURA_GATEWAY_PORT_ENV, target.port.to_string());
    if let Some(provider_config) = provider_config_for_runtime_root(runtime_root) {
        command.env("TURA_PROVIDER_CONFIG", provider_config);
    }
    if let Some(env_path) = env_path_for_runtime_root(runtime_root) {
        command.env("TURA_ENV_PATH", env_path);
    }
    command
}

fn provider_config_for_runtime_root(runtime_root: &Path) -> Option<PathBuf> {
    [
        runtime_root.join("config").join("provider_config.json"),
        runtime_root
            .join("crates")
            .join("provider")
            .join("config")
            .join("provider_config.json"),
    ]
    .into_iter()
    .find(|path| path.is_file())
}

fn env_path_for_runtime_root(runtime_root: &Path) -> Option<PathBuf> {
    let path = runtime_root.join(".env");
    path.is_file().then_some(path)
}

#[tauri::command]
fn read_input_file(path: String) -> Result<Vec<NativeInputFile>, String> {
    native_input_files_from_path(Path::new(&path))
}

#[tauri::command]
fn read_clipboard_image() -> Result<Option<NativeInputFile>, String> {
    native_input_file_from_clipboard_image()
}

fn native_input_file_from_path(path: &Path) -> Result<NativeInputFile, String> {
    if !path.is_file() {
        return Err(format!("input path is not a file: {}", path.display()));
    }
    let size = std::fs::metadata(path)
        .map_err(|err| format!("failed to inspect input file {}: {err}", path.display()))?
        .len();
    if size > MAX_NATIVE_INPUT_FILE_BYTES {
        return Err(format!(
            "input file exceeds the 25 MB limit: {}",
            path.display()
        ));
    }
    let bytes = std::fs::read(path)
        .map_err(|err| format!("failed to read input file {}: {err}", path.display()))?;
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("attachment")
        .to_string();
    Ok(NativeInputFile {
        name,
        content_base64: general_purpose::STANDARD.encode(bytes),
        mime_type: mime_type_for_path(path),
    })
}

fn native_input_files_from_path(path: &Path) -> Result<Vec<NativeInputFile>, String> {
    if path.is_file() {
        return native_input_file_from_path(path).map(|file| vec![file]);
    }
    if !path.is_dir() {
        return Err(format!("input path does not exist: {}", path.display()));
    }

    let mut paths = Vec::new();
    collect_input_file_paths(path, &mut paths)?;
    paths.sort();
    paths
        .iter()
        .map(|file| native_input_file_from_path(file))
        .collect()
}

fn collect_input_file_paths(directory: &Path, files: &mut Vec<PathBuf>) -> Result<(), String> {
    let entries = std::fs::read_dir(directory)
        .map_err(|err| format!("failed to read input directory {}: {err}", directory.display()))?;
    for entry in entries {
        let entry = entry.map_err(|err| {
            format!("failed to read input directory {}: {err}", directory.display())
        })?;
        let file_type = entry
            .file_type()
            .map_err(|err| format!("failed to inspect input path {}: {err}", entry.path().display()))?;
        if file_type.is_file() {
            files.push(entry.path());
            if files.len() > MAX_NATIVE_INPUT_FILES {
                return Err(format!(
                    "input directory contains more than {MAX_NATIVE_INPUT_FILES} files"
                ));
            }
        } else if file_type.is_dir() {
            collect_input_file_paths(&entry.path(), files)?;
        }
    }
    Ok(())
}

fn native_input_file_from_clipboard_image() -> Result<Option<NativeInputFile>, String> {
    let mut clipboard = arboard::Clipboard::new()
        .map_err(|err| format!("failed to access system clipboard: {err}"))?;
    let image = match clipboard.get_image() {
        Ok(image) => image,
        Err(arboard::Error::ContentNotAvailable) => return Ok(None),
        Err(err) => return Err(format!("failed to read clipboard image: {err}")),
    };
    native_input_file_from_rgba(
        "clipboard.png",
        image.width,
        image.height,
        image.bytes.into_owned(),
    )
    .map(Some)
}

fn native_input_file_from_rgba(
    name: &str,
    width: usize,
    height: usize,
    rgba: Vec<u8>,
) -> Result<NativeInputFile, String> {
    let buffer = image::RgbaImage::from_raw(width as u32, height as u32, rgba)
        .ok_or_else(|| "clipboard image has invalid RGBA dimensions".to_string())?;
    let mut png = Vec::new();
    image::DynamicImage::ImageRgba8(buffer)
        .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
        .map_err(|err| format!("failed to encode clipboard image: {err}"))?;
    Ok(NativeInputFile {
        name: name.to_string(),
        content_base64: general_purpose::STANDARD.encode(png),
        mime_type: Some("image/png"),
    })
}

fn mime_type_for_path(path: &Path) -> Option<&'static str> {
    match path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("png") => Some("image/png"),
        Some("jpg") | Some("jpeg") => Some("image/jpeg"),
        Some("webp") => Some("image/webp"),
        Some("gif") => Some("image/gif"),
        Some("txt") | Some("md") | Some("log") => Some("text/plain"),
        Some("json") => Some("application/json"),
        Some("pdf") => Some("application/pdf"),
        _ => None,
    }
}

fn open_url_in_default_browser(url: &str) -> Result<(), String> {
    let mut command = default_browser_command(url);
    tura_path::process_hardening::hide_child_console_window(&mut command);
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

fn is_runtime_root(candidate: &Path) -> bool {
    is_source_checkout_root(candidate)
        || (candidate.join("agents").join("src").is_dir()
            && candidate.join("personas").join("src").is_dir())
        || candidate
            .join("config")
            .join("provider_config.json")
            .exists()
}

fn is_source_checkout_root(candidate: &Path) -> bool {
    candidate.join("Cargo.toml").exists() && candidate.join("crates").join("gateway").is_dir()
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
    runtime_root_from_start(start)
}

fn runtime_root_from_start(start: &Path) -> PathBuf {
    if let Some(source_root) = start
        .ancestors()
        .find(|candidate| is_source_checkout_root(candidate))
    {
        return normalize_path(source_root);
    }
    start
        .ancestors()
        .find(|candidate| is_runtime_root(candidate))
        .map(normalize_path)
        .unwrap_or_else(|| normalize_path(start))
}

fn current_project_root() -> PathBuf {
    std::env::var_os("TURA_PROJECT_ROOT")
        .map(PathBuf::from)
        .filter(|path| !path.as_os_str().is_empty())
        .map(|path| normalize_path(&path))
        .unwrap_or_else(current_runtime_root)
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

#[cfg_attr(not(test), allow(dead_code))]
fn gateway_health_reachable(endpoint: &GatewayEndpoint) -> bool {
    gateway_identity(endpoint).is_some()
}

fn default_gateway_endpoint() -> GatewayEndpoint {
    if let Ok(port) = std::env::var(tura_path::TURA_GATEWAY_PORT_ENV)
        .unwrap_or_default()
        .trim()
        .parse::<u16>()
    {
        return GatewayEndpoint {
            host: "127.0.0.1".to_string(),
            port,
            explicit_port: Some(port),
        };
    }
    GatewayEndpoint::parse(&tura_path::default_gateway_url_for_build_kind(
        GATEWAY_BUILD_KIND,
    ))
}

fn resolve_gateway_binary(my_root: &Path) -> Result<PathBuf, String> {
    let exe_name = if cfg!(windows) {
        "tura_gateway.exe"
    } else {
        "tura_gateway"
    };
    let mut candidates = Vec::new();
    if let Some(value) =
        std::env::var_os("TURA_GATEWAY_BIN").or_else(|| std::env::var_os("TURA_GATEWAY_EXE"))
    {
        candidates.push(PathBuf::from(value));
    }
    if let Ok(current_exe) = std::env::current_exe() {
        if let Some(dir) = current_exe.parent() {
            candidates.push(dir.join(exe_name));
        }
    }
    candidates.push(my_root.join("target").join("release").join(exe_name));
    candidates.push(my_root.join("bin").join(exe_name));
    candidates.push(my_root.join(exe_name));
    candidates
        .into_iter()
        .find(|candidate| candidate.is_file())
        .ok_or_else(|| "gateway binary not found; build or install tura_gateway first".to_string())
}

fn select_gateway_endpoint(
    requested_url: &str,
    explicit: bool,
    my_root: &Path,
    instance_home: &Path,
) -> Result<GatewayEndpoint, String> {
    let default_endpoint = default_gateway_endpoint();
    if let Some(requested_url) = non_empty_gateway_url(requested_url) {
        let requested_endpoint = GatewayEndpoint::parse(&requested_url);
        if explicit {
            return Ok(requested_endpoint);
        }
    }
    let candidates = gateway_endpoint_candidates(requested_url, instance_home, &default_endpoint);
    for candidate in candidates {
        if let Some(identity) = gateway_identity(&candidate) {
            if gateway_identity_matches_instance(&identity, my_root, instance_home) {
                write_active_gateway_url(instance_home, &candidate)?;
                return Ok(candidate);
            }
        }
    }
    if let Some(candidate) = same_home_gateway_process_endpoint(instance_home)
        .filter(|candidate| endpoint_is_usable(candidate, false, my_root, instance_home))
    {
        write_active_gateway_url(instance_home, &candidate)?;
        return Ok(candidate);
    }
    Ok(default_endpoint)
}

fn same_home_gateway_process_endpoint(instance_home: &Path) -> Option<GatewayEndpoint> {
    let mut system = System::new();
    system.refresh_processes_specifics(
        ProcessRefreshKind::new()
            .with_cmd(UpdateKind::Always)
            .with_cwd(UpdateKind::Always)
            .with_environ(UpdateKind::Always)
            .with_exe(UpdateKind::Always),
    );
    system.processes().values().find_map(|process| {
        let snapshot = GatewayProcessSnapshot {
            name: process.name().to_string(),
            exe: process.exe().map(Path::to_path_buf),
            cmd: process.cmd().to_vec(),
            environ: process.environ().to_vec(),
            cwd: process.cwd().map(Path::to_path_buf),
        };
        gateway_process_endpoint_from_snapshot(&snapshot, instance_home)
    })
}

#[derive(Debug)]
struct GatewayProcessSnapshot {
    name: String,
    exe: Option<PathBuf>,
    cmd: Vec<String>,
    environ: Vec<String>,
    cwd: Option<PathBuf>,
}

fn gateway_process_endpoint_from_snapshot(
    process: &GatewayProcessSnapshot,
    instance_home: &Path,
) -> Option<GatewayEndpoint> {
    if !is_gateway_process(process) || !process_matches_instance_home(process, instance_home) {
        return None;
    }
    gateway_process_port(process)
        .map(|port| GatewayEndpoint {
            host: "127.0.0.1".to_string(),
            port,
            explicit_port: Some(port),
        })
        .or_else(|| {
            process_env_value(&process.environ, tura_path::TURA_GATEWAY_URL_ENV)
                .map(|url| GatewayEndpoint::parse(&url))
        })
}

fn is_gateway_process(process: &GatewayProcessSnapshot) -> bool {
    process_binary_name_matches(&process.name)
        || process.exe.as_deref().is_some_and(path_is_gateway_binary)
        || process
            .cmd
            .first()
            .map(Path::new)
            .is_some_and(path_is_gateway_binary)
}

fn process_matches_instance_home(process: &GatewayProcessSnapshot, instance_home: &Path) -> bool {
    process_env_value(&process.environ, "TURA_HOME")
        .map(|home| comparable_path(Path::new(&home)) == comparable_path(instance_home))
        .or_else(|| {
            process
                .cwd
                .as_deref()
                .map(|cwd| comparable_path(cwd) == comparable_path(instance_home))
        })
        .unwrap_or(false)
}

fn gateway_process_port(process: &GatewayProcessSnapshot) -> Option<u16> {
    [tura_path::TURA_GATEWAY_PORT_ENV, "PORT"]
        .into_iter()
        .find_map(|key| {
            process_env_value(&process.environ, key).and_then(|value| parse_port(&value))
        })
}

fn process_env_value(environ: &[String], key: &str) -> Option<String> {
    environ.iter().find_map(|entry| {
        let (entry_key, value) = entry.split_once('=')?;
        entry_key
            .eq_ignore_ascii_case(key)
            .then(|| value.trim().to_string())
            .filter(|value| !value.is_empty())
    })
}

fn path_is_gateway_binary(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(process_binary_name_matches)
}

fn process_binary_name_matches(name: &str) -> bool {
    let name = name.trim();
    let name = name
        .strip_suffix(".exe")
        .or_else(|| name.strip_suffix(".EXE"))
        .unwrap_or(name);
    name.eq_ignore_ascii_case("tura_gateway")
}

fn parse_port(value: &str) -> Option<u16> {
    value.trim().parse::<u16>().ok()
}

fn gateway_endpoint_candidates(
    requested_url: &str,
    instance_home: &Path,
    default_endpoint: &GatewayEndpoint,
) -> Vec<GatewayEndpoint> {
    let values = [
        non_empty_gateway_url(requested_url),
        std::env::var(tura_path::TURA_GATEWAY_URL_ENV)
            .ok()
            .and_then(|value| non_empty_gateway_url(&value)),
        tura_path::read_active_gateway_url_for_home(instance_home),
        Some(default_endpoint.url()),
    ];
    let mut urls = Vec::new();
    for value in values.into_iter().flatten() {
        let endpoint = GatewayEndpoint::parse(&value);
        if !urls
            .iter()
            .any(|existing: &GatewayEndpoint| existing.url() == endpoint.url())
        {
            urls.push(endpoint);
        }
    }
    urls
}

fn non_empty_gateway_url(value: &str) -> Option<String> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

fn write_active_gateway_url(
    instance_home: &Path,
    endpoint: &GatewayEndpoint,
) -> Result<(), String> {
    tura_path::write_active_gateway_url_for_home(instance_home, &endpoint.url())
        .map_err(|error| format!("failed to write active gateway URL: {error}"))
}

#[derive(Debug, Clone)]
struct ActiveGatewayProcessRecord {
    pid: u32,
    process_start_time: u64,
}

fn terminate_active_gateway_process(instance_home: &Path) -> bool {
    let Some(record) = read_active_gateway_process_record(instance_home) else {
        return false;
    };
    terminate_gateway_process_record(&record)
}

fn read_active_gateway_process_record(instance_home: &Path) -> Option<ActiveGatewayProcessRecord> {
    let raw =
        std::fs::read_to_string(tura_path::active_gateway_env_path_for_home(instance_home)).ok()?;
    let mut pid = None;
    let mut process_start_time = None;
    for line in raw.lines() {
        let Some((key, value)) = line.trim().split_once('=') else {
            continue;
        };
        let value = value.trim().trim_matches('"').trim_matches('\'');
        if key.eq_ignore_ascii_case(tura_path::TURA_GATEWAY_PID_ENV) {
            pid = value.parse::<u32>().ok();
        }
        if key.eq_ignore_ascii_case(tura_path::TURA_GATEWAY_PROCESS_START_TIME_ENV) {
            process_start_time = value.parse::<u64>().ok();
        }
    }
    Some(ActiveGatewayProcessRecord {
        pid: pid?,
        process_start_time: process_start_time?,
    })
}

fn terminate_gateway_process_record(record: &ActiveGatewayProcessRecord) -> bool {
    let mut system = System::new_all();
    system.refresh_all();
    let Some(process) = system.process(Pid::from_u32(record.pid)) else {
        return false;
    };
    if process.start_time() != record.process_start_time {
        return false;
    }
    let snapshot = GatewayProcessSnapshot {
        name: process.name().to_string(),
        exe: process.exe().map(Path::to_path_buf),
        cmd: process.cmd().to_vec(),
        environ: process.environ().to_vec(),
        cwd: process.cwd().map(Path::to_path_buf),
    };
    if !is_gateway_process(&snapshot) {
        return false;
    }
    process.kill()
}

#[derive(Debug, Clone, Default)]
struct GatewayIdentity {
    root: String,
    home: String,
    pid: Option<u32>,
    process_start_time: Option<u64>,
}

/// Probe `/global/health`; on a healthy gateway return its reported identity,
/// otherwise `None`.
fn gateway_identity(endpoint: &GatewayEndpoint) -> Option<GatewayIdentity> {
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
        let identity = response
            .split("\r\n\r\n")
            .nth(1)
            .and_then(|body| serde_json::from_str::<serde_json::Value>(body.trim()).ok())
            .map(|value| {
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
                let pid = value
                    .get("pid")
                    .and_then(serde_json::Value::as_u64)
                    .and_then(|value| u32::try_from(value).ok());
                let process_start_time = value
                    .get("process_start_time")
                    .and_then(serde_json::Value::as_u64);
                GatewayIdentity {
                    root,
                    home,
                    pid,
                    process_start_time,
                }
            })
            .unwrap_or_default();
        Some(identity)
    })
}

fn gateway_identity_matches_instance(
    identity: &GatewayIdentity,
    my_root: &Path,
    instance_home: &Path,
) -> bool {
    if !identity.home.trim().is_empty() {
        return same_root(&identity.home, instance_home);
    }
    same_root(&identity.root, my_root)
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
            port: explicit_port.unwrap_or_else(|| {
                tura_path::default_gateway_port_for_build_kind(GATEWAY_BUILD_KIND)
            }),
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

    fn url(&self) -> String {
        let host = if self.host.contains(':') {
            format!("[{}]", self.host)
        } else {
            self.host.clone()
        };
        format!("http://{host}:{}", self.port)
    }
}

impl Default for GatewayEndpoint {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: tura_path::default_gateway_port_for_build_kind(GATEWAY_BUILD_KIND),
            explicit_port: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::engine::general_purpose;
    use std::fs;
    use std::net::TcpListener;
    use std::sync::Mutex;

    static TEST_ENV_LOCK: Mutex<()> = Mutex::new(());

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
    fn current_project_root_prefers_project_root_env() {
        let _guard = TEST_ENV_LOCK.lock().expect("env test lock");
        let project_root = test_temp_dir("current-project-root-env");
        let project_root_text = project_root.to_string_lossy().to_string();
        let env = TestEnv::set([("TURA_PROJECT_ROOT", project_root_text.as_str())]);

        assert_eq!(current_project_root(), normalize_path(&project_root));

        drop(env);
        let _ = fs::remove_dir_all(project_root);
    }

    #[test]
    fn runtime_root_prefers_source_checkout_over_target_release_runtime_copy() {
        let root = test_temp_dir("runtime-root-source-checkout");
        let target_release = root.join("target").join("release");
        create_source_checkout_root(&root);
        create_release_runtime_root(&target_release);

        assert_eq!(
            runtime_root_from_start(&target_release),
            normalize_path(&root)
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn gui_startup_args_build_app_deeplink() {
        let url = gui_startup_url_from_args(
            Url::parse("http://127.0.0.1:5174/?old=1").expect("base url"),
            vec![
                "--gateway-url".to_string(),
                "http://127.0.0.1:4126".to_string(),
                "--workspace".to_string(),
                "C:\\repo with spaces".to_string(),
                "--session-id".to_string(),
                "session-123".to_string(),
            ],
        )
        .expect("startup url");
        let pairs = url
            .query_pairs()
            .collect::<std::collections::HashMap<_, _>>();

        assert_eq!(
            url.as_str().split('?').next(),
            Some("http://127.0.0.1:5174/")
        );
        assert_eq!(
            pairs.get("gatewayUrl").map(|value| value.as_ref()),
            Some("http://127.0.0.1:4126")
        );
        assert_eq!(
            pairs.get("tab").map(|value| value.as_ref()),
            Some("conversation")
        );
        assert_eq!(
            pairs.get("workspace").map(|value| value.as_ref()),
            Some("C:\\repo with spaces")
        );
        assert_eq!(
            pairs.get("sessionId").map(|value| value.as_ref()),
            Some("session-123")
        );
    }

    #[test]
    fn gateway_launch_args_are_remembered_for_tray_process_matching() {
        let _guard = TEST_ENV_LOCK.lock().expect("env test lock");
        let env = TestEnv::set([(tura_path::TURA_GATEWAY_URL_ENV, "")]);

        remember_gateway_url_from_args(&[
            "--gateway-url".to_string(),
            "http://127.0.0.1:4126/".to_string(),
            "--workspace".to_string(),
            "C:\\repo".to_string(),
        ]);

        assert_eq!(
            std::env::var(tura_path::TURA_GATEWAY_URL_ENV).as_deref(),
            Ok("http://127.0.0.1:4126")
        );
        drop(env);
    }

    #[test]
    fn gateway_launch_command_uses_runtime_root_for_cwd_and_config() {
        let runtime_root = test_temp_dir("gateway-launch-runtime-root");
        let home = test_temp_dir("gateway-launch-home");
        let provider_config = runtime_root.join("config").join("provider_config.json");
        fs::create_dir_all(provider_config.parent().expect("provider config parent"))
            .expect("provider config dir");
        fs::write(&provider_config, "{}").expect("provider config");
        let env_path = runtime_root.join(".env");
        fs::write(&env_path, "OPENAI_LOGIN=oauth\n").expect("env file");

        let endpoint = GatewayEndpoint::parse("http://127.0.0.1:4999");
        let mut command = Command::new("tura_gateway");
        configure_gateway_runtime_command(&mut command, &runtime_root, &home, &endpoint);

        let envs = command
            .get_envs()
            .filter_map(|(key, value)| {
                value.map(|value| (key.to_string_lossy().to_string(), PathBuf::from(value)))
            })
            .collect::<std::collections::HashMap<_, _>>();
        assert_eq!(command.get_current_dir(), Some(runtime_root.as_path()));
        assert_eq!(envs.get("TURA_HOME"), Some(&home));
        assert_eq!(envs.get("TURA_PROJECT_ROOT"), Some(&runtime_root));
        assert_eq!(envs.get("TURA_PROVIDER_CONFIG"), Some(&provider_config));
        assert_eq!(envs.get("TURA_ENV_PATH"), Some(&env_path));

        let port = command.get_envs().find_map(|(key, value)| {
            (key == tura_path::TURA_GATEWAY_PORT_ENV)
                .then(|| value.map(|value| value.to_string_lossy().to_string()))
                .flatten()
        });
        assert_eq!(port.as_deref(), Some("4999"));

        let _ = fs::remove_dir_all(runtime_root);
        let _ = fs::remove_dir_all(home);
    }

    #[test]
    fn gui_startup_args_require_gateway_url() {
        assert!(gui_startup_url_from_args(
            Url::parse("http://127.0.0.1:5174/").expect("base url"),
            vec!["--workspace".to_string(), "C:\\repo".to_string()],
        )
        .is_none());
    }

    #[test]
    fn gui_startup_args_ignore_blank_transient_url() {
        assert!(gui_startup_url_from_args(
            Url::parse("about:blank").expect("blank url"),
            vec![
                "--gateway-url".to_string(),
                "http://127.0.0.1:4126".to_string(),
            ],
        )
        .is_none());
    }

    #[test]
    fn blank_transient_url_does_not_consume_pending_cold_start_args() {
        let _guard = TEST_ENV_LOCK.lock().expect("test env lock");
        let _ = take_pending_main_window_args();
        let args = vec![
            "--gateway-url".to_string(),
            "http://127.0.0.1:4126".to_string(),
        ];

        assert!(queue_main_window_restore(args.clone()));
        assert!(take_pending_main_window_args_for_base_url(
            &Url::parse("about:blank").expect("blank url")
        )
        .is_none());
        assert_eq!(take_pending_main_window_args(), Some(args));
    }

    #[test]
    fn cold_start_args_are_retained_for_page_load_restore() {
        let _guard = TEST_ENV_LOCK.lock().expect("test env lock");
        let _ = take_pending_main_window_args();
        let args = vec![
            "--gateway-url".to_string(),
            "http://127.0.0.1:4126".to_string(),
            "--workspace".to_string(),
            "C:\\repo with spaces".to_string(),
            "--session-id".to_string(),
            "session-123".to_string(),
        ];

        assert!(queue_main_window_restore(args.clone()));
        assert_eq!(take_pending_main_window_args(), Some(args));
        assert_eq!(take_pending_main_window_args(), None);
    }

    #[test]
    fn page_load_restore_queue_ignores_launches_without_gateway_url() {
        let _guard = TEST_ENV_LOCK.lock().expect("test env lock");
        let _ = take_pending_main_window_args();
        assert!(!queue_main_window_restore(vec![
            "--workspace".to_string(),
            "C:\\repo".to_string(),
        ]));
        assert_eq!(take_pending_main_window_args(), None);
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

    #[test]
    fn native_input_file_reads_path_as_base64_payload() {
        let temp = test_temp_dir("native-input-file");
        let file = temp.join("shot.png");
        fs::write(&file, [137_u8, 80, 78, 71, 13, 10, 26, 10]).expect("write png");

        let payloads = native_input_files_from_path(&file).expect("read input file");
        let payload = &payloads[0];

        assert_eq!(payload.name, "shot.png");
        assert_eq!(payload.mime_type, Some("image/png"));
        assert_eq!(
            general_purpose::STANDARD
                .decode(&payload.content_base64)
                .expect("decode payload"),
            vec![137_u8, 80, 78, 71, 13, 10, 26, 10]
        );
        let _ = fs::remove_dir_all(temp);
    }

    #[test]
    fn native_input_directory_recursively_reads_files_in_stable_order() {
        let temp = test_temp_dir("native-input-directory");
        let nested = temp.join("nested");
        fs::create_dir_all(&nested).expect("nested directory");
        fs::write(temp.join("b.txt"), b"b").expect("root file");
        fs::write(nested.join("a.txt"), b"a").expect("nested file");

        let payloads = native_input_files_from_path(&temp).expect("read input directory");

        assert_eq!(payloads.len(), 2);
        assert_eq!(payloads[0].name, "b.txt");
        assert_eq!(payloads[1].name, "a.txt");
        let _ = fs::remove_dir_all(temp);
    }

    #[test]
    fn native_clipboard_image_payload_encodes_rgba_as_png() {
        let payload = native_input_file_from_rgba("clipboard.png", 1, 1, vec![255_u8, 0, 0, 255])
            .expect("encode clipboard image");

        assert_eq!(payload.name, "clipboard.png");
        assert_eq!(payload.mime_type, Some("image/png"));
        let png = general_purpose::STANDARD
            .decode(payload.content_base64)
            .expect("decode png payload");
        assert!(png.starts_with(&[137, 80, 78, 71, 13, 10, 26, 10]));
    }

    #[test]
    fn native_clipboard_image_payload_rejects_invalid_rgba_dimensions() {
        let err = native_input_file_from_rgba("clipboard.png", 2, 2, vec![255_u8; 4])
            .expect_err("invalid RGBA dimensions should fail");

        assert!(err.contains("invalid RGBA dimensions"));
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
                port: tura_path::default_gateway_port_for_build_kind(GATEWAY_BUILD_KIND),
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
    fn requested_gateway_endpoint_precedes_active_and_default_candidates() {
        let _guard = TEST_ENV_LOCK.lock().expect("env test lock");
        let temp = test_temp_dir("requested-endpoint-first");
        let env = TestEnv::set([(tura_path::TURA_GATEWAY_URL_ENV, "")]);
        tura_path::write_active_gateway_url_for_home(&temp, "http://127.0.0.1:4998")
            .expect("write active gateway url");

        let default_endpoint = GatewayEndpoint::parse("http://127.0.0.1:4126");
        let candidates =
            gateway_endpoint_candidates("http://127.0.0.1:4999", &temp, &default_endpoint);

        assert_eq!(
            candidates
                .iter()
                .map(GatewayEndpoint::url)
                .collect::<Vec<_>>(),
            vec![
                "http://127.0.0.1:4999".to_string(),
                "http://127.0.0.1:4998".to_string(),
                "http://127.0.0.1:4126".to_string(),
            ]
        );
        drop(env);
        let _ = fs::remove_dir_all(temp);
    }

    #[test]
    fn default_gateway_endpoint_can_adopt_active_gateway() {
        let _guard = TEST_ENV_LOCK.lock().expect("env test lock");
        let temp = test_temp_dir("default-endpoint-adopts-active");
        let env = TestEnv::set([(tura_path::TURA_GATEWAY_URL_ENV, "")]);
        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind local listener");
        let port = listener.local_addr().expect("local addr").port();
        tura_path::write_active_gateway_url_for_home(&temp, &format!("http://127.0.0.1:{port}"))
            .expect("write active gateway url");
        std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept health check");
            let mut buffer = [0_u8; 512];
            let _ = stream.read(&mut buffer);
            stream
                .write_all(
                    format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\r\n{{\"healthy\":true,\"root\":{}}}",
                        serde_json::to_string(
                            &std::env::current_dir().expect("cwd").to_string_lossy().to_string()
                        )
                        .expect("json root")
                    )
                    .as_bytes(),
                )
                .expect("write health response");
        });

        let selected = select_gateway_endpoint(
            "http://127.0.0.1:4126",
            false,
            &std::env::current_dir().expect("cwd"),
            &temp,
        )
        .expect("select endpoint");

        assert_eq!(selected.url(), format!("http://127.0.0.1:{port}"));
        drop(env);
        let _ = fs::remove_dir_all(temp);
    }

    #[test]
    fn default_gateway_endpoint_reuses_same_home_active_gateway_with_different_project_root() {
        let _guard = TEST_ENV_LOCK.lock().expect("env test lock");
        let home = test_temp_dir("default-endpoint-same-home-active");
        let project_root = test_temp_dir("default-endpoint-current-root");
        let other_root = test_temp_dir("default-endpoint-other-root");
        let env = TestEnv::set([(tura_path::TURA_GATEWAY_URL_ENV, "")]);
        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind local listener");
        let port = listener.local_addr().expect("local addr").port();
        tura_path::write_active_gateway_url_for_home(&home, &format!("http://127.0.0.1:{port}"))
            .expect("write active gateway url");
        let home_text = home.to_string_lossy().to_string();
        let other_root_text = other_root.to_string_lossy().to_string();
        std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept health check");
            let mut buffer = [0_u8; 512];
            let _ = stream.read(&mut buffer);
            stream
                .write_all(
                    format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\r\n{{\"healthy\":true,\"root\":{},\"home\":{}}}",
                        serde_json::to_string(&other_root_text).expect("json root"),
                        serde_json::to_string(&home_text).expect("json home")
                    )
                    .as_bytes(),
                )
                .expect("write health response");
        });

        let selected =
            select_gateway_endpoint("http://127.0.0.1:4126", false, &project_root, &home)
                .expect("select endpoint");

        assert_eq!(selected.url(), format!("http://127.0.0.1:{port}"));
        drop(env);
        let _ = fs::remove_dir_all(home);
        let _ = fs::remove_dir_all(project_root);
        let _ = fs::remove_dir_all(other_root);
    }

    #[test]
    fn select_gateway_endpoint_does_not_probe_explicit_requested_url() {
        let temp = test_temp_dir("select-requested-no-probe");
        let requested =
            select_gateway_endpoint("http://127.0.0.1:4997", true, Path::new("."), &temp)
                .expect("select requested endpoint");

        assert_eq!(requested.url(), "http://127.0.0.1:4997");
        let _ = fs::remove_dir_all(temp);
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
    fn same_home_gateway_process_snapshot_provides_port_endpoint() {
        let home = test_temp_dir("same-home-process-port");
        let snapshot = gateway_process_snapshot(
            "tura_gateway.exe",
            Some(home.join("bin").join("tura_gateway.exe")),
            vec![home
                .join("bin")
                .join("tura_gateway.exe")
                .display()
                .to_string()],
            vec![
                format!("TURA_HOME={}", home.display()),
                "TURA_GATEWAY_PORT=4789".to_string(),
            ],
            Some(home.clone()),
        );

        let endpoint = gateway_process_endpoint_from_snapshot(&snapshot, &home)
            .expect("same home gateway endpoint");

        assert_eq!(endpoint.url(), "http://127.0.0.1:4789");
        let _ = fs::remove_dir_all(home);
    }

    #[test]
    fn same_home_gateway_process_snapshot_falls_back_to_gateway_url() {
        let home = test_temp_dir("same-home-process-url");
        let snapshot = gateway_process_snapshot(
            "tura_gateway",
            None,
            vec!["tura_gateway".to_string()],
            vec![
                format!("TURA_HOME={}", home.display()),
                "TURA_GATEWAY_URL=http://127.0.0.1:4790".to_string(),
            ],
            None,
        );

        let endpoint = gateway_process_endpoint_from_snapshot(&snapshot, &home)
            .expect("same home gateway endpoint");

        assert_eq!(endpoint.url(), "http://127.0.0.1:4790");
        let _ = fs::remove_dir_all(home);
    }

    #[test]
    fn same_home_gateway_process_snapshot_rejects_foreign_home() {
        let home = test_temp_dir("same-home-process-local");
        let foreign_home = test_temp_dir("same-home-process-foreign");
        let snapshot = gateway_process_snapshot(
            "tura_gateway",
            None,
            vec!["tura_gateway".to_string()],
            vec![
                format!("TURA_HOME={}", foreign_home.display()),
                "TURA_GATEWAY_PORT=4791".to_string(),
            ],
            Some(foreign_home.clone()),
        );

        assert!(gateway_process_endpoint_from_snapshot(&snapshot, &home).is_none());
        let _ = fs::remove_dir_all(home);
        let _ = fs::remove_dir_all(foreign_home);
    }

    #[test]
    fn same_home_gateway_process_snapshot_rejects_non_gateway_binary() {
        let home = test_temp_dir("same-home-process-other");
        let snapshot = gateway_process_snapshot(
            "not_gateway",
            Some(home.join("not_gateway.exe")),
            vec![home.join("not_gateway.exe").display().to_string()],
            vec![
                format!("TURA_HOME={}", home.display()),
                "TURA_GATEWAY_PORT=4792".to_string(),
            ],
            Some(home.clone()),
        );

        assert!(gateway_process_endpoint_from_snapshot(&snapshot, &home).is_none());
        let _ = fs::remove_dir_all(home);
    }

    #[test]
    fn start_gateway_returns_connected_when_endpoint_is_reachable() {
        let _guard = TEST_ENV_LOCK.lock().expect("env test lock");
        let home = test_temp_dir("start-gateway-connected-home");
        let home_text = home.to_string_lossy().to_string();
        let env = TestEnv::set([
            ("TURA_HOME", home_text.as_str()),
            (tura_path::TURA_GATEWAY_URL_ENV, ""),
        ]);
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

        let response = start_gateway(format!("http://127.0.0.1:{port}"), Some(true))
            .expect("start gateway response");

        assert!(response.ok);
        assert_eq!(response.status, "connected");
        assert_eq!(response.gateway_path, None);
        assert_eq!(
            response.gateway_url,
            Some(format!("http://127.0.0.1:{port}"))
        );
        assert_eq!(
            tura_path::read_active_gateway_url_for_home(&home).as_deref(),
            Some(format!("http://127.0.0.1:{port}").as_str())
        );
        assert_eq!(
            std::env::var(tura_path::TURA_GATEWAY_URL_ENV).as_deref(),
            Ok(format!("http://127.0.0.1:{port}").as_str())
        );
        drop(env);
        let _ = fs::remove_dir_all(home);
    }

    #[test]
    fn start_gateway_errors_when_endpoint_is_absent() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind local listener");
        let port = listener.local_addr().expect("local addr").port();
        drop(listener);

        let error = start_gateway(format!("http://127.0.0.1:{port}"), Some(true))
            .expect_err("explicit absent gateway must fail without spawning");

        assert!(error.contains("explicit gateway is not running at"));
    }

    #[test]
    fn start_gateway_non_explicit_launches_when_no_same_root_gateway_exists() {
        let _guard = TEST_ENV_LOCK.lock().expect("env test lock");
        let home = test_temp_dir("start-gateway-launches-home");
        let project_root = test_temp_dir("start-gateway-project-root");
        let home_text = home.to_string_lossy().to_string();
        let project_root_text = project_root.to_string_lossy().to_string();
        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind local listener");
        let port = listener.local_addr().expect("local addr").port();
        let requested_port = port.saturating_sub(1).max(1024);
        let env = TestEnv::set([
            ("TURA_HOME", home_text.as_str()),
            ("TURA_PROJECT_ROOT", project_root_text.as_str()),
            (tura_path::TURA_GATEWAY_URL_ENV, ""),
            (
                tura_path::TURA_GATEWAY_PORT_ENV,
                &requested_port.to_string(),
            ),
        ]);
        let my_root = current_project_root();
        let root_text = my_root.to_string_lossy().to_string();
        std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept health check");
            let mut buffer = [0_u8; 512];
            let _ = stream.read(&mut buffer);
            stream
                .write_all(
                    format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\r\n{{\"healthy\":true,\"root\":{}}}",
                        serde_json::to_string(&root_text).expect("json root")
                    )
                    .as_bytes(),
                )
                .expect("write health response");
        });
        let launched_url = format!("http://127.0.0.1:{port}");
        let response = start_gateway_with_launcher(
            "http://127.0.0.1:65530",
            false,
            |target, root, home_arg| {
                assert_eq!(target.url(), format!("http://127.0.0.1:{requested_port}"));
                assert_eq!(root, normalize_path(&project_root).as_path());
                assert_eq!(home_arg, normalize_path(&home).as_path());
                Ok(GatewayEndpoint::parse(&launched_url))
            },
        )
        .expect("start gateway response");

        assert_eq!(response.status, "connected");
        assert_eq!(response.gateway_url, Some(launched_url.clone()));
        assert_eq!(
            tura_path::read_active_gateway_url_for_home(&home).as_deref(),
            Some(launched_url.as_str())
        );
        drop(env);
        let _ = fs::remove_dir_all(home);
        let _ = fs::remove_dir_all(project_root);
    }

    #[test]
    fn active_gateway_process_record_requires_pid_and_start_time() {
        let home = test_temp_dir("active-gateway-record-home");
        fs::create_dir_all(home.join(".tura")).expect("runtime dir");
        fs::write(
            home.join(".tura").join("gateway-active.env"),
            "TURA_GATEWAY_URL=http://127.0.0.1:4125\nTURA_GATEWAY_PID=42\nTURA_GATEWAY_PROCESS_START_TIME=777\n",
        )
        .expect("active gateway env");

        let record = read_active_gateway_process_record(&home).expect("active record");

        assert_eq!(record.pid, 42);
        assert_eq!(record.process_start_time, 777);
        let _ = fs::remove_dir_all(home);
    }

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
                std::env::set_var(key, value);
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

    fn gateway_process_snapshot(
        name: &str,
        exe: Option<PathBuf>,
        cmd: Vec<String>,
        environ: Vec<String>,
        cwd: Option<PathBuf>,
    ) -> GatewayProcessSnapshot {
        GatewayProcessSnapshot {
            name: name.to_string(),
            exe,
            cmd,
            environ,
            cwd,
        }
    }

    fn test_temp_dir(name: &str) -> PathBuf {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("..")
            .join("target")
            .join("tauri-tests")
            .join(format!("{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    fn create_source_checkout_root(root: &Path) {
        fs::create_dir_all(root.join("agents").join("src")).expect("agents dir");
        fs::create_dir_all(root.join("personas").join("src")).expect("personas dir");
        fs::create_dir_all(root.join("crates").join("gateway")).expect("gateway crate dir");
        fs::create_dir_all(
            root.join("crates")
                .join("tools")
                .join("src")
                .join("command_run"),
        )
        .expect("command_run dir");
        fs::create_dir_all(root.join("config")).expect("config dir");
        fs::write(root.join("Cargo.toml"), "[workspace]\n").expect("cargo toml");
        fs::write(root.join("config").join("provider_config.json"), "{}").expect("provider config");
        fs::write(
            root.join("crates")
                .join("tools")
                .join("src")
                .join("command_run")
                .join("schema.json"),
            "{}",
        )
        .expect("command_run schema");
    }

    fn create_release_runtime_root(root: &Path) {
        fs::create_dir_all(root.join("agents").join("src")).expect("agents dir");
        fs::create_dir_all(root.join("personas").join("src")).expect("personas dir");
        fs::create_dir_all(
            root.join("crates")
                .join("tools")
                .join("src")
                .join("command_run"),
        )
        .expect("command_run dir");
        fs::create_dir_all(root.join("config")).expect("config dir");
        fs::write(root.join("config").join("provider_config.json"), "{}").expect("provider config");
        fs::write(
            root.join("crates")
                .join("tools")
                .join("src")
                .join("command_run")
                .join("schema.json"),
            "{}",
        )
        .expect("command_run schema");
    }
}
