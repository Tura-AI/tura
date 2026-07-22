use super::helpers::*;

struct EnvRestore(Vec<(&'static str, Option<std::ffi::OsString>)>);

impl EnvRestore {
    fn capture(keys: &[&'static str]) -> Self {
        Self(
            keys.iter()
                .map(|key| (*key, std::env::var_os(key)))
                .collect(),
        )
    }
}

impl Drop for EnvRestore {
    fn drop(&mut self) {
        for (key, value) in self.0.drain(..) {
            if let Some(value) = value {
                std::env::set_var(key, value);
            } else {
                std::env::remove_var(key);
            }
        }
    }
}

#[tokio::test]
async fn codex_oauth_uses_rotated_local_auth_across_two_calls() {
    let _lock = ENV_LOCK.lock().await;
    let _restore = EnvRestore::capture(&[
        "CODEX_HOME",
        "TURA_ENV_PATH",
        "OPENAI_CODEX_ENDPOINT",
        "OPENAI_LOGIN",
        "OPENAI_API_KEY",
        "OPENAI_REFRESH_TOKEN",
        "OPENAI_TOKEN_EXPIRES",
        "OPENAI_ACCOUNT_ID",
    ]);
    let root = tempfile::tempdir().expect("create e2e temp dir");
    let codex_home = root.path().join("codex-home");
    std::fs::create_dir_all(&codex_home).expect("create codex home");
    std::fs::write(
        codex_home.join("auth.json"),
        r#"{
            "tokens": {
                "access_token": "fresh-codex-access",
                "refresh_token": "fresh-codex-refresh",
                "account_id": "fresh-account"
            }
        }"#,
    )
    .expect("write codex auth");
    let env_path = root.path().join("tura.env");
    std::fs::write(
        &env_path,
        concat!(
            "OPENAI_LOGIN=oauth\n",
            "OPENAI_API_KEY=stale-env-access\n",
            "OPENAI_REFRESH_TOKEN=stale-env-refresh\n",
            "OPENAI_TOKEN_EXPIRES=4102444800000\n",
        ),
    )
    .expect("write tura env");

    let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock Codex server");
    let endpoint = format!(
        "http://{}/backend-api/codex/responses",
        listener.local_addr().expect("mock server address")
    );
    let server = thread::spawn(move || {
        let mut requests = Vec::new();
        for _ in 0..2 {
            let (mut stream, _) = listener.accept().expect("accept Codex request");
            requests.push(read_http_request(&mut stream));
            let body = concat!(
                "data: {\"type\":\"response.output_text.delta\",\"delta\":\"ok\"}\n\n",
                "data: {\"type\":\"response.completed\",\"response\":{\"output_text\":\"ok\",\"usage\":{\"input_tokens\":1,\"output_tokens\":1,\"total_tokens\":2}}}\n\n",
                "data: [DONE]\n\n",
            );
            write!(
                stream,
                "HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                body.len(),
                body
            )
            .expect("write Codex response");
        }
        requests
    });

    std::env::set_var("CODEX_HOME", &codex_home);
    std::env::set_var("TURA_ENV_PATH", &env_path);
    std::env::set_var("OPENAI_CODEX_ENDPOINT", &endpoint);
    for key in [
        "OPENAI_LOGIN",
        "OPENAI_API_KEY",
        "OPENAI_REFRESH_TOKEN",
        "OPENAI_TOKEN_EXPIRES",
        "OPENAI_ACCOUNT_ID",
    ] {
        std::env::remove_var(key);
    }

    let config = TuraConfig::new(".env.missing");
    let provider = ProviderConfig {
        provider: "codex".to_string(),
        model: "gpt-5.6-sol".to_string(),
        base_url: "https://chatgpt.com/backend-api/codex/responses".to_string(),
        temperature: 0.0,
    };
    let system_messages =
        vec![json!({"role": "user", "content": "Report the current system version"})];
    let time_messages = vec![json!({"role": "user", "content": "Report the current system time"})];

    // Two calls cover the stale-first-call boundary seen in resumed tool sessions.
    provider
        .call(&config, system_messages, CallOptions::default())
        .await
        .expect("first Codex call");
    provider
        .call(&config, time_messages, CallOptions::default())
        .await
        .expect("second Codex call");

    let requests = server.join().expect("join mock Codex server");
    assert_eq!(requests.len(), 2);
    assert_eq!(
        requests[0].body["input"][0]["content"][0]["text"],
        "Report the current system version"
    );
    assert_eq!(
        requests[1].body["input"][0]["content"][0]["text"],
        "Report the current system time"
    );
    for request in requests {
        assert!(
            request
                .headers
                .to_ascii_lowercase()
                .contains("authorization: bearer fresh-codex-access\r\n"),
            "request did not use the rotated Codex access token: {}",
            request.headers
        );
        assert!(
            request
                .headers
                .to_ascii_lowercase()
                .contains("chatgpt-account-id: fresh-account\r\n"),
            "request did not propagate the Codex account id: {}",
            request.headers
        );
    }
}
