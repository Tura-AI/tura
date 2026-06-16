use crate::prompt_style::{runtime_fallback, tool_progress};
use crate::state_machine::session_management::SessionManagement;
use std::io::Write;
use tracing::warn;

use crate::manas::constants::gateway_callbacks_disabled;
use crate::manas::final_response::summarize_tool_results_for_user;
use crate::manas::tool_catalog::env_flag;
use crate::state_machine::runtime_management::{RuntimeManagement, RuntimeSessionSyncStatus};

const DEFAULT_GATEWAY_CALLBACK_TIMEOUT_MS: u64 = 2_000;

pub(crate) fn publish_runtime_failure_message(
    session: &SessionManagement,
    runtime_id: &str,
    error: &str,
) {
    let reply_message = summarize_tool_results_for_user(session).map_or_else(
        || runtime_fallback::no_tool_results_runtime_failed(error),
        |summary| runtime_fallback::tool_results_then_runtime_failed(&summary, error),
    );
    emit_cli_agent_message(&reply_message);

    if let Err(publish_error) = publish_gateway_agent_message(
        &session.session_id,
        runtime_id,
        reply_message,
        tool_progress::runtime_failed_after_tool_execution(error),
    ) {
        warn!(
            session_id = %session.session_id,
            runtime_id = %runtime_id,
            error = %publish_error,
            "failed to publish visible runtime failure"
        );
    }
}

fn emit_cli_agent_message(reply_message: &str) {
    if !env_flag("TURA_CLI_LIVE_JSONL") {
        return;
    }
    let event = serde_json::json!({
        "type": "item.completed",
        "item": {
            "id": "item_runtime_failure",
            "type": "agent_message",
            "text": reply_message,
        }
    });
    println!("{event}");
    let _ = std::io::stdout().flush();
}

/// Canonical frontend message id for a runtime-owned assistant turn.
///
/// Runtime callbacks, live overlays, and persisted session snapshots all derive
/// the same id from `runtime_id` so one provider call has one assistant message.
pub(crate) fn runtime_message_id(runtime_id: &str) -> String {
    format!("{runtime_id}.message")
}

/// Canonical text part id paired with [`runtime_message_id`].
pub(crate) fn runtime_text_part_id(runtime_id: &str) -> String {
    format!("{runtime_id}.message")
}

/// Canonical tool part id for a runtime-owned assistant turn.
pub(crate) fn runtime_tool_part_id(runtime_id: &str, tool_name: &str) -> String {
    format!("{runtime_id}.tool.{tool_name}")
}

/// Publish one incremental assistant text delta to the gateway, which re-emits it
/// as a `message.part.delta` so the frontend renders tokens as they arrive.
pub(crate) async fn publish_streamed_agent_text(session_id: &str, runtime_id: &str, delta: &str) {
    if gateway_callbacks_disabled() || delta.is_empty() {
        return;
    }
    let target_session_id = gateway_callback_session_id(session_id);
    let endpoint = format!(
        "{}/session/{target_session_id}/message/agent/stream",
        gateway_callback_base_url()
    );
    let payload = serde_json::json!({
        "delta": delta,
        "runtime_id": runtime_id,
    });
    if let Err(error) = gateway_callback_http_client()
        .post(endpoint)
        .json(&payload)
        .send()
        .await
    {
        warn!(
            session_id = %session_id,
            runtime_id = %runtime_id,
            error = %error,
            "failed to publish streamed agent text delta"
        );
    }
}

pub(crate) fn publish_gateway_agent_message(
    session_id: &str,
    runtime_id: &str,
    reply_message: String,
    new_learning: String,
) -> Result<(), String> {
    publish_gateway_agent_message_with_sync(
        session_id,
        runtime_id,
        reply_message,
        new_learning,
        None,
        None,
        None,
    )
}

pub(crate) fn publish_gateway_agent_message_from_runtime(
    session_id: &str,
    runtime: &RuntimeManagement,
    reply_message: String,
    new_learning: String,
) -> Result<(), String> {
    let (created_at, updated_at) = runtime.assistant_message_timestamps();
    publish_gateway_agent_message_with_sync(
        session_id,
        &runtime.runtime_id,
        reply_message,
        new_learning,
        Some(runtime.session_sync_status()),
        Some(created_at),
        Some(updated_at),
    )
}

fn publish_gateway_agent_message_with_sync(
    session_id: &str,
    runtime_id: &str,
    reply_message: String,
    new_learning: String,
    runtime_status: Option<RuntimeSessionSyncStatus>,
    created_at: Option<i64>,
    updated_at: Option<i64>,
) -> Result<(), String> {
    if gateway_callbacks_disabled() {
        return Ok(());
    }

    let target_session_id = gateway_callback_session_id(session_id);
    let gateway_base = gateway_callback_base_url();
    let endpoint = format!("{gateway_base}/session/{target_session_id}/message/agent");
    let payload = serde_json::json!({
        "reply_message": reply_message,
        "new_learning": new_learning,
        "media": [],
        "runtime_id": runtime_id,
        "runtime_status": runtime_status,
        "created_at": created_at,
        "updated_at": updated_at,
    });

    tokio::runtime::Runtime::new()
        .map_err(|err| format!("failed to create gateway callback runtime: {err}"))?
        .block_on(async {
            let response = gateway_callback_http_client()
                .post(endpoint)
                .json(&payload)
                .send()
                .await
                .map_err(|err| format!("failed to call gateway: {err}"))?;
            if response.status().is_success() {
                Ok(())
            } else {
                let status = response.status();
                let body = response.text().await.unwrap_or_default();
                Err(format!("gateway returned {status}: {body}"))
            }
        })
}

pub(crate) fn gateway_callback_http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(gateway_callback_http_timeout())
        .build()
        .unwrap_or_else(|_| reqwest::Client::new())
}

pub(crate) fn post_gateway_callback_detached(
    endpoint: String,
    payload: serde_json::Value,
    session_id: String,
    runtime_id: String,
    context: &'static str,
) {
    std::thread::spawn(move || {
        let Ok(runtime) = tokio::runtime::Runtime::new() else {
            warn!(
                session_id = %session_id,
                runtime_id = %runtime_id,
                context = context,
                "failed to create detached gateway callback runtime"
            );
            return;
        };
        runtime.block_on(async move {
            match gateway_callback_http_client()
                .post(endpoint)
                .json(&payload)
                .send()
                .await
            {
                Ok(response) if response.status().is_success() => {}
                Ok(response) => {
                    let status = response.status();
                    let body = response.text().await.unwrap_or_default();
                    warn!(
                        session_id = %session_id,
                        runtime_id = %runtime_id,
                        context = context,
                        gateway_status = %status,
                        body = %body,
                        "detached gateway callback returned non-success"
                    );
                }
                Err(error) => {
                    warn!(
                        session_id = %session_id,
                        runtime_id = %runtime_id,
                        context = context,
                        error = %error,
                        "detached gateway callback failed"
                    );
                }
            }
        });
    });
}

pub(crate) fn gateway_callback_http_timeout() -> std::time::Duration {
    let millis = std::env::var("TURA_GATEWAY_CALLBACK_TIMEOUT_MS")
        .ok()
        .and_then(|value| value.trim().parse::<u64>().ok())
        .unwrap_or(DEFAULT_GATEWAY_CALLBACK_TIMEOUT_MS);
    std::time::Duration::from_millis(millis.max(1))
}

pub(super) fn gateway_callback_base_url() -> String {
    std::env::var("TURA_GATEWAY_URL")
        .or_else(|_| std::env::var("GATEWAY_BASE_URL"))
        .unwrap_or_else(|_| {
            let port = std::env::var("TURA_GATEWAY_PORT")
                .or_else(|_| std::env::var("PORT"))
                .unwrap_or_else(|_| "4156".to_string());
            format!("http://127.0.0.1:{port}")
        })
        .trim_end_matches('/')
        .to_string()
}

pub(super) fn gateway_callback_session_id(session_id: &str) -> String {
    if planning_child_depth_from_env() > 0 {
        if let Ok(parent_session_id) = std::env::var("TURA_PARENT_SESSION_ID") {
            let parent_session_id = parent_session_id.trim();
            if !parent_session_id.is_empty() {
                return parent_session_id.to_string();
            }
        }
    }

    session_id.to_string()
}

fn planning_child_depth_from_env() -> usize {
    std::env::var("TURA_PLANNING_DEPTH")
        .or_else(|_| std::env::var("TURA_EXECUTE_TOOLS_DEPTH"))
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn stream_message_ids_are_stable_and_runtime_scoped() {
        assert_eq!(runtime_message_id("runtime-123"), "runtime-123.message");
        assert_eq!(runtime_text_part_id("runtime-123"), "runtime-123.message");
        assert_ne!(
            runtime_message_id("runtime-123"),
            runtime_message_id("runtime-456")
        );
    }

    #[test]
    fn gateway_callback_base_url_prefers_explicit_urls_then_port_defaults() {
        let _guard = ENV_LOCK.lock().expect("env lock");
        clear_gateway_env();

        std::env::set_var("TURA_GATEWAY_URL", "http://127.0.0.1:9000///");
        std::env::set_var("GATEWAY_BASE_URL", "http://127.0.0.1:8000");
        std::env::set_var("TURA_GATEWAY_PORT", "7000");
        assert_eq!(gateway_callback_base_url(), "http://127.0.0.1:9000");

        std::env::remove_var("TURA_GATEWAY_URL");
        assert_eq!(gateway_callback_base_url(), "http://127.0.0.1:8000");

        std::env::remove_var("GATEWAY_BASE_URL");
        assert_eq!(gateway_callback_base_url(), "http://127.0.0.1:7000");

        std::env::remove_var("TURA_GATEWAY_PORT");
        std::env::set_var("PORT", "6000");
        assert_eq!(gateway_callback_base_url(), "http://127.0.0.1:6000");

        clear_gateway_env();
        assert_eq!(gateway_callback_base_url(), "http://127.0.0.1:4156");
    }

    #[test]
    fn callback_session_id_uses_parent_only_for_planning_child_depth() {
        let _guard = ENV_LOCK.lock().expect("env lock");
        clear_gateway_env();

        std::env::set_var("TURA_PARENT_SESSION_ID", " parent-session ");
        assert_eq!(
            gateway_callback_session_id("child-session"),
            "child-session"
        );

        std::env::set_var("TURA_PLANNING_DEPTH", "1");
        assert_eq!(
            gateway_callback_session_id("child-session"),
            "parent-session"
        );

        std::env::set_var("TURA_PARENT_SESSION_ID", "   ");
        assert_eq!(
            gateway_callback_session_id("child-session"),
            "child-session"
        );

        std::env::remove_var("TURA_PLANNING_DEPTH");
        std::env::set_var("TURA_EXECUTE_TOOLS_DEPTH", "2");
        std::env::set_var("TURA_PARENT_SESSION_ID", "execute-parent");
        assert_eq!(
            gateway_callback_session_id("child-session"),
            "execute-parent"
        );

        std::env::set_var("TURA_EXECUTE_TOOLS_DEPTH", "not-a-number");
        assert_eq!(
            gateway_callback_session_id("child-session"),
            "child-session"
        );
    }

    #[test]
    fn publish_gateway_agent_message_returns_ok_when_callbacks_are_disabled() {
        let _guard = ENV_LOCK.lock().expect("env lock");
        clear_gateway_env();
        std::env::set_var("TURA_GATEWAY_CALLBACKS", "off");
        std::env::set_var("TURA_GATEWAY_URL", "http://127.0.0.1:9");

        let result = publish_gateway_agent_message(
            "session-1",
            "runtime-1",
            "reply".to_string(),
            "learning".to_string(),
        );

        assert_eq!(result, Ok(()));
    }

    #[test]
    fn publish_gateway_agent_message_posts_visible_reply_payload() {
        let _guard = ENV_LOCK.lock().expect("env lock");
        clear_gateway_env();
        let server = TestServer::spawn(200, r#"{"ok":true}"#);
        std::env::set_var("TURA_GATEWAY_URL", server.base_url());
        std::env::set_var("TURA_PLANNING_DEPTH", "1");
        std::env::set_var("TURA_PARENT_SESSION_ID", "parent-session");

        let result = publish_gateway_agent_message(
            "child-session",
            "runtime-42",
            "visible reply".to_string(),
            "new learning".to_string(),
        );

        assert_eq!(result, Ok(()));
        let request = server.join();
        assert!(
            request.starts_with("POST /session/parent-session/message/agent "),
            "{request}"
        );
        let body = request_body(&request);
        let payload: serde_json::Value = serde_json::from_str(body).expect("json body");
        assert_eq!(payload["reply_message"], "visible reply");
        assert_eq!(payload["new_learning"], "new learning");
        assert_eq!(payload["runtime_id"], "runtime-42");
        assert!(payload.get("message_id").is_none());
        assert!(payload.get("part_id").is_none());
        assert_eq!(payload["media"], serde_json::json!([]));
        assert_eq!(payload["runtime_status"], serde_json::Value::Null);
        assert_eq!(payload["created_at"], serde_json::Value::Null);
        assert_eq!(payload["updated_at"], serde_json::Value::Null);
    }

    #[test]
    fn publish_gateway_agent_message_reports_non_success_status_and_body() {
        let _guard = ENV_LOCK.lock().expect("env lock");
        clear_gateway_env();
        let server = TestServer::spawn(503, "gateway unavailable");
        std::env::set_var("TURA_GATEWAY_URL", server.base_url());

        let error = publish_gateway_agent_message(
            "session-1",
            "runtime-1",
            "reply".to_string(),
            "learning".to_string(),
        )
        .expect_err("gateway failure should be reported");

        assert!(error.contains("gateway returned 503 Service Unavailable"));
        assert!(error.contains("gateway unavailable"));
        let request = server.join();
        assert!(
            request.starts_with("POST /session/session-1/message/agent "),
            "{request}"
        );
    }

    #[test]
    fn publish_streamed_agent_text_posts_delta_payload_and_skips_empty_delta() {
        let _guard = ENV_LOCK.lock().expect("env lock");
        clear_gateway_env();
        let server = TestServer::spawn(200, r#"{"ok":true}"#);
        std::env::set_var("TURA_GATEWAY_URL", server.base_url());

        tokio_test::block_on(async {
            publish_streamed_agent_text("session-1", "runtime-7", "").await;
            publish_streamed_agent_text("session-1", "runtime-7", "hello").await;
        });

        let request = server.join();
        assert!(
            request.starts_with("POST /session/session-1/message/agent/stream "),
            "{request}"
        );
        let body = request_body(&request);
        let payload: serde_json::Value = serde_json::from_str(body).expect("json body");
        assert_eq!(payload["delta"], "hello");
        assert_eq!(payload["runtime_id"], "runtime-7");
        assert!(payload.get("message_id").is_none());
        assert!(payload.get("part_id").is_none());
    }

    struct TestServer {
        addr: std::net::SocketAddr,
        handle: std::thread::JoinHandle<String>,
    }

    impl TestServer {
        fn spawn(status: u16, body: &'static str) -> Self {
            let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind gateway");
            let addr = listener.local_addr().expect("local addr");
            let handle = std::thread::spawn(move || {
                let (mut stream, _) = listener.accept().expect("accept gateway request");
                let request = read_http_request(&mut stream);
                let reason = match status {
                    200 => "OK",
                    503 => "Service Unavailable",
                    _ => "Test",
                };
                let response = format!(
                    "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                stream
                    .write_all(response.as_bytes())
                    .expect("write response");
                request
            });
            Self { addr, handle }
        }

        fn base_url(&self) -> String {
            format!("http://{}", self.addr)
        }

        fn join(self) -> String {
            self.handle.join().expect("server thread")
        }
    }

    fn read_http_request(stream: &mut std::net::TcpStream) -> String {
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(2)))
            .expect("set read timeout");
        let mut data = Vec::new();
        let mut buffer = [0_u8; 1024];
        loop {
            let size = stream.read(&mut buffer).expect("read request");
            assert!(size > 0, "request stream closed before body completed");
            data.extend_from_slice(&buffer[..size]);
            if http_request_complete(&data) {
                return String::from_utf8_lossy(&data).into_owned();
            }
        }
    }

    fn http_request_complete(data: &[u8]) -> bool {
        let request = String::from_utf8_lossy(data);
        let Some((headers, body)) = request.split_once("\r\n\r\n") else {
            return false;
        };
        let content_length = headers
            .lines()
            .find_map(|line| line.split_once(':'))
            .filter(|(name, _)| name.eq_ignore_ascii_case("content-length"))
            .and_then(|(_, value)| value.trim().parse::<usize>().ok())
            .unwrap_or(0);
        body.len() >= content_length
    }

    fn request_body(request: &str) -> &str {
        request
            .split_once("\r\n\r\n")
            .map(|(_, body)| body)
            .unwrap_or_default()
    }

    fn clear_gateway_env() {
        for key in [
            "TURA_GATEWAY_URL",
            "GATEWAY_BASE_URL",
            "TURA_GATEWAY_PORT",
            "PORT",
            "TURA_PARENT_SESSION_ID",
            "TURA_PLANNING_DEPTH",
            "TURA_EXECUTE_TOOLS_DEPTH",
            "TURA_GATEWAY_CALLBACKS",
            "TURA_CLI_LIVE_JSONL",
        ] {
            std::env::remove_var(key);
        }
    }
}
