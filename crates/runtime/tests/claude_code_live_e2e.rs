//! Live end-to-end check that drives the gateway's session engine through the
//! dedicated `claude-code` compat layer against the real Anthropic Messages API.
//!
//! This exercises the full runtime path a gateway session takes — route
//! resolution, the coding agent's state machine, tool calling, and tool-result
//! ingestion — using `mano::process_from_gateway_session_in_directory` (the same
//! entry point the gateway uses; no HTTP/redis needed for the engine itself).
//!
//! It talks to the real API and is therefore gated:
//!
//! ```text
//! TURA_CLAUDE_CODE_E2E=1 cargo test -p runtime --test claude_code_live_e2e -- --nocapture
//! ```
//!
//! Credentials are read from the process env first, then from the project root
//! `.env`. The OAuth subscription token (`CLAUDE_CODE_OAUTH_TOKEN`) is
//! preferred; the provider layer auto-detects the route from the token prefix.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use runtime::mano;
use runtime::state_machine::session_management::{SessionInput, SessionState};
use serde_json::{json, Value};

const ROUTES: &[&str] = &[
    "flagship_thinking",
    "thinking",
    "fast",
    "instant",
    "embedding_high",
    "embedding_low",
];
const BASE_URL: &str = "https://api.anthropic.com/v1";
const MODEL: &str = "claude-opus-4-8";

static ENV_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn claude_code_gateway_session_tool_calling_e2e() {
    if std::env::var("TURA_CLAUDE_CODE_E2E").ok().as_deref() != Some("1") {
        eprintln!("skipping claude-code live e2e; set TURA_CLAUDE_CODE_E2E=1");
        return;
    }

    let Some((token_env, token)) = resolve_credential() else {
        eprintln!(
            "SKIP: no CLAUDE_CODE_OAUTH_TOKEN or ANTHROPIC_API_KEY in env or project root .env"
        );
        return;
    };
    eprintln!("claude-code live e2e using credential {token_env}");

    let _lock = ENV_LOCK.lock().expect("e2e env lock should be available");
    let workspace = create_rust_workspace();
    let llm_config = write_llm_config(&workspace);
    let _env = EnvGuard::set(&[
        ("TURALLM_CONFIG", llm_config.to_string_lossy().as_ref()),
        (token_env, token.as_str()),
        ("ANTHROPIC_LOGIN", "oauth"),
        ("TURA_DISABLE_GATEWAY_CALLBACKS", "1"),
        ("TURA_MANAS_MAX_TURNS", "6"),
    ]);

    let result = mano::process_from_gateway_session_in_directory(
        "claude-code-e2e-tool-calling".to_string(),
        SessionInput {
            user_input: "Use the command_run tool to run `pwd` (a shell_command), then reply with a short normal assistant message confirming you are done."
                .to_string(),
            file_input: vec![],
            agent: None,
            runtime_context: None,
                planning_mode_override: None,
        },
        workspace.clone(),
    )
    .expect("claude-code gateway session should complete");

    eprintln!("final session state: {:?}", result.session.state);
    assert_eq!(result.agents.len(), 1);
    assert_eq!(result.agents[0].agent_name, "thinking-planning");
    assert_eq!(
        result.session.state,
        SessionState::Completed,
        "session should reach Completed; log={:#?}",
        result.session.session_log
    );

    let tool_results = tool_results(&result.session.session_log);
    assert!(
        !tool_results.is_empty(),
        "expected at least one tool result; session_log={:#?}",
        result.session.session_log
    );
    assert_tool_success(&tool_results, "command_run");
}

/// Prefer the OAuth subscription token, then the API key. Read from the process
/// env first, then fall back to the project root `.env`.
fn resolve_credential() -> Option<(&'static str, String)> {
    for key in ["CLAUDE_CODE_OAUTH_TOKEN", "ANTHROPIC_API_KEY"] {
        if let Some(value) = env_or_dotenv(key) {
            if !value.trim().is_empty() {
                return Some((key, value));
            }
        }
    }
    None
}

fn env_or_dotenv(key: &str) -> Option<String> {
    if let Ok(value) = std::env::var(key) {
        return Some(value);
    }
    let dotenv = root_dotenv_path();
    let contents = std::fs::read_to_string(dotenv).ok()?;
    for line in contents.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((name, value)) = line.split_once('=') {
            if name.trim() == key {
                let value = value.trim().trim_matches('"').trim_matches('\'');
                return Some(value.to_string());
            }
        }
    }
    None
}

fn root_dotenv_path() -> PathBuf {
    // This test crate lives under crates/runtime; the project root is two levels up.
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join(".env")
}

fn write_llm_config(workspace: &Path) -> PathBuf {
    let mut routes = serde_json::Map::new();
    for route in ROUTES {
        routes.insert(
            (*route).to_string(),
            json!({
                "default_temperature": 0.0,
                "providers": [{
                    "provider": "claude-code",
                    "model": MODEL,
                    "temperature": 0.0
                }]
            }),
        );
    }
    let config = json!({
        "provider_base_url": {
            "claude-code": BASE_URL,
            "anthropic": BASE_URL
        },
        "routes": routes
    });
    let path = workspace.join("provider_config.json");
    std::fs::write(
        &path,
        serde_json::to_string_pretty(&config).expect("config should serialize"),
    )
    .expect("provider_config.json should be written");
    path
}

fn create_rust_workspace() -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "tura-claude-code-e2e-{}",
        chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default()
    ));
    let src = root.join("src");
    std::fs::create_dir_all(&src).expect("test workspace src should be created");
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"tura-claude-code-e2e\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .expect("Cargo.toml should be written");
    std::fs::write(
        src.join("lib.rs"),
        "pub fn run() -> String {\n    \"demo\".to_string()\n}\n",
    )
    .expect("lib.rs should be written");
    root
}

fn tool_results(log: &[String]) -> Vec<Value> {
    log.iter()
        .filter_map(|entry| serde_json::from_str::<Value>(entry).ok())
        .filter(|value| value.get("type").and_then(Value::as_str) == Some("tool_result"))
        .collect()
}

fn assert_tool_success(tool_results: &[Value], tool_name: &str) {
    let result = tool_results
        .iter()
        .find(|result| result.get("tool_name").and_then(Value::as_str) == Some(tool_name))
        .unwrap_or_else(|| panic!("missing tool result for {tool_name}; saw {tool_results:#?}"));
    assert_eq!(
        result.get("success").and_then(Value::as_bool),
        Some(true),
        "tool {tool_name} should succeed: {result}"
    );
}

struct EnvGuard {
    previous: Vec<(String, Option<String>)>,
}

impl EnvGuard {
    fn set(values: &[(&str, &str)]) -> Self {
        let previous = values
            .iter()
            .map(|(key, _)| ((*key).to_string(), std::env::var(key).ok()))
            .collect::<Vec<_>>();
        for (key, value) in values {
            std::env::set_var(key, value);
        }
        Self { previous }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        for (key, value) in &self.previous {
            if let Some(value) = value {
                std::env::set_var(key, value);
            } else {
                std::env::remove_var(key);
            }
        }
    }
}
