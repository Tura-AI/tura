use anyhow::{anyhow, Result};
use serde_json::Value;
use session_log::{
    CommandCheckpoint, GetSessionRequest, ListSessionRecordsRequest, ListSessionsRequest,
    SessionLogCommand, SessionLogResponse, UpsertSessionRequest,
};

pub use session_log::{Page, SessionRecord, SessionSnapshot, WorkspaceSummary};

#[derive(Debug, Clone, Default)]
pub struct SessionLogClient;

impl SessionLogClient {
    pub fn discover() -> Result<Self> {
        Ok(Self)
    }

    pub fn upsert_session(
        &self,
        session: Value,
        parent_id: Option<String>,
        messages: Vec<Value>,
        todos: Vec<Value>,
    ) -> Result<()> {
        match self.call(SessionLogCommand::UpsertSession(UpsertSessionRequest {
            session,
            parent_id,
            messages,
            todos,
        }))? {
            SessionLogResponse::Ok => Ok(()),
            SessionLogResponse::Error { error } => Err(anyhow!(error)),
            other => Err(anyhow!("unexpected session_log response: {other:?}")),
        }
    }

    pub fn apply_command_checkpoint(&self, checkpoint: CommandCheckpoint) -> Result<()> {
        match self.call(SessionLogCommand::ApplyCommandCheckpoint(Box::new(
            checkpoint,
        )))? {
            SessionLogResponse::Ok => Ok(()),
            SessionLogResponse::Error { error } => Err(anyhow!(error)),
            other => Err(anyhow!("unexpected session_log response: {other:?}")),
        }
    }

    pub fn list_workspaces(&self) -> Result<Vec<WorkspaceSummary>> {
        match self.call(SessionLogCommand::ListWorkspaces)? {
            SessionLogResponse::Workspaces { workspaces } => Ok(workspaces),
            SessionLogResponse::Error { error } => Err(anyhow!(error)),
            other => Err(anyhow!("unexpected session_log response: {other:?}")),
        }
    }

    pub fn list_sessions(
        &self,
        workspace: String,
        page: u64,
        page_size: u64,
    ) -> Result<(Page, Vec<SessionSnapshot>)> {
        match self.call(SessionLogCommand::ListSessions(ListSessionsRequest {
            workspace,
            page,
            page_size,
        }))? {
            SessionLogResponse::Sessions { page, sessions } => Ok((page, sessions)),
            SessionLogResponse::Error { error } => Err(anyhow!(error)),
            other => Err(anyhow!("unexpected session_log response: {other:?}")),
        }
    }

    pub fn get_session(&self, session_id: String) -> Result<Option<SessionSnapshot>> {
        match self.call(SessionLogCommand::GetSession(GetSessionRequest {
            session_id,
        }))? {
            SessionLogResponse::Session { session } => Ok(session.map(|session| *session)),
            SessionLogResponse::Error { error } => Err(anyhow!(error)),
            other => Err(anyhow!("unexpected session_log response: {other:?}")),
        }
    }

    pub fn list_session_records(
        &self,
        session_id: String,
        page: u64,
        page_size: u64,
    ) -> Result<(Page, Vec<SessionRecord>)> {
        match self.call(SessionLogCommand::ListSessionRecords(
            ListSessionRecordsRequest {
                session_id,
                page,
                page_size,
            },
        ))? {
            SessionLogResponse::Records { page, records } => Ok((page, records)),
            SessionLogResponse::Error { error } => Err(anyhow!(error)),
            other => Err(anyhow!("unexpected session_log response: {other:?}")),
        }
    }

    fn call(&self, command: SessionLogCommand) -> Result<SessionLogResponse> {
        let raw = serde_json::to_vec(&command)?;
        let router_bin = std::env::var("TURA_ROUTER_BIN")
            .map(std::path::PathBuf::from)
            .ok()
            .or_else(resolve_router_binary)
            .ok_or_else(|| anyhow!("session_db service command not found: tura_router"))?;
        let mut command = std::process::Command::new(&router_bin);
        command
            .arg("session-db-call")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());
        hide_child_window(&mut command);
        let mut child = command.spawn()?;
        if let Some(mut stdin) = child.stdin.take() {
            use std::io::Write;
            stdin.write_all(&raw)?;
        }
        let output = wait_with_timeout(child, session_db_call_timeout())?;
        if !output.status.success() {
            return Err(anyhow!(
                "{}",
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
        Ok(serde_json::from_slice(&output.stdout)?)
    }
}

fn session_db_call_timeout() -> std::time::Duration {
    std::env::var("TURA_SESSION_DB_CALL_TIMEOUT_SECS")
        .ok()
        .and_then(|value| value.trim().parse::<u64>().ok())
        .filter(|seconds| *seconds > 0)
        .map(std::time::Duration::from_secs)
        .unwrap_or_else(|| std::time::Duration::from_secs(15))
}

fn wait_with_timeout(
    mut child: std::process::Child,
    timeout: std::time::Duration,
) -> Result<std::process::Output> {
    let started = std::time::Instant::now();
    loop {
        if child.try_wait()?.is_some() {
            return child.wait_with_output().map_err(Into::into);
        }
        if started.elapsed() >= timeout {
            kill_process_tree(child.id());
            let _ = child.kill();
            let output = child.wait_with_output()?;
            return Err(anyhow!(
                "session_db call timed out after {}s; stdout={} stderr={}",
                timeout.as_secs(),
                String::from_utf8_lossy(&output.stdout).trim(),
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
}

fn kill_process_tree(pid: u32) {
    #[cfg(windows)]
    {
        let _ = std::process::Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/T", "/F"])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .and_then(|mut child| child.wait());
    }
    #[cfg(not(windows))]
    {
        let _ = pid;
    }
}

fn resolve_router_binary() -> Option<std::path::PathBuf> {
    let root = std::env::var("TURA_PROJECT_ROOT")
        .ok()
        .map(std::path::PathBuf::from)
        .or_else(|| {
            std::env::current_dir()
                .ok()
                .as_deref()
                .and_then(find_repo_root)
        })
        .or_else(|| {
            std::env::current_exe()
                .ok()
                .as_deref()
                .and_then(find_repo_root)
        })?;
    let executable = if cfg!(windows) {
        "tura_router.exe"
    } else {
        "tura_router"
    };
    let mut candidates = Vec::new();
    if let Ok(current_exe) = std::env::current_exe() {
        candidates.push(current_exe.with_file_name(executable));
    }
    candidates.push(root.join("bin").join(executable));
    candidates.push(root.join("target").join("release").join(executable));
    candidates.push(root.join("target").join("debug").join(executable));
    candidates.into_iter().find(|path| path.exists())
}

fn find_repo_root(path: &std::path::Path) -> Option<std::path::PathBuf> {
    let start = if path.is_dir() {
        path
    } else {
        path.parent().unwrap_or(path)
    };
    start
        .ancestors()
        .find(|candidate| candidate.join("crates").join("router").is_dir())
        .map(std::path::Path::to_path_buf)
}

fn hide_child_window(command: &mut std::process::Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
}
