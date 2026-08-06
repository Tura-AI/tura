use std::collections::HashMap;
use std::ffi::{OsStr, OsString};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::Path;
use std::sync::{Mutex, OnceLock};
use std::thread;

use serde_json::{Value, json};
use tura_llm_rust::{
    CallOptions, ModelCatalog, ProviderEnumCatalog, RootConfig, Settings, TuraConfig,
    configured_token_envs, load_settings, merge_projection, project_catalog,
};

const FIXTURE: &[u8] = include_bytes!("fixtures/models_dev_minimal.json");

fn env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

struct EnvRestore {
    key: &'static str,
    value: Option<OsString>,
}

impl EnvRestore {
    fn set(key: &'static str, value: impl AsRef<OsStr>) -> Self {
        let previous = std::env::var_os(key);
        unsafe { std::env::set_var(key, value) };
        Self {
            key,
            value: previous,
        }
    }
}

impl Drop for EnvRestore {
    fn drop(&mut self) {
        match &self.value {
            Some(value) => unsafe { std::env::set_var(self.key, value) },
            None => unsafe { std::env::remove_var(self.key) },
        }
    }
}

#[test]
fn projection_imports_only_safe_openai_compatible_models_with_provenance() {
    let projection = project_catalog(FIXTURE, "fixture/api.json").expect("project fixture");

    for (provider, base_url) in [
        ("opencode", "https://opencode.ai/zen/v1"),
        ("opencode-go", "https://opencode.ai/zen/go/v1"),
        ("cline-pass", "https://api.cline.bot/api/v1"),
    ] {
        assert_eq!(
            projection
                .provider_base_url
                .get(provider)
                .map(String::as_str),
            Some(base_url)
        );
    }
    assert!(!projection.providers.contains_key("native-provider"));
    let opencode = &projection.providers["opencode"];
    assert_eq!(opencode.env, vec!["OPENCODE_API_KEY"]);
    assert_eq!(
        projection.providers["opencode-go"].env,
        vec!["OPENCODE_API_KEY"]
    );
    assert_eq!(
        projection.providers["cline-pass"].env,
        vec!["CLINE_API_KEY"]
    );
    let opencode_models: Vec<_> = opencode.models.values().flatten().collect();
    assert!(opencode_models.iter().any(|model| model.id() == "glm-5"));
    assert!(
        !opencode_models
            .iter()
            .any(|model| model.id() == "gpt-native")
    );
    let custom_models: Vec<_> = projection.providers["custom-compatible"]
        .models
        .values()
        .flatten()
        .collect();
    assert!(
        custom_models
            .iter()
            .any(|model| model.id() == "custom-chat")
    );
    assert!(
        !custom_models
            .iter()
            .any(|model| model.id() == "audio-only-invalid")
    );
    assert!(
        !custom_models
            .iter()
            .any(|model| model.id() == "transcription-only-invalid")
    );
    for unsupported in [
        "incomplete-chat-limits",
        "embedding-only",
        "nvidia/nv-embedcode-7b-v1",
    ] {
        assert!(!custom_models.iter().any(|model| model.id() == unsupported));
    }

    let cline = &projection.providers["cline-pass"];
    let detail = cline
        .models
        .values()
        .flatten()
        .find(|model| model.id() == "cline-pass/deepseek-v4-flash")
        .and_then(|model| model.detail())
        .expect("project ClinePass detail");
    assert_eq!(detail.limit.context, 1_000_000);
    assert_eq!(detail.limit.input, 1_000_000);
    assert_eq!(detail.limit.output, 384_000);
    assert_eq!(detail.modalities.input, vec!["text"]);
    assert!(detail.reasoning);
    assert!(detail.tool_call);
    assert_eq!(
        detail.options["models_dev_source"]["sha256"],
        projection.provenance.sha256
    );
    assert_eq!(projection.provenance.sha256.len(), 64);
    assert_eq!(
        projection.provenance.revision,
        format!("sha256:{}", projection.provenance.sha256)
    );
}

#[test]
fn projection_fails_closed_on_schema_drift_and_ambiguous_ids() {
    let mut missing_api: Value = serde_json::from_slice(FIXTURE).expect("fixture JSON");
    missing_api["opencode"]
        .as_object_mut()
        .expect("provider object")
        .remove("api");
    let error = project_catalog(
        &serde_json::to_vec(&missing_api).expect("serialize mutation"),
        "missing-api.json",
    )
    .expect_err("missing compatible API must fail");
    assert!(error.to_string().contains("has no API URL"));

    let mut mismatched_model: Value = serde_json::from_slice(FIXTURE).expect("fixture JSON");
    mismatched_model["opencode"]["models"]["glm-5"]["id"] = json!("other-id");
    let error = project_catalog(
        &serde_json::to_vec(&mismatched_model).expect("serialize mutation"),
        "mismatch.json",
    )
    .expect_err("model key mismatch must fail");
    assert!(error.to_string().contains("does not match object key"));

    let provider = serde_json::to_string(&missing_api["opencode"]).expect("provider JSON");
    let duplicate = format!(r#"{{"duplicate":{provider},"duplicate":{provider}}}"#);
    let error =
        project_catalog(duplicate.as_bytes(), "duplicate.json").expect_err("duplicate must fail");
    assert!(error.to_string().contains("duplicate object key"));
}

#[test]
fn merge_preserves_tura_owned_collisions_and_adds_non_colliding_providers() {
    let projection = project_catalog(FIXTURE, "fixture/api.json").expect("project fixture");
    let mut config = RootConfig {
        provider_base_url: HashMap::from([(
            "OpenCode".to_string(),
            "https://tura-owned.invalid/v1".to_string(),
        )]),
        routes: HashMap::new(),
        model_catalog: ModelCatalog::default(),
        provider_enums: ProviderEnumCatalog::default(),
        provider_auth: HashMap::new(),
        provider_latency: Default::default(),
    };

    merge_projection(&mut config, projection).expect("merge non-colliding providers");
    assert_eq!(
        config.provider_base_url["OpenCode"],
        "https://tura-owned.invalid/v1"
    );
    assert!(!config.provider_base_url.contains_key("opencode"));
    assert!(!config.model_catalog.providers.contains_key("opencode"));
    assert_eq!(
        config.provider_base_url["opencode-go"],
        "https://opencode.ai/zen/go/v1"
    );
    assert!(config.model_catalog.providers.contains_key("opencode-go"));
    assert!(config.model_catalog.providers.contains_key("cline-pass"));
}

#[tokio::test(flavor = "current_thread")]
async fn opt_in_load_preserves_routes_and_uses_custom_env_and_prefixed_model_ids() {
    let _guard = env_lock().lock().expect("env lock");
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind local provider");
    let addr = listener.local_addr().expect("local provider address");
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept local request");
        let mut request = Vec::new();
        let mut buffer = [0_u8; 4096];
        loop {
            let count = stream.read(&mut buffer).expect("read request");
            if count == 0 {
                break;
            }
            request.extend_from_slice(&buffer[..count]);
            if request.windows(4).any(|window| window == b"\r\n\r\n") {
                break;
            }
        }
        let body = json!({
            "id": "chatcmpl-models-dev",
            "choices": [{"message": {"role": "assistant", "content": "ok"}, "finish_reason": "stop"}]
        })
        .to_string();
        write!(
            stream,
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
            body.len(),
            body
        )
        .expect("write response");
        String::from_utf8(request).expect("request is UTF-8")
    });

    let temp = tempfile::tempdir().expect("tempdir");
    let mut imported: Value = serde_json::from_slice(FIXTURE).expect("fixture JSON");
    imported["custom-compatible"]["api"] = json!(format!("http://{addr}"));
    let models_path = temp.path().join("models-dev.json");
    std::fs::write(
        &models_path,
        serde_json::to_vec(&imported).expect("serialize import"),
    )
    .expect("write import");
    let config_path = temp.path().join("provider-config.json");
    write_root_config(&config_path);
    let env_path = temp.path().join(".env");
    std::fs::write(&env_path, "CUSTOM_SECONDARY_KEY=fixture-secret\n").expect("write env");

    let _provider_config = EnvRestore::set("TURA_PROVIDER_CONFIG", &config_path);
    let _models_catalog = EnvRestore::set("TURA_MODELS_DEV_CATALOG", &models_path);
    let settings = load_settings().await.expect("load merged settings");

    assert_eq!(settings.routes.len(), 1);
    assert_eq!(settings.model_catalog.tiers, vec!["thinking"]);
    assert_eq!(
        settings.provider_base_url("opencode-go").as_deref(),
        Some("https://opencode.ai/zen/go/v1")
    );
    assert_eq!(
        configured_token_envs("custom-compatible"),
        vec!["CUSTOM_PRIMARY_KEY", "CUSTOM_SECONDARY_KEY"]
    );
    assert_eq!(
        Settings::normalize_model_name("cline-pass", "cline-pass/deepseek-v4-flash"),
        "cline-pass/deepseek-v4-flash"
    );

    let mut provider = Settings::make_provider(
        &settings.provider_base_url,
        "custom-compatible",
        "custom-chat",
        None,
        0.2,
    )
    .expect("select imported provider");
    provider.base_url = format!("http://{addr}");
    provider
        .call(
            &TuraConfig::new(env_path.to_str().expect("env path")),
            vec![json!({"role": "user", "content": "hello"})],
            CallOptions::default(),
        )
        .await
        .expect("call imported compatible provider");

    let request = server.join().expect("server joins");
    assert!(request.starts_with("POST /chat/completions "));
    assert!(
        request
            .to_ascii_lowercase()
            .contains("authorization: bearer fixture-secret")
    );
}

fn write_root_config(path: &Path) {
    std::fs::write(
        path,
        json!({
            "provider_base_url": {"baseline": "https://baseline.invalid/v1"},
            "routes": {
                "thinking": {
                    "default_temperature": 0.2,
                    "providers": [{"provider": "baseline", "model": "baseline-model"}]
                }
            },
            "model_catalog": {"tiers": ["thinking"], "providers": {}}
        })
        .to_string(),
    )
    .expect("write root config");
}
