use crate::session_db_client::SessionDbClient;
use anyhow::{anyhow, Context, Result};
use session_log::{SessionSummary, WorkspaceSummary};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::Duration;
use tao::event::Event;
use tao::event_loop::{ControlFlow, EventLoop, EventLoopBuilder};
use tray_icon::menu::{Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};

const MAX_TRAY_SESSIONS: usize = 12;
const SESSION_TITLE_MAX_CHARS: usize = 24;
const SESSION_WORKSPACE_MAX_CHARS: usize = 18;
const REFRESH_INTERVAL: Duration = Duration::from_secs(5);
const OPEN_GUI_ID: &str = "action:open-gui";
const KILL_BACKGROUND_PROCESSES_ID: &str = "action:kill-background-processes";
const QUIT_ID: &str = "action:quit";

#[derive(Debug, Clone)]
enum TrayUserEvent {
    Menu(MenuEvent),
    Tray(TrayIconEvent),
    Refresh,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum TrayLanguage {
    ZhCN,
    #[default]
    En,
}

#[derive(Debug, Clone)]
struct ActiveSessionItem {
    session_id: String,
    workspace: String,
    label: String,
}

#[derive(Debug, Clone, Default)]
struct TraySnapshot {
    active_sessions: Vec<ActiveSessionItem>,
    last_workspace: Option<String>,
    session_process_directory: Option<String>,
    background_process_count: usize,
    language: TrayLanguage,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TrayMenuEntry {
    id: Option<String>,
    label: Option<String>,
    enabled: bool,
}

pub struct GatewayTrayApp {
    port: u16,
    event_loop: EventLoop<TrayUserEvent>,
    menu: Menu,
    tray_icon: TrayIcon,
    snapshot: TraySnapshot,
    session_actions: HashMap<String, ActiveSessionItem>,
}

pub fn tray_enabled() -> bool {
    let enabled = std::env::var("TURA_GATEWAY_TRAY")
        .map(|value| !matches!(value.trim(), "0" | "false" | "off" | "no"))
        .unwrap_or(true);
    enabled && display_session_available()
}

fn display_session_available() -> bool {
    if !cfg!(target_os = "linux") {
        return true;
    }
    env_var_has_value("DISPLAY") || env_var_has_value("WAYLAND_DISPLAY")
}

fn env_var_has_value(name: &str) -> bool {
    std::env::var_os(name)
        .and_then(|value| value.into_string().ok())
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false)
}

impl GatewayTrayApp {
    pub fn new(port: u16) -> Result<Self> {
        let event_loop = EventLoopBuilder::<TrayUserEvent>::with_user_event().build();
        let proxy = event_loop.create_proxy();
        MenuEvent::set_event_handler(Some(move |event| {
            let _ = proxy.send_event(TrayUserEvent::Menu(event));
        }));
        let proxy = event_loop.create_proxy();
        TrayIconEvent::set_event_handler(Some(move |event| {
            let _ = proxy.send_event(TrayUserEvent::Tray(event));
        }));

        let menu = Menu::new();
        let snapshot = read_snapshot();
        let session_actions = rebuild_menu(&menu, &snapshot)?;
        let tray_icon = TrayIconBuilder::new()
            .with_tooltip("Tura Gateway")
            .with_icon(load_tura_icon()?)
            .with_menu(Box::new(menu.clone()))
            .with_menu_on_left_click(false)
            .build()
            .context("failed to create Tura gateway tray icon")?;

        Ok(Self {
            port,
            event_loop,
            menu,
            tray_icon,
            snapshot,
            session_actions,
        })
    }

    pub fn run(self) {
        let GatewayTrayApp {
            port,
            event_loop,
            menu,
            tray_icon,
            snapshot,
            session_actions,
        } = self;
        let mut state = GatewayTrayState {
            port,
            menu,
            tray_icon,
            snapshot,
            session_actions,
            launched_clients: Vec::new(),
        };
        let proxy = event_loop.create_proxy();
        std::thread::spawn(move || loop {
            std::thread::sleep(REFRESH_INTERVAL);
            if proxy.send_event(TrayUserEvent::Refresh).is_err() {
                break;
            }
        });

        event_loop.run(move |event, _, control_flow| {
            *control_flow = ControlFlow::Wait;
            match event {
                Event::UserEvent(TrayUserEvent::Refresh) => state.refresh(),
                Event::UserEvent(TrayUserEvent::Tray(event)) => state.handle_tray_event(event),
                Event::UserEvent(TrayUserEvent::Menu(event)) => {
                    if state.handle_menu_event(event) {
                        *control_flow = ControlFlow::Exit;
                    }
                }
                _ => {}
            }
        });
    }
}

struct GatewayTrayState {
    port: u16,
    menu: Menu,
    tray_icon: TrayIcon,
    snapshot: TraySnapshot,
    session_actions: HashMap<String, ActiveSessionItem>,
    launched_clients: Vec<Child>,
}

impl GatewayTrayState {
    fn handle_tray_event(&mut self, event: TrayIconEvent) {
        if !is_left_click_release(&event) {
            return;
        }
        self.launch_gui(self.snapshot.last_workspace.clone(), None);
    }

    fn refresh(&mut self) {
        self.reap_finished_clients();
        let snapshot = read_snapshot();
        if snapshots_equal(&self.snapshot, &snapshot) {
            return;
        }
        match rebuild_menu(&self.menu, &snapshot) {
            Ok(actions) => {
                self.snapshot = snapshot;
                self.session_actions = actions;
                self.tray_icon.set_menu(Some(Box::new(self.menu.clone())));
            }
            Err(error) => tracing::warn!(error = %error, "failed to refresh gateway tray menu"),
        }
    }

    fn handle_menu_event(&mut self, event: MenuEvent) -> bool {
        let id = event.id.as_ref();
        if id == OPEN_GUI_ID {
            self.launch_gui(self.snapshot.last_workspace.clone(), None);
            return false;
        }
        if id == KILL_BACKGROUND_PROCESSES_ID {
            self.kill_background_processes();
            return false;
        }
        if id == QUIT_ID {
            self.shutdown_clients();
            return true;
        }
        if let Some(session) = self.session_actions.get(id) {
            self.launch_gui(
                Some(session.workspace.clone()),
                Some(session.session_id.clone()),
            );
        }
        false
    }

    fn launch_gui(&mut self, workspace: Option<String>, session_id: Option<String>) {
        self.reap_finished_clients();
        match open_gui(self.port, workspace.as_deref(), session_id.as_deref()) {
            Ok(child) => self.launched_clients.push(child),
            Err(error) => tracing::warn!(error = %error, "failed to open Tura GUI from tray"),
        }
    }

    fn reap_finished_clients(&mut self) {
        self.launched_clients
            .retain_mut(|child| !matches!(child.try_wait(), Ok(Some(_))));
    }

    fn kill_background_processes(&mut self) {
        let Some(directory) = self.snapshot.session_process_directory.clone() else {
            return;
        };
        stop_all_session_processes(Path::new(&directory));
        self.refresh();
    }

    fn shutdown_clients(&mut self) {
        let gateway_url = format!("http://127.0.0.1:{}", self.port);
        terminate_tracked_clients(&mut self.launched_clients);
        terminate_gateway_clients_by_command_line(&gateway_url);
    }
}

fn rebuild_menu(
    menu: &Menu,
    snapshot: &TraySnapshot,
) -> Result<HashMap<String, ActiveSessionItem>> {
    while !menu.items().is_empty() {
        let _ = menu.remove_at(0);
    }

    let mut session_actions = HashMap::new();
    for entry in menu_model(snapshot) {
        match (entry.id, entry.label) {
            (Some(id), Some(label)) => {
                menu.append(&MenuItem::with_id(
                    MenuId::new(&id),
                    &label,
                    entry.enabled,
                    None,
                ))?;
                if let Some(session) = id
                    .strip_prefix("session:")
                    .and_then(|session_id| {
                        snapshot
                            .active_sessions
                            .iter()
                            .find(|session| session.session_id == session_id)
                    })
                    .cloned()
                {
                    session_actions.insert(id, session);
                }
            }
            (None, None) => menu.append(&PredefinedMenuItem::separator())?,
            _ => {}
        }
    }
    Ok(session_actions)
}

fn menu_model(snapshot: &TraySnapshot) -> Vec<TrayMenuEntry> {
    let mut entries = Vec::new();
    if snapshot.active_sessions.is_empty() {
        entries.push(TrayMenuEntry {
            id: Some("status:no-active-sessions".to_string()),
            label: Some(tray_text(snapshot.language, TrayText::NoActiveSessions).to_string()),
            enabled: false,
        });
    } else {
        entries.extend(
            snapshot
                .active_sessions
                .iter()
                .map(|session| TrayMenuEntry {
                    id: Some(format!("session:{}", session.session_id)),
                    label: Some(session.label.clone()),
                    enabled: true,
                }),
        );
    }

    entries.push(separator_entry());
    entries.push(TrayMenuEntry {
        id: Some(OPEN_GUI_ID.to_string()),
        label: Some(tray_text(snapshot.language, TrayText::OpenGui).to_string()),
        enabled: true,
    });
    entries.push(TrayMenuEntry {
        id: Some("status:background-processes".to_string()),
        label: Some(background_process_count_label(
            snapshot.language,
            snapshot.background_process_count,
        )),
        enabled: false,
    });
    entries.push(TrayMenuEntry {
        id: Some(KILL_BACKGROUND_PROCESSES_ID.to_string()),
        label: Some(tray_text(snapshot.language, TrayText::KillBackgroundProcesses).to_string()),
        enabled: snapshot.background_process_count > 0,
    });
    entries.push(separator_entry());
    entries.push(TrayMenuEntry {
        id: Some(QUIT_ID.to_string()),
        label: Some(tray_text(snapshot.language, TrayText::Quit).to_string()),
        enabled: true,
    });
    entries
}

fn separator_entry() -> TrayMenuEntry {
    TrayMenuEntry {
        id: None,
        label: None,
        enabled: false,
    }
}

fn read_snapshot() -> TraySnapshot {
    let Ok(client) = SessionDbClient::discover() else {
        return TraySnapshot::default();
    };
    let Ok(mut workspaces) = client.list_workspaces() else {
        return TraySnapshot::default();
    };
    workspaces.sort_by(|a, b| {
        b.last_updated_at
            .cmp(&a.last_updated_at)
            .then_with(|| a.directory.cmp(&b.directory))
    });
    let last_workspace = workspaces
        .first()
        .map(|workspace| workspace.directory.clone());
    let language = read_tray_language(last_workspace.as_deref());
    let active_sessions = active_sessions(&client, &workspaces, language);
    let session_process_directory = session_process_directory(last_workspace.as_deref());
    let background_process_count = session_process_directory
        .as_deref()
        .map(|directory| {
            crate::session::process_snapshot::collect_session_process_snapshot(Path::new(directory))
                .processes
                .len()
        })
        .unwrap_or(0);
    TraySnapshot {
        active_sessions,
        last_workspace,
        session_process_directory,
        background_process_count,
        language,
    }
}

fn session_process_directory(last_workspace: Option<&str>) -> Option<String> {
    crate::mock::global_store()
        .get_current_directory()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            last_workspace
                .filter(|value| !value.trim().is_empty())
                .map(ToString::to_string)
        })
}

fn active_sessions(
    client: &SessionDbClient,
    workspaces: &[WorkspaceSummary],
    language: TrayLanguage,
) -> Vec<ActiveSessionItem> {
    let mut sessions = Vec::new();
    for workspace in workspaces.iter().take(24) {
        let Ok((_page, summaries)) =
            client.list_session_summaries(workspace.directory.clone(), 0, 50)
        else {
            continue;
        };
        for summary in summaries.into_iter().filter(is_active_session) {
            sessions.push(ActiveSessionItem {
                label: session_label(&summary, language),
                session_id: summary.session_id,
                workspace: summary.workspace,
            });
        }
    }
    sessions.sort_by(|a, b| a.label.cmp(&b.label));
    sessions.truncate(MAX_TRAY_SESSIONS);
    sessions
}

fn is_active_session(session: &SessionSummary) -> bool {
    let state = session
        .status
        .as_deref()
        .or(session.state.as_deref())
        .unwrap_or_default()
        .to_ascii_lowercase();
    matches!(
        state.as_str(),
        "active" | "busy" | "queued" | "running" | "starting"
    ) || session
        .task_management
        .get("state")
        .and_then(serde_json::Value::as_str)
        .is_some_and(|value| matches!(value, "active" | "busy" | "queued" | "running"))
}

fn session_label(session: &SessionSummary, language: TrayLanguage) -> String {
    let title = session
        .name
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| tray_text(language, TrayText::Session));
    let workspace = Path::new(&session.workspace)
        .file_name()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .unwrap_or(session.workspace.as_str());
    format!(
        "{} - {}",
        truncate_label(title, SESSION_TITLE_MAX_CHARS),
        truncate_label(workspace, SESSION_WORKSPACE_MAX_CHARS)
    )
}

fn truncate_label(value: &str, max_chars: usize) -> String {
    let mut output = String::new();
    for (index, ch) in value.chars().enumerate() {
        if index >= max_chars {
            output.push_str("...");
            return output;
        }
        output.push(ch);
    }
    output
}

enum TrayText {
    NoActiveSessions,
    OpenGui,
    KillBackgroundProcesses,
    Quit,
    Session,
}

fn tray_text(language: TrayLanguage, text: TrayText) -> &'static str {
    match (language, text) {
        (TrayLanguage::ZhCN, TrayText::NoActiveSessions) => "无活动会话",
        (TrayLanguage::ZhCN, TrayText::OpenGui) => "打开 GUI",
        (TrayLanguage::ZhCN, TrayText::KillBackgroundProcesses) => "杀死所有后台进程",
        (TrayLanguage::ZhCN, TrayText::Quit) => "退出",
        (TrayLanguage::ZhCN, TrayText::Session) => "会话",
        (TrayLanguage::En, TrayText::NoActiveSessions) => "No active sessions",
        (TrayLanguage::En, TrayText::OpenGui) => "Open GUI",
        (TrayLanguage::En, TrayText::KillBackgroundProcesses) => "Kill all background processes",
        (TrayLanguage::En, TrayText::Quit) => "Quit",
        (TrayLanguage::En, TrayText::Session) => "Session",
    }
}

fn background_process_count_label(language: TrayLanguage, count: usize) -> String {
    match language {
        TrayLanguage::ZhCN => format!("后台进程：{count}"),
        TrayLanguage::En => format!("Background processes: {count}"),
    }
}

fn read_tray_language(workspace: Option<&str>) -> TrayLanguage {
    workspace
        .filter(|value| !value.trim().is_empty())
        .and_then(|workspace| crate::session::config::load_config(workspace).language)
        .as_deref()
        .and_then(parse_tray_language)
        .or_else(|| {
            crate::mock::global_store()
                .get_config()
                .language
                .as_deref()
                .and_then(parse_tray_language)
        })
        .or_else(|| {
            std::env::var("TURA_LANG")
                .ok()
                .as_deref()
                .and_then(parse_tray_language)
        })
        .or_else(|| {
            std::env::var("LANG")
                .ok()
                .as_deref()
                .and_then(parse_tray_language)
        })
        .unwrap_or_default()
}

fn parse_tray_language(value: &str) -> Option<TrayLanguage> {
    match value.trim().to_ascii_lowercase().as_str() {
        "zh" | "zh-cn" | "cn" => Some(TrayLanguage::ZhCN),
        "en" | "en-us" | "en-gb" => Some(TrayLanguage::En),
        _ => None,
    }
}

fn is_left_click_release(event: &TrayIconEvent) -> bool {
    matches!(
        event,
        TrayIconEvent::Click {
            button: MouseButton::Left,
            button_state: MouseButtonState::Up,
            ..
        }
    )
}

fn snapshots_equal(left: &TraySnapshot, right: &TraySnapshot) -> bool {
    left.last_workspace == right.last_workspace
        && left.session_process_directory == right.session_process_directory
        && left.background_process_count == right.background_process_count
        && left.language == right.language
        && left.active_sessions.len() == right.active_sessions.len()
        && left
            .active_sessions
            .iter()
            .zip(&right.active_sessions)
            .all(|(left, right)| {
                left.session_id == right.session_id
                    && left.workspace == right.workspace
                    && left.label == right.label
            })
}

fn load_tura_icon() -> Result<Icon> {
    let image = image::load_from_memory(include_bytes!("../../../assets/tura/32x32.png"))
        .context("failed to decode Tura tray icon")?
        .into_rgba8();
    let (width, height) = image.dimensions();
    Icon::from_rgba(image.into_raw(), width, height).map_err(|error| anyhow!(error))
}

fn open_gui(port: u16, workspace: Option<&str>, session_id: Option<&str>) -> Result<Child> {
    spawn_detached(gui_command(port, workspace, session_id)?)
}

fn gui_command(port: u16, workspace: Option<&str>, session_id: Option<&str>) -> Result<Command> {
    let binary = resolve_gui_binary().ok_or_else(|| {
        anyhow!("tura_gui binary not found; set TURA_GUI_BIN to the desktop GUI executable")
    })?;
    let mut command = Command::new(binary);
    command.args(gui_args(port, workspace, session_id));
    Ok(command)
}

fn gui_args(port: u16, workspace: Option<&str>, session_id: Option<&str>) -> Vec<String> {
    let gateway_url = format!("http://127.0.0.1:{port}");
    let mut args = vec!["--gateway-url".to_string(), gateway_url];
    if let Some(workspace) = workspace.filter(|value| !value.trim().is_empty()) {
        args.push("--workspace".to_string());
        args.push(workspace.to_string());
    }
    if let Some(session_id) = session_id.filter(|value| !value.trim().is_empty()) {
        args.push("--session-id".to_string());
        args.push(session_id.to_string());
    }
    args
}

fn resolve_gui_binary() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("TURA_GUI_BIN") {
        let path = PathBuf::from(path);
        if !path.as_os_str().is_empty() {
            return Some(path);
        }
    }

    let binary_name = if cfg!(windows) {
        "tura_gui.exe"
    } else {
        "tura_gui"
    };
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(Path::to_path_buf))
        .unwrap_or_default();
    let root = std::env::var_os("TURA_PROJECT_ROOT")
        .map(PathBuf::from)
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_else(|| PathBuf::from("."));
    let parent = exe_dir.parent().unwrap_or(&exe_dir);
    let build_kind = tura_path::build_kind();
    let candidates = [
        exe_dir.join(binary_name),
        exe_dir.join("bin").join(binary_name),
        parent.join(binary_name),
        parent.join("bin").join(binary_name),
        root.join("target").join(build_kind).join(binary_name),
        root.join("target").join("debug").join(binary_name),
        root.join("target").join("release").join(binary_name),
    ];

    candidates.into_iter().find(|path| path.exists())
}

fn spawn_detached(mut command: Command) -> Result<Child> {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    tura_path::process_hardening::hide_child_console_window(&mut command);
    command.spawn().map_err(Into::into)
}

fn terminate_tracked_clients(children: &mut Vec<Child>) {
    for mut child in children.drain(..) {
        match child.try_wait() {
            Ok(Some(_)) => {}
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
            }
            Err(error) => {
                tracing::warn!(error = %error, "failed to inspect launched gateway client")
            }
        }
    }
}

fn stop_all_session_processes(session_directory: &Path) {
    let snapshot =
        crate::session::process_snapshot::collect_session_process_snapshot(session_directory);
    for process in snapshot.processes {
        if let Err(error) =
            crate::session::process_snapshot::stop_session_process(session_directory, process.pid)
        {
            tracing::warn!(
                pid = process.pid,
                error,
                "failed to stop session background process"
            );
        }
    }
}

fn terminate_gateway_clients_by_command_line(gateway_url: &str) {
    let mut system = sysinfo::System::new_all();
    system.refresh_processes();
    let current_pid = std::process::id();
    let gateway_url = gateway_url.trim_end_matches('/');
    for process in system.processes().values() {
        if process.pid().as_u32() == current_pid {
            continue;
        }
        if !is_gateway_client_process(process, gateway_url) {
            continue;
        }
        if !process.kill() {
            tracing::warn!(
                pid = process.pid().as_u32(),
                "failed to terminate gateway client"
            );
        }
    }
}

fn is_gateway_client_process(process: &sysinfo::Process, gateway_url: &str) -> bool {
    let fields = process
        .cmd()
        .iter()
        .chain(process.environ().iter())
        .map(ToString::to_string)
        .collect::<Vec<String>>();
    let has_gateway_url = fields.iter().any(|value| {
        value.trim_end_matches('/').contains(gateway_url)
            || value.contains(&format!("TURA_GATEWAY_URL={gateway_url}"))
    });
    if !has_gateway_url {
        return false;
    }
    fields.iter().any(|value| {
        let value = value.to_ascii_lowercase();
        value.contains("tura_gui")
            || value.contains("apps/tui")
            || value.contains("apps\\tui")
            || value.ends_with("tura")
            || value.ends_with("tura.cmd")
    })
}

#[cfg(test)]
mod tests {
    use super::{
        gui_args, is_active_session, load_tura_icon, menu_model, parse_tray_language,
        session_label, tray_enabled, tray_text, TrayLanguage, TraySnapshot, TrayText,
    };
    use serde_json::json;
    use session_log::SessionSummary;
    use std::ffi::OsString;
    use std::process::{Child, Command, Stdio};
    use std::sync::Mutex;
    use std::thread;
    use std::time::{Duration, Instant};

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn summary(status: Option<&str>, state: Option<&str>) -> SessionSummary {
        SessionSummary {
            session_id: "session-123456".to_string(),
            workspace: "/tmp/workspace".to_string(),
            name: Some("Build the tray".to_string()),
            parent_id: None,
            created_at: 1,
            updated_at: 2,
            last_user_message_at: Some(1),
            state: state.map(str::to_string),
            status: status.map(str::to_string),
            message_count: 1,
            task_management: json!({}),
        }
    }

    #[test]
    fn tray_menu_contract_shows_background_processes_and_removes_tui_entry() {
        let snapshot = TraySnapshot {
            active_sessions: Vec::new(),
            last_workspace: Some("C:\\repo".to_string()),
            session_process_directory: Some("C:\\repo".to_string()),
            background_process_count: 0,
            language: TrayLanguage::En,
        };

        let labels = menu_model(&snapshot)
            .into_iter()
            .filter_map(|entry| entry.label)
            .collect::<Vec<_>>();

        assert!(
            labels
                .iter()
                .any(|label| label == "Background processes: 0"),
            "tray menu should always expose the background process count: {labels:?}"
        );
        assert!(
            labels.iter().any(|label| label == "Kill all background processes"),
            "tray menu should expose a kill-all action even when there are no active sessions: {labels:?}"
        );
        assert!(
            labels.iter().all(|label| label != "Open TUI"),
            "tray menu should no longer expose the Open TUI action: {labels:?}"
        );
    }

    #[test]
    fn tray_menu_enables_kill_all_when_background_processes_exist() {
        let snapshot = TraySnapshot {
            active_sessions: Vec::new(),
            last_workspace: Some("C:\\repo".to_string()),
            session_process_directory: Some("C:\\repo".to_string()),
            background_process_count: 3,
            language: TrayLanguage::En,
        };

        let entries = menu_model(&snapshot);
        assert!(entries.iter().any(|entry| {
            entry.label.as_deref() == Some("Background processes: 3") && !entry.enabled
        }));
        assert!(entries.iter().any(|entry| {
            entry.label.as_deref() == Some("Kill all background processes") && entry.enabled
        }));
    }

    #[test]
    fn tray_kill_all_background_processes_stops_session_processes() -> anyhow::Result<()> {
        let workspace = tempfile::tempdir()?;
        let mut child = spawn_long_running_child(workspace.path())?;
        wait_until(Duration::from_secs(8), || {
            let snapshot = crate::session::process_snapshot::collect_session_process_snapshot(
                workspace.path(),
            );
            snapshot
                .processes
                .iter()
                .any(|process| process.pid == child.id())
                .then_some(())
                .ok_or_else(|| anyhow::anyhow!("child process not visible yet"))
        })?;

        super::stop_all_session_processes(workspace.path());

        wait_until(Duration::from_secs(8), || match child.try_wait()? {
            Some(_status) => Ok(()),
            None => Err(anyhow::anyhow!("child process still running")),
        })?;
        Ok(())
    }

    #[test]
    fn active_session_filter_accepts_running_statuses() {
        assert!(is_active_session(&summary(Some("busy"), None)));
        assert!(is_active_session(&summary(None, Some("running"))));
        assert!(!is_active_session(&summary(Some("idle"), None)));
    }

    #[test]
    fn tray_enabled_respects_explicit_disable() {
        let _guard = ENV_LOCK.lock().expect("env lock");
        let previous = std::env::var_os("TURA_GATEWAY_TRAY");
        std::env::set_var("TURA_GATEWAY_TRAY", "0");

        assert!(!tray_enabled());

        restore_env_var("TURA_GATEWAY_TRAY", previous);
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn tray_enabled_rejects_headless_linux_session() {
        let _guard = ENV_LOCK.lock().expect("env lock");
        let previous_tray = std::env::var_os("TURA_GATEWAY_TRAY");
        let previous_display = std::env::var_os("DISPLAY");
        let previous_wayland = std::env::var_os("WAYLAND_DISPLAY");
        std::env::remove_var("TURA_GATEWAY_TRAY");
        std::env::remove_var("DISPLAY");
        std::env::remove_var("WAYLAND_DISPLAY");

        assert!(!tray_enabled());

        restore_env_var("TURA_GATEWAY_TRAY", previous_tray);
        restore_env_var("DISPLAY", previous_display);
        restore_env_var("WAYLAND_DISPLAY", previous_wayland);
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn tray_enabled_accepts_linux_display_session() {
        let _guard = ENV_LOCK.lock().expect("env lock");
        let previous_tray = std::env::var_os("TURA_GATEWAY_TRAY");
        let previous_display = std::env::var_os("DISPLAY");
        let previous_wayland = std::env::var_os("WAYLAND_DISPLAY");
        std::env::remove_var("TURA_GATEWAY_TRAY");
        std::env::set_var("DISPLAY", ":99");
        std::env::remove_var("WAYLAND_DISPLAY");

        assert!(tray_enabled());

        restore_env_var("TURA_GATEWAY_TRAY", previous_tray);
        restore_env_var("DISPLAY", previous_display);
        restore_env_var("WAYLAND_DISPLAY", previous_wayland);
    }

    #[test]
    fn session_labels_include_title_workspace_and_state() {
        let label = session_label(&summary(Some("busy"), None), TrayLanguage::En);
        assert!(label.contains("Build the tray"));
        assert!(label.contains("workspace"));
        assert!(!label.contains("busy"));
    }

    #[test]
    fn tray_icon_loads_tura_asset_without_creating_os_tray() {
        load_tura_icon().expect("load Tura tray icon asset");
    }

    #[test]
    fn tray_session_action_builds_desktop_gui_args() {
        let args = gui_args(4126, Some("C:\\repo with spaces"), Some("session-123"));

        assert_eq!(
            args,
            vec![
                "--gateway-url".to_string(),
                "http://127.0.0.1:4126".to_string(),
                "--workspace".to_string(),
                "C:\\repo with spaces".to_string(),
                "--session-id".to_string(),
                "session-123".to_string(),
            ]
        );
    }

    #[test]
    fn tray_menu_text_uses_configured_language_and_short_labels() {
        assert_eq!(parse_tray_language("zh-CN"), Some(TrayLanguage::ZhCN));
        assert_eq!(tray_text(TrayLanguage::ZhCN, TrayText::OpenGui), "打开 GUI");
        let label = session_label(&summary(Some("busy"), None), TrayLanguage::ZhCN);

        assert!(label.contains("Build the tray"));
        assert!(label.chars().count() <= 46);
    }

    fn restore_env_var(name: &str, value: Option<OsString>) {
        if let Some(value) = value {
            std::env::set_var(name, value);
        } else {
            std::env::remove_var(name);
        }
    }

    fn spawn_long_running_child(workspace: &std::path::Path) -> anyhow::Result<Child> {
        let mut command = if cfg!(windows) {
            let mut command = Command::new("powershell");
            command.args([
                "-NoProfile",
                "-ExecutionPolicy",
                "Bypass",
                "-Command",
                "Set-Content -Path tray-child-ready.txt -Value $PID; Start-Sleep -Seconds 60",
            ]);
            command
        } else {
            let mut command = Command::new("sh");
            command.args(["-c", "echo $$ > tray-child-ready.txt; sleep 60"]);
            command
        };
        command
            .current_dir(workspace)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(Into::into)
    }

    fn wait_until<T>(
        timeout: Duration,
        mut attempt: impl FnMut() -> anyhow::Result<T>,
    ) -> anyhow::Result<T> {
        let started = Instant::now();
        let mut last_error = None;
        while started.elapsed() < timeout {
            match attempt() {
                Ok(value) => return Ok(value),
                Err(error) => last_error = Some(error),
            }
            thread::sleep(Duration::from_millis(100));
        }
        Err(last_error.unwrap_or_else(|| anyhow::anyhow!("condition timed out")))
    }
}
