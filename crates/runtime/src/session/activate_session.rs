use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use chrono::Utc;
use serde_json::json;

use crate::state_machine::session_management::{SessionInput, SessionManagement};

const DEFAULT_SESSION_DIRECTORY: &str = "test_session";
const CODING_SESSION_TOPIC: &str = "coding";
const LSP_SCAN_THRESHOLD: usize = 2;

#[derive(Clone, Debug, serde::Serialize)]
struct CodeScanCount {
    suffix: String,
    count: usize,
}

#[derive(Clone, Debug, serde::Serialize)]
struct CodeScanResult {
    ok: bool,
    counts: Vec<CodeScanCount>,
    errors: Vec<String>,
}

pub fn activate_session(input: SessionInput) -> Result<SessionManagement, String> {
    let session_directory = std::env::current_dir()
        .map_err(|err| format!("failed to resolve project directory: {err}"))?
        .join(DEFAULT_SESSION_DIRECTORY);

    activate_session_with_topic(session_directory, "general", input)
}

pub fn activate_session_with_topic(
    session_directory: PathBuf,
    session_topic: impl Into<String>,
    input: SessionInput,
) -> Result<SessionManagement, String> {
    let session_topic = session_topic.into();
    let mut session = create_session_for_topic(session_directory, session_topic.clone(), input)?;
    session.use_last_tool_call_response = session_topic != CODING_SESSION_TOPIC;

    if session_topic == CODING_SESSION_TOPIC {
        run_coding_session_lsp_scan(&mut session)?;
    }

    Ok(session)
}

fn create_session_for_topic(
    session_directory: PathBuf,
    session_topic: String,
    input: SessionInput,
) -> Result<SessionManagement, String> {
    let now = Utc::now();
    let session_id = format!(
        "session-{:x}",
        Utc::now().timestamp_nanos_opt().unwrap_or_default()
    );
    let session_name = format!("temp-session-{}", now.format("%Y%m%d%H%M%S"));
    let user_goal = input.user_input.clone();

    let mut session = SessionManagement::new(
        session_id,
        session_name,
        session_directory,
        false,
        session_topic,
        input,
        user_goal,
        now,
    );
    session.is_child_session = std::env::var("TURA_PARENT_SESSION_ID")
        .ok()
        .is_some_and(|value| !value.trim().is_empty());
    Ok(session)
}

fn run_coding_session_lsp_scan(session: &mut SessionManagement) -> Result<(), String> {
    let project_directory = project_directory_with_lsp_service()?;
    let session_directory = absolute_session_path(&session.session_directory);
    let scan_result = scan_code_suffix_counts(&session_directory, lsp_supported_file_suffixes());

    session.push_log(
        json!({
            "type": "code_scan",
            "scan": scan_result,
        })
        .to_string(),
        Utc::now(),
    );

    if !scan_result.ok {
        return Err(format!(
            "coding session code scan failed: {}",
            scan_result.errors.join("; ")
        ));
    }

    let languages = languages_over_threshold(&scan_result);
    if languages.is_empty() {
        return Ok(());
    }

    match call_router_lsp_scan(&project_directory, &session_directory, &languages) {
        Ok(response) => {
            session.push_log(
                json!({
                    "type": "lsp_scan",
                    "languages": languages,
                    "response": response,
                })
                .to_string(),
                Utc::now(),
            );
        }
        Err(err) => {
            tracing::warn!(
                session_id = %session.session_id,
                error = %err,
                "coding session LSP scan failed; continuing without LSP"
            );
            session.push_log(
                json!({
                    "type": "lsp_scan",
                    "languages": languages,
                    "ok": false,
                    "error": err,
                })
                .to_string(),
                Utc::now(),
            );
        }
    }

    Ok(())
}

fn lsp_supported_file_suffixes() -> &'static [&'static str] {
    &[
        "py", "ts", "tsx", "js", "jsx", "mjs", "cjs", "go", "java", "rs",
    ]
}

fn scan_code_suffix_counts(directory: &Path, suffixes: &[&str]) -> CodeScanResult {
    let mut counts = suffixes
        .iter()
        .map(|suffix| ((*suffix).to_string(), 0_usize))
        .collect::<BTreeMap<_, _>>();
    let mut errors = Vec::new();
    scan_directory(directory, &mut counts, &mut errors);
    CodeScanResult {
        ok: errors.is_empty(),
        counts: counts
            .into_iter()
            .map(|(suffix, count)| CodeScanCount { suffix, count })
            .collect(),
        errors,
    }
}

fn scan_directory(
    directory: &Path,
    counts: &mut BTreeMap<String, usize>,
    errors: &mut Vec<String>,
) {
    let entries = match fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(err) => {
            errors.push(format!("failed to read {}: {err}", directory.display()));
            return;
        }
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or_default();
        if name == "target" || name == ".git" || name == "node_modules" {
            continue;
        }
        if path.is_dir() {
            scan_directory(&path, counts, errors);
        } else if let Some(suffix) = path.extension().and_then(|value| value.to_str()) {
            if let Some(count) = counts.get_mut(&suffix.to_ascii_lowercase()) {
                *count += 1;
            }
        }
    }
}

fn languages_over_threshold(scan_result: &CodeScanResult) -> Vec<String> {
    let mut totals = BTreeMap::<String, usize>::new();
    for item in &scan_result.counts {
        let Some(language) = language_for_suffix(&item.suffix) else {
            continue;
        };
        *totals.entry(language.to_string()).or_insert(0) += item.count;
    }

    totals
        .into_iter()
        .filter_map(|(language, count)| (count > LSP_SCAN_THRESHOLD).then_some(language))
        .collect()
}

fn language_for_suffix(suffix: &str) -> Option<&'static str> {
    match suffix {
        "py" => Some("py"),
        "ts" | "tsx" => Some("ts"),
        "js" | "jsx" | "mjs" | "cjs" => Some("js"),
        "go" => Some("go"),
        "java" => Some("java"),
        "rs" => Some("rs"),
        _ => None,
    }
}

fn call_router_lsp_scan(
    project_directory: &Path,
    session_directory: &Path,
    languages: &[String],
) -> Result<serde_json::Value, String> {
    let router_url = router_base_url();
    let services_dir = project_directory.join("services").join("lsp");
    let payload = json!({
        "services_dir": services_dir,
        "input": {
            "start_lsp": languages,
            "start_checks": languages,
            "session_path": absolute_session_path(session_directory),
        }
    });

    let runtime = tokio::runtime::Runtime::new()
        .map_err(|err| format!("failed to create router call runtime: {err}"))?;

    runtime.block_on(async {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(120))
            .no_proxy()
            .build()
            .map_err(|err| format!("failed to create router client: {err}"))?;

        match send_router_lsp_scan(&client, &router_url, &payload).await {
            Ok(value) => Ok(value),
            Err(first_err) => {
                ensure_router_ready(&client, &router_url, project_directory).await?;
                send_router_lsp_scan(&client, &router_url, &payload)
                    .await
                    .map_err(|retry_err| {
                        format!("{retry_err}; initial router call failed with: {first_err}")
                    })
            }
        }
    })
}

async fn send_router_lsp_scan(
    client: &reqwest::Client,
    router_url: &str,
    payload: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let response = client
        .post(format!("{}/run_service", router_url.trim_end_matches('/')))
        .json(payload)
        .send()
        .await
        .map_err(|err| format!("failed to call router lsp service: {err}"))?;

    let status = response.status();
    let value = response
        .json::<serde_json::Value>()
        .await
        .map_err(|err| format!("failed to decode router lsp response: {err}"))?;

    if !status.is_success() {
        return Err(format!("router lsp call returned {status}: {value}"));
    }

    let router_ok = value.get("ok").and_then(|v| v.as_bool()).unwrap_or(false);
    let invocation_ok = value
        .get("invocation")
        .and_then(|v| v.get("ok"))
        .and_then(|v| v.as_bool())
        .unwrap_or(router_ok);

    if !router_ok || !invocation_ok {
        return Err(format!("router lsp scan failed: {value}"));
    }

    Ok(value)
}

async fn ensure_router_ready(
    client: &reqwest::Client,
    router_url: &str,
    project_directory: &Path,
) -> Result<(), String> {
    if router_health_ok(client, router_url).await {
        return Ok(());
    }
    if router_autostart_disabled() {
        return Err(format!("router is not healthy at {router_url}/health"));
    }

    start_local_router(router_url, project_directory)?;

    let deadline = tokio::time::Instant::now() + Duration::from_secs(90);
    while tokio::time::Instant::now() < deadline {
        if router_health_ok(client, router_url).await {
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }

    Err(format!(
        "router did not become healthy at {router_url}/health"
    ))
}

fn router_autostart_disabled() -> bool {
    std::env::var("TURA_DISABLE_ROUTER_AUTOSTART")
        .map(|value| matches!(value.as_str(), "1" | "true" | "TRUE" | "yes" | "YES"))
        .unwrap_or(false)
}

async fn router_health_ok(client: &reqwest::Client, router_url: &str) -> bool {
    match client
        .get(format!("{}/health", router_url.trim_end_matches('/')))
        .send()
        .await
    {
        Ok(response) => response.status().is_success(),
        Err(_) => false,
    }
}

fn start_local_router(router_url: &str, project_directory: &Path) -> Result<(), String> {
    let router_executable = router_executable_candidates(project_directory)
        .into_iter()
        .find(|path| path.exists());

    let mut command = if let Some(executable) = router_executable {
        Command::new(executable)
    } else {
        let mut command = Command::new("cargo");
        command.args(["run", "-p", "tura_router"]);
        command
    };

    command
        .current_dir(project_directory)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    if let Some(port) = router_port_from_url(router_url) {
        command.env("TURA_ROUTER_PORT", port.to_string());
    }

    command
        .spawn()
        .map(|_| ())
        .map_err(|err| format!("failed to start local router: {err}"))
}

fn router_executable_candidates(project_directory: &Path) -> Vec<PathBuf> {
    let executable = if cfg!(windows) {
        "tura_router.exe"
    } else {
        "tura_router"
    };
    let mut candidates = Vec::new();
    if let Ok(current_exe) = std::env::current_exe() {
        if let Some(dir) = current_exe.parent() {
            candidates.push(dir.join(executable));
        }
    }
    candidates.push(
        project_directory
            .join("target")
            .join("release")
            .join(executable),
    );
    candidates.push(
        project_directory
            .join("target")
            .join("debug")
            .join(executable),
    );
    candidates
}

fn router_port_from_url(router_url: &str) -> Option<u16> {
    reqwest::Url::parse(router_url)
        .ok()?
        .port_or_known_default()
}

fn router_base_url() -> String {
    std::env::var("TURA_ROUTER_URL")
        .or_else(|_| std::env::var("ROUTER_BASE_URL"))
        .unwrap_or_else(|_| "http://127.0.0.1:8080".to_string())
}

fn absolute_session_path(session_directory: &Path) -> PathBuf {
    if session_directory.is_absolute() {
        return session_directory.to_path_buf();
    }

    std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(session_directory)
}

fn project_directory_with_lsp_service() -> Result<PathBuf, String> {
    let current = std::env::current_dir()
        .map_err(|err| format!("failed to resolve project directory: {err}"))?;
    for candidate in current.ancestors() {
        if candidate
            .join("crates")
            .join("router")
            .join("Cargo.toml")
            .exists()
        {
            return Ok(candidate.to_path_buf());
        }
    }
    Ok(current)
}

#[cfg(test)]
mod tests {
    use super::activate_session_with_topic;
    use crate::state_machine::session_management::SessionInput;
    use chrono::Utc;
    use std::fs;
    use std::sync::{Mutex, OnceLock};

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    #[test]
    fn coding_session_survives_unavailable_router_lsp_scan() {
        let _guard = env_lock().lock().expect("env lock should not be poisoned");
        let previous_router_url = std::env::var("TURA_ROUTER_URL").ok();
        let previous_disable_autostart = std::env::var("TURA_DISABLE_ROUTER_AUTOSTART").ok();
        std::env::set_var("TURA_ROUTER_URL", "http://127.0.0.1:1");
        std::env::set_var("TURA_DISABLE_ROUTER_AUTOSTART", "1");

        let root = std::env::temp_dir().join(format!(
            "tura-mano-lsp-router-down-{}",
            Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ));
        fs::create_dir_all(root.join("src")).expect("test workspace should be created");
        fs::write(root.join("src").join("a.rs"), "fn a() {}\n").expect("a.rs should be written");
        fs::write(root.join("src").join("b.rs"), "fn b() {}\n").expect("b.rs should be written");
        fs::write(root.join("src").join("c.rs"), "fn c() {}\n").expect("c.rs should be written");

        let result = activate_session_with_topic(
            root.clone(),
            "coding",
            SessionInput {
                user_input: "fix bug".to_string(),
                file_input: Vec::new(),
                agent: None,
                runtime_context: None,
            },
        );

        match previous_router_url {
            Some(value) => std::env::set_var("TURA_ROUTER_URL", value),
            None => std::env::remove_var("TURA_ROUTER_URL"),
        }
        match previous_disable_autostart {
            Some(value) => std::env::set_var("TURA_DISABLE_ROUTER_AUTOSTART", value),
            None => std::env::remove_var("TURA_DISABLE_ROUTER_AUTOSTART"),
        }
        let _ = fs::remove_dir_all(&root);

        let session = result.expect("router startup failure should not fail session creation");
        assert!(
            session
                .session_log
                .iter()
                .any(|entry| entry.contains("\"type\":\"lsp_scan\"")
                    && entry.contains("\"ok\":false")),
            "session log should record the best-effort LSP failure"
        );
    }
}
