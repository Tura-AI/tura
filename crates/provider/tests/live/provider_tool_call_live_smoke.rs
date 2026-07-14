use serde_json::{json, Value};
use tura_llm_rust::tura_conf::TuraConfig;
use tura_llm_rust::tura_llm::{CallOptions, ProviderConfig};

#[derive(Debug)]
struct Probe {
    provider: &'static str,
    model: &'static str,
    env_key: &'static str,
    supports_stream_usage: bool,
    supports_openapi_extras: bool,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    if std::env::var("TURA_PROVIDER_TOOL_CALL_SMOKE")
        .ok()
        .as_deref()
        != Some("1")
    {
        println!("skipping provider tool-call live smoke; set TURA_PROVIDER_TOOL_CALL_SMOKE=1");
        return Ok(());
    }

    let conf = TuraConfig::new(".env");
    let probes = [
        Probe {
            provider: "codex",
            model: "gpt-5.1-codex-mini",
            env_key: "OPENAI_API_KEY",
            supports_stream_usage: true,
            supports_openapi_extras: true,
        },
        Probe {
            provider: "openai",
            model: "gpt-5.2",
            env_key: "OPENAI_API_KEY",
            supports_stream_usage: true,
            supports_openapi_extras: true,
        },
        Probe {
            provider: "google",
            model: "gemini-2.5-flash",
            env_key: "GOOGLE_API_KEY",
            supports_stream_usage: false,
            supports_openapi_extras: false,
        },
        Probe {
            provider: "minimax",
            model: "minimax-m2.7",
            env_key: "MINIMAX_API_KEY",
            supports_stream_usage: true,
            supports_openapi_extras: true,
        },
        Probe {
            provider: "deepseek",
            model: "deepseek-v4-pro",
            env_key: "DEEPSEEK_API_KEY",
            supports_stream_usage: true,
            supports_openapi_extras: true,
        },
        Probe {
            provider: "moonshotai",
            model: "kimi-k2.5-0127",
            env_key: "MOONSHOTAI_API_KEY",
            supports_stream_usage: true,
            supports_openapi_extras: true,
        },
        Probe {
            provider: "openrouter",
            model: "minimax/minimax-m2.7",
            env_key: "OPENROUTER_API_KEY",
            supports_stream_usage: true,
            supports_openapi_extras: true,
        },
        Probe {
            provider: "qwen",
            model: "qwen3-max-2026-01-23",
            env_key: "QWEN_API_KEY",
            supports_stream_usage: true,
            supports_openapi_extras: true,
        },
        Probe {
            provider: "anthropic",
            model: "claude-sonnet-4-20250514",
            env_key: "ANTHROPIC_API_KEY",
            supports_stream_usage: true,
            supports_openapi_extras: true,
        },
    ];

    println!("provider tool-call live smoke");
    println!("env path: {}", conf.env_path().display());

    for probe in probes {
        match run_probe(&conf, &probe).await {
            ProbeStatus::Passed => println!("PASS {} {}", probe.provider, probe.model),
            ProbeStatus::Skipped(reason) => {
                println!("SKIP {} {}: {}", probe.provider, probe.model, reason)
            }
            ProbeStatus::Failed(reason) => {
                println!("FAIL {} {}: {}", probe.provider, probe.model, reason)
            }
        }
    }

    Ok(())
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
        && !(probe.provider == "codex" && conf.get("OPENAI_LOGIN").as_deref() == Some("oauth"))
    {
        return ProbeStatus::Skipped(format!("missing {}", probe.env_key));
    }

    let Some(base_url) = base_url(probe.provider) else {
        return ProbeStatus::Skipped("provider has no base URL in smoke test".to_string());
    };

    let provider = ProviderConfig {
        provider: probe.provider.to_string(),
        base_url: base_url.to_string(),
        model: probe.model.to_string(),
        temperature: 0.0,
    };

    let messages = vec![json!({
        "role": "user",
        "content": "Call the echo_answer tool exactly once with answer set to pong."
    })];
    let options = CallOptions {
        temperature: Some(0.0),
        max_tokens: Some(128),
        stream: probe.supports_stream_usage.then_some(true),
        stream_options: probe
            .supports_stream_usage
            .then(|| json!({"include_usage": true})),
        reasoning_effort: probe.supports_openapi_extras.then(|| "low".to_string()),
        prompt_cache_key: probe
            .supports_openapi_extras
            .then(|| "provider-tool-call-live-smoke".to_string()),
        tools: Some(vec![json!({
            "type": "function",
            "function": {
                "name": "echo_answer",
                "description": "Return a tiny answer for provider tool-call smoke tests.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "answer": {
                            "type": "string",
                            "enum": ["pong"]
                        }
                    },
                    "required": ["answer"],
                    "additionalProperties": false
                }
            }
        })]),
        tool_choice: Some(tool_choice(probe.provider)),
        ..Default::default()
    };

    match provider.call(conf, messages, options).await {
        Ok(response)
            if contains_tool_call(&response.content) || contains_tool_call(&response.raw) =>
        {
            if let Some(reason) = metrics_failure(probe, &response) {
                return ProbeStatus::Failed(reason);
            }
            ProbeStatus::Passed
        }
        Ok(response) => ProbeStatus::Failed(format!(
            "response did not contain echo_answer tool call: {}",
            response.content
        )),
        Err(err) => ProbeStatus::Failed(err.to_string()),
    }
}

fn metrics_failure(
    probe: &Probe,
    response: &tura_llm_rust::tura_llm::ProviderResponse,
) -> Option<String> {
    let metrics = response.metrics.as_ref()?;
    if probe.supports_stream_usage && metrics.usage.total_tokens.unwrap_or(0) == 0 {
        return Some("stream usage was requested but total token usage was missing".to_string());
    }
    if probe.provider == "google" && response.raw.get("usageMetadata").is_none() {
        return Some("google response did not include usageMetadata".to_string());
    }
    if metrics.tool_call_count == 0 && !contains_tool_call(&response.raw) {
        return Some("metrics did not record tool calls".to_string());
    }
    None
}

fn base_url(provider: &str) -> Option<&'static str> {
    match provider {
        "anthropic" => Some("https://api.anthropic.com/v1"),
        "codex" => Some("https://chatgpt.com/backend-api/codex/responses"),
        "deepseek" => Some("https://api.deepseek.com"),
        "google" => Some("https://generativelanguage.googleapis.com/v1beta"),
        "minimax" => Some("https://api.minimax.io/v1"),
        "moonshotai" => Some("https://api.moonshot.ai/v1"),
        "openai" => Some("https://api.openai.com/v1"),
        "openrouter" => Some("https://openrouter.ai/api/v1"),
        "qwen" => Some("https://dashscope-intl.aliyuncs.com/compatible-mode/v1"),
        _ => None,
    }
}

fn tool_choice(provider: &str) -> Value {
    if provider == "google" {
        return Value::Null;
    }
    json!({
        "type": "function",
        "function": { "name": "echo_answer" }
    })
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
