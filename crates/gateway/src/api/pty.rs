//! PTY-ish terminal API handlers used by the desktop UI terminal panel.

use crate::api::types::{PtyCreateRequest, PtyResponse, PtyUpdateRequest};
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Path, Query,
    },
    response::Response,
    Json,
};
use futures::{SinkExt, StreamExt};
use parking_lot::RwLock;
use serde::Deserialize;
use std::{collections::HashMap, process::Stdio};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWriteExt},
    process::Command,
    sync::mpsc,
};
use uuid::Uuid;

lazy_static::lazy_static! {
    static ref PTY_STORE: RwLock<HashMap<String, PtyInstance>> = RwLock::new(HashMap::new());
}

#[derive(Debug, Clone)]
pub struct PtyInstance {
    pub id: String,
    pub title: String,
    pub command: String,
    pub args: Vec<String>,
    pub env: HashMap<String, String>,
    pub cwd: String,
    pub rows: u16,
    pub cols: u16,
}

#[derive(Debug, Deserialize)]
pub struct PtyQuery {
    pub directory: Option<String>,
    pub workspace: Option<String>,
    pub cursor: Option<usize>,
}

pub async fn list_pty(Query(_query): Query<PtyQuery>) -> Json<Vec<PtyResponse>> {
    Json(PTY_STORE.read().values().map(pty_response).collect())
}

pub async fn create_pty(
    Query(query): Query<PtyQuery>,
    Json(payload): Json<PtyCreateRequest>,
) -> Json<PtyResponse> {
    let pty_id = Uuid::new_v4().to_string();
    let cwd = payload
        .cwd
        .or(query.directory)
        .or(query.workspace)
        .unwrap_or_else(current_dir);
    let command = payload
        .command
        .or(payload.shell)
        .unwrap_or_else(default_shell_command);
    let args = payload.args.unwrap_or_else(default_shell_args);
    let env = payload.env.unwrap_or_default();
    let rows = payload.rows.unwrap_or(24);
    let cols = payload.cols.unwrap_or(80);
    let title = payload.title.unwrap_or_else(|| "Terminal".to_string());

    let instance = PtyInstance {
        id: pty_id.clone(),
        title,
        command,
        args,
        env,
        cwd,
        rows,
        cols,
    };
    let response = pty_response(&instance);
    PTY_STORE.write().insert(pty_id, instance);
    Json(response)
}

pub async fn get_pty(Path(pty_id): Path<String>) -> Json<PtyResponse> {
    let response = PTY_STORE
        .read()
        .get(&pty_id)
        .map(pty_response)
        .unwrap_or_else(|| missing_pty_response(pty_id));
    Json(response)
}

pub async fn update_pty(
    Path(pty_id): Path<String>,
    Json(payload): Json<PtyUpdateRequest>,
) -> Json<PtyResponse> {
    let mut store = PTY_STORE.write();
    let entry = store
        .entry(pty_id.clone())
        .or_insert_with(|| missing_pty(pty_id));
    if let Some(title) = payload.title {
        entry.title = title;
    }
    if let Some(size) = payload.size {
        entry.rows = size.rows;
        entry.cols = size.cols;
    }
    Json(pty_response(entry))
}

pub async fn delete_pty(Path(pty_id): Path<String>) -> Json<bool> {
    PTY_STORE.write().remove(&pty_id);
    Json(true)
}

pub async fn pty_connect(
    Path(pty_id): Path<String>,
    Query(query): Query<PtyQuery>,
    ws: WebSocketUpgrade,
) -> Response {
    let instance = PTY_STORE
        .read()
        .get(&pty_id)
        .cloned()
        .unwrap_or_else(|| missing_pty(pty_id));
    ws.on_upgrade(move |socket| connect_shell(socket, instance, query.cursor.unwrap_or(0)))
}

async fn connect_shell(socket: WebSocket, instance: PtyInstance, _cursor: usize) {
    let mut command = Command::new(&instance.command);
    command
        .args(&instance.args)
        .envs(&instance.env)
        .current_dir(&instance.cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let Ok(mut child) = command.spawn() else {
        let (mut sender, _) = socket.split();
        let _ = sender
            .send(Message::Text(
                format!("failed to start shell: {}\r\n", instance.command).into(),
            ))
            .await;
        return;
    };

    let mut stdin = child.stdin.take();
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let (tx, mut rx) = mpsc::unbounded_channel::<String>();

    if let Some(mut stdout) = stdout {
        let tx = tx.clone();
        tokio::spawn(async move {
            read_process_stream(&mut stdout, tx).await;
        });
    }
    if let Some(mut stderr) = stderr {
        let tx = tx.clone();
        tokio::spawn(async move {
            read_process_stream(&mut stderr, tx).await;
        });
    }
    drop(tx);

    let (mut sender, mut receiver) = socket.split();
    let input_task = tokio::spawn(async move {
        while let Some(Ok(message)) = receiver.next().await {
            let bytes = match message {
                Message::Text(text) => text.as_bytes().to_vec(),
                Message::Binary(bytes) => bytes.to_vec(),
                Message::Close(_) => break,
                _ => continue,
            };
            if let Some(stdin) = stdin.as_mut() {
                if stdin.write_all(&bytes).await.is_err() {
                    break;
                }
                let _ = stdin.flush().await;
            }
        }
    });

    while let Some(chunk) = rx.recv().await {
        if sender.send(Message::Text(chunk.into())).await.is_err() {
            break;
        }
    }

    input_task.abort();
    let _ = child.kill().await;
    let _ = child.wait().await;
}

async fn read_process_stream<R>(reader: &mut R, tx: mpsc::UnboundedSender<String>)
where
    R: AsyncRead + Unpin,
{
    let mut buffer = [0_u8; 8192];
    loop {
        match reader.read(&mut buffer).await {
            Ok(0) => break,
            Ok(n) => {
                let text = String::from_utf8_lossy(&buffer[..n]).to_string();
                if tx.send(text).is_err() {
                    break;
                }
            }
            Err(_) => break,
        }
    }
}

fn pty_response(pty: &PtyInstance) -> PtyResponse {
    PtyResponse {
        id: pty.id.clone(),
        pty_id: pty.id.clone(),
        title: pty.title.clone(),
        command: pty.command.clone(),
        args: pty.args.clone(),
        cwd: pty.cwd.clone(),
        status: "running".to_string(),
        pid: 0,
    }
}

fn missing_pty_response(id: String) -> PtyResponse {
    pty_response(&missing_pty(id))
}

fn missing_pty(id: String) -> PtyInstance {
    PtyInstance {
        id,
        title: "Terminal".to_string(),
        command: default_shell_command(),
        args: default_shell_args(),
        env: HashMap::new(),
        cwd: current_dir(),
        rows: 24,
        cols: 80,
    }
}

fn current_dir() -> String {
    std::env::current_dir()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string()
}

fn default_shell_command() -> String {
    if cfg!(windows) {
        "powershell.exe".to_string()
    } else {
        "sh".to_string()
    }
}

fn default_shell_args() -> Vec<String> {
    if cfg!(windows) {
        vec!["-NoLogo".to_string()]
    } else {
        Vec::new()
    }
}
