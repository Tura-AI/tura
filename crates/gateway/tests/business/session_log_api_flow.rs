use anyhow::{anyhow, bail, Context, Result};
use axum::body;
use axum::extract::{Path, Query};
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use gateway::api::session_log::{
    session_log_records, session_log_sessions, session_log_workspaces,
};
use gateway::contracts::{SessionLogListParams, SessionLogRecordsParams};
use serde_json::{json, Value};
use session_log::{SessionLogCommand, SessionLogStore};
use std::path::Path as FsPath;
use std::time::{Duration, Instant};

static ENV_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

#[tokio::test]
async fn gateway_session_log_api_business_flow_lists_workspaces_sessions_and_records() -> Result<()>
{
    let _guard = ENV_LOCK.lock().await;
    let root = tempfile::tempdir().context("temp root")?;
    let home = root.path().join("home");
    let workspace = root.path().join("workspace with space");
    std::fs::create_dir_all(&home)?;
    std::fs::create_dir_all(&workspace)?;
    let _env = EnvGuard::new(&home);
    let service = ServiceThread::start()?;

    let session_id = format!("gateway-session-log-api-{}", uuid::Uuid::new_v4());
    let workspace_key = session_log::path::normalize_workspace(&workspace.to_string_lossy());
    assert_ok(session_log::ipc::call_service(
        &SessionLogCommand::UpsertSession(upsert_request(&session_id, &workspace_key, 1)),
    )?)?;
    assert_ok(session_log::ipc::call_service(
        &SessionLogCommand::UpsertSession(upsert_request(&session_id, &workspace_key, 2)),
    )?)?;

    let (status, workspaces) =
        response_json(session_log_workspaces().await.into_response()).await?;
    assert_eq!(status, StatusCode::OK);
    assert!(
        workspaces["workspaces"]
            .as_array()
            .unwrap_or(&Vec::new())
            .iter()
            .any(|workspace| workspace["directory"] == workspace_key
                && workspace["session_count"] == 1),
        "workspaces response should include written workspace: {workspaces}"
    );

    let (status, sessions) = response_json(
        session_log_sessions(
            HeaderMap::new(),
            Query(SessionLogListParams {
                workspace: Some(workspace_key.clone()),
                page: 0,
                page_size: 10,
            }),
        )
        .await
        .into_response(),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(sessions["page"]["total"], 1);
    assert_eq!(sessions["sessions"][0]["session_id"], session_id);
    assert_eq!(sessions["sessions"][0]["message_count"], 2);
    assert_eq!(sessions["sessions"][0]["state"], "created");
    assert_eq!(sessions["sessions"][0]["status"], "idle");

    let mut headers = HeaderMap::new();
    headers.insert(
        "x-opencode-directory",
        HeaderValue::from_str(&percent_encode_workspace(&workspace_key))?,
    );
    let (status, header_sessions) = response_json(
        session_log_sessions(
            headers,
            Query(SessionLogListParams {
                workspace: None,
                page: 0,
                page_size: 1,
            }),
        )
        .await
        .into_response(),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(header_sessions["page"]["total"], 1);
    assert_eq!(
        header_sessions["sessions"].as_array().map(Vec::len),
        Some(1)
    );

    let (status, records) = response_json(
        session_log_records(
            Path(session_id.clone()),
            Query(SessionLogRecordsParams {
                page: 0,
                page_size: 10,
            }),
        )
        .await
        .into_response(),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(records["page"]["total"], 2);
    assert_eq!(
        records["records"]
            .as_array()
            .ok_or_else(|| anyhow!("records response should contain array"))?
            .iter()
            .map(|record| record["message_id"].as_str().unwrap_or_default())
            .collect::<Vec<_>>(),
        vec!["message-1", "message-2"]
    );

    drop(service);
    wait_until(Duration::from_secs(5), || {
        !session_log::ipc::service_is_running()
    })?;
    Ok(())
}

#[tokio::test]
async fn gateway_session_log_api_business_flow_reports_bad_gateway_when_session_db_is_down(
) -> Result<()> {
    let _guard = ENV_LOCK.lock().await;
    let root = tempfile::tempdir().context("temp root")?;
    let home = root.path().join("home");
    std::fs::create_dir_all(&home)?;
    let _env = EnvGuard::new(&home);

    let (status, workspaces) =
        response_json(session_log_workspaces().await.into_response()).await?;
    assert_eq!(status, StatusCode::BAD_GATEWAY);
    assert!(
        workspaces["error"]
            .as_str()
            .unwrap_or_default()
            .contains("session_db service is not running"),
        "missing service should be explicit: {workspaces}"
    );

    let (status, sessions) = response_json(
        session_log_sessions(
            HeaderMap::new(),
            Query(SessionLogListParams {
                workspace: Some("C:/missing/workspace".to_string()),
                page: 0,
                page_size: 50,
            }),
        )
        .await
        .into_response(),
    )
    .await?;
    assert_eq!(status, StatusCode::BAD_GATEWAY);
    assert!(sessions["error"]
        .as_str()
        .unwrap_or_default()
        .contains("session_db"));

    let (status, records) = response_json(
        session_log_records(
            Path("missing-session".to_string()),
            Query(SessionLogRecordsParams {
                page: 0,
                page_size: 50,
            }),
        )
        .await
        .into_response(),
    )
    .await?;
    assert_eq!(status, StatusCode::BAD_GATEWAY);
    assert!(records["error"]
        .as_str()
        .unwrap_or_default()
        .contains("session_db"));
    Ok(())
}

async fn response_json(response: Response) -> Result<(StatusCode, Value)> {
    let status = response.status();
    let bytes = body::to_bytes(response.into_body(), usize::MAX)
        .await
        .context("read response body")?;
    let value = serde_json::from_slice(&bytes).with_context(|| {
        format!(
            "decode response body as json: {}",
            String::from_utf8_lossy(&bytes)
        )
    })?;
    Ok((status, value))
}

fn upsert_request(
    session_id: &str,
    workspace: &str,
    message_count: usize,
) -> session_log::UpsertSessionRequest {
    session_log::UpsertSessionRequest {
        session: json!({
            "id": session_id,
            "name": "Gateway Session Log API",
            "directory": workspace,
            "created_at": 1,
            "updated_at": 100 + message_count as i64,
            "status": "idle",
            "management": {
                "session_id": session_id,
                "session_name": "Gateway Session Log API",
                "state": "created"
            }
        }),
        parent_id: None,
        messages: (1..=message_count)
            .map(|index| {
                json!({
                    "id": format!("message-{index}"),
                    "role": if index % 2 == 0 { "assistant" } else { "user" },
                    "created_at": index as i64,
                    "updated_at": index as i64,
                    "content": format!("message body {index}")
                })
            })
            .collect(),
        todos: vec![json!({
            "id": "todo-session-log-api",
            "content": "verify gateway session log api",
            "status": "done"
        })],
    }
}

fn assert_ok(response: session_log::SessionLogResponse) -> Result<()> {
    match response {
        session_log::SessionLogResponse::Ok => Ok(()),
        session_log::SessionLogResponse::Error { error } => bail!("{error}"),
        other => bail!("unexpected session_log response: {other:?}"),
    }
}

fn percent_encode_workspace(value: &str) -> String {
    value
        .as_bytes()
        .iter()
        .map(|byte| match *byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'/' => {
                char::from(*byte).to_string()
            }
            other => format!("%{other:02X}"),
        })
        .collect::<String>()
}

struct ServiceThread {
    handle: Option<std::thread::JoinHandle<Result<()>>>,
}

impl ServiceThread {
    fn start() -> Result<Self> {
        let store = SessionLogStore::open_default().context("open session_log store")?;
        let handle = std::thread::spawn(move || session_log::ipc::serve_blocking(store));
        wait_until(
            Duration::from_secs(10),
            session_log::ipc::service_is_running,
        )?;
        Ok(Self {
            handle: Some(handle),
        })
    }
}

impl Drop for ServiceThread {
    fn drop(&mut self) {
        let _ = session_log::ipc::call_service(&SessionLogCommand::Shutdown);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

struct EnvGuard {
    previous: Vec<(&'static str, Option<std::ffi::OsString>)>,
}

impl EnvGuard {
    fn new(home: &FsPath) -> Self {
        let keys = ["TURA_HOME", "TURA_DB_ROOT", "SESSION_LOG_DB_ROOT"];
        let previous = keys
            .iter()
            .map(|key| (*key, std::env::var_os(key)))
            .collect::<Vec<_>>();
        std::env::set_var("TURA_HOME", home);
        std::env::remove_var("TURA_DB_ROOT");
        std::env::remove_var("SESSION_LOG_DB_ROOT");
        Self { previous }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        for (key, value) in self.previous.drain(..).rev() {
            match value {
                Some(value) => std::env::set_var(key, value),
                None => std::env::remove_var(key),
            }
        }
    }
}

fn wait_until(timeout: Duration, mut condition: impl FnMut() -> bool) -> Result<()> {
    let started = Instant::now();
    while started.elapsed() < timeout {
        if condition() {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    bail!("condition was not met within {}ms", timeout.as_millis())
}
