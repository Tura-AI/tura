use axum::extract::{Json, Path};
use gateway::api::provider::{provider_auth_logout, set_auth};
use gateway::api::types::ProviderAuth;
use serde_json::json;
use std::collections::HashMap;
use std::ffi::OsString;
use std::path::Path as FsPath;
use std::process::Command;

const CHILD_PROVIDER_ENV: &str = "TURA_PROVIDER_AUTH_CHURN_CHILD_PROVIDER";
const CHILD_KEY_ENV: &str = "TURA_PROVIDER_AUTH_CHURN_CHILD_KEY";
const CHILD_ROUNDS_ENV: &str = "TURA_PROVIDER_AUTH_CHURN_CHILD_ROUNDS";

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn provider_auth_performance_churn_keeps_shared_files_consistent() {
    if let Ok(provider_id) = std::env::var(CHILD_PROVIDER_ENV) {
        let key = std::env::var(CHILD_KEY_ENV).expect("child key");
        let rounds = std::env::var(CHILD_ROUNDS_ENV)
            .ok()
            .and_then(|value| value.parse::<usize>().ok())
            .unwrap_or(10);
        for round in 0..rounds {
            assert!(
                write_api_auth(&provider_id, &format!("{key}-round-{round}")).await,
                "churn write should persist"
            );
            let Json(response) = provider_auth_logout(Path(provider_id.clone())).await;
            assert!(
                response.ok,
                "churn logout should persist: {}",
                response.message
            );
        }
        assert!(
            write_api_auth(&provider_id, &key).await,
            "final churn write should persist"
        );
        return;
    }

    let root = tempfile::tempdir().expect("temp auth churn root");
    let env_path = root.path().join(".env.gateway-auth-churn-performance");
    let provider_config = root.path().join("provider_config.json");
    write_provider_config(&provider_config);
    let provider_count = 10usize;
    let rounds = 10usize;
    let provider_ids = (0..provider_count)
        .map(|index| format!("performance_churn_{index}"))
        .collect::<Vec<_>>();
    let dynamic_env_keys = provider_ids
        .iter()
        .flat_map(|provider_id| {
            [
                format!("{}_API_KEY", provider_id.to_ascii_uppercase()),
                format!("{}_LOGIN", provider_id.to_ascii_uppercase()),
            ]
        })
        .collect::<Vec<_>>();
    let _env = EnvGuard::new(&[
        (
            "TURA_ENV_PATH",
            Some(env_path.to_string_lossy().to_string()),
        ),
        (
            "TURA_PROVIDER_CONFIG",
            Some(provider_config.to_string_lossy().to_string()),
        ),
    ]);
    let _dynamic_env = DynamicEnvGuard::capture(dynamic_env_keys);

    let current_exe = std::env::current_exe().expect("current test exe");
    let mut children = Vec::new();
    for (index, provider_id) in provider_ids.iter().enumerate() {
        children.push((
            provider_id.clone(),
            format!("performance-churn-final-key-{index}"),
            Command::new(&current_exe)
                .arg("--exact")
                .arg("provider_auth_performance_churn_keeps_shared_files_consistent")
                .arg("--nocapture")
                .arg("--test-threads=1")
                .env("TURA_ENV_PATH", &env_path)
                .env("TURA_PROVIDER_CONFIG", &provider_config)
                .env(CHILD_PROVIDER_ENV, provider_id)
                .env(
                    CHILD_KEY_ENV,
                    format!("performance-churn-final-key-{index}"),
                )
                .env(CHILD_ROUNDS_ENV, rounds.to_string())
                .spawn()
                .expect("spawn auth churn child"),
        ));
    }
    for (provider_id, _key, mut child) in children {
        let status = child.wait().expect("wait auth churn child");
        assert!(
            status.success(),
            "auth churn child for {provider_id} should exit successfully: {status}"
        );
    }

    let env_content = std::fs::read_to_string(&env_path).expect("churn env file");
    let provider_config: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(&provider_config).expect("churn provider config"),
    )
    .expect("provider config remains valid JSON");
    let provider_auth = provider_config["provider_auth"]
        .as_object()
        .expect("provider_auth object");

    for (index, provider_id) in provider_ids.iter().enumerate() {
        assert_persisted_api_auth(
            &env_content,
            provider_auth,
            provider_id,
            &format!("performance-churn-final-key-{index}"),
        );
    }
}

async fn write_api_auth(provider_id: &str, key: &str) -> bool {
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

fn assert_persisted_api_auth(
    env_content: &str,
    provider_auth: &serde_json::Map<String, serde_json::Value>,
    provider_id: &str,
    key: &str,
) {
    let token_env = format!("{}_API_KEY", provider_id.to_ascii_uppercase());
    let login_env = format!("{}_LOGIN", provider_id.to_ascii_uppercase());
    assert_eq!(
        line_count_with_prefix(env_content, &format!("{token_env}=")),
        1,
        "env file should contain exactly one token line for {provider_id}:\n{env_content}"
    );
    assert_eq!(
        line_count_with_prefix(env_content, &format!("{login_env}=")),
        1,
        "env file should contain exactly one login line for {provider_id}:\n{env_content}"
    );
    assert!(
        env_content.contains(&format!("{token_env}=\"{key}\"")),
        "env file should keep final token for {provider_id}; content:\n{env_content}"
    );
    assert!(
        env_content.contains(&format!("{login_env}=\"api\"")),
        "env file should keep final login for {provider_id}; content:\n{env_content}"
    );
    let entry = provider_auth
        .get(provider_id)
        .unwrap_or_else(|| panic!("missing provider auth entry for {provider_id}"));
    assert_eq!(entry["login"], "api");
    assert_eq!(entry["status"], "connected");
    assert_eq!(entry["token_env"], token_env);
    assert_eq!(entry["login_env"], login_env);
}

fn line_count_with_prefix(content: &str, prefix: &str) -> usize {
    content
        .lines()
        .filter(|line| line.trim_start().starts_with(prefix))
        .count()
}

fn write_provider_config(path: &FsPath) {
    let config = json!({
        "provider_base_url": {},
        "routes": {},
        "provider_auth": {},
        "model_catalog": {
            "providers": {}
        }
    });
    std::fs::write(
        path,
        format!(
            "{}\n",
            serde_json::to_string_pretty(&config).expect("provider config json")
        ),
    )
    .expect("write provider config");
}

struct EnvGuard {
    previous: Vec<(&'static str, Option<OsString>)>,
}

impl EnvGuard {
    fn new(values: &[(&'static str, Option<String>)]) -> Self {
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

struct DynamicEnvGuard {
    previous: Vec<(String, Option<OsString>)>,
}

impl DynamicEnvGuard {
    fn capture(keys: Vec<String>) -> Self {
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
