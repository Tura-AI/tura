use base64::{engine::general_purpose, Engine as _};
use serde_json::json;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::thread;
use tura_command_generate_media::{access, execute};

static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[test]
fn generate_media_business_flow_openai_saves_base64_image() {
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
    set_env("TURA_GENERATE_MEDIA_OPENAI_ENDPOINT", &endpoint);
    let response = execute(
        "--prompt 'studio cat' --provider openai --output-dir out --width 1024 --height 1024",
        dir.path(),
        30,
    );
    clear_env("CODEX_HOME");
    clear_env("OPENAI_API_KEY");
    clear_env("TURA_GENERATE_MEDIA_OPENAI_ENDPOINT");
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
fn generate_media_business_flow_openai_prefers_codex_oauth() {
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
    set_env("TURA_GENERATE_MEDIA_CODEX_RESPONSES_ENDPOINT", &endpoint);
    let response = execute(
        "--prompt 'oauth cat' --provider openai --output-dir out",
        dir.path(),
        30,
    );
    clear_env("CODEX_HOME");
    clear_env("CODEX_OPENAI_OAUTH_TOKEN");
    clear_env("OPENAI_OPENAPI_KEY");
    clear_env("OPENAI_ACCOUNT_ID");
    clear_env("TURA_GENERATE_MEDIA_CODEX_RESPONSES_ENDPOINT");
    server.join().expect("server");

    assert!(response.success, "{}", response.stderr);
    assert_eq!(response.output["provider"], "chatgpt_image_2");
}

#[test]
fn generate_media_business_flow_openai_reads_codex_auth_json() {
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
    set_env("TURA_GENERATE_MEDIA_CODEX_RESPONSES_ENDPOINT", &endpoint);
    let response = execute(
        "--prompt 'codex file cat' --provider openai --output-dir out",
        dir.path(),
        30,
    );
    clear_env("CODEX_HOME");
    clear_env("TURA_GENERATE_MEDIA_CODEX_RESPONSES_ENDPOINT");
    server.join().expect("server");

    assert!(response.success, "{}", response.stderr);
    assert_eq!(response.output["provider"], "chatgpt_image_2");
}

#[test]
fn generate_media_business_flow_openai_falls_back_to_openapi_key() {
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
        "TURA_GENERATE_MEDIA_CODEX_RESPONSES_ENDPOINT",
        &codex_endpoint,
    );
    set_env("TURA_GENERATE_MEDIA_OPENAI_ENDPOINT", &openai_endpoint);
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
    clear_env("TURA_GENERATE_MEDIA_CODEX_RESPONSES_ENDPOINT");
    clear_env("TURA_GENERATE_MEDIA_OPENAI_ENDPOINT");
    server.join().expect("server");

    assert!(response.success, "{}", response.stderr);
    assert_eq!(response.output["provider"], "chatgpt_image_2");
}

#[test]
fn generate_media_business_flow_falls_back_to_replicate_url_output() {
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
    set_env("TURA_GENERATE_MEDIA_OPENAI_ENDPOINT", &openai_endpoint);
    set_env(
        "TURA_GENERATE_MEDIA_REPLICATE_ENDPOINT",
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
    clear_env("TURA_GENERATE_MEDIA_OPENAI_ENDPOINT");
    clear_env("TURA_GENERATE_MEDIA_REPLICATE_ENDPOINT");
    server.join().expect("server");

    assert!(response.success, "{}", response.stderr);
    assert_eq!(response.output["provider"], "replicate_z_image_turbo");
    assert_eq!(response.output["attempts"][0]["success"], false);
    assert_eq!(response.output["attempts"][1]["success"], true);
}

#[test]
fn generate_media_business_flow_configured_order_falls_back_to_openai_and_keeps_attempts() {
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
    set_env("TURA_GENERATE_MEDIA_PROVIDER_ORDER", "replicate,openai");
    set_env(
        "TURA_GENERATE_MEDIA_REPLICATE_ENDPOINT",
        &replicate_endpoint,
    );
    set_env("TURA_GENERATE_MEDIA_OPENAI_ENDPOINT", &openai_endpoint);
    let response = execute("--prompt poster --output-dir out", dir.path(), 30);
    clear_env("CODEX_HOME");
    clear_env("REPLICATE_API_TOKEN");
    clear_env("OPENAI_API_KEY");
    clear_env("TURA_GENERATE_MEDIA_PROVIDER_ORDER");
    clear_env("TURA_GENERATE_MEDIA_REPLICATE_ENDPOINT");
    clear_env("TURA_GENERATE_MEDIA_OPENAI_ENDPOINT");
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
fn generate_media_business_flow_returns_provider_errors_when_all_fallbacks_fail() {
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
    set_env("TURA_GENERATE_MEDIA_PROVIDER_ORDER", "replicate,openai");
    set_env(
        "TURA_GENERATE_MEDIA_REPLICATE_ENDPOINT",
        &replicate_endpoint,
    );
    set_env("TURA_GENERATE_MEDIA_OPENAI_ENDPOINT", &openai_endpoint);
    let response = execute("--prompt poster --output-dir out", dir.path(), 30);
    clear_env("CODEX_HOME");
    clear_env("REPLICATE_API_TOKEN");
    clear_env("OPENAI_API_KEY");
    clear_env("TURA_GENERATE_MEDIA_PROVIDER_ORDER");
    clear_env("TURA_GENERATE_MEDIA_REPLICATE_ENDPOINT");
    clear_env("TURA_GENERATE_MEDIA_OPENAI_ENDPOINT");
    server.join().expect("server");

    assert!(!response.success);
    assert!(response
        .stderr
        .contains("replicate_z_image_turbo: Replicate Z-Image Turbo failed"));
    assert!(response
        .stderr
        .contains("chatgpt_image_2: OpenAI image generation failed"));
    assert!(response
        .stderr
        .contains("set one of the matching media generation keys to enable generate_media"));
    assert!(response.stderr.contains("REPLICATE_API_TOKEN"));
    assert!(response.stderr.contains("OPENAI_API_KEY"));
    assert_eq!(response.output["error"], response.stderr);
}

#[test]
fn generate_media_business_flow_reports_media_generation_key_help_when_no_provider_is_available() {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|err| err.into_inner());
    let dir = tempfile::tempdir().expect("tempdir");
    let saved_replicate = std::env::var("REPLICATE_API_TOKEN").ok();
    let saved_replicate_key = std::env::var("REPLICATE_API_KEY").ok();
    clear_env("REPLICATE_API_TOKEN");
    clear_env("REPLICATE_API_KEY");

    let response = execute(
        "--prompt poster --provider replicate --output-dir out",
        dir.path(),
        30,
    );

    restore_env("REPLICATE_API_TOKEN", saved_replicate);
    restore_env("REPLICATE_API_KEY", saved_replicate_key);

    assert!(!response.success);
    assert!(response
        .stderr
        .contains("all generate_media providers failed"));
    assert!(response
        .stderr
        .contains("set one of the matching media generation keys to enable generate_media"));
    assert!(response.stderr.contains("REPLICATE_API_TOKEN"));
    assert_eq!(response.output["error"], response.stderr);
}

#[test]
fn generate_media_business_flow_reports_speech_key_help_when_no_provider_is_available() {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|err| err.into_inner());
    let dir = tempfile::tempdir().expect("tempdir");
    let saved_elevenlabs = std::env::var("ELEVENLABS_API_KEY").ok();
    let saved_xi = std::env::var("XI_API_KEY").ok();
    clear_env("ELEVENLABS_API_KEY");
    clear_env("XI_API_KEY");

    let response = execute(
        r#"{
            "media_type":"speech",
            "text":"hello from speech",
            "text_language":"en_us",
            "role":"female_gentle",
            "tone":"calm",
            "speech_provider_order":"elevenlabs",
            "output_dir":"voice-out"
        }"#,
        dir.path(),
        30,
    );

    restore_env("ELEVENLABS_API_KEY", saved_elevenlabs);
    restore_env("XI_API_KEY", saved_xi);

    assert!(!response.success);
    assert!(response
        .stderr
        .contains("all generate_media speech providers failed"));
    assert!(response
        .stderr
        .contains("set one of the matching media generation keys to enable generate_media speech"));
    assert!(response.stderr.contains("ELEVENLABS_API_KEY"));
    assert_eq!(response.output["error"], response.stderr);
}

#[test]
fn generate_media_business_protocol_access_and_dry_run_are_stable() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(dir.path().join("ref.png"), tiny_png_bytes()).expect("ref");

    let access = access(
        "--prompt logo --reference ref.png --output-dir generated",
        dir.path(),
    );
    assert_eq!(access.read_paths, vec!["ref.png".to_string()]);
    assert_eq!(
        access.write_paths,
        vec!["generated/.generate_media".to_string()]
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

#[test]
fn generate_media_business_flow_speech_provider_matrix_saves_audio() {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|err| err.into_inner());
    for provider in [
        "openai_tts",
        "elevenlabs",
        "qwen_dashscope",
        "azure_edge_tts",
        "azure_speech",
        "replicate_qwen3_tts",
        "replicate_chatterbox",
    ] {
        run_speech_provider_mock(provider);
    }
}

#[test]
fn generate_media_business_flow_speech_falls_back_after_missing_key() {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|err| err.into_inner());
    let dir = tempfile::tempdir().expect("tempdir");
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().expect("addr");
    let audio_bytes = b"fallback-audio-bytes".to_vec();
    let expected_audio_bytes = audio_bytes.clone();
    let previous_openai = std::env::var("OPENAI_OPENAPI_KEY").ok();
    let previous_chatgpt = std::env::var("CHATGPT_API_KEY").ok();
    clear_env("OPENAI_OPENAPI_KEY");
    clear_env("CHATGPT_API_KEY");
    set_env("QWEN_API_KEY", "test-qwen-key");
    set_env(
        "TURA_GENERATE_MEDIA_QWEN_TTS_ENDPOINT",
        &format!("http://{addr}/qwen"),
    );
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept qwen fallback");
        let request = read_request(&mut stream);
        assert!(request.contains("POST /qwen"));
        assert!(request.contains("fallback speech"));
        write_json(
            &mut stream,
            &json!({ "output": { "audio": general_purpose::STANDARD.encode(&audio_bytes) } })
                .to_string(),
        );
    });

    let response = execute(
        r#"{
            "media_type":"speech",
            "text":"fallback speech",
            "text_language":"en_us",
            "role":"female_gentle",
            "tone":"calm",
            "speech_provider_order":"openai_tts,qwen_dashscope",
            "output_dir":"voice-out"
        }"#,
        dir.path(),
        30,
    );
    restore_env("OPENAI_OPENAPI_KEY", previous_openai);
    restore_env("CHATGPT_API_KEY", previous_chatgpt);
    clear_env("QWEN_API_KEY");
    clear_env("TURA_GENERATE_MEDIA_QWEN_TTS_ENDPOINT");
    server.join().expect("server");

    assert!(response.success, "{}", response.stderr);
    assert_eq!(response.output["attempts"][0]["provider"], "openai_tts");
    assert_eq!(response.output["attempts"][0]["success"], false);
    assert_eq!(response.output["attempts"][1]["provider"], "qwen_dashscope");
    assert_eq!(response.output["attempts"][1]["success"], true);
    let path = response.output["audio"]["path"]
        .as_str()
        .expect("audio path");
    assert_eq!(
        std::fs::read(dir.path().join(path)).expect("audio"),
        expected_audio_bytes
    );
}

fn run_speech_provider_mock(provider: &str) {
    let dir = tempfile::tempdir().expect("tempdir");
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().expect("addr");
    let base = format!("http://{addr}");
    let audio_bytes = b"mock-audio-bytes".to_vec();
    let expected_audio_bytes = audio_bytes.clone();
    configure_speech_provider_env(provider, &base);
    let provider_for_thread = provider.to_string();
    let server = thread::spawn(move || match provider_for_thread.as_str() {
        "openai_tts" => {
            let (mut stream, _) = listener.accept().expect("accept openai");
            let request = read_request(&mut stream);
            assert!(request.contains("POST /openai/audio"));
            assert!(request.contains("\"voice\""));
            assert!(request.contains("hello from speech"));
            write_status_content_type(&mut stream, 200, "audio/mpeg", &audio_bytes);
        }
        "elevenlabs" => {
            let (mut stream, _) = listener.accept().expect("accept elevenlabs");
            let request = read_request(&mut stream);
            assert!(request.contains("POST /eleven/"));
            assert!(request.contains("xi-api-key: test-elevenlabs-key"));
            assert!(request.contains("\"model_id\""));
            write_status_content_type(&mut stream, 200, "audio/mpeg", &audio_bytes);
        }
        "qwen_dashscope" => {
            let (mut stream, _) = listener.accept().expect("accept qwen");
            let request = read_request(&mut stream);
            assert!(request.contains("POST /qwen"));
            assert!(request
                .to_ascii_lowercase()
                .contains("authorization: bearer test-qwen-key"));
            assert!(request.contains("\"instructions\""));
            write_json(
                &mut stream,
                &json!({ "output": { "audio": general_purpose::STANDARD.encode(&audio_bytes) } })
                    .to_string(),
            );
        }
        "azure_edge_tts" => {
            let (mut stream, _) = listener.accept().expect("accept azure edge");
            let request = read_request(&mut stream);
            assert!(request.contains("POST /azure-edge/tts"));
            assert!(!request
                .to_ascii_lowercase()
                .contains("ocp-apim-subscription-key"));
            assert!(request.contains("\"voice\""));
            assert!(request.contains("\"rate\""));
            assert!(request.contains("\"volume\""));
            write_status_content_type(&mut stream, 200, "audio/mpeg", &audio_bytes);
        }
        "azure_speech" => {
            let (mut token_stream, _) = listener.accept().expect("accept azure token");
            let token_request = read_request(&mut token_stream);
            assert!(token_request.contains("POST /azure/token"));
            assert!(token_request
                .to_ascii_lowercase()
                .contains("ocp-apim-subscription-key: test-azure-key"));
            write_status_content_type(&mut token_stream, 200, "text/plain", b"azure-token");

            let (mut speech_stream, _) = listener.accept().expect("accept azure speech");
            let speech_request = read_request(&mut speech_stream);
            assert!(speech_request.contains("POST /azure/speech"));
            assert!(speech_request.contains("<speak"));
            assert!(speech_request.contains("mstts:express-as"));
            write_status_content_type(&mut speech_stream, 200, "audio/mpeg", &audio_bytes);
        }
        "replicate_qwen3_tts" | "replicate_chatterbox" => {
            let (mut create_stream, _) = listener.accept().expect("accept replicate create");
            let create_request = read_request(&mut create_stream);
            assert!(create_request.contains("POST /replicate"));
            assert!(create_request
                .to_ascii_lowercase()
                .contains("authorization: bearer test-replicate-key"));
            assert!(create_request.contains("hello from speech"));
            let url = format!("http://{addr}/asset.wav");
            write_json(&mut create_stream, &json!({ "output": url }).to_string());

            let (mut asset_stream, _) = listener.accept().expect("accept replicate asset");
            let asset_request = read_request(&mut asset_stream);
            assert!(asset_request.contains("GET /asset.wav"));
            write_status_content_type(&mut asset_stream, 200, "audio/wav", &audio_bytes);
        }
        other => panic!("unexpected provider {other}"),
    });

    let command = format!(
        r#"{{
            "media_type":"speech",
            "text":"hello from speech",
            "text_language":"en_us",
            "role":"female_gentle",
            "tone":"calm",
            "custom_tone_description":"near-field narration",
            "custom_voice_description":"soft texture",
            "speech_provider_order":"{provider}",
            "output_dir":"voice-out"
        }}"#
    );
    let response = execute(&command, dir.path(), 30);
    clear_speech_provider_env(provider);
    server.join().expect("server");

    assert!(response.success, "{}", response.stderr);
    assert_eq!(response.output["media_type"], "speech");
    assert_eq!(response.output["result_count"], 1);
    assert_eq!(response.output["attempts"][0]["provider"], provider);
    let path = response.output["audio"]["path"]
        .as_str()
        .expect("audio path");
    assert_eq!(
        std::fs::read(dir.path().join(path)).expect("audio"),
        expected_audio_bytes
    );
}

fn configure_speech_provider_env(provider: &str, base: &str) {
    match provider {
        "openai_tts" => {
            set_env("OPENAI_OPENAPI_KEY", "test-openai-key");
            set_env(
                "TURA_GENERATE_MEDIA_OPENAI_TTS_ENDPOINT",
                &format!("{base}/openai/audio"),
            );
        }
        "elevenlabs" => {
            set_env("ELEVENLABS_API_KEY", "test-elevenlabs-key");
            set_env(
                "TURA_GENERATE_MEDIA_ELEVENLABS_ENDPOINT",
                &format!("{base}/eleven/{{voice_id}}"),
            );
        }
        "qwen_dashscope" => {
            set_env("QWEN_API_KEY", "test-qwen-key");
            set_env(
                "TURA_GENERATE_MEDIA_QWEN_TTS_ENDPOINT",
                &format!("{base}/qwen"),
            );
        }
        "azure_edge_tts" => {
            set_env(
                "TURA_GENERATE_MEDIA_AZURE_EDGE_TTS_ENDPOINT",
                &format!("{base}/azure-edge/tts"),
            );
        }
        "azure_speech" => {
            set_env("AZURE_SPEECH_KEY", "test-azure-key");
            set_env("AZURE_SPEECH_REGION", "test-region");
            set_env(
                "TURA_GENERATE_MEDIA_AZURE_SPEECH_TOKEN_ENDPOINT",
                &format!("{base}/azure/token"),
            );
            set_env(
                "TURA_GENERATE_MEDIA_AZURE_SPEECH_ENDPOINT",
                &format!("{base}/azure/speech"),
            );
        }
        "replicate_qwen3_tts" => {
            set_env("REPLICATE_API_TOKEN", "test-replicate-key");
            set_env(
                "TURA_GENERATE_MEDIA_REPLICATE_QWEN_TTS_ENDPOINT",
                &format!("{base}/replicate/qwen"),
            );
        }
        "replicate_chatterbox" => {
            set_env("REPLICATE_API_TOKEN", "test-replicate-key");
            set_env(
                "TURA_GENERATE_MEDIA_REPLICATE_CHATTERBOX_ENDPOINT",
                &format!("{base}/replicate/chatterbox"),
            );
        }
        other => panic!("unexpected provider {other}"),
    }
}

fn clear_speech_provider_env(provider: &str) {
    match provider {
        "openai_tts" => {
            clear_env("OPENAI_OPENAPI_KEY");
            clear_env("TURA_GENERATE_MEDIA_OPENAI_TTS_ENDPOINT");
        }
        "elevenlabs" => {
            clear_env("ELEVENLABS_API_KEY");
            clear_env("TURA_GENERATE_MEDIA_ELEVENLABS_ENDPOINT");
        }
        "qwen_dashscope" => {
            clear_env("QWEN_API_KEY");
            clear_env("TURA_GENERATE_MEDIA_QWEN_TTS_ENDPOINT");
        }
        "azure_edge_tts" => {
            clear_env("TURA_GENERATE_MEDIA_AZURE_EDGE_TTS_ENDPOINT");
        }
        "azure_speech" => {
            clear_env("AZURE_SPEECH_KEY");
            clear_env("AZURE_SPEECH_REGION");
            clear_env("TURA_GENERATE_MEDIA_AZURE_SPEECH_TOKEN_ENDPOINT");
            clear_env("TURA_GENERATE_MEDIA_AZURE_SPEECH_ENDPOINT");
        }
        "replicate_qwen3_tts" => {
            clear_env("REPLICATE_API_TOKEN");
            clear_env("TURA_GENERATE_MEDIA_REPLICATE_QWEN_TTS_ENDPOINT");
        }
        "replicate_chatterbox" => {
            clear_env("REPLICATE_API_TOKEN");
            clear_env("TURA_GENERATE_MEDIA_REPLICATE_CHATTERBOX_ENDPOINT");
        }
        other => panic!("unexpected provider {other}"),
    }
}

fn set_env(key: &str, value: &str) {
    std::env::set_var(key, value);
}

fn clear_env(key: &str) {
    std::env::remove_var(key);
}

fn restore_env(key: &str, value: Option<String>) {
    match value {
        Some(value) => std::env::set_var(key, value),
        None => std::env::remove_var(key),
    }
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
    write_status_content_type(stream, status, "application/json", body);
}

fn write_status_content_type(
    stream: &mut std::net::TcpStream,
    status: u16,
    content_type: &str,
    body: &[u8],
) {
    let status_text = if status == 200 { "OK" } else { "ERR" };
    let header = format!(
        "HTTP/1.1 {status} {status_text}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
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
