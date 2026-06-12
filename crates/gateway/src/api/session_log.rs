//! Session log API handlers.

use crate::mock::global_store;
use crate::session_db_client::SessionDbClient;
use axum::{
    extract::{Path, Query},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use serde::Deserialize;

#[derive(Debug, Clone, Default, Deserialize)]
pub struct SessionLogListParams {
    pub workspace: Option<String>,
    #[serde(default)]
    pub page: u64,
    #[serde(default = "default_session_log_page_size")]
    pub page_size: u64,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct SessionLogRecordsParams {
    #[serde(default)]
    pub page: u64,
    #[serde(default = "default_session_log_page_size")]
    pub page_size: u64,
}

fn default_session_log_page_size() -> u64 {
    50
}

pub async fn session_log_workspaces() -> impl IntoResponse {
    match tokio::task::spawn_blocking(|| {
        SessionDbClient::discover().and_then(|client| client.list_workspaces())
    })
    .await
    {
        Ok(Ok(workspaces)) => Json(serde_json::json!({ "workspaces": workspaces })).into_response(),
        Ok(Err(err)) => (
            StatusCode::BAD_GATEWAY,
            Json(serde_json::json!({ "error": err.to_string() })),
        )
            .into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": err.to_string() })),
        )
            .into_response(),
    }
}

pub async fn session_log_sessions(
    headers: HeaderMap,
    Query(params): Query<SessionLogListParams>,
) -> impl IntoResponse {
    let workspace = params
        .workspace
        .or_else(|| encoded_header(&headers, "x-opencode-directory"))
        .or_else(|| global_store().get_current_directory())
        .unwrap_or_default();
    let page = params.page;
    let page_size = params.page_size;
    match tokio::task::spawn_blocking(move || {
        SessionDbClient::discover()
            .and_then(|client| client.list_sessions(workspace, page, page_size))
    })
    .await
    {
        Ok(Ok((page, sessions))) => {
            Json(serde_json::json!({ "page": page, "sessions": sessions })).into_response()
        }
        Ok(Err(err)) => (
            StatusCode::BAD_GATEWAY,
            Json(serde_json::json!({ "error": err.to_string() })),
        )
            .into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": err.to_string() })),
        )
            .into_response(),
    }
}

pub async fn session_log_records(
    Path(session_id): Path<String>,
    Query(params): Query<SessionLogRecordsParams>,
) -> impl IntoResponse {
    let page = params.page;
    let page_size = params.page_size;
    match tokio::task::spawn_blocking(move || {
        SessionDbClient::discover()
            .and_then(|client| client.list_session_records(session_id, page, page_size))
    })
    .await
    {
        Ok(Ok((page, records))) => {
            Json(serde_json::json!({ "page": page, "records": records })).into_response()
        }
        Ok(Err(err)) => (
            StatusCode::BAD_GATEWAY,
            Json(serde_json::json!({ "error": err.to_string() })),
        )
            .into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": err.to_string() })),
        )
            .into_response(),
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    #[test]
    fn session_log_list_params_default_to_first_page_and_safe_page_size() {
        let empty: SessionLogListParams =
            serde_json::from_value(serde_json::json!({})).expect("deserialize defaults");
        assert_eq!(empty.workspace, None);
        assert_eq!(empty.page, 0);
        assert_eq!(empty.page_size, 50);

        let explicit: SessionLogListParams = serde_json::from_value(serde_json::json!({
            "workspace": "C:/work/tura",
            "page": 3,
            "page_size": 25
        }))
        .expect("deserialize explicit");
        assert_eq!(explicit.workspace.as_deref(), Some("C:/work/tura"));
        assert_eq!(explicit.page, 3);
        assert_eq!(explicit.page_size, 25);
    }

    #[test]
    fn session_log_records_params_use_same_paging_contract() {
        let defaulted: SessionLogRecordsParams =
            serde_json::from_value(serde_json::json!({ "page": 2 }))
                .expect("deserialize records params");

        assert_eq!(defaulted.page, 2);
        assert_eq!(defaulted.page_size, 50);
        assert_eq!(default_session_log_page_size(), 50);
    }

    #[test]
    fn encoded_header_percent_decodes_workspace_paths() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-opencode-directory",
            HeaderValue::from_static("C%3A%5CUsers%5Cliuliu%5CDocuments%5Ctura"),
        );

        assert_eq!(
            encoded_header(&headers, "x-opencode-directory").as_deref(),
            Some(r"C:\Users\liuliu\Documents\tura")
        );
        assert_eq!(encoded_header(&headers, "missing"), None);
    }

    #[test]
    fn percent_decode_keeps_invalid_escapes_literal_and_decodes_utf8_lossily() {
        assert_eq!(percent_decode("plain%20space"), "plain space");
        assert_eq!(percent_decode("bad%2Gescape%"), "bad%2Gescape%");
        assert_eq!(percent_decode("%E4%BD%A0%E5%A5%BD"), "你好");
        assert_eq!(percent_decode("%FF"), "\u{FFFD}");
    }

    #[test]
    fn hex_accepts_both_cases_and_rejects_non_hex_bytes() {
        assert_eq!(hex(b'0'), Some(0));
        assert_eq!(hex(b'9'), Some(9));
        assert_eq!(hex(b'a'), Some(10));
        assert_eq!(hex(b'f'), Some(15));
        assert_eq!(hex(b'A'), Some(10));
        assert_eq!(hex(b'F'), Some(15));
        assert_eq!(hex(b'g'), None);
        assert_eq!(hex(b'/'), None);
    }
}
