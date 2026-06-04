use std::time::Instant;

use serde::Serialize;
use serde_json::{json, Value};
use tura_llm_rust::tura_conf::TuraConfig;
use tura_llm_rust::tura_llm::{CallOptions, ProviderConfig, ProviderResponse};

#[derive(Debug, Clone, Serialize)]
struct RoundStats {
    round: usize,
    service_tier: String,
    success: bool,
    duration_ms: u128,
    output_tokens: u64,
    total_tokens: u64,
    output_tps: f64,
    total_tps: f64,
    zero_count: usize,
    error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct TierSummary {
    service_tier: String,
    rounds: usize,
    successes: usize,
    duration_ms_avg: f64,
    output_tokens: u64,
    total_tokens: u64,
    output_tps_avg: f64,
    total_tps_avg: f64,
    output_tps_weighted: f64,
    total_tps_weighted: f64,
}

#[tokio::test]
async fn codex_priority_tps_live() {
    if std::env::var("TURA_CODEX_PRIORITY_TPS").ok().as_deref() != Some("1") {
        println!("skipping Codex priority TPS live test; set TURA_CODEX_PRIORITY_TPS=1");
        return;
    }

    let rounds = std::env::var("TURA_CODEX_PRIORITY_TPS_ROUNDS")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(10)
        .max(2);
    let model = std::env::var("TURA_CODEX_PRIORITY_TPS_MODEL")
        .unwrap_or_else(|_| "gpt-5.1-codex".to_string());
    let conf = TuraConfig::new(".env");
    let provider = ProviderConfig {
        provider: "codex".to_string(),
        base_url: "https://chatgpt.com/backend-api/codex/responses".to_string(),
        model,
        temperature: 0.0,
    };

    let priority_rounds = rounds / 2;
    let auto_rounds = rounds - priority_rounds;
    let mut all = Vec::with_capacity(rounds);

    for index in 0..priority_rounds {
        all.push(run_round(&provider, &conf, index + 1, Some("priority")).await);
    }
    for index in 0..auto_rounds {
        all.push(run_round(&provider, &conf, priority_rounds + index + 1, None).await);
    }

    let priority = summarize(
        "priority",
        all.iter().filter(|item| item.service_tier == "priority"),
    );
    let auto = summarize(
        "auto",
        all.iter().filter(|item| item.service_tier == "auto"),
    );
    let report = json!({
        "rounds_requested": rounds,
        "rounds": all,
        "summary": {
            "priority": priority,
            "auto": auto,
        }
    });

    println!(
        "{}",
        serde_json::to_string_pretty(&report).expect("serialize TPS report")
    );
}

async fn run_round(
    provider: &ProviderConfig,
    conf: &TuraConfig,
    round: usize,
    service_tier: Option<&str>,
) -> RoundStats {
    let tier_label = service_tier.unwrap_or("auto").to_string();
    let messages = vec![json!({
        "role": "user",
        "content": "Output exactly 1000 zero characters and nothing else. Do not use markdown, spaces, newlines, punctuation, or explanation. The entire response must be 1000 repetitions of the character 0."
    })];
    let options = CallOptions {
        temperature: Some(0.0),
        reasoning_effort: Some("low".to_string()),
        service_tier: service_tier.map(ToString::to_string),
        prompt_cache_key: Some(format!("codex-priority-tps-{tier_label}-{round}")),
        ..Default::default()
    };
    let started = Instant::now();
    match provider.call(conf, messages, options).await {
        Ok(response) => {
            let duration_ms = started.elapsed().as_millis();
            let output_tokens = response
                .metrics
                .as_ref()
                .and_then(|metrics| metrics.usage.output_tokens)
                .unwrap_or(0);
            let total_tokens = response
                .metrics
                .as_ref()
                .and_then(|metrics| metrics.usage.total_tokens)
                .unwrap_or(0);
            let seconds = (duration_ms as f64 / 1000.0).max(0.001);
            let text = response_text(&response);
            let zero_count = text.chars().filter(|ch| *ch == '0').count();
            RoundStats {
                round,
                service_tier: tier_label,
                success: true,
                duration_ms,
                output_tokens,
                total_tokens,
                output_tps: output_tokens as f64 / seconds,
                total_tps: total_tokens as f64 / seconds,
                zero_count,
                error: None,
            }
        }
        Err(error) => {
            let duration_ms = started.elapsed().as_millis();
            RoundStats {
                round,
                service_tier: tier_label,
                success: false,
                duration_ms,
                output_tokens: 0,
                total_tokens: 0,
                output_tps: 0.0,
                total_tps: 0.0,
                zero_count: 0,
                error: Some(error.to_string()),
            }
        }
    }
}

fn summarize<'a>(service_tier: &str, rounds: impl Iterator<Item = &'a RoundStats>) -> TierSummary {
    let rows: Vec<&RoundStats> = rounds.collect();
    let successes: Vec<&RoundStats> = rows.iter().copied().filter(|item| item.success).collect();
    let duration_sum: u128 = successes.iter().map(|item| item.duration_ms).sum();
    let output_tokens: u64 = successes.iter().map(|item| item.output_tokens).sum();
    let total_tokens: u64 = successes.iter().map(|item| item.total_tokens).sum();
    let duration_seconds = (duration_sum as f64 / 1000.0).max(0.001);
    let success_count = successes.len();

    TierSummary {
        service_tier: service_tier.to_string(),
        rounds: rows.len(),
        successes: success_count,
        duration_ms_avg: if success_count == 0 {
            0.0
        } else {
            duration_sum as f64 / success_count as f64
        },
        output_tokens,
        total_tokens,
        output_tps_avg: average(successes.iter().map(|item| item.output_tps)),
        total_tps_avg: average(successes.iter().map(|item| item.total_tps)),
        output_tps_weighted: output_tokens as f64 / duration_seconds,
        total_tps_weighted: total_tokens as f64 / duration_seconds,
    }
}

fn average(values: impl Iterator<Item = f64>) -> f64 {
    let mut total = 0.0;
    let mut count = 0usize;
    for value in values {
        total += value;
        count += 1;
    }
    if count == 0 {
        0.0
    } else {
        total / count as f64
    }
}

fn response_text(response: &ProviderResponse) -> String {
    if let Some(text) = response.content.as_str() {
        return text.to_string();
    }
    collect_text(&response.content).unwrap_or_else(|| response.content.to_string())
}

fn collect_text(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => Some(text.clone()),
        Value::Array(items) => {
            let text = items.iter().filter_map(collect_text).collect::<String>();
            (!text.is_empty()).then_some(text)
        }
        Value::Object(object) => {
            for key in ["text", "content", "output_text"] {
                if let Some(text) = object.get(key).and_then(collect_text) {
                    return Some(text);
                }
            }
            let text = object.values().filter_map(collect_text).collect::<String>();
            (!text.is_empty()).then_some(text)
        }
        _ => None,
    }
}
