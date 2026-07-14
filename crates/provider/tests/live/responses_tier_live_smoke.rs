//! Gated live smoke for the openapi **Responses** tier (`chatgpt`, `grok`,
//! `qwen`). These providers share the codex Responses request shape but are
//! driven by an API key via `openapi::responses_api_key_call`. The probe
//! confirms each endpoint accepts the native Responses payload and round-trips a
//! forced tool call back into the runtime's OpenAI-shaped `tool_calls`.
//!
//! Gated (talks to real APIs, costs quota):
//!
//! ```text
//! TURA_RESPONSES_TIER_SMOKE=1 cargo test -p provider \
//!     --test responses_tier_live_smoke -- --nocapture
//! ```
//!
//! Credentials are read from the process env, then the project root `.env`.
//! Providers without a key are SKIPped (e.g. qwen if `QWEN_API_KEY` is absent).
//! qwen deliberately uses the *international* DashScope endpoint.

use serde_json::{json, Value};
use tura_llm_rust::tura_conf::TuraConfig;
use tura_llm_rust::tura_llm::{CallOptions, ProviderConfig};

struct Probe {
    provider: &'static str,
    model: &'static str,
    env_key: &'static str,
    base_url: &'static str,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    if std::env::var("TURA_RESPONSES_TIER_SMOKE").ok().as_deref() != Some("1") {
        println!("skipping responses-tier live smoke; set TURA_RESPONSES_TIER_SMOKE=1");
        return Ok(());
    }

    let conf = TuraConfig::new(".env");
    println!("responses-tier live smoke");
    println!("env path: {}", conf.env_path().display());

    let probes = [
        Probe {
            provider: "openai",
            model: "gpt-5.2",
            env_key: "OPENAI_API_KEY",
            base_url: "https://api.openai.com/v1",
        },
        Probe {
            provider: "xai",
            model: "grok-4",
            env_key: "XAI_API_KEY",
            base_url: "https://api.x.ai/v1",
        },
        Probe {
            // International DashScope endpoint, per the requirement that qwen
            // must use the intl route (provider id `qwen`, not `qwen_cn`).
            provider: "qwen",
            model: "qwen3-max",
            env_key: "QWEN_API_KEY",
            base_url: "https://dashscope-intl.aliyuncs.com/compatible-mode/v1",
        },
    ];

    let mut failures = Vec::new();
    for probe in probes {
        match run_probe(&conf, &probe).await {
            ProbeStatus::Passed => println!("PASS {} {}", probe.provider, probe.model),
            ProbeStatus::Skipped(reason) => {
                println!("SKIP {} {}: {}", probe.provider, probe.model, reason)
            }
            ProbeStatus::Failed(reason) => {
                println!("FAIL {} {}: {}", probe.provider, probe.model, reason);
                failures.push(format!("{} {}", probe.provider, probe.model));
            }
        }
    }

    if failures.is_empty() {
        Ok(())
    } else {
        Err(format!("responses-tier probes failed: {}", failures.join(", ")).into())
    }
}

enum ProbeStatus {
    Passed,
    Skipped(String),
    Failed(String),
}

async fn run_probe(conf: &TuraConfig, probe: &Probe) -> ProbeStatus {
    if conf
        .get(probe.env_key)
        .filter(|value| !value.trim().is_empty())
        .is_none()
    {
        return ProbeStatus::Skipped(format!("missing {}", probe.env_key));
    }

    let provider = ProviderConfig {
        provider: probe.provider.to_string(),
        base_url: probe.base_url.to_string(),
        model: probe.model.to_string(),
        temperature: 0.0,
    };

    let messages = vec![
        json!({"role": "system", "content": "You are a terse assistant. Use the provided tool when asked."}),
        json!({
            "role": "user",
            "content": "Call the echo_answer tool exactly once with answer set to pong."
        }),
    ];
    let options = CallOptions {
        temperature: Some(0.0),
        max_tokens: Some(256),
        stream: Some(true),
        reasoning_effort: Some("low".to_string()),
        tools: Some(vec![json!({
            "type": "function",
            "function": {
                "name": "echo_answer",
                "description": "Return a tiny answer for responses-tier smoke tests.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "answer": { "type": "string", "enum": ["pong"] }
                    },
                    "required": ["answer"],
                    "additionalProperties": false
                }
            }
        })]),
        tool_choice: Some(json!({
            "type": "function",
            "function": { "name": "echo_answer" }
        })),
        ..Default::default()
    };

    match provider.call(conf, messages, options).await {
        Ok(response)
            if contains_tool_call(&response.content) || contains_tool_call(&response.raw) =>
        {
            ProbeStatus::Passed
        }
        Ok(response) => ProbeStatus::Failed(format!(
            "response did not contain echo_answer tool call: {}",
            response.content
        )),
        Err(err) => ProbeStatus::Failed(err.to_string()),
    }
}

fn contains_tool_call(value: &Value) -> bool {
    match value {
        Value::Object(object) => {
            object
                .get("name")
                .and_then(Value::as_str)
                .is_some_and(|name| name == "echo_answer")
                || object
                    .get("function")
                    .and_then(|function| function.get("name"))
                    .and_then(Value::as_str)
                    .is_some_and(|name| name == "echo_answer")
                || object.values().any(contains_tool_call)
        }
        Value::Array(items) => items.iter().any(contains_tool_call),
        Value::String(text) => text.contains("echo_answer") && text.contains("pong"),
        _ => false,
    }
}
