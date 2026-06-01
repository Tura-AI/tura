//! OpenAI-compatible provider family.
//!
//! Historically this lived in one oversized `openapi.rs`. It is now split into
//! three focused submodules:
//!
//! * [`common`] — option-normalization + content-flattening helpers shared by
//!   both tiers.
//! * [`response`] — the **Responses API** tier (`/responses`, SSE): codex
//!   (OAuth) plus the API-key sub-providers `chatgpt`, `grok`, `qwen`.
//! * [`chat`] — the **Chat Completions** tier (`/chat/completions`): the
//!   default route for every other OpenAI-compatible provider.
//!
//! This module re-exports the small public surface the rest of the crate uses,
//! so external callers keep importing `crate::llm::openapi::{…}` unchanged.

mod chat;
mod common;
mod response;

pub(crate) use chat::force_search;
pub use chat::{call, call_with_stream_events, embed, embed_for_provider};
pub(crate) use response::{codex_oauth_call, responses_api_key_call};

#[cfg(test)]
pub(crate) use chat::process_chat_stream_line_for_test;
#[cfg(test)]
pub(crate) use chat::{
    build_chat_payload, emit_completed_tool_call, last_complete_minimax_invoke,
    normalize_messages_for_provider, StreamingToolCall,
};
#[cfg(test)]
pub(crate) use common::should_pass_service_tier;
#[cfg(test)]
pub(crate) use response::{
    append_codex_stream_text, build_codex_oauth_payload, build_responses_payload_for_provider,
    codex_event_tool_calls, complete_codex_tool_calls, normalize_codex_response_content,
    ready_streaming_tool_call, CodexCommandRunCommandCollector, CodexToolCallStreamCollector,
};

#[cfg(test)]
mod tests {
    use super::{
        build_chat_payload, build_codex_oauth_payload, build_responses_payload_for_provider,
        normalize_messages_for_provider, process_chat_stream_line_for_test,
        should_pass_service_tier,
    };
    use crate::tura_llm::CallOptions;
    use serde_json::json;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::{mpsc, OnceLock};
    use tokio::sync::Mutex;

    async fn codex_endpoint_env_lock() -> tokio::sync::MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(())).lock().await
    }

    #[test]
    fn openai_compatible_chat_messages_preserve_native_roles() {
        let messages = vec![
            json!({"role": "system", "content": "Use tools carefully."}),
            json!({"role": "assistant", "content": null}),
            json!({"role": "user", "content": "Inspect files."}),
        ];

        let normalized = normalize_messages_for_provider("minimax", &messages);

        assert_eq!(normalized.len(), 2);
        assert_eq!(normalized[0]["role"], "system");
        assert_eq!(normalized[0]["content"], "Use tools carefully.");
        assert_eq!(normalized[1]["role"], "user");
        assert_eq!(normalized[1]["content"], "Inspect files.");
    }

    #[test]
    fn openai_compatible_chat_messages_keep_assistant_tool_calls() {
        let messages = vec![
            json!({"role": "user", "content": "run pwd"}),
            json!({
                "role": "assistant",
                "content": null,
                "tool_calls": [{
                    "id": "call_1",
                    "type": "function",
                    "function": {"name": "command_run", "arguments": "{}"}
                }]
            }),
            json!({"role": "tool", "tool_call_id": "call_1", "content": "/tmp"}),
        ];

        let normalized = normalize_messages_for_provider("minimax", &messages);

        assert_eq!(normalized.len(), 3);
        assert_eq!(normalized[1]["role"], "assistant");
        assert_eq!(normalized[1]["content"], "");
        assert_eq!(normalized[1]["tool_calls"][0]["id"], "call_1");
        assert_eq!(normalized[2]["role"], "tool");
        assert_eq!(normalized[2]["tool_call_id"], "call_1");
        assert_eq!(normalized[2]["content"], "/tmp");
    }

    #[test]
    fn openai_compatible_chat_messages_preserve_tool_call_and_output_pairs() {
        let messages = vec![
            json!({
                "type": "function_call",
                "name": "command_run",
                "call_id": "call_abc",
                "arguments": "{\"commands\":[]}",
                "status": "completed"
            }),
            json!({
                "type": "function_call_output",
                "call_id": "call_abc",
                "output": "Exit code: 0\nOutput:\nTURA_PROBE_OK\n"
            }),
        ];

        let normalized = normalize_messages_for_provider("minimax", &messages);

        assert_eq!(normalized.len(), 2);
        assert_eq!(normalized[0]["role"], "assistant");
        assert_eq!(normalized[0]["tool_calls"][0]["id"], "call_abc");
        assert_eq!(
            normalized[0]["tool_calls"][0]["function"]["name"],
            "command_run"
        );
        assert_eq!(normalized[1]["role"], "tool");
        assert_eq!(normalized[1]["tool_call_id"], "call_abc");
        let content = normalized[1]["content"]
            .as_str()
            .expect("normalized tool content should be a string");
        assert!(content.contains("TURA_PROBE_OK"));
    }

    #[test]
    fn non_minimax_keeps_assistant_empty_content_for_openai_compatibility() {
        let messages = vec![json!({"role": "assistant", "content": null})];

        let normalized = normalize_messages_for_provider("openai", &messages);

        assert_eq!(normalized[0]["role"], "assistant");
        assert_eq!(normalized[0]["content"], "");
    }

    #[test]
    fn service_tier_is_limited_to_openai_gpt_family_models() {
        assert!(should_pass_service_tier("openai", "gpt-5.2"));
        assert!(should_pass_service_tier("openai", "o3"));
        assert!(should_pass_service_tier("openai", "gpt-5.3-codex"));
        assert!(!should_pass_service_tier("openrouter", "openai/gpt-5.2"));
        assert!(!should_pass_service_tier("minimax", "minimax-m2.5"));
    }

    #[test]
    fn chat_stream_counts_reasoning_deltas_as_output_activity() {
        // OpenRouter/DeepSeek-style reasoning field.
        let (event, content, reasoning) = process_chat_stream_line_for_test(
            r#"data: {"choices":[{"delta":{"reasoning":"thinking hard"}}]}"#,
        );
        assert!(event, "reasoning delta must count as output activity");
        assert!(content.is_empty(), "reasoning is not assistant content");
        assert_eq!(reasoning, "thinking hard");

        // Alternate `reasoning_content` field used by some providers.
        let (event2, _, reasoning2) = process_chat_stream_line_for_test(
            r#"data: {"choices":[{"delta":{"reasoning_content":"step 1"}}]}"#,
        );
        assert!(event2);
        assert_eq!(reasoning2, "step 1");

        // Empty reasoning must not be treated as activity.
        let (event3, _, reasoning3) =
            process_chat_stream_line_for_test(r#"data: {"choices":[{"delta":{"reasoning":""}}]}"#);
        assert!(!event3);
        assert!(reasoning3.is_empty());
    }

    #[test]
    fn responses_payload_only_forwards_service_tier_for_openai_family() {
        let messages = vec![json!({"role": "user", "content": "ping"})];
        let options = CallOptions {
            service_tier: Some("priority".to_string()),
            ..CallOptions::default()
        };
        // OpenAI family (codex/chatgpt) accepts service_tier.
        let codex = build_responses_payload_for_provider("codex", "gpt-5.2", &messages, &options);
        assert_eq!(codex["service_tier"], "priority");
        // xAI rejects it with 400 Argument not supported: service_tier.
        let grok = build_responses_payload_for_provider("xai", "grok-4.3", &messages, &options);
        assert!(grok.get("service_tier").is_none());
        // Qwen Responses branch must also omit it.
        let qwen = build_responses_payload_for_provider("qwen", "qwen3.7-max", &messages, &options);
        assert!(qwen.get("service_tier").is_none());
    }

    #[test]
    fn responses_payload_preserves_canonical_media_content() {
        let messages = vec![json!({
            "role": "user",
            "content": [
                { "type": "input_text", "text": "see image" },
                { "type": "input_image", "image_url": "data:image/jpeg;base64,AAA" }
            ]
        })];

        let payload = build_responses_payload_for_provider(
            "chatgpt",
            "gpt-5.2",
            &messages,
            &CallOptions::default(),
        );

        assert_eq!(payload["input"][0]["content"][0]["type"], "input_text");
        assert_eq!(payload["input"][0]["content"][1]["type"], "input_image");
        assert_eq!(
            payload["input"][0]["content"][1]["image_url"],
            "data:image/jpeg;base64,AAA"
        );
    }

    #[test]
    fn provider_payload_passes_reasoning_and_acceleration_for_openai_gpt() {
        let messages = vec![json!({"role": "user", "content": "ping"})];
        let options = CallOptions {
            reasoning_effort: Some("high".to_string()),
            service_tier: Some("priority".to_string()),
            ..CallOptions::default()
        };

        let payload = build_chat_payload("openai", "gpt-5.2", &messages, &options);

        assert_eq!(payload["reasoning_effort"], "high");
        assert_eq!(payload["service_tier"], "priority");
    }

    #[test]
    fn provider_payload_maps_highest_reasoning_to_xhigh() {
        let messages = vec![json!({"role": "user", "content": "ping"})];
        let options = CallOptions {
            reasoning_effort: Some("highest".to_string()),
            ..CallOptions::default()
        };

        let payload = build_chat_payload("openai", "gpt-5.2", &messages, &options);

        assert_eq!(payload["reasoning_effort"], "xhigh");
    }

    #[test]
    fn provider_payload_omits_default_reasoning_and_acceleration() {
        let messages = vec![json!({"role": "user", "content": "ping"})];
        let options = CallOptions {
            reasoning_effort: Some(" default ".to_string()),
            service_tier: Some("default".to_string()),
            ..CallOptions::default()
        };

        let payload = build_chat_payload("openai", "gpt-5.2", &messages, &options);

        assert!(payload.get("reasoning_effort").is_none());
        assert!(payload.get("service_tier").is_none());
    }

    #[test]
    fn provider_payload_does_not_pass_acceleration_to_non_openai_models() {
        let messages = vec![json!({"role": "user", "content": "ping"})];
        let options = CallOptions {
            reasoning_effort: Some("medium".to_string()),
            service_tier: Some("priority".to_string()),
            ..CallOptions::default()
        };

        let payload = build_chat_payload("minimax", "minimax-m2.5", &messages, &options);

        assert_eq!(payload["reasoning_effort"], "medium");
        assert!(payload.get("service_tier").is_none());
    }

    #[tokio::test]
    async fn direct_provider_call_sends_reasoning_and_acceleration() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind test server");
        let addr = listener.local_addr().expect("local addr");
        let (tx, rx) = mpsc::channel();

        std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept request");
            let mut buffer = Vec::new();
            let mut chunk = [0_u8; 1024];
            loop {
                let read = stream.read(&mut chunk).expect("read request");
                if read == 0 {
                    break;
                }
                buffer.extend_from_slice(&chunk[..read]);
                if let Some(header_end) = find_header_end(&buffer) {
                    let headers = String::from_utf8_lossy(&buffer[..header_end]);
                    let content_length = headers
                        .lines()
                        .find_map(|line| {
                            let (name, value) = line.split_once(':')?;
                            name.eq_ignore_ascii_case("content-length")
                                .then(|| value.trim().parse::<usize>().ok())
                                .flatten()
                        })
                        .unwrap_or(0);
                    let body_start = header_end + 4;
                    if buffer.len() >= body_start + content_length {
                        let body = String::from_utf8(
                            buffer[body_start..body_start + content_length].to_vec(),
                        )
                        .expect("utf8 body");
                        tx.send(body).expect("send request body");
                        break;
                    }
                }
            }

            let response = concat!(
                "HTTP/1.1 200 OK\r\n",
                "Content-Type: application/json\r\n",
                "Content-Length: 69\r\n",
                "\r\n",
                r#"{"choices":[{"message":{"content":"ok"}}],"usage":{"total_tokens":1}}"#
            );
            stream
                .write_all(response.as_bytes())
                .expect("write response");
        });

        let messages = vec![json!({"role": "user", "content": "ping"})];
        let options = CallOptions {
            reasoning_effort: Some("high".to_string()),
            service_tier: Some("priority".to_string()),
            ..CallOptions::default()
        };

        super::call(
            &format!("http://{addr}"),
            "gpt-5.2",
            "openai",
            "test-key",
            &messages,
            &options,
        )
        .await
        .expect("provider call");

        let body: serde_json::Value =
            serde_json::from_str(&rx.recv().expect("request body")).expect("json body");
        assert_eq!(body["reasoning_effort"], "high");
        assert_eq!(body["service_tier"], "priority");
    }

    #[tokio::test]
    async fn streaming_provider_drains_usage_after_tool_arguments_complete() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind test server");
        let addr = listener.local_addr().expect("local addr");

        std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept request");
            let mut buffer = Vec::new();
            let mut chunk = [0_u8; 1024];
            loop {
                let read = stream.read(&mut chunk).expect("read request");
                if read == 0 {
                    break;
                }
                buffer.extend_from_slice(&chunk[..read]);
                if let Some(header_end) = find_header_end(&buffer) {
                    let headers = String::from_utf8_lossy(&buffer[..header_end]);
                    let content_length = headers
                        .lines()
                        .find_map(|line| {
                            let (name, value) = line.split_once(':')?;
                            name.eq_ignore_ascii_case("content-length")
                                .then(|| value.trim().parse::<usize>().ok())
                                .flatten()
                        })
                        .unwrap_or(0);
                    let body_start = header_end + 4;
                    if buffer.len() >= body_start + content_length {
                        break;
                    }
                }
            }

            let first = json!({
                "choices": [{
                    "delta": {
                        "tool_calls": [{
                            "index": 0,
                            "id": "call_1",
                            "type": "function",
                            "function": {
                                "name": "grep",
                                "arguments": "{\"pattern\""
                            }
                        }]
                    }
                }]
            });
            let second = json!({
                "choices": [{
                    "delta": {
                        "tool_calls": [{
                            "index": 0,
                            "function": {
                                "arguments": ":\"foo\"}"
                            }
                        }]
                    }
                }]
            });
            let late_text = json!({
                "choices": [{
                    "delta": {
                        "content": "late text after tool call"
                    }
                }]
            });
            let usage = json!({
                "choices": [],
                "usage": {
                    "prompt_tokens": 3000,
                    "completion_tokens": 8,
                    "total_tokens": 3008,
                    "prompt_tokens_details": {"cached_tokens": 2048}
                }
            });
            let body =
                format!("data: {first}\n\ndata: {second}\n\ndata: {late_text}\n\ndata: {usage}\n\ndata: [DONE]\n\n");
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\n\r\n{}",
                body.len(),
                body
            );
            stream
                .write_all(response.as_bytes())
                .expect("write response");
        });

        let messages = vec![json!({"role": "user", "content": "search"})];
        let options = CallOptions {
            stream: Some(true),
            ..CallOptions::default()
        };

        let result = super::call(
            &format!("http://{addr}"),
            "gpt-test",
            "openai",
            "test-key",
            &messages,
            &options,
        )
        .await
        .expect("provider call");

        assert_eq!(result.content["tool_calls"][0]["function"]["name"], "grep");
        assert_eq!(
            result.content["tool_calls"][0]["function"]["arguments"]["pattern"],
            "foo"
        );
        assert!(!result
            .content
            .to_string()
            .contains("late text after tool call"));
        let metrics = result.metrics.expect("metrics");
        assert_eq!(metrics.usage.input_tokens, Some(3000));
        assert_eq!(metrics.usage.cached_input_tokens, Some(2048));
        assert!(metrics.cache_hit);
    }

    #[tokio::test]
    async fn streaming_provider_reads_usage_and_cached_tokens() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind test server");
        let addr = listener.local_addr().expect("local addr");
        let (tx, rx) = mpsc::channel();

        std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept request");
            let mut buffer = Vec::new();
            let mut chunk = [0_u8; 1024];
            loop {
                let read = stream.read(&mut chunk).expect("read request");
                if read == 0 {
                    break;
                }
                buffer.extend_from_slice(&chunk[..read]);
                if let Some(header_end) = find_header_end(&buffer) {
                    let headers = String::from_utf8_lossy(&buffer[..header_end]);
                    let content_length = headers
                        .lines()
                        .find_map(|line| {
                            let (name, value) = line.split_once(':')?;
                            name.eq_ignore_ascii_case("content-length")
                                .then(|| value.trim().parse::<usize>().ok())
                                .flatten()
                        })
                        .unwrap_or(0);
                    let body_start = header_end + 4;
                    if buffer.len() >= body_start + content_length {
                        let body = String::from_utf8(
                            buffer[body_start..body_start + content_length].to_vec(),
                        )
                        .expect("utf8 body");
                        tx.send(body).expect("send request body");
                        break;
                    }
                }
            }

            let content = json!({
                "choices": [{
                    "delta": {"content": "ok"}
                }]
            });
            let usage = json!({
                "choices": [],
                "usage": {
                    "prompt_tokens": 3000,
                    "completion_tokens": 3,
                    "total_tokens": 3003,
                    "prompt_tokens_details": {"cached_tokens": 2048}
                }
            });
            let body = format!("data: {content}\n\ndata: {usage}\n\ndata: [DONE]\n\n");
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\n\r\n{}",
                body.len(),
                body
            );
            stream
                .write_all(response.as_bytes())
                .expect("write response");
        });

        let messages = vec![json!({"role": "user", "content": "cache"})];
        let options = CallOptions {
            stream: Some(true),
            stream_options: Some(json!({ "include_usage": true })),
            ..CallOptions::default()
        };

        let result = super::call(
            &format!("http://{addr}"),
            "gpt-test",
            "openai",
            "test-key",
            &messages,
            &options,
        )
        .await
        .expect("provider call");

        let request_body: serde_json::Value =
            serde_json::from_str(&rx.recv().expect("request body")).expect("json body");
        assert_eq!(request_body["stream_options"]["include_usage"], true);
        let metrics = result.metrics.expect("metrics");
        assert_eq!(metrics.usage.input_tokens, Some(3000));
        assert_eq!(metrics.usage.output_tokens, Some(3));
        assert_eq!(metrics.usage.cached_input_tokens, Some(2048));
        assert!(metrics.cache_hit);
    }

    #[test]
    fn qwen_stream_options_request_usage_for_cache_accounting() {
        let payload = build_chat_payload(
            "qwen",
            "qwen3-max-2026-01-23",
            &[json!({"role": "user", "content": "cache"})],
            &CallOptions {
                stream: Some(true),
                stream_options: Some(json!({ "include_usage": true })),
                ..CallOptions::default()
            },
        );

        assert_eq!(payload["stream"], true);
        assert_eq!(payload["stream_options"]["include_usage"], true);
    }

    #[test]
    fn metrics_read_minimax_anthropic_cache_usage_fields() {
        let metrics = crate::metrics::extract_openapi_metrics(
            &json!({
                "usage": {
                    "input_tokens": 108,
                    "output_tokens": 91,
                    "cache_creation_input_tokens": 512,
                    "cache_read_input_tokens": 14813
                }
            }),
            None,
        );

        assert_eq!(metrics.usage.input_tokens, Some(108));
        assert_eq!(metrics.usage.output_tokens, Some(91));
        assert_eq!(metrics.usage.cached_input_tokens, Some(14813));
        assert_eq!(metrics.usage.cache_write_tokens, Some(512));
        assert_eq!(metrics.usage.total_tokens, Some(15524));
        assert!(metrics.cache_hit);
    }

    #[tokio::test]
    async fn codex_oauth_call_sends_responses_reasoning_and_acceleration() {
        let _env_guard = codex_endpoint_env_lock().await;
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind test server");
        let addr = listener.local_addr().expect("local addr");
        let endpoint = format!("http://{addr}/backend-api/codex/responses");
        let previous_endpoint = std::env::var_os("OPENAI_CODEX_ENDPOINT");
        std::env::set_var("OPENAI_CODEX_ENDPOINT", &endpoint);
        let (tx, rx) = mpsc::channel();

        std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept request");
            let mut buffer = Vec::new();
            let mut chunk = [0_u8; 1024];
            loop {
                let read = stream.read(&mut chunk).expect("read request");
                if read == 0 {
                    break;
                }
                buffer.extend_from_slice(&chunk[..read]);
                if let Some(header_end) = find_header_end(&buffer) {
                    let headers = String::from_utf8_lossy(&buffer[..header_end]);
                    let content_length = headers
                        .lines()
                        .find_map(|line| {
                            let (name, value) = line.split_once(':')?;
                            name.eq_ignore_ascii_case("content-length")
                                .then(|| value.trim().parse::<usize>().ok())
                                .flatten()
                        })
                        .unwrap_or(0);
                    let body_start = header_end + 4;
                    if buffer.len() >= body_start + content_length {
                        let body = String::from_utf8(
                            buffer[body_start..body_start + content_length].to_vec(),
                        )
                        .expect("utf8 body");
                        tx.send(body).expect("send request body");
                        break;
                    }
                }
            }

            let body = concat!(
                "data: {\"type\":\"response.output_text.delta\",\"delta\":\"ok\"}\n\n",
                "data: {\"type\":\"response.completed\",\"response\":{\"output_text\":\"ok\",\"usage\":{\"input_tokens\":1,\"output_tokens\":1,\"total_tokens\":2}}}\n\n",
                "data: [DONE]\n\n"
            );
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\n\r\n{}",
                body.len(),
                body
            );
            stream
                .write_all(response.as_bytes())
                .expect("write response");
        });

        let messages = vec![json!({"role": "user", "content": "ping"})];
        let options = CallOptions {
            reasoning_effort: Some("high".to_string()),
            service_tier: Some("priority".to_string()),
            ..CallOptions::default()
        };

        let result =
            super::codex_oauth_call("gpt-5.1-codex", "test-token", &messages, &options, None).await;

        match previous_endpoint {
            Some(value) => std::env::set_var("OPENAI_CODEX_ENDPOINT", value),
            None => std::env::remove_var("OPENAI_CODEX_ENDPOINT"),
        }

        result.expect("codex oauth call");
        let body: serde_json::Value =
            serde_json::from_str(&rx.recv().expect("request body")).expect("json body");
        assert!(body.get("reasoning_effort").is_none());
        assert_eq!(body["reasoning"]["effort"], "high");
        assert_eq!(body["service_tier"], "priority");
    }

    #[tokio::test]
    async fn codex_oauth_stream_reads_completed_usage_after_tool_call() {
        let _env_guard = codex_endpoint_env_lock().await;
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind test server");
        let addr = listener.local_addr().expect("local addr");
        let endpoint = format!("http://{addr}/backend-api/codex/responses");
        let previous_endpoint = std::env::var_os("OPENAI_CODEX_ENDPOINT");
        std::env::set_var("OPENAI_CODEX_ENDPOINT", &endpoint);

        std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept request");
            let mut buffer = Vec::new();
            let mut chunk = [0_u8; 1024];
            loop {
                let read = stream.read(&mut chunk).expect("read request");
                if read == 0 {
                    break;
                }
                buffer.extend_from_slice(&chunk[..read]);
                if let Some(header_end) = find_header_end(&buffer) {
                    let headers = String::from_utf8_lossy(&buffer[..header_end]);
                    let content_length = headers
                        .lines()
                        .find_map(|line| {
                            let (name, value) = line.split_once(':')?;
                            name.eq_ignore_ascii_case("content-length")
                                .then(|| value.trim().parse::<usize>().ok())
                                .flatten()
                        })
                        .unwrap_or(0);
                    let body_start = header_end + 4;
                    if buffer.len() >= body_start + content_length {
                        break;
                    }
                }
            }

            let args = r#"{"commands":[{"step":1,"command":"rg","command_line":"rg -n bug ."}]}"#;
            let tool_event = json!({
                "item": {
                    "type": "function_call",
                    "call_id": "call_1",
                    "name": "command_run",
                    "arguments": args
                }
            });
            let completed = json!({
                "type": "response.completed",
                "response": {
                    "output": [{
                        "type": "function_call",
                        "call_id": "call_1",
                        "name": "command_run",
                        "arguments": args
                    }],
                    "usage": {
                        "input_tokens": 3000,
                        "input_tokens_details": {"cached_tokens": 2048},
                        "output_tokens": 20,
                        "total_tokens": 3020
                    }
                }
            });
            let body = format!("data: {tool_event}\n\ndata: {completed}\n\ndata: [DONE]\n\n");
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\n\r\n{}",
                body.len(),
                body
            );
            stream
                .write_all(response.as_bytes())
                .expect("write response");
        });

        let messages = vec![json!({"role": "user", "content": "run command"})];
        let options = CallOptions {
            tools: Some(vec![json!({
                "type": "function",
                "function": {
                    "name": "command_run",
                    "parameters": {"type": "object"}
                }
            })]),
            ..CallOptions::default()
        };

        let result = super::codex_oauth_call(
            "gpt-5.1-codex-mini",
            "test-token",
            &messages,
            &options,
            None,
        )
        .await;

        match previous_endpoint {
            Some(value) => std::env::set_var("OPENAI_CODEX_ENDPOINT", value),
            None => std::env::remove_var("OPENAI_CODEX_ENDPOINT"),
        }

        let metrics = result.expect("codex oauth call").metrics.expect("metrics");
        assert_eq!(metrics.usage.cached_input_tokens, Some(2048));
        assert!(metrics.cache_hit);
    }

    #[test]
    fn codex_oauth_payload_omits_default_reasoning_and_acceleration() {
        let messages = vec![json!({"role": "user", "content": "ping"})];
        let options = CallOptions {
            reasoning_effort: Some("default".to_string()),
            service_tier: Some(" default ".to_string()),
            ..CallOptions::default()
        };

        let payload = build_codex_oauth_payload("gpt-5.1-codex", &messages, &options);

        assert!(payload.get("reasoning").is_none());
        assert!(payload.get("reasoning_effort").is_none());
        assert!(payload.get("service_tier").is_none());
    }

    #[test]
    fn codex_oauth_payload_passes_prompt_cache_key_only() {
        let messages = vec![json!({"role": "user", "content": "ping"})];
        let options = CallOptions {
            prompt_cache_key: Some("turaosv2:test:abc".to_string()),
            ..CallOptions::default()
        };

        let payload = build_codex_oauth_payload("gpt-5.1-codex-mini", &messages, &options);

        assert_eq!(payload["prompt_cache_key"], "turaosv2:test:abc");
        assert!(payload.get("prompt_cache_retention").is_none());
    }

    #[test]
    fn codex_oauth_payload_keeps_system_messages_in_input() {
        let messages = vec![
            json!({"role": "system", "content": "You are Tura an agent based on gpt-5.1-codex from LLM provider: openai."}),
            json!({"role": "user", "content": "task"}),
            json!({"role": "system", "content": "dynamic runtime state"}),
            json!({"role": "assistant", "content": "progress"}),
        ];

        let payload =
            build_codex_oauth_payload("gpt-5.1-codex-mini", &messages, &CallOptions::default());

        assert_eq!(
            payload["instructions"],
            "Follow the user request and answer concisely."
        );
        assert_eq!(payload["input"][0]["role"], "system");
        assert_eq!(
            payload["input"][0]["content"],
            "You are Tura an agent based on gpt-5.1-codex from LLM provider: openai."
        );
        assert_eq!(payload["input"][1]["role"], "user");
        assert_eq!(payload["input"][1]["content"], "task");
        assert_eq!(payload["input"][2]["role"], "system");
        assert_eq!(payload["input"][2]["content"], "dynamic runtime state");
        assert_eq!(payload["input"][3]["role"], "assistant");
        assert_eq!(payload["input"][3]["content"], "progress");
        assert_eq!(payload["tool_choice"], "auto");
    }

    #[test]
    fn codex_oauth_usage_falls_back_to_estimate_when_stream_stops_before_usage() {
        let payload = json!({
            "model": "gpt-5.1-codex",
            "input": [{"role": "user", "content": "Run tests"}],
            "tools": [{"type": "function", "name": "command_run"}]
        });
        let content = json!({
            "tool_calls": [{
                "function": {
                    "name": "command_run",
                    "arguments": {"commands": [{"step": 1, "command": "npm", "command_line": "npm test"}]}
                }
            }]
        });
        let mut metrics = crate::metrics::extract_openapi_metrics(&json!({}), None);

        crate::metrics::fill_missing_estimated_usage(
            &mut metrics,
            &payload,
            &content,
            "codex_oauth_stream_returned_before_provider_usage",
        );

        assert!(
            metrics
                .usage
                .input_tokens
                .expect("estimated input tokens should be present")
                > 0
        );
        assert!(
            metrics
                .usage
                .output_tokens
                .expect("estimated output tokens should be present")
                > 0
        );
        assert_eq!(
            metrics
                .raw_usage
                .as_ref()
                .and_then(|usage| usage.get("estimated"))
                .and_then(serde_json::Value::as_bool),
            Some(true)
        );
    }

    #[test]
    fn command_run_streaming_waits_for_complete_json_arguments() {
        let call = json!({
            "type": "function",
            "function": {
                "name": "command_run",
                "arguments": r#"{"commands":[{"step":1,"command":"rg","command_line":"rg -n bug ."},"#
            }
        });

        assert!(super::ready_streaming_tool_call(call).is_none());
    }

    #[test]
    fn command_run_command_streaming_emits_each_complete_command_object() {
        let mut collector = super::CodexCommandRunCommandCollector::default();
        collector.push_event(&json!({
            "type": "response.output_item.added",
            "item": {
                "id": "fc_1",
                "call_id": "call_1",
                "type": "function_call",
                "name": "command_run"
            }
        }));
        let first = collector.push_event(&json!({
            "type": "response.function_call_arguments.delta",
            "item_id": "fc_1",
            "delta": "{\"commands\":[{\"step\":1,\"command_type\":\"shell_command\",\"command_line\":\"echo {one}\"},"
        }));
        assert_eq!(first.len(), 1);
        assert_eq!(command_index_for_test(&first[0]), Some(0));

        let second = collector.push_event(&json!({
            "type": "response.function_call_arguments.delta",
            "item_id": "fc_1",
            "delta": "{\"step\":2,\"command_type\":\"shell_command\",\"command_line\":\"echo two\"}"
        }));
        assert_eq!(second.len(), 1);
        assert_eq!(command_index_for_test(&second[0]), Some(1));

        let done = collector.push_event(&json!({
            "type": "response.function_call_arguments.done",
            "item_id": "fc_1",
            "arguments": "{\"commands\":[{\"step\":1,\"command_type\":\"shell_command\",\"command_line\":\"echo {one}\"},{\"step\":2,\"command_type\":\"shell_command\",\"command_line\":\"echo two\"}]}"
        }));
        assert!(done.is_empty());
    }

    #[test]
    fn command_run_command_streaming_emits_split_python_command_object() {
        let mut collector = super::CodexCommandRunCommandCollector::default();
        collector.push_event(&json!({
            "type": "response.output_item.added",
            "item": {
                "id": "fc_stream_probe",
                "call_id": "call_stream_probe",
                "type": "function_call",
                "name": "command_run",
                "arguments": ""
            }
        }));
        let open = collector.push_event(&json!({
            "type": "response.function_call_arguments.delta",
            "item_id": "fc_stream_probe",
            "delta": "{\"commands\":["
        }));
        assert!(open.is_empty());
        let first_command = json!({
            "step": 1,
            "command_type": "shell_command",
            "command_line": json!({
                "command": "python -c \"from pathlib import Path; Path('streamed-first.txt').write_text('first')\"",
                "timeout_ms": 20000
            }).to_string()
        })
        .to_string()
            + ",";
        let first = collector.push_event(&json!({
            "type": "response.function_call_arguments.delta",
            "item_id": "fc_stream_probe",
            "delta": first_command
        }));
        assert_eq!(first.len(), 1);
        assert_eq!(command_index_for_test(&first[0]), Some(0));
    }

    fn command_index_for_test(event: &crate::tura_llm::ProviderStreamEvent) -> Option<usize> {
        match event {
            crate::tura_llm::ProviderStreamEvent::CommandRunCommandReady {
                command_index, ..
            } => Some(*command_index),
            crate::tura_llm::ProviderStreamEvent::ProviderOutputStarted => None,
        }
    }

    #[test]
    fn command_run_streaming_emits_complete_json_arguments() {
        let call = json!({
            "type": "function",
            "function": {
                "name": "command_run",
                "arguments": r#"{"commands":[{"step":1,"command":"npm","command_line":"npm test"}]}"#
            }
        });
        let ready = super::ready_streaming_tool_call(call).expect("complete command_run call");

        assert_eq!(
            ready["function"]["arguments"]["commands"]
                .as_array()
                .expect("ready command_run commands should be an array")
                .len(),
            1
        );
        assert_eq!(
            ready["function"]["arguments"]["commands"][0]["command"],
            "npm"
        );
        assert!(ready["function"]["arguments"].get("commands").is_some());
    }

    #[test]
    fn codex_event_tool_calls_accumulates_argument_deltas_before_emit() {
        let events = vec![
            json!({
                "type": "response.output_item.added",
                "item": {
                    "type": "function_call",
                    "call_id": "call_1",
                    "name": "command_run",
                    "arguments": ""
                }
            }),
            json!({
                "type": "response.function_call_arguments.delta",
                "delta": "{\"commands\":[{\"step\":1,"
            }),
            json!({
                "type": "response.function_call_arguments.delta",
                "delta": "\"command\":\"shell_command\",\"command_line\":\"pwd\"}]}"
            }),
        ];
        let calls = super::codex_event_tool_calls(&events);
        let ready = calls
            .into_iter()
            .filter_map(super::ready_streaming_tool_call)
            .collect::<Vec<_>>();

        assert_eq!(ready.len(), 1);
        assert_eq!(
            ready[0]["function"]["arguments"]["commands"][0]["command"],
            "shell_command"
        );
    }

    #[test]
    fn codex_stream_collector_emits_on_arguments_done_before_completed_response() {
        let mut collector = super::CodexToolCallStreamCollector::default();
        let added = json!({
            "type": "response.output_item.added",
            "item": {
                "type": "function_call",
                "id": "fc_early",
                "call_id": "call_early",
                "name": "command_run",
                "arguments": ""
            }
        });
        let delta = json!({
            "type": "response.function_call_arguments.delta",
            "item_id": "call_early",
            "delta": "{\"commands\":[{\"command_type\":\"shell_command\","
        });
        let done = json!({
            "type": "response.function_call_arguments.done",
            "item_id": "call_early",
            "arguments": "{\"commands\":[{\"command_type\":\"shell_command\",\"command_line\":\"pwd\"}]}"
        });

        assert!(collector.push_event(&added).is_empty());
        assert!(collector.push_event(&delta).is_empty());
        let ready = collector.push_event(&done);

        assert_eq!(ready.len(), 1);
        assert_eq!(ready[0]["function"]["name"], "command_run");
        assert_eq!(
            ready[0]["function"]["arguments"]["commands"][0]["command_type"],
            "shell_command"
        );
    }

    #[test]
    fn codex_stream_collector_does_not_emit_incomplete_arguments() {
        let mut collector = super::CodexToolCallStreamCollector::default();
        let added = json!({
            "type": "response.output_item.added",
            "item": {
                "type": "function_call",
                "id": "fc_incomplete",
                "call_id": "call_incomplete",
                "name": "command_run",
                "arguments": ""
            }
        });
        let delta = json!({
            "type": "response.function_call_arguments.delta",
            "item_id": "call_incomplete",
            "delta": "{\"commands\":["
        });

        assert!(collector.push_event(&added).is_empty());
        assert!(collector.push_event(&delta).is_empty());
        assert!(collector.finish().is_empty());
    }

    #[test]
    fn codex_responses_stream_tool_call_does_not_pollute_output_text() {
        let events = vec![
            json!({
                "type": "response.output_item.added",
                "item": {
                    "type": "function_call",
                    "id": "fc_real",
                    "call_id": "call_real",
                    "name": "command_run",
                    "arguments": ""
                }
            }),
            json!({
                "type": "response.function_call_arguments.delta",
                "item_id": "fc_real",
                "delta": "{\"commands\":[{\"command_type\":\"shell_command\","
            }),
            json!({
                "type": "response.function_call_arguments.delta",
                "item_id": "fc_real",
                "delta": "\"command_line\":\"Get-Content -Raw src/app.txt\"}]}"
            }),
            json!({
                "type": "response.function_call_arguments.done",
                "item_id": "fc_real",
                "arguments": "{\"commands\":[{\"command_type\":\"shell_command\",\"command_line\":\"Get-Content -Raw src/app.txt\"}]}"
            }),
            json!({
                "type": "response.output_item.done",
                "item": {
                    "type": "function_call",
                    "id": "fc_real",
                    "call_id": "call_real",
                    "name": "command_run",
                    "status": "completed",
                    "arguments": "{\"commands\":[{\"command_type\":\"shell_command\",\"command_line\":\"Get-Content -Raw src/app.txt\"}]}"
                }
            }),
        ];

        let mut output_text = String::new();
        for event in &events {
            super::append_codex_stream_text(event, &mut output_text);
        }
        assert!(output_text.is_empty());

        let normalized = super::normalize_codex_response_content(&json!({
            "events": events,
            "output_text": output_text,
        }));
        let tool_calls = normalized["tool_calls"]
            .as_array()
            .expect("Responses function_call events should normalize to tool_calls");

        assert_eq!(tool_calls.len(), 1);
        assert_eq!(tool_calls[0]["id"], "call_real");
        assert_eq!(tool_calls[0]["function"]["name"], "command_run");
        assert_eq!(
            tool_calls[0]["function"]["arguments"]["commands"][0]["command_type"],
            "shell_command"
        );
        assert!(normalized.get("text").is_none());
    }

    #[test]
    fn codex_event_tool_calls_does_not_emit_incomplete_command_run_arguments() {
        let events = vec![
            json!({
                "type": "response.output_item.added",
                "item": {
                    "type": "function_call",
                    "call_id": "call_1",
                    "name": "command_run",
                    "arguments": ""
                }
            }),
            json!({
                "type": "response.function_call_arguments.delta",
                "delta": "{\"commands\":["
            }),
        ];
        let ready = super::codex_event_tool_calls(&events)
            .into_iter()
            .filter_map(super::ready_streaming_tool_call)
            .collect::<Vec<_>>();

        assert!(ready.is_empty());
    }

    #[test]
    fn codex_event_tool_calls_prefers_done_arguments_over_added_empty_arguments() {
        let events = vec![
            json!({
                "type": "response.output_item.added",
                "item": {
                    "type": "function_call",
                    "call_id": "call_1",
                    "id": "fc_1",
                    "name": "command_run",
                    "arguments": ""
                }
            }),
            json!({
                "type": "response.function_call_arguments.done",
                "item_id": "fc_1",
                "arguments": "{\"commands\":[{\"step\":1,\"command\":\"echo ok\"}]}"
            }),
            json!({
                "type": "response.output_item.done",
                "item": {
                    "type": "function_call",
                    "call_id": "call_1",
                    "id": "fc_1",
                    "name": "command_run",
                    "status": "completed",
                    "arguments": "{\"commands\":[{\"step\":1,\"command\":\"echo ok\"}]}"
                }
            }),
        ];
        let ready = super::complete_codex_tool_calls(&json!({ "events": events }));

        assert_eq!(ready.len(), 1);
        assert_eq!(
            ready[0]["function"]["arguments"]["commands"][0]["command"],
            "echo ok"
        );
    }

    #[test]
    fn streaming_tool_call_buffer_waits_for_complete_json_arguments() {
        let mut buffer = super::StreamingToolCall {
            id: Some("call_1".to_string()),
            name: Some("command_run".to_string()),
            arguments: r#"{"commands":[{"step":1,"command":"rg","command_line":"rg -n bug ."},"#
                .to_string(),
            emitted: false,
        };
        let mut calls = Vec::new();

        assert!(!super::emit_completed_tool_call(&mut buffer, &mut calls));
        assert!(calls.is_empty());
    }

    #[test]
    fn streaming_tool_call_buffer_emits_complete_json_arguments() {
        let mut buffer = super::StreamingToolCall {
            id: Some("call_1".to_string()),
            name: Some("command_run".to_string()),
            arguments: r#"{"commands":[{"step":1,"command":"npm","command_line":"npm test"}]}"#
                .to_string(),
            emitted: false,
        };
        let mut calls = Vec::new();

        assert!(super::emit_completed_tool_call(&mut buffer, &mut calls));
        assert_eq!(calls.len(), 1);
        assert_eq!(
            calls[0]["function"]["arguments"]["commands"][0]["command"],
            "npm"
        );
    }

    #[test]
    fn minimax_xml_streaming_tool_call_supports_complete_command_run() {
        let text = r#"<minimax:tool_call><invoke name="command_run"><parameter name="commands">[{"step":1,"command":"npm","command_line":"npm test"}]</parameter></invoke></minimax:tool_call>"#;
        let (name, arguments) = super::last_complete_minimax_invoke(text).expect("xml tool call");

        assert_eq!(name, "command_run");
        assert_eq!(arguments["commands"][0]["command"], "npm");
    }

    fn find_header_end(buffer: &[u8]) -> Option<usize> {
        buffer.windows(4).position(|window| window == b"\r\n\r\n")
    }
}
