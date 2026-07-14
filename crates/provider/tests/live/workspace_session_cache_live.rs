//! Gated live check for prompt-cache reuse when workspace/session identity changes.
//!
//! The stable prefix includes a fresh nonce every run so previous live runs
//! cannot satisfy the assertions. Workspace/session identifiers are appended
//! after the long prefix, matching runtime tail metadata that should not break
//! cache reuse for the stable context.
//!
//! ```text
//! TURA_WORKSPACE_SESSION_CACHE_LIVE=1 cargo test -p provider --features live-tests \
//!     --test workspace_session_cache_live -- --nocapture
//! ```

use serde_json::json;
use tura_llm_rust::tura_conf::TuraConfig;
use tura_llm_rust::tura_llm::{CallMetrics, CallOptions, ProviderConfig, ProviderResponse};
use uuid::Uuid;

#[derive(Debug, Clone)]
struct CacheCase {
    label: &'static str,
    warm_workspace_id: String,
    warm_session_id: String,
    probe_workspace_id: String,
    probe_session_id: String,
}

#[derive(Debug)]
struct ProbeReport {
    label: &'static str,
    warm_workspace_id: String,
    warm_session_id: String,
    probe_workspace_id: String,
    probe_session_id: String,
    warm_cache_key: String,
    probe_cache_key: String,
    cached_input_tokens: u64,
    input_tokens: u64,
    cache_ratio: f64,
    output_tokens: Option<u64>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    if std::env::var("TURA_WORKSPACE_SESSION_CACHE_LIVE")
        .ok()
        .as_deref()
        != Some("1")
    {
        println!(
            "skipping workspace/session cache live test; set TURA_WORKSPACE_SESSION_CACHE_LIVE=1"
        );
        return Ok(());
    }

    let conf = TuraConfig::new(".env");
    if conf
        .get("OPENAI_API_KEY")
        .filter(|value| !value.trim().is_empty())
        .is_none()
    {
        println!("skipping workspace/session cache live test; missing OPENAI_API_KEY");
        return Ok(());
    }

    let provider = provider_from_env();
    let run_nonce = format!("ws-session-cache-{}", Uuid::new_v4().simple());
    let min_ratio = min_cache_ratio();
    let retry_count = retry_count();
    let cases = vec![
        CacheCase {
            label: "same_workspace_different_session",
            warm_workspace_id: format!("{run_nonce}-workspace-a"),
            warm_session_id: format!("{run_nonce}-session-a1"),
            probe_workspace_id: format!("{run_nonce}-workspace-a"),
            probe_session_id: format!("{run_nonce}-session-a2"),
        },
        CacheCase {
            label: "different_workspace_different_session",
            warm_workspace_id: format!("{run_nonce}-workspace-b1"),
            warm_session_id: format!("{run_nonce}-session-b1"),
            probe_workspace_id: format!("{run_nonce}-workspace-b2"),
            probe_session_id: format!("{run_nonce}-session-b2"),
        },
    ];

    let mut reports = Vec::new();
    for case in cases {
        reports.push(
            run_cache_case(&provider, &conf, &run_nonce, case, min_ratio, retry_count).await?,
        );
    }

    for report in reports {
        println!(
            "PASS workspace/session cache live: label={}, warm_workspace={}, warm_session={}, probe_workspace={}, probe_session={}, warm_cache_key={}, probe_cache_key={}, cached_input_tokens={}, input_tokens={}, cache_ratio={:.2}%, output_tokens={:?}",
            report.label,
            report.warm_workspace_id,
            report.warm_session_id,
            report.probe_workspace_id,
            report.probe_session_id,
            report.warm_cache_key,
            report.probe_cache_key,
            report.cached_input_tokens,
            report.input_tokens,
            report.cache_ratio * 100.0,
            report.output_tokens,
        );
    }

    Ok(())
}

async fn run_cache_case(
    provider: &ProviderConfig,
    conf: &TuraConfig,
    run_nonce: &str,
    case: CacheCase,
    min_ratio: f64,
    retry_count: usize,
) -> Result<ProbeReport, Box<dyn std::error::Error>> {
    let stable_prefix = stable_cache_prefix(run_nonce, case.label);
    let warm_cache_key = cache_key(run_nonce, &case.warm_workspace_id);
    let probe_cache_key = cache_key(run_nonce, &case.probe_workspace_id);

    let _warm = provider
        .call(
            conf,
            messages(
                &stable_prefix,
                case.label,
                "warm",
                &case.warm_workspace_id,
                &case.warm_session_id,
            ),
            call_options(warm_cache_key.clone()),
        )
        .await?;

    let mut last_response = None;
    for attempt in 1..=retry_count {
        let response = provider
            .call(
                conf,
                messages(
                    &stable_prefix,
                    case.label,
                    &format!("probe-{attempt}"),
                    &case.probe_workspace_id,
                    &case.probe_session_id,
                ),
                call_options(probe_cache_key.clone()),
            )
            .await?;
        let metrics = response.metrics.as_ref().ok_or_else(|| {
            format!(
                "{} probe attempt {} did not return metrics; raw={}",
                case.label, attempt, response.raw
            )
        })?;
        if cache_ratio(metrics).is_some_and(|ratio| ratio >= min_ratio) {
            return report_from_response(case, warm_cache_key, probe_cache_key, response);
        }
        last_response = Some(response);
        std::thread::sleep(std::time::Duration::from_millis(750));
    }

    let response = last_response.expect("at least one probe attempt should run");
    let metrics = response
        .metrics
        .as_ref()
        .expect("probe metrics should have been checked");
    let cached = metrics.usage.cached_input_tokens.unwrap_or(0);
    let input = metrics.usage.input_tokens.unwrap_or(0);
    let ratio = cache_ratio(metrics).unwrap_or(0.0);
    panic!(
        "{} cache hit ratio too small after {} probe attempts: {:.2}% < {:.2}%; cached_input_tokens={}; input_tokens={}; warm_workspace={}; warm_session={}; probe_workspace={}; probe_session={}; warm_cache_key={}; probe_cache_key={}; metrics={:#?}; raw={}",
        case.label,
        retry_count,
        ratio * 100.0,
        min_ratio * 100.0,
        cached,
        input,
        case.warm_workspace_id,
        case.warm_session_id,
        case.probe_workspace_id,
        case.probe_session_id,
        warm_cache_key,
        probe_cache_key,
        metrics,
        response.raw
    );
}

fn provider_from_env() -> ProviderConfig {
    let provider = std::env::var("TURA_WORKSPACE_SESSION_CACHE_PROVIDER")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "openai".to_string());
    let base_url = std::env::var("TURA_WORKSPACE_SESSION_CACHE_BASE_URL")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "https://api.openai.com/v1".to_string());
    let model = std::env::var("TURA_WORKSPACE_SESSION_CACHE_MODEL")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "gpt-5.5".to_string());
    ProviderConfig {
        provider,
        base_url,
        model,
        temperature: 0.0,
    }
}

fn messages(
    stable_prefix: &str,
    label: &str,
    phase: &str,
    workspace_id: &str,
    session_id: &str,
) -> Vec<serde_json::Value> {
    vec![
        json!({
            "role": "system",
            "content": stable_prefix,
        }),
        json!({
            "role": "user",
            "content": format!(
                "Return exactly ok. Cache live case: {label}. Phase: {phase}. Workspace id: {workspace_id}. Session id: {session_id}."
            ),
        }),
    ]
}

fn call_options(prompt_cache_key: String) -> CallOptions {
    CallOptions {
        temperature: Some(0.0),
        max_tokens: Some(16),
        stream: Some(true),
        stream_options: Some(json!({"include_usage": true})),
        reasoning_effort: Some("low".to_string()),
        prompt_cache_key: Some(prompt_cache_key),
        store: Some(false),
        ..Default::default()
    }
}

fn stable_cache_prefix(run_nonce: &str, label: &str) -> String {
    let block = format!(
        "Stable workspace/session cache prefix. run_nonce={run_nonce}. case={label}. Keep this exact text before workspace and session metadata. "
    );
    format!(
        "{}\nThe assistant must answer with exactly ok.",
        block.repeat(900)
    )
}

fn cache_key(run_nonce: &str, workspace_id: &str) -> String {
    format!("tura-workspace-session-cache:{run_nonce}:{workspace_id}")
}

fn report_from_response(
    case: CacheCase,
    warm_cache_key: String,
    probe_cache_key: String,
    response: ProviderResponse,
) -> Result<ProbeReport, Box<dyn std::error::Error>> {
    let metrics = response
        .metrics
        .as_ref()
        .ok_or("probe response should include metrics")?;
    let input = metrics
        .usage
        .input_tokens
        .filter(|tokens| *tokens > 0)
        .ok_or_else(|| {
            format!("probe response did not include positive input_tokens: {metrics:#?}")
        })?;
    let cached = metrics.usage.cached_input_tokens.unwrap_or(0);
    let ratio = cached as f64 / input as f64;
    Ok(ProbeReport {
        label: case.label,
        warm_workspace_id: case.warm_workspace_id,
        warm_session_id: case.warm_session_id,
        probe_workspace_id: case.probe_workspace_id,
        probe_session_id: case.probe_session_id,
        warm_cache_key,
        probe_cache_key,
        cached_input_tokens: cached,
        input_tokens: input,
        cache_ratio: ratio,
        output_tokens: metrics.usage.output_tokens,
    })
}

fn cache_ratio(metrics: &CallMetrics) -> Option<f64> {
    let input = metrics.usage.input_tokens?;
    (input > 0).then_some(metrics.usage.cached_input_tokens.unwrap_or(0) as f64 / input as f64)
}

fn min_cache_ratio() -> f64 {
    std::env::var("TURA_WORKSPACE_SESSION_CACHE_MIN_RATIO")
        .ok()
        .and_then(|value| value.trim().parse::<f64>().ok())
        .filter(|ratio| (0.0..=1.0).contains(ratio))
        .unwrap_or(0.80)
}

fn retry_count() -> usize {
    std::env::var("TURA_WORKSPACE_SESSION_CACHE_RETRIES")
        .ok()
        .and_then(|value| value.trim().parse::<usize>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(3)
}
