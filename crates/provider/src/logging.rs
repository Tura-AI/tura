use std::path::{Path, PathBuf};

use chrono::{DateTime, Local, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::fs;
use uuid::Uuid;

use crate::tura_llm::{CallMetrics, TuraError};

const DEFAULT_LOG_CRATE_DIR: &str = "provider";

/// Whether provider LLM-call logs should be written to disk.
///
/// Decided by build-kind, not by the executable path: a `dev` build (the
/// repo-local `bin/` package) always writes, while a `release` build stays
/// silent unless `LOG_PATH` is set as an explicit opt-in (the `-dev` flag).
pub fn logging_enabled() -> bool {
    if std::env::var_os("LOG_PATH").is_some() {
        return true;
    }
    tura_path::build_kind() == "dev"
}

fn get_log_root() -> PathBuf {
    std::env::var("LOG_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|_| project_root().join("log").join(DEFAULT_LOG_CRATE_DIR))
}

/// Returns the log root directory when provider logging is enabled, or `None`.
pub fn log_root_if_enabled() -> Option<PathBuf> {
    if logging_enabled() {
        Some(get_log_root())
    } else {
        None
    }
}

fn project_root() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .ancestors()
        .find(|candidate| candidate.join("Cargo.lock").exists())
        .map(Path::to_path_buf)
        .unwrap_or(manifest_dir)
}

fn log_day_dir(now: DateTime<Local>) -> PathBuf {
    get_log_root().join(now.format("%Y-%m-%d").to_string())
}

fn build_filename(now: DateTime<Local>, call_id: Option<&str>) -> String {
    let id = call_id
        .map(str::to_string)
        .unwrap_or_else(|| Uuid::new_v4().simple().to_string());
    format!("{}_{}.json", now.format("%H%M%S_%3f"), id)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmCallLog {
    pub r#type: String,
    pub call_id: String,
    pub success: bool,
    pub provider: String,
    pub model: String,
    pub base_url: String,
    pub started_at: DateTime<Utc>,
    pub finished_at: DateTime<Utc>,
    pub duration_ms: f64,
    pub request: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metrics: Option<CallMetrics>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub traceback: Option<String>,
}

#[allow(clippy::too_many_arguments)]
pub fn build_call_log(
    provider: &str,
    model: &str,
    base_url: &str,
    messages: Value,
    response: Option<Value>,
    request_params: Value,
    response_format: Option<Value>,
    started_at: DateTime<Utc>,
    finished_at: DateTime<Utc>,
    duration_ms: f64,
    success: bool,
    call_id: &str,
    metrics: Option<CallMetrics>,
    error: Option<String>,
    traceback_text: Option<String>,
) -> LlmCallLog {
    LlmCallLog {
        r#type: "llm_call".to_string(),
        call_id: call_id.to_string(),
        success,
        provider: provider.to_string(),
        model: model.to_string(),
        base_url: base_url.to_string(),
        started_at,
        finished_at,
        duration_ms,
        request: json!({
            "messages": messages,
            "response_format": response_format,
            "params": request_params,
        }),
        response,
        metrics,
        error,
        traceback: traceback_text,
    }
}

pub async fn write_llm_log(
    payload: &LlmCallLog,
    call_id: Option<&str>,
) -> Result<PathBuf, TuraError> {
    let now = Local::now();
    let out_dir = log_day_dir(now);
    fs::create_dir_all(&out_dir).await.map_err(TuraError::io)?;
    let path = out_dir.join(build_filename(now, call_id));
    let tmp_path = path.with_extension("tmp");
    let data = serde_json::to_vec_pretty(payload).map_err(TuraError::from)?;

    fs::write(&tmp_path, data).await.map_err(TuraError::io)?;
    fs::rename(&tmp_path, &path).await.map_err(TuraError::io)?;
    Ok(path)
}

pub fn display_path(path: &Path) -> String {
    path.display().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn logging_enabled_follows_log_path_and_build_kind() {
        let _guard = ENV_LOCK.lock().expect("env lock should not be poisoned");
        // Explicit LOG_PATH always enables logging.
        set_env("LOG_PATH", "some/dir");
        assert!(logging_enabled());
        remove_env("LOG_PATH");
        // Without LOG_PATH, logging follows build-kind. `cargo test` builds with
        // no `TURA_BUILD_KIND`, so the kind defaults to `dev` and logging is on.
        assert_eq!(tura_path::build_kind(), "dev");
        assert_eq!(logging_enabled(), tura_path::build_kind() == "dev");
    }

    #[test]
    fn default_log_root_is_project_level_provider_log_dir() {
        let _guard = ENV_LOCK.lock().expect("env lock should not be poisoned");
        remove_env("LOG_PATH");

        let root = get_log_root();
        assert!(root.ends_with(Path::new("log").join("provider")));
        assert!(root.parent().and_then(Path::parent).is_some_and(|project| {
            project.join("Cargo.toml").exists() && project.join("Cargo.lock").exists()
        }));
    }

    fn set_env(key: &str, value: &str) {
        // SAFETY: these tests serialize environment mutation through ENV_LOCK.
        unsafe {
            std::env::set_var(key, value);
        }
    }

    fn remove_env(key: &str) {
        // SAFETY: these tests serialize environment mutation through ENV_LOCK.
        unsafe {
            std::env::remove_var(key);
        }
    }
}
