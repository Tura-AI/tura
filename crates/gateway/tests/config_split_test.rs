use axum::body::to_bytes;
use axum::http::{Method, Request, StatusCode};
use serde_json::Value;
use std::fs;
use std::path::PathBuf;
use tokio::sync::Mutex;
use tower::ServiceExt;

static ENV_LOCK: Mutex<()> = Mutex::const_new(());

async fn request(
    method: Method,
    uri: &str,
    content_type: &str,
    body: String,
) -> (StatusCode, String) {
    let request = Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", content_type)
        .body(axum::body::Body::from(body))
        .expect("request");
    let response = gateway::web::build_router()
        .oneshot(request)
        .await
        .expect("response");
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 1024 * 1024)
        .await
        .expect("body");
    (status, String::from_utf8(bytes.to_vec()).expect("utf8"))
}

#[tokio::test]
async fn tura_config_reads_configured_provider_model_options_and_updates_one_tier() {
    let _guard = ENV_LOCK.lock().await;
    let original = std::env::var("TURA_PROVIDER_CONFIG").ok();
    let original_codex_key = std::env::var("OPENAI_API_KEY").ok();
    let original_google_key = std::env::var("GOOGLE_API_KEY").ok();
    let original_google_refresh = std::env::var("GOOGLE_REFRESH_TOKEN").ok();
    let original_claude_code_token = std::env::var("CLAUDE_CODE_OAUTH_TOKEN").ok();
    let original_claude_code_refresh = std::env::var("CLAUDE_CODE_REFRESH_TOKEN").ok();
    let temp_dir = temp_dir("tura-config");
    fs::create_dir_all(&temp_dir).expect("temp dir");
    let config_path = temp_dir.join("provider_config.json");
    fs::write(
        &config_path,
        r#"{
  "routes": {
    "fast": {
      "default_temperature": 0.2,
      "providers": [
        { "provider": "codex", "model": "gpt-5.1-codex-mini" }
      ]
    }
  },
  "model_catalog": {
    "tiers": ["fast"],
    "providers": {
      "codex": {
        "display_name": "Codex",
        "env": ["OPENAI_API_KEY"],
        "models": {
          "fast": [
            { "id": "gpt-5.1-codex-mini", "name": "GPT-5.1 Codex mini" }
          ]
        }
      },
      "google": {
        "display_name": "Google",
        "env": ["GOOGLE_API_KEY"],
        "models": {
          "fast": [
            { "id": "gemini-2.5-flash", "name": "Gemini Flash" }
          ]
        }
      },
      "claude-code": {
        "display_name": "Claude Code",
        "models": {
          "fast": [
            { "id": "claude-sonnet-4-5", "name": "Claude Sonnet 4.5" }
          ]
        }
      }
    }
  }
}"#,
    )
    .expect("config");
    std::env::set_var("TURA_PROVIDER_CONFIG", &config_path);
    std::env::set_var("OPENAI_API_KEY", "configured-codex");
    std::env::set_var("GOOGLE_API_KEY", "");
    std::env::set_var("GOOGLE_REFRESH_TOKEN", "");
    std::env::set_var("CLAUDE_CODE_OAUTH_TOKEN", "");
    std::env::set_var("CLAUDE_CODE_REFRESH_TOKEN", "configured-refresh");

    let (status, body) = request(
        Method::GET,
        "/model_config",
        "application/json",
        String::new(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let value: Value = serde_json::from_str(&body).expect("json");
    assert_eq!(value["path"], config_path.to_string_lossy().to_string());
    assert_eq!(value["tiers"][0]["tier"], "fast");
    assert_eq!(value["tiers"][0]["current"]["provider"], "codex");
    assert_eq!(
        value["tiers"][0]["options"]
            .as_array()
            .expect("options")
            .len(),
        2
    );
    let option_providers = value["tiers"][0]["options"]
        .as_array()
        .expect("options")
        .iter()
        .filter_map(|option| option.get("provider").and_then(Value::as_str))
        .collect::<Vec<_>>();
    assert!(option_providers.contains(&"codex"));
    assert!(option_providers.contains(&"claude-code"));

    let update = serde_json::json!({
        "tier": "fast",
        "provider": "google",
        "model": "gemini-2.5-flash"
    });
    let (status, body) = request(
        Method::PUT,
        "/model_config",
        "application/json",
        update.to_string(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let value: Value = serde_json::from_str(&body).expect("json");
    assert!(value["error"]
        .as_str()
        .expect("unconfigured provider should be rejected")
        .contains("not available"));

    std::env::set_var("GOOGLE_API_KEY", "configured-google");
    std::env::set_var("GOOGLE_REFRESH_TOKEN", "");
    let (status, body) = request(
        Method::PUT,
        "/model_config",
        "application/json",
        update.to_string(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let value: Value = serde_json::from_str(&body).expect("json");
    assert!(value.get("error").is_none());
    assert_eq!(value["tiers"][0]["current"]["provider"], "google");
    assert_eq!(value["tiers"][0]["current"]["model"], "gemini-2.5-flash");

    restore_env("TURA_PROVIDER_CONFIG", original);
    restore_env("OPENAI_API_KEY", original_codex_key);
    restore_env("GOOGLE_API_KEY", original_google_key);
    restore_env("GOOGLE_REFRESH_TOKEN", original_google_refresh);
    restore_env("CLAUDE_CODE_OAUTH_TOKEN", original_claude_code_token);
    restore_env("CLAUDE_CODE_REFRESH_TOKEN", original_claude_code_refresh);
    let _ = fs::remove_dir_all(temp_dir);
}

#[tokio::test]
async fn gui_config_reads_and_writes_exact_text() {
    let _guard = ENV_LOCK.lock().await;
    let config_path = std::env::current_dir()
        .expect("cwd")
        .join("config")
        .join("gui_config.toml");
    let original_content = fs::read_to_string(&config_path).ok();

    let content = "theme = \"dark\"\nraw = true\n";
    let (status, body) = request(
        Method::PUT,
        "/gui_config",
        "text/plain; charset=utf-8",
        content.to_string(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, content);

    let (status, body) = request(Method::GET, "/gui_config", "text/plain", String::new()).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, content);

    if let Some(original_content) = original_content {
        fs::write(&config_path, original_content).expect("restore gui config");
    } else {
        let _ = fs::remove_file(&config_path);
    }
}

fn temp_dir(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "tura-{name}-{}-{}",
        std::process::id(),
        chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default()
    ))
}

fn restore_env(key: &str, value: Option<String>) {
    if let Some(value) = value {
        std::env::set_var(key, value);
    } else {
        std::env::remove_var(key);
    }
}
