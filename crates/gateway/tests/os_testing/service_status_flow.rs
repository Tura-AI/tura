use std::io::{BufRead, BufReader, Write};
use std::net::{Shutdown, TcpListener};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{anyhow, Context, Result};
use gateway::api::service::get_service_status;
use gateway::mock::global_store;

static ENV_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

#[tokio::test]
async fn gateway_service_status_business_flow_reports_router_session_processes_and_docker_shape(
) -> Result<()> {
    let _guard = ENV_LOCK.lock().await;
    let temp = tempfile::tempdir().context("service status temp root")?;
    let home = temp.path().join("home");
    let workspace = temp.path().join("workspace");
    std::fs::create_dir_all(&home)?;
    std::fs::create_dir_all(&workspace)?;
    let _env = EnvGuard::new(&home);
    let _current_directory = CurrentDirectoryGuard::set(workspace.display().to_string());
    let router_server = FakeRouterEndpoint::start(&home)?;
    let child = ChildGuard::spawn(&workspace)?;

    let started = Instant::now();
    let mut saw_workspace_child = false;
    while started.elapsed() < Duration::from_secs(8) {
        let response = get_service_status().await.0;
        if response.session_processes.as_ref().is_some_and(|snapshot| {
            snapshot
                .processes
                .iter()
                .any(|process| process.pid == child.id())
        }) {
            saw_workspace_child = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert!(
        saw_workspace_child,
        "service status should observe the workspace child process within the timeout"
    );

    let response = get_service_status().await.0;
    assert_eq!(response.mano.status, "connected");
    assert_eq!(response.router.status, "running");
    assert_eq!(response.router.pid, Some(std::process::id()));
    assert!(response.router.process_start_time.is_some());
    assert!(
        response.router.error.is_none(),
        "fake reachable router should avoid daemon startup errors: {:?}",
        response.router.error
    );

    let session_processes = response
        .session_processes
        .as_ref()
        .ok_or_else(|| anyhow!("service status should include session process snapshot"))?;
    assert_eq!(
        session_processes.session_directory,
        workspace.display().to_string()
    );
    let process = session_processes
        .processes
        .iter()
        .find(|process| process.pid == child.id())
        .ok_or_else(|| anyhow!("workspace child process should be present in service status"))?;
    assert_eq!(process.kind, "workspace");
    assert!(
        process
            .cwd
            .as_deref()
            .is_some_and(|cwd| path_text_mentions(cwd, &workspace))
            || process
                .command_line
                .replace('\\', "/")
                .to_ascii_lowercase()
                .contains(
                    &workspace
                        .display()
                        .to_string()
                        .replace('\\', "/")
                        .to_ascii_lowercase()
                ),
        "service status should preserve workspace location context for the process: {process:?}"
    );

    assert!(
        response.docker.available || response.docker.error.is_some(),
        "docker snapshot should report either availability or an explanatory error"
    );
    assert!(
        response.docker.containers.len() <= 32,
        "docker snapshot should remain bounded for service status responses"
    );

    drop(router_server);
    Ok(())
}

struct FakeRouterEndpoint {
    handle: Option<std::thread::JoinHandle<Result<()>>>,
    stop: Arc<AtomicBool>,
}

impl FakeRouterEndpoint {
    fn start(home: &Path) -> Result<Self> {
        let listener = TcpListener::bind(("127.0.0.1", 0)).context("bind fake router")?;
        let addr = listener.local_addr()?.to_string();
        let process_start_time =
            wait_for_current_process_start_time(std::process::id(), Duration::from_secs(2))?;
        let path = home.join("db").join("session_log").join("router.addr");
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(
            &path,
            serde_json::json!({
                "addr": addr,
                "version": tura_path::instance_version(),
                "pid": std::process::id(),
                "process_start_time": process_start_time,
            })
            .to_string(),
        )?;
        let stop = Arc::new(AtomicBool::new(false));
        let thread_stop = Arc::clone(&stop);
        let handle = thread::spawn(move || -> Result<()> {
            listener
                .set_nonblocking(true)
                .context("set fake router nonblocking")?;
            let started = Instant::now();
            while started.elapsed() < Duration::from_secs(10) && !thread_stop.load(Ordering::SeqCst)
            {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        stream
                            .set_read_timeout(Some(Duration::from_secs(2)))
                            .context("set fake router read timeout")?;
                        let mut request_line = String::new();
                        let _ = BufReader::new(stream.try_clone()?).read_line(&mut request_line);
                        if request_line.trim().is_empty() {
                            continue;
                        }
                        let request: serde_json::Value = serde_json::from_str(request_line.trim())
                            .context("decode fake router health request")?;
                        stream.write_all(
                            serde_json::json!({
                                "ok": true,
                                "request_id": request.get("request_id").cloned().unwrap_or(serde_json::Value::Null),
                                "payload": {
                                    "pid": std::process::id(),
                                    "process_start_time": process_start_time,
                                }
                            })
                            .to_string()
                            .as_bytes(),
                        )?;
                        stream.write_all(b"\n")?;
                        stream.flush()?;
                        stream.shutdown(Shutdown::Write)?;
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(20));
                    }
                    Err(error) => return Err(error).context("accept fake router probe"),
                }
            }
            Ok(())
        });
        Ok(Self {
            handle: Some(handle),
            stop,
        })
    }
}

fn current_process_start_time(pid: u32) -> Option<u64> {
    let mut system = sysinfo::System::new_all();
    system.refresh_processes();
    system
        .process(sysinfo::Pid::from_u32(pid))
        .map(sysinfo::Process::start_time)
}

fn wait_for_current_process_start_time(pid: u32, timeout: Duration) -> Result<u64> {
    let started = Instant::now();
    loop {
        if let Some(start_time) = current_process_start_time(pid) {
            return Ok(start_time);
        }
        if started.elapsed() >= timeout {
            return Err(anyhow!(
                "fake router process {pid} was absent from the process snapshot"
            ));
        }
        thread::sleep(Duration::from_millis(20));
    }
}

impl Drop for FakeRouterEndpoint {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

struct CurrentDirectoryGuard {
    previous: Option<String>,
}

impl CurrentDirectoryGuard {
    fn set(directory: String) -> Self {
        let previous = global_store().get_current_directory();
        global_store().set_current_directory(directory);
        Self { previous }
    }
}

impl Drop for CurrentDirectoryGuard {
    fn drop(&mut self) {
        match self.previous.take() {
            Some(directory) => global_store().set_current_directory(directory),
            None => *global_store().current_directory.write() = None,
        }
    }
}

struct EnvGuard {
    previous: Vec<(&'static str, Option<std::ffi::OsString>)>,
}

impl EnvGuard {
    fn new(home: &Path) -> Self {
        let keys = [
            "TURA_HOME",
            "SESSION_LOG_DB_ROOT",
            "TURA_DB_ROOT",
            "TURA_GATEWAY_ALLOW_IN_PROCESS_FAKE_ROUTER",
        ];
        let previous = keys
            .iter()
            .map(|key| (*key, std::env::var_os(key)))
            .collect::<Vec<_>>();
        std::env::set_var("TURA_HOME", home);
        std::env::remove_var("SESSION_LOG_DB_ROOT");
        std::env::remove_var("TURA_DB_ROOT");
        std::env::set_var("TURA_GATEWAY_ALLOW_IN_PROCESS_FAKE_ROUTER", "1");
        Self { previous }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        for (key, value) in self.previous.drain(..) {
            match value {
                Some(value) => std::env::set_var(key, value),
                None => std::env::remove_var(key),
            }
        }
    }
}

struct ChildGuard {
    child: Child,
}

impl ChildGuard {
    fn spawn(workspace: &Path) -> Result<Self> {
        let mut command = if cfg!(windows) {
            let mut command = Command::new("powershell");
            command.args([
                "-NoProfile",
                "-ExecutionPolicy",
                "Bypass",
                "-Command",
                "Set-Content -Path service-status-ready.txt -Value $PID; Start-Sleep -Seconds 60",
            ]);
            command
        } else {
            let mut command = Command::new("sh");
            command.args(["-c", "echo $$ > service-status-ready.txt; sleep 60"]);
            command
        };
        let child = command
            .current_dir(workspace)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .context("spawn service status child")?;
        Ok(Self { child })
    }

    fn id(&self) -> u32 {
        self.child.id()
    }
}

impl Drop for ChildGuard {
    fn drop(&mut self) {
        if matches!(self.child.try_wait(), Ok(Some(_))) {
            return;
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn path_text_mentions(text: &str, path: &Path) -> bool {
    let normalized_text = text.replace('\\', "/").to_ascii_lowercase();
    let normalized_path = path
        .display()
        .to_string()
        .replace('\\', "/")
        .to_ascii_lowercase();
    normalized_text.contains(&normalized_path)
}
