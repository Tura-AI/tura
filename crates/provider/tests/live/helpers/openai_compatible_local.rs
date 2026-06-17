pub(crate) use serde_json::{json, Value};
pub(crate) use std::collections::HashMap;
pub(crate) use std::io::{Read, Write};
pub(crate) use std::net::TcpListener;
pub(crate) use std::sync::{Arc, Mutex};
pub(crate) use std::thread;
pub(crate) use std::time::{Duration, Instant};
pub(crate) use tokio::sync::Mutex as AsyncMutex;
pub(crate) use tura_llm_rust::{
    extract_response_text, extract_tool_calls, normalize_command_run_tool_input,
    openai_compatible_usage_stream_supported, prompt_cache_key_supported,
    provider_latency_timeouts, provider_media_fallback, provider_unsupported_content_type,
    replace_unsupported_content_type_in_messages, strip_thought_blocks, CallOptions,
    ProviderConfig, ProviderLatencyTimeouts, ProviderMediaFallback, ProviderStreamEvent,
    RouteConfig, Settings, TuraConfig, TuraError,
};

pub(crate) static ENV_LOCK: AsyncMutex<()> = AsyncMutex::const_new(());

#[derive(Debug)]
pub(crate) struct CapturedHttpRequest {
    pub(crate) method: String,
    pub(crate) path: String,
    pub(crate) headers: String,
    pub(crate) body: Value,
}

pub(crate) fn read_http_request(stream: &mut std::net::TcpStream) -> CapturedHttpRequest {
    let mut buffer = Vec::new();
    let mut chunk = [0_u8; 1024];
    let (header_end, content_length) = loop {
        let read = stream.read(&mut chunk).expect("read request");
        assert!(read > 0, "provider client closed before headers");
        buffer.extend_from_slice(&chunk[..read]);
        if let Some(header_end) = find_header_end(&buffer) {
            let headers = String::from_utf8_lossy(&buffer[..header_end]).to_string();
            let content_length = header_value(&headers, "content-length")
                .and_then(|value| value.parse::<usize>().ok())
                .expect("content-length header");
            break (header_end, content_length);
        }
    };
    let body_start = header_end + 4;
    while buffer.len() < body_start + content_length {
        let read = stream.read(&mut chunk).expect("read request body");
        assert!(read > 0, "provider client closed before body");
        buffer.extend_from_slice(&chunk[..read]);
    }

    let headers = String::from_utf8_lossy(&buffer[..header_end]).to_string();
    let request_line = headers.lines().next().expect("request line");
    let mut parts = request_line.split_whitespace();
    let method = parts.next().expect("method").to_string();
    let path = parts.next().expect("path").to_string();
    let headers_lower = headers.to_ascii_lowercase();
    let body_text = String::from_utf8(buffer[body_start..body_start + content_length].to_vec())
        .expect("utf8 request body");
    let body = serde_json::from_str(&body_text).expect("json request body");

    CapturedHttpRequest {
        method,
        path,
        headers: headers_lower,
        body,
    }
}

pub(crate) fn accept_optional_provider_request(
    listener: TcpListener,
    timeout_ms: u64,
) -> Option<CapturedHttpRequest> {
    listener
        .set_nonblocking(true)
        .expect("set fallback listener nonblocking");
    let started = Instant::now();
    while started.elapsed() < Duration::from_millis(timeout_ms) {
        match listener.accept() {
            Ok((mut stream, _)) => {
                let request = read_http_request(&mut stream);
                let body = json!({
                    "choices": [{
                        "message": {
                            "role": "assistant",
                            "content": "unexpected fallback"
                        }
                    }]
                })
                .to_string();
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                    body.len(),
                    body
                );
                stream
                    .write_all(response.as_bytes())
                    .expect("write unexpected fallback response");
                return Some(request);
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(10));
            }
            Err(error) => panic!("fallback provider listener failed: {error}"),
        }
    }
    None
}

pub(crate) fn read_llm_logs(root: &std::path::Path) -> Vec<Value> {
    let mut logs = Vec::new();
    for day in std::fs::read_dir(root).expect("read log root") {
        let day = day.expect("read day entry");
        if !day.path().is_dir() {
            continue;
        }
        for entry in std::fs::read_dir(day.path()).expect("read log day") {
            let entry = entry.expect("read log entry");
            if entry.path().extension().and_then(|value| value.to_str()) != Some("json") {
                continue;
            }
            let content = std::fs::read_to_string(entry.path()).expect("read llm log");
            logs.push(serde_json::from_str(&content).expect("parse llm log"));
        }
    }
    logs
}

pub(crate) fn find_header_end(buffer: &[u8]) -> Option<usize> {
    buffer.windows(4).position(|window| window == b"\r\n\r\n")
}

pub(crate) fn header_value(headers: &str, name: &str) -> Option<String> {
    headers.lines().find_map(|line| {
        let (key, value) = line.split_once(':')?;
        key.eq_ignore_ascii_case(name)
            .then(|| value.trim().to_string())
    })
}
