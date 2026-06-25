pub(crate) use axum::extract::{Json, Path, Query};
pub(crate) use gateway::api::provider::{
    oauth_callback, oauth_callback_info, provider_auth_logout, provider_auth_refresh,
    provider_auth_validate, set_auth,
};
pub(crate) use gateway::contracts::{
    OAuthCallbackParams, OAuthCallbackPayload, OAuthRedirectCallbackParams, ProviderAuth,
    ProviderAuthActionDetail,
};
pub(crate) use gateway::mock::global_store;
pub(crate) use serde_json::json;
pub(crate) use std::collections::HashMap;
pub(crate) use std::ffi::OsString;
pub(crate) use std::io::{Read, Write};
pub(crate) use std::net::{TcpListener, TcpStream};
pub(crate) use std::path::{Path as FsPath, PathBuf};
pub(crate) use std::process::Command;
pub(crate) use std::thread;

pub(crate) static ENV_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

pub(crate) async fn write_api_auth(provider_id: &str, key: &str) -> bool {
    let token_env = format!("{}_API_KEY", provider_id.to_ascii_uppercase());
    let mut metadata = HashMap::new();
    metadata.insert("login".to_string(), json!("api"));
    metadata.insert("token_env".to_string(), json!(token_env));
    let auth = ProviderAuth {
        auth_type: "api".to_string(),
        key: Some(key.to_string()),
        access: None,
        refresh: None,
        expires: None,
        account_id: None,
        metadata: Some(metadata),
    };
    let Json(saved) = set_auth(Path(provider_id.to_string()), Json(auth)).await;
    saved
}

pub(crate) fn assert_persisted_api_auth(
    env_content: &str,
    provider_auth: &serde_json::Map<String, serde_json::Value>,
    provider_id: &str,
    key: &str,
) {
    let token_env = format!("{}_API_KEY", provider_id.to_ascii_uppercase());
    let login_env = format!("{}_LOGIN", provider_id.to_ascii_uppercase());
    assert!(
        env_content.contains(&format!("{token_env}=\"{key}\"")),
        "env file should keep token for {provider_id}; content:\n{env_content}"
    );
    assert!(
        env_content.contains(&format!("{login_env}=\"api\"")),
        "env file should keep login for {provider_id}; content:\n{env_content}"
    );
    let entry = provider_auth
        .get(provider_id)
        .unwrap_or_else(|| panic!("missing provider auth entry for {provider_id}"));
    assert_eq!(entry["login"], "api");
    assert_eq!(entry["status"], "connected");
    assert_eq!(entry["token_env"], token_env);
    assert_eq!(entry["login_env"], login_env);
}

pub(crate) fn assert_revoked_api_auth(
    env_content: &str,
    provider_auth: &serde_json::Map<String, serde_json::Value>,
    provider_id: &str,
) {
    let token_env = format!("{}_API_KEY", provider_id.to_ascii_uppercase());
    let login_env = format!("{}_LOGIN", provider_id.to_ascii_uppercase());
    assert!(
        env_content.contains(&format!("{token_env}=\"\"")),
        "env file should clear token for logged out {provider_id}; content:\n{env_content}"
    );
    assert!(
        env_content.contains(&format!("{login_env}=\"\"")),
        "env file should clear login for logged out {provider_id}; content:\n{env_content}"
    );
    let entry = provider_auth
        .get(provider_id)
        .unwrap_or_else(|| panic!("missing provider auth entry for logged out {provider_id}"));
    assert_eq!(entry["status"], "revoked");
    assert_eq!(entry["token_env"], token_env);
    assert_eq!(entry["login_env"], login_env);
}

#[derive(Debug)]
pub(crate) struct CapturedTokenRequest {
    pub(crate) method: String,
    pub(crate) path: String,
    pub(crate) form: HashMap<String, String>,
}

pub(crate) struct LocalTokenServer {
    pub(crate) addr: std::net::SocketAddr,
    pub(crate) handle: thread::JoinHandle<CapturedTokenRequest>,
}

impl LocalTokenServer {
    pub(crate) fn start(body: serde_json::Value) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind token server");
        let addr = listener.local_addr().expect("token server addr");
        let handle = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept token request");
            let request = read_token_request(&mut stream);
            let body = body.to_string();
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                body.len(),
                body
            );
            stream
                .write_all(response.as_bytes())
                .expect("write token response");
            request
        });
        Self { addr, handle }
    }

    pub(crate) fn url(&self) -> String {
        format!("http://{}/token", self.addr)
    }

    pub(crate) fn join(self) -> CapturedTokenRequest {
        self.handle.join().expect("token server joins")
    }
}

#[derive(Debug)]
pub(crate) struct CapturedModelRequest {
    pub(crate) method: String,
    pub(crate) path: String,
    pub(crate) headers: String,
}

pub(crate) struct LocalModelServer {
    pub(crate) addr: std::net::SocketAddr,
    pub(crate) handle: thread::JoinHandle<CapturedModelRequest>,
}

impl LocalModelServer {
    pub(crate) fn start(status: u16, body: String) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind model server");
        let addr = listener.local_addr().expect("model server addr");
        let handle = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept model request");
            let request = read_model_request(&mut stream);
            let response = format!(
                "HTTP/1.1 {status} OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                body.len(),
                body
            );
            stream
                .write_all(response.as_bytes())
                .expect("write model response");
            request
        });
        Self { addr, handle }
    }

    pub(crate) fn base_url(&self) -> String {
        format!("http://{}", self.addr)
    }

    pub(crate) fn join(self) -> CapturedModelRequest {
        self.handle.join().expect("model server joins")
    }
}

pub(crate) fn read_model_request(stream: &mut TcpStream) -> CapturedModelRequest {
    let mut buffer = Vec::new();
    let mut temp = [0u8; 4096];
    loop {
        let read = stream.read(&mut temp).expect("read model request");
        if read == 0 {
            break;
        }
        buffer.extend_from_slice(&temp[..read]);
        if let Some(headers_end) = find_headers_end(&buffer) {
            let headers = String::from_utf8_lossy(&buffer[..headers_end]).to_string();
            let request_line = headers.lines().next().unwrap_or_default();
            let mut parts = request_line.split_whitespace();
            return CapturedModelRequest {
                method: parts.next().unwrap_or_default().to_string(),
                path: parts.next().unwrap_or_default().to_string(),
                headers: headers.to_ascii_lowercase(),
            };
        }
    }
    panic!("model request did not contain complete headers")
}

pub(crate) fn read_token_request(stream: &mut TcpStream) -> CapturedTokenRequest {
    let mut buffer = Vec::new();
    let mut temp = [0u8; 4096];
    loop {
        let read = stream.read(&mut temp).expect("read token request");
        if read == 0 {
            break;
        }
        buffer.extend_from_slice(&temp[..read]);
        if let Some(headers_end) = find_headers_end(&buffer) {
            let headers = String::from_utf8_lossy(&buffer[..headers_end]).to_string();
            let content_length = headers
                .lines()
                .find_map(|line| {
                    line.split_once(':').and_then(|(name, value)| {
                        name.eq_ignore_ascii_case("content-length")
                            .then(|| value.trim().parse::<usize>().ok())
                            .flatten()
                    })
                })
                .unwrap_or(0);
            let body_start = headers_end + 4;
            while buffer.len() < body_start + content_length {
                let read = stream.read(&mut temp).expect("read token body");
                if read == 0 {
                    break;
                }
                buffer.extend_from_slice(&temp[..read]);
            }
            let request_line = headers.lines().next().unwrap_or_default();
            let mut parts = request_line.split_whitespace();
            let method = parts.next().unwrap_or_default().to_string();
            let path = parts.next().unwrap_or_default().to_string();
            let body = String::from_utf8_lossy(&buffer[body_start..body_start + content_length]);
            return CapturedTokenRequest {
                method,
                path,
                form: parse_form_body(&body),
            };
        }
    }
    panic!("token request did not contain complete headers")
}

pub(crate) fn find_headers_end(buffer: &[u8]) -> Option<usize> {
    buffer.windows(4).position(|window| window == b"\r\n\r\n")
}

pub(crate) fn parse_form_body(body: &str) -> HashMap<String, String> {
    body.split('&')
        .filter_map(|pair| pair.split_once('='))
        .map(|(key, value)| (percent_decode(key), percent_decode(value)))
        .collect()
}

pub(crate) fn percent_decode(value: &str) -> String {
    let mut out = Vec::new();
    let bytes = value.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'+' => {
                out.push(b' ');
                index += 1;
            }
            b'%' if index + 2 < bytes.len() => {
                if let Ok(hex) = u8::from_str_radix(&value[index + 1..index + 3], 16) {
                    out.push(hex);
                    index += 3;
                } else {
                    out.push(bytes[index]);
                    index += 1;
                }
            }
            byte => {
                out.push(byte);
                index += 1;
            }
        }
    }
    String::from_utf8(out).expect("form value utf8")
}

pub(crate) fn assert_detail(details: &[ProviderAuthActionDetail], code: &str) {
    assert!(
        details.iter().any(|detail| detail.code == code),
        "missing detail {code}; got {:?}",
        details
            .iter()
            .map(|detail| detail.code.as_str())
            .collect::<Vec<_>>()
    );
}

pub(crate) fn assert_detail_value(
    details: &[ProviderAuthActionDetail],
    code: &str,
    expected_value: &str,
) {
    let detail = details
        .iter()
        .find(|detail| detail.code == code)
        .unwrap_or_else(|| panic!("missing detail {code}"));
    assert_eq!(detail.value.as_deref(), Some(expected_value));
}

pub(crate) fn copy_provider_config(path: &FsPath) {
    std::fs::copy(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("workspace crates dir")
            .join("provider")
            .join("config")
            .join("provider_config.json"),
        path,
    )
    .expect("copy provider config");
}

pub(crate) fn write_local_provider_config(path: &FsPath, base_url: &str) {
    let config = json!({
        "provider_base_url": {},
        "routes": {},
        "provider_auth": {},
        "model_catalog": {
            "providers": {
                "business-local": {
                    "display_name": "Business Local Provider",
                    "runtime_provider": "openai",
                    "api_style": "openapi",
                    "base_url": base_url,
                    "token_env": "BUSINESS_LOCAL_API_KEY",
                    "env": ["BUSINESS_LOCAL_API_KEY"],
                    "domains": ["llm"],
                    "models": {
                        "fast": ["business-local-model"]
                    }
                }
            }
        }
    });
    std::fs::write(
        path,
        format!(
            "{}\n",
            serde_json::to_string_pretty(&config).expect("provider config json")
        ),
    )
    .expect("write local provider config");
}

pub(crate) struct EnvGuard {
    previous: Vec<(&'static str, Option<std::ffi::OsString>)>,
}

impl EnvGuard {
    pub(crate) fn new(values: &[(&'static str, Option<String>)]) -> Self {
        let previous = values
            .iter()
            .map(|(key, _)| (*key, std::env::var_os(key)))
            .collect::<Vec<_>>();
        for (key, value) in values {
            match value {
                Some(value) => std::env::set_var(key, value),
                None => std::env::remove_var(key),
            }
        }
        Self { previous }
    }
}

pub(crate) struct DynamicEnvGuard {
    previous: Vec<(String, Option<OsString>)>,
}

impl DynamicEnvGuard {
    pub(crate) fn capture(keys: Vec<String>) -> Self {
        let previous = keys
            .into_iter()
            .map(|key| {
                let value = std::env::var_os(&key);
                std::env::remove_var(&key);
                (key, value)
            })
            .collect();
        Self { previous }
    }
}

impl Drop for DynamicEnvGuard {
    fn drop(&mut self) {
        for (key, value) in self.previous.drain(..) {
            match value {
                Some(value) => std::env::set_var(key, value),
                None => std::env::remove_var(key),
            }
        }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        for (key, value) in self.previous.drain(..) {
            match value {
                Some(value) => std::env::set_var(key, value),
                None => std::env::remove_var(key),
            }
        }
    }
}
