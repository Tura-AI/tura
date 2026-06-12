use crate::api::types::PathResponse;
use axum::{extract::Query, http::HeaderMap, Json};

pub async fn get_paths(headers: HeaderMap, Query(params): Query<PathParams>) -> Json<PathResponse> {
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .unwrap_or_else(|_| "C:\\Users\\default".to_string());
    let appdata = std::env::var("APPDATA").unwrap_or_else(|_| format!("{home}\\AppData\\Roaming"));
    let state = std::env::var("LOCALAPPDATA").unwrap_or_else(|_| format!("{home}\\AppData\\Local"));
    let cwd = std::env::current_dir()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let worktree = params
        .directory
        .or_else(|| encoded_header(&headers, "x-opencode-directory"))
        .unwrap_or(cwd);
    Json(PathResponse {
        home,
        state,
        config: appdata,
        worktree: worktree.clone(),
        directory: worktree,
    })
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct PathParams {
    pub directory: Option<String>,
}

fn encoded_header(headers: &HeaderMap, name: &str) -> Option<String> {
    let value = headers.get(name)?.to_str().ok()?;
    Some(percent_decode(value))
}

fn percent_decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut output = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            if let (Some(high), Some(low)) = (hex(bytes[index + 1]), hex(bytes[index + 2])) {
                output.push((high << 4) | low);
                index += 3;
                continue;
            }
        }
        output.push(bytes[index]);
        index += 1;
    }
    String::from_utf8_lossy(&output).into_owned()
}

fn hex(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        b'A'..=b'F' => Some(value - b'A' + 10),
        _ => None,
    }
}
