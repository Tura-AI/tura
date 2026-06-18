use base64::{engine::general_purpose, Engine as _};
use serde_json::json;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::thread;
use tura_command_image_generate::{access, execute};

static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[test]
fn image_generate_business_flow_openai_saves_base64_image() {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|err| err.into_inner());
    let dir = tempfile::tempdir().expect("tempdir");
    let image_bytes = tiny_png_bytes();
    let encoded = general_purpose::STANDARD.encode(image_bytes);
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let endpoint = format!(
        "http://{}/v1/images/generations",
        listener.local_addr().expect("addr")
    );
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept");
        let request = read_request(&mut stream);
        assert!(request.contains("POST /v1/images/generations"));
        assert!(request
            .to_ascii_lowercase()
            .contains("authorization: bearer sk-test-openai-key"));
        assert!(request.contains("\"prompt\":\"studio cat"));
        assert!(request.contains("\"size\":\"1024x1024\""));
        write_json(
            &mut stream,
            &json!({ "data": [{ "b64_json": encoded }] }).to_string(),
        );
    });

    let codex_home = tempfile::tempdir().expect("codex home");
    set_env("CODEX_HOME", &codex_home.path().display().to_string());
    set_env("OPENAI_API_KEY", "sk-test-openai-key");
    set_env("TURA_IMAGE_GENERATE_OPENAI_ENDPOINT", &endpoint);
    let response = execute(
        "--prompt 'studio cat' --provider openai --output-dir out --width 1024 --height 1024",
        dir.path(),
        30,
    );
    clear_env("CODEX_HOME");
    clear_env("OPENAI_API_KEY");
    clear_env("TURA_IMAGE_GENERATE_OPENAI_ENDPOINT");
    server.join().expect("server");

    assert!(response.success, "{}", response.stderr);
    assert_eq!(response.output["provider"], "chatgpt_image_2");
    let path = response.output["images"][0]["path"].as_str().expect("path");
    assert_eq!(
        std::fs::read(dir.path().join(path)).expect("image"),
        image_bytes
    );
}

#[test]
fn image_generate_business_flow_openai_prefers_codex_oauth() {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|err| err.into_inner());
    let dir = tempfile::tempdir().expect("tempdir");
    let image_bytes = tiny_png_bytes();
    let encoded = general_purpose::STANDARD.encode(image_bytes);
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let endpoint = format!(
        "http://{}/codex/responses",
        listener.local_addr().expect("addr")
    );
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept");
        let request = read_request(&mut stream);
        let lower = request.to_ascii_lowercase();
        assert!(request.contains("POST /codex/responses"));
        assert!(lower.contains("authorization: bearer eyjcodex.oauth.token"));
        assert!(lower.contains("originator: codex_cli_rs"));
        assert!(lower.contains("chatgpt-account-id: acct-test"));
        assert!(!lower.contains("sk-fallback-openai-key"));
        assert!(request.contains("\"type\":\"image_generation\""));
        assert!(request.contains("\"output_format\":\"png\""));
        assert!(request.contains("oauth cat"));
        write_json(
            &mut stream,
            &json!({
                "output": [{
                    "type": "image_generation_call",
                    "id": "ig_test",
                    "status": "completed",
                    "result": encoded
                }]
            })
            .to_string(),
        );
    });

    let codex_home = tempfile::tempdir().expect("codex home");
    set_env("CODEX_HOME", &codex_home.path().display().to_string());
    set_env("CODEX_OPENAI_OAUTH_TOKEN", "eyJcodex.oauth.token");
    set_env("OPENAI_OPENAPI_KEY", "sk-fallback-openai-key");
    set_env("OPENAI_ACCOUNT_ID", "acct-test");
    set_env("TURA_IMAGE_GENERATE_CODEX_RESPONSES_ENDPOINT", &endpoint);
    let response = execute(
        "--prompt 'oauth cat' --provider openai --output-dir out",
        dir.path(),
        30,
    );
    clear_env("CODEX_HOME");
    clear_env("CODEX_OPENAI_OAUTH_TOKEN");
    clear_env("OPENAI_OPENAPI_KEY");
    clear_env("OPENAI_ACCOUNT_ID");
    clear_env("TURA_IMAGE_GENERATE_CODEX_RESPONSES_ENDPOINT");
    server.join().expect("server");

    assert!(response.success, "{}", response.stderr);
    assert_eq!(response.output["provider"], "chatgpt_image_2");
}

#[test]
fn image_generate_business_flow_openai_reads_codex_auth_json() {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|err| err.into_inner());
    let dir = tempfile::tempdir().expect("tempdir");
    let codex_home = tempfile::tempdir().expect("codex home");
    std::fs::write(
        codex_home.path().join("auth.json"),
        r#"{"tokens":{"access_token":"eyJcodex.file.token","account_id":"acct-file"}}"#,
    )
    .expect("codex auth");
    let image_bytes = tiny_png_bytes();
    let encoded = general_purpose::STANDARD.encode(image_bytes);
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let endpoint = format!(
        "http://{}/codex/responses",
        listener.local_addr().expect("addr")
    );
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept");
        let request = read_request(&mut stream);
        let lower = request.to_ascii_lowercase();
        assert!(request.contains("POST /codex/responses"));
        assert!(lower.contains("authorization: bearer eyjcodex.file.token"));
        assert!(lower.contains("chatgpt-account-id: acct-file"));
        let body = format!(
            "event: response.output_item.done\ndata: {}\n\nevent: response.completed\ndata: {{\"type\":\"response.completed\"}}\n\n",
            json!({
                "type": "response.output_item.done",
                "item": {
                    "type": "image_generation_call",
                    "id": "ig_file",
                    "status": "completed",
                    "result": encoded
                }
            })
        );
        write_status(&mut stream, 200, body.as_bytes());
    });

    clear_env("OPENAI_API_KEY");
    clear_env("OPENAI_OPENAPI_KEY");
    clear_env("CODEX_OPENAI_OAUTH_TOKEN");
    clear_env("CODEX_OAUTH_TOKEN");
    set_env("CODEX_HOME", &codex_home.path().display().to_string());
    set_env("TURA_IMAGE_GENERATE_CODEX_RESPONSES_ENDPOINT", &endpoint);
    let response = execute(
        "--prompt 'codex file cat' --provider openai --output-dir out",
        dir.path(),
        30,
    );
    clear_env("CODEX_HOME");
    clear_env("TURA_IMAGE_GENERATE_CODEX_RESPONSES_ENDPOINT");
    server.join().expect("server");

    assert!(response.success, "{}", response.stderr);
    assert_eq!(response.output["provider"], "chatgpt_image_2");
}

#[test]
fn image_generate_business_flow_openai_falls_back_to_openapi_key() {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|err| err.into_inner());
    let dir = tempfile::tempdir().expect("tempdir");
    let image_bytes = tiny_png_bytes();
    let encoded = general_purpose::STANDARD.encode(image_bytes);
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().expect("addr");
    let codex_endpoint = format!("http://{addr}/codex/responses");
    let openai_endpoint = format!("http://{addr}/openai");
    let server = thread::spawn(move || {
        for index in 0..2 {
            let (mut stream, _) = listener.accept().expect("accept");
            let request = read_request(&mut stream);
            let lower = request.to_ascii_lowercase();
            if index == 0 {
                assert!(request.contains("POST /codex/responses"));
                assert!(lower.contains("authorization: bearer eyjcodex.oauth.token"));
                write_status(&mut stream, 401, br#"{"error":"expired oauth"}"#);
            } else {
                assert!(request.contains("POST /openai"));
                assert!(lower.contains("authorization: bearer sk-fallback-openai-key"));
                write_json(
                    &mut stream,
                    &json!({ "data": [{ "b64_json": encoded }] }).to_string(),
                );
            }
        }
    });

    let codex_home = tempfile::tempdir().expect("codex home");
    set_env("CODEX_HOME", &codex_home.path().display().to_string());
    set_env("CODEX_OPENAI_OAUTH_TOKEN", "eyJcodex.oauth.token");
    set_env("CODEX_OAUTH_TOKEN", "not-oauth");
    set_env("OPENAI_OAUTH_TOKEN", "not-oauth");
    set_env("CHATGPT_OAUTH_TOKEN", "not-oauth");
    set_env("OPENAI_OPENAPI_KEY", "sk-fallback-openai-key");
    set_env("OPENAI_API_KEY", "not-oauth");
    set_env("CHATGPT_API_KEY", "not-oauth");
    set_env(
        "TURA_IMAGE_GENERATE_CODEX_RESPONSES_ENDPOINT",
        &codex_endpoint,
    );
    set_env("TURA_IMAGE_GENERATE_OPENAI_ENDPOINT", &openai_endpoint);
    let response = execute(
        "--prompt 'fallback cat' --provider openai --output-dir out",
        dir.path(),
        30,
    );
    clear_env("CODEX_OPENAI_OAUTH_TOKEN");
    clear_env("CODEX_OAUTH_TOKEN");
    clear_env("OPENAI_OAUTH_TOKEN");
    clear_env("CHATGPT_OAUTH_TOKEN");
    clear_env("OPENAI_OPENAPI_KEY");
    clear_env("OPENAI_API_KEY");
    clear_env("CHATGPT_API_KEY");
    clear_env("CODEX_HOME");
    clear_env("TURA_IMAGE_GENERATE_CODEX_RESPONSES_ENDPOINT");
    clear_env("TURA_IMAGE_GENERATE_OPENAI_ENDPOINT");
    server.join().expect("server");

    assert!(response.success, "{}", response.stderr);
    assert_eq!(response.output["provider"], "chatgpt_image_2");
}

#[test]
fn image_generate_business_flow_falls_back_to_replicate_url_output() {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|err| err.into_inner());
    let dir = tempfile::tempdir().expect("tempdir");
    let image_bytes = tiny_png_bytes();
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().expect("addr");
    let openai_endpoint = format!("http://{addr}/openai");
    let replicate_endpoint = format!("http://{addr}/replicate");
    let image_url = format!("http://{addr}/asset.png");
    let server = thread::spawn(move || {
        for _ in 0..3 {
            let (mut stream, _) = listener.accept().expect("accept");
            let request = read_request(&mut stream);
            if request.contains("POST /openai") {
                write_status(&mut stream, 500, "{}".as_bytes());
            } else if request.contains("POST /replicate") {
                assert!(request.contains("\"width\":1440"));
                assert!(request.contains("\"height\":960"));
                write_json(&mut stream, &json!({ "output": image_url }).to_string());
            } else {
                write_status(&mut stream, 200, image_bytes);
            }
        }
    });

    let codex_home = tempfile::tempdir().expect("codex home");
    set_env("CODEX_HOME", &codex_home.path().display().to_string());
    set_env("OPENAI_API_KEY", "sk-test-openai-key");
    set_env("REPLICATE_API_TOKEN", "test-replicate-key");
    set_env("TURA_IMAGE_GENERATE_OPENAI_ENDPOINT", &openai_endpoint);
    set_env(
        "TURA_IMAGE_GENERATE_REPLICATE_ENDPOINT",
        &replicate_endpoint,
    );
    let response = execute(
        "--prompt poster --provider-order openai,replicate --width 1536 --height 1024 --output-dir out",
        dir.path(),
        30,
    );
    clear_env("CODEX_HOME");
    clear_env("OPENAI_API_KEY");
    clear_env("REPLICATE_API_TOKEN");
    clear_env("TURA_IMAGE_GENERATE_OPENAI_ENDPOINT");
    clear_env("TURA_IMAGE_GENERATE_REPLICATE_ENDPOINT");
    server.join().expect("server");

    assert!(response.success, "{}", response.stderr);
    assert_eq!(response.output["provider"], "replicate_z_image_turbo");
    assert_eq!(response.output["attempts"][0]["success"], false);
    assert_eq!(response.output["attempts"][1]["success"], true);
}

#[test]
fn image_generate_business_flow_configured_order_falls_back_to_openai_and_keeps_attempts() {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|err| err.into_inner());
    let dir = tempfile::tempdir().expect("tempdir");
    let image_bytes = tiny_png_bytes();
    let encoded = general_purpose::STANDARD.encode(image_bytes);
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().expect("addr");
    let replicate_endpoint = format!("http://{addr}/replicate");
    let openai_endpoint = format!("http://{addr}/openai");
    let server = thread::spawn(move || {
        for _ in 0..2 {
            let (mut stream, _) = listener.accept().expect("accept");
            let request = read_request(&mut stream);
            if request.contains("POST /replicate") {
                write_status(&mut stream, 500, br#"{"error":"replicate unavailable"}"#);
            } else {
                assert!(request.contains("POST /openai"));
                write_json(
                    &mut stream,
                    &json!({ "data": [{ "b64_json": encoded }] }).to_string(),
                );
            }
        }
    });

    let codex_home = tempfile::tempdir().expect("codex home");
    set_env("CODEX_HOME", &codex_home.path().display().to_string());
    set_env("REPLICATE_API_TOKEN", "test-replicate-key");
    set_env("OPENAI_API_KEY", "sk-test-openai-key");
    set_env("TURA_IMAGE_GENERATE_PROVIDER_ORDER", "replicate,openai");
    set_env(
        "TURA_IMAGE_GENERATE_REPLICATE_ENDPOINT",
        &replicate_endpoint,
    );
    set_env("TURA_IMAGE_GENERATE_OPENAI_ENDPOINT", &openai_endpoint);
    let response = execute("--prompt poster --output-dir out", dir.path(), 30);
    clear_env("CODEX_HOME");
    clear_env("REPLICATE_API_TOKEN");
    clear_env("OPENAI_API_KEY");
    clear_env("TURA_IMAGE_GENERATE_PROVIDER_ORDER");
    clear_env("TURA_IMAGE_GENERATE_REPLICATE_ENDPOINT");
    clear_env("TURA_IMAGE_GENERATE_OPENAI_ENDPOINT");
    server.join().expect("server");

    assert!(response.success, "{}", response.stderr);
    assert_eq!(response.output["provider"], "chatgpt_image_2");
    assert_eq!(
        response.output["attempts"][0]["provider"],
        "replicate_z_image_turbo"
    );
    assert_eq!(response.output["attempts"][0]["success"], false);
    assert!(response.output["attempts"][0]["error"]
        .as_str()
        .expect("first attempt error")
        .contains("Replicate Z-Image Turbo failed"));
    assert_eq!(
        response.output["attempts"][1]["provider"],
        "chatgpt_image_2"
    );
    assert_eq!(response.output["attempts"][1]["success"], true);
}

#[test]
fn image_generate_business_flow_returns_provider_errors_when_all_fallbacks_fail() {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|err| err.into_inner());
    let dir = tempfile::tempdir().expect("tempdir");
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().expect("addr");
    let replicate_endpoint = format!("http://{addr}/replicate");
    let openai_endpoint = format!("http://{addr}/openai");
    let server = thread::spawn(move || {
        for _ in 0..2 {
            let (mut stream, _) = listener.accept().expect("accept");
            let request = read_request(&mut stream);
            if request.contains("POST /replicate") {
                write_status(&mut stream, 503, br#"{"error":"replicate down"}"#);
            } else {
                assert!(request.contains("POST /openai"));
                write_status(&mut stream, 502, br#"{"error":"openai down"}"#);
            }
        }
    });

    let codex_home = tempfile::tempdir().expect("codex home");
    set_env("CODEX_HOME", &codex_home.path().display().to_string());
    set_env("REPLICATE_API_TOKEN", "test-replicate-key");
    set_env("OPENAI_API_KEY", "sk-test-openai-key");
    set_env("TURA_IMAGE_GENERATE_PROVIDER_ORDER", "replicate,openai");
    set_env(
        "TURA_IMAGE_GENERATE_REPLICATE_ENDPOINT",
        &replicate_endpoint,
    );
    set_env("TURA_IMAGE_GENERATE_OPENAI_ENDPOINT", &openai_endpoint);
    let response = execute("--prompt poster --output-dir out", dir.path(), 30);
    clear_env("CODEX_HOME");
    clear_env("REPLICATE_API_TOKEN");
    clear_env("OPENAI_API_KEY");
    clear_env("TURA_IMAGE_GENERATE_PROVIDER_ORDER");
    clear_env("TURA_IMAGE_GENERATE_REPLICATE_ENDPOINT");
    clear_env("TURA_IMAGE_GENERATE_OPENAI_ENDPOINT");
    server.join().expect("server");

    assert!(!response.success);
    assert!(response
        .stderr
        .contains("replicate_z_image_turbo: Replicate Z-Image Turbo failed"));
    assert!(response
        .stderr
        .contains("chatgpt_image_2: OpenAI image generation failed"));
    assert_eq!(response.output["error"], response.stderr);
}

#[test]
fn image_generate_business_protocol_access_and_dry_run_are_stable() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(dir.path().join("ref.png"), tiny_png_bytes()).expect("ref");

    let access = access(
        "--prompt logo --reference ref.png --output-dir generated",
        dir.path(),
    );
    assert_eq!(access.read_paths, vec!["ref.png".to_string()]);
    assert_eq!(
        access.write_paths,
        vec!["generated/.image_generate".to_string()]
    );

    let response = execute(
        r#"{"prompt":"logo","references":["ref.png"],"provider":"grok3","dry_run":true,"aspect_ratio":"1:1"}"#,
        dir.path(),
        30,
    );
    assert!(response.success, "{}", response.stderr);
    assert_eq!(response.output["dry_run"], true);
    assert_eq!(response.output["providers"][0]["provider"], "grok3");
    assert!(response.output["providers"][0]["request"]
        .get("image")
        .is_some());
}

fn set_env(key: &str, value: &str) {
    std::env::set_var(key, value);
}

fn clear_env(key: &str) {
    std::env::remove_var(key);
}

fn read_request(stream: &mut std::net::TcpStream) -> String {
    let mut buffer = [0u8; 16384];
    let read = stream.read(&mut buffer).expect("read request");
    String::from_utf8_lossy(&buffer[..read]).to_string()
}

fn write_json(stream: &mut std::net::TcpStream, body: &str) {
    write_status(stream, 200, body.as_bytes());
}

fn write_status(stream: &mut std::net::TcpStream, status: u16, body: &[u8]) {
    let status_text = if status == 200 { "OK" } else { "ERR" };
    let header = format!(
        "HTTP/1.1 {status} {status_text}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(header.as_bytes()).expect("header");
    stream.write_all(body).expect("body");
}

fn tiny_png_bytes() -> &'static [u8] {
    &[
        0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, b'I', b'H', b'D',
        b'R', 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x02, 0x00, 0x00, 0x00, 0x90,
        0x77, 0x53, 0xde, 0x00, 0x00, 0x00, 0x0c, b'I', b'D', b'A', b'T', 0x08, 0xd7, 0x63, 0xf8,
        0xcf, 0xc0, 0x00, 0x00, 0x03, 0x01, 0x01, 0x00, 0x18, 0xdd, 0x8d, 0xb0, 0x00, 0x00, 0x00,
        0x00, b'I', b'E', b'N', b'D', 0xae, 0x42, 0x60, 0x82,
    ]
}
