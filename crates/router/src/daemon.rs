use serde_json::json;
use std::sync::{atomic::Ordering, Arc};

use crate::app::build_state;
use crate::ipc;
use crate::ipc_handlers::{
    enqueue_turn_session_id, handle_ipc_request, handle_ipc_request_with_notifications,
};
use crate::process_info::current_process_start_time;
use crate::services::{
    recovery::recover_after_start, runtime_orphans::cleanup_orphan_runtime_workers,
};
use crate::shutdown::start_idle_shutdown_monitor;

pub(crate) async fn serve_stdio() -> anyhow::Result<()> {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

    let state = build_state();
    let _ = recover_after_start(&state.session_db)?;
    let stdin = tokio::io::stdin();
    // Shared, locked writer: each request is handled on its own task and writes
    // its response (tagged with `request_id`) when ready, so a slow call (e.g. a
    // long-running `execution.enqueue_turn`) never head-of-line blocks a
    // concurrent `health_check`. The gateway client multiplexes responses back
    // to per-call mailboxes by `request_id`.
    let stdout = Arc::new(tokio::sync::Mutex::new(tokio::io::stdout()));
    let mut lines = BufReader::new(stdin).lines();
    while let Some(line) = lines.next_line().await? {
        let trimmed = line.trim().to_string();
        if trimmed.is_empty() {
            continue;
        }
        let state = state.clone();
        let stdout = Arc::clone(&stdout);
        tokio::spawn(async move {
            let response = match serde_json::from_str::<ipc::IpcRequest>(&trimmed) {
                Ok(request) => handle_ipc_request(&state, request).await,
                Err(error) => {
                    ipc::IpcResponse::error("invalid", format!("invalid ipc request: {error}"))
                }
            };
            if let Ok(encoded) = serde_json::to_string(&response) {
                let mut out = stdout.lock().await;
                let _ = out.write_all(format!("{encoded}\n").as_bytes()).await;
                let _ = out.flush().await;
            }
        });
    }
    Ok(())
}

/// File (under the instance's db dir) recording the running router daemon's
/// socket endpoint, so any front can probe-and-connect rather than spawn its own.
pub(crate) fn router_addr_path() -> std::path::PathBuf {
    session_log::path::default_db_dir().join("router.addr")
}

fn publish_router_addr(addr: &std::net::SocketAddr) -> anyhow::Result<()> {
    let path = router_addr_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let pid = std::process::id();
    let record = json!({
        "addr": addr.to_string(),
        "version": tura_path::instance_version(),
        "pid": pid,
        "process_start_time": current_process_start_time(pid),
    });
    let tmp = path.with_extension("addr.tmp");
    std::fs::write(&tmp, serde_json::to_string(&record)?)?;
    std::fs::rename(&tmp, &path)?;
    Ok(())
}

pub(crate) fn unpublish_router_addr() {
    let _ = std::fs::remove_file(router_addr_path());
}

pub(crate) async fn serve_socket() -> anyhow::Result<()> {
    use tokio::net::TcpListener;
    use tokio::time::{timeout, Duration};

    let _router_lock = RouterDaemonLock::acquire()?;
    let orphan_report = cleanup_orphan_runtime_workers();
    if !orphan_report.killed.is_empty() {
        eprintln!(
            "router startup cleanup: killed orphan runtime workers {:?}",
            orphan_report.killed
        );
    }
    let state = build_state();
    let _ = recover_after_start(&state.session_db)?;
    // The daemon owns the backend: bring up the single session_db owner now.
    let _ = state.session_db.start();

    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;
    publish_router_addr(&addr)?;
    std::env::set_var("TURA_ROUTER_ADDR", addr.to_string());
    eprintln!("router socket daemon listening on {addr}");
    start_idle_shutdown_monitor(state.clone());

    while !state.shutdown.load(Ordering::SeqCst) {
        let accepted = match timeout(Duration::from_millis(250), listener.accept()).await {
            Ok(accepted) => accepted?,
            Err(_) => continue,
        };
        let (stream, _) = accepted;
        let state = state.clone();
        tokio::spawn(async move {
            let _ = handle_socket_connection(state, stream).await;
        });
    }
    unpublish_router_addr();
    code_tools::shell_executor::terminate_retained_shell_process_scopes();
    Ok(())
}

async fn handle_socket_connection(
    state: crate::app::AppState,
    stream: tokio::net::TcpStream,
) -> anyhow::Result<()> {
    use std::collections::HashSet;
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    use tokio::sync::Mutex as AsyncMutex;

    state.lifecycle.connection_opened();
    let (read, write) = stream.into_split();
    let write = Arc::new(AsyncMutex::new(write));
    let active_sessions = Arc::new(AsyncMutex::new(HashSet::<String>::new()));
    let pending_tasks = Arc::new(AsyncMutex::new(Vec::<tokio::task::JoinHandle<()>>::new()));
    let mut lines = BufReader::new(read).lines();
    while let Some(line) = lines.next_line().await? {
        let trimmed = line.trim().to_string();
        if trimmed.is_empty() {
            continue;
        }
        let parsed = match serde_json::from_str::<ipc::IpcRequest>(&trimmed) {
            Ok(request) => request,
            Err(error) => {
                let response =
                    ipc::IpcResponse::error("invalid", format!("invalid ipc request: {error}"));
                if let Ok(encoded) = serde_json::to_string(&response) {
                    let mut writer = write.lock().await;
                    let _ = writer.write_all(format!("{encoded}\n").as_bytes()).await;
                    let _ = writer.flush().await;
                }
                continue;
            }
        };
        state.lifecycle.mark_activity();
        let active_session_id = enqueue_turn_session_id(&parsed);
        if let Some(session_id) = active_session_id.as_ref() {
            active_sessions.lock().await.insert(session_id.clone());
        }
        let abort_on_disconnect = should_abort_request_on_connection_close(&parsed);
        let state_for_task = state.clone();
        let write_for_task = Arc::clone(&write);
        let active_sessions_for_task = Arc::clone(&active_sessions);
        let handle = tokio::spawn(async move {
            let (notification_tx, mut notification_rx) =
                tokio::sync::mpsc::unbounded_channel::<ipc::IpcNotification>();
            let notification_writer = {
                let write = Arc::clone(&write_for_task);
                tokio::spawn(async move {
                    while let Some(notification) = notification_rx.recv().await {
                        if let Ok(encoded) = serde_json::to_string(&notification) {
                            let mut writer = write.lock().await;
                            let _ = writer.write_all(format!("{encoded}\n").as_bytes()).await;
                            let _ = writer.flush().await;
                        }
                    }
                })
            };
            let response = handle_ipc_request_with_notifications(
                &state_for_task,
                parsed,
                Some(notification_tx),
            )
            .await;
            let _ = notification_writer.await;
            if let Some(session_id) = active_session_id.as_ref() {
                active_sessions_for_task.lock().await.remove(session_id);
            }
            if let Ok(encoded) = serde_json::to_string(&response) {
                let mut writer = write_for_task.lock().await;
                let _ = writer.write_all(format!("{encoded}\n").as_bytes()).await;
                let _ = writer.flush().await;
            }
        });
        if abort_on_disconnect {
            pending_tasks.lock().await.push(handle);
        }
    }
    let sessions = active_sessions
        .lock()
        .await
        .iter()
        .cloned()
        .collect::<Vec<_>>();
    for session_id in sessions {
        let _ = state
            .execution
            .cancel_turn(&state, json!({ "session_id": session_id }))
            .await;
    }
    let tasks = pending_tasks.lock().await.drain(..).collect::<Vec<_>>();
    for task in tasks {
        task.abort();
    }
    state.lifecycle.connection_closed();
    Ok(())
}

fn should_abort_request_on_connection_close(request: &ipc::IpcRequest) -> bool {
    request.method != "execution.command_run"
}

struct RouterDaemonLock {
    file: std::fs::File,
    path: std::path::PathBuf,
}

impl RouterDaemonLock {
    fn acquire() -> anyhow::Result<Self> {
        use fs2::FileExt;
        use std::io::{Seek, SeekFrom, Write};

        let dir = tura_path::locks_dir();
        std::fs::create_dir_all(&dir)?;
        let path = dir.join(format!("router-{}.lock", tura_path::build_kind()));
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(&path)?;
        file.try_lock_exclusive().map_err(|error| {
            anyhow::anyhow!(
                "another router daemon already owns {}: {error}",
                path.display()
            )
        })?;
        file.set_len(0)?;
        file.seek(SeekFrom::Start(0))?;
        writeln!(file, "pid={}", std::process::id())?;
        writeln!(file, "kind=router")?;
        writeln!(file, "build_kind={}", tura_path::build_kind())?;
        writeln!(file, "home={}", tura_path::instance_home().display())?;
        Ok(Self { file, path })
    }
}

impl Drop for RouterDaemonLock {
    fn drop(&mut self) {
        let _ = self.file.unlock();
        let _ = std::fs::remove_file(&self.path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::AsyncWriteExt;

    #[test]
    fn command_run_requests_are_detached_from_runtime_socket_disconnect() {
        let request = ipc::IpcRequest {
            request_id: "command-run".to_string(),
            kind: "call".to_string(),
            method: "execution.command_run".to_string(),
            payload: json!({}),
            deadline_ms: None,
        };
        assert!(!should_abort_request_on_connection_close(&request));

        let request = ipc::IpcRequest {
            method: "execution.enqueue_turn".to_string(),
            ..request
        };
        assert!(should_abort_request_on_connection_close(&request));
    }

    #[tokio::test]
    async fn command_run_survives_runtime_socket_disconnect_until_router_finishes(
    ) -> anyhow::Result<()> {
        let state = build_state();
        let workspace = tempfile::tempdir()?;
        let started = workspace.path().join("started.txt");
        let done = workspace.path().join("done.txt");

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let addr = listener.local_addr()?;
        let server_state = state.clone();
        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await?;
            let task = tokio::spawn(async move {
                let _ = handle_socket_connection(server_state, stream).await;
            });
            Ok::<_, anyhow::Error>(task)
        });

        let mut client = tokio::net::TcpStream::connect(addr).await?;
        let request = ipc::IpcRequest {
            request_id: "disconnect-command-run".to_string(),
            kind: "call".to_string(),
            method: "execution.command_run".to_string(),
            payload: json!({
                "session_id": "disconnect-session",
                "runtime_id": "disconnect-runtime",
                "session_directory": workspace.path().display().to_string(),
                "arguments": {
                    "commands": [{
                        "command": "shell_command",
                        "command_line": json!({
                            "command": disconnect_survival_command(),
                            "timeout_ms": 5000
                        }).to_string()
                    }]
                }
            }),
            deadline_ms: None,
        };
        client
            .write_all(format!("{}\n", serde_json::to_string(&request)?).as_bytes())
            .await?;
        client.flush().await?;

        wait_for_path(&started, std::time::Duration::from_secs(2)).await?;
        drop(client);

        wait_for_path(&done, std::time::Duration::from_secs(4)).await?;
        let connection_task = server.await??;
        connection_task.await?;
        wait_for_active_command_runs(&state, 0, std::time::Duration::from_secs(2)).await?;
        Ok(())
    }

    async fn wait_for_path(
        path: &std::path::Path,
        timeout: std::time::Duration,
    ) -> anyhow::Result<()> {
        let deadline = std::time::Instant::now() + timeout;
        while std::time::Instant::now() < deadline {
            if path.exists() {
                return Ok(());
            }
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
        anyhow::bail!("timed out waiting for {}", path.display())
    }

    async fn wait_for_active_command_runs(
        state: &crate::app::AppState,
        expected: usize,
        timeout: std::time::Duration,
    ) -> anyhow::Result<()> {
        let deadline = std::time::Instant::now() + timeout;
        while std::time::Instant::now() < deadline {
            if state.command_run.active_count() == expected {
                return Ok(());
            }
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
        anyhow::bail!(
            "timed out waiting for active command_run count {expected}; got {}",
            state.command_run.active_count()
        )
    }

    fn disconnect_survival_command() -> &'static str {
        if cfg!(windows) {
            "$ErrorActionPreference='Stop'; Set-Content -LiteralPath 'started.txt' -Value 'started'; Start-Sleep -Milliseconds 800; Set-Content -LiteralPath 'done.txt' -Value 'done'"
        } else {
            "set -eu; printf started > started.txt; sleep 0.8; printf done > done.txt"
        }
    }
}
