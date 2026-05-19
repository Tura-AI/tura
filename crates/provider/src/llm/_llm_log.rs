use std::path::{Path, PathBuf};

use chrono::{DateTime, Local, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::fs;
use uuid::Uuid;

use crate::tura_llm::{CallMetrics, TuraError};

fn get_log_root() -> PathBuf {
    std::env::var("LOG_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("log"))
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
