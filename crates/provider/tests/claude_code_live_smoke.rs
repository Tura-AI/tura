//! Live round-trip smoke test for the `claude-code` provider against the native
//! Anthropic Messages API. Exercises both the text path and a forced tool call.
//!
//! This talks to the real API and is therefore gated behind an env var. Run it
//! with credentials loaded from the crate `.env`:
//!
//! ```text
//! TURA_CLAUDE_CODE_SMOKE=1 cargo test -p tura-llm-rust --test claude_code_live_smoke
//! ```
//!
//! It auto-detects the auth route from whichever token is configured:
//! `CLAUDE_CODE_OAUTH_TOKEN` (subscription OAuth) or `ANTHROPIC_API_KEY` (API
//! key). The provider layer picks headers/system-prompt rules based on the token
//! prefix, so the same code path serves both.

use serde_json::{json, Value};
use tura_llm_rust::tura_conf::TuraConfig;
use tura_llm_rust::tura_llm::{CallOptions, ProviderConfig, ProviderResponse};

const BASE_URL: &str = "https://api.anthropic.com/v1";
const MODEL: &str = "claude-opus-4-8";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    if std::env::var("TURA_CLAUDE_CODE_SMOKE").ok().as_deref() != Some("1") {
        println!("skipping claude-code live smoke; set TURA_CLAUDE_CODE_SMOKE=1");
        return Ok(());
    }

    let conf = TuraConfig::new(".env");
    println!("claude-code live smoke");
    println!("env path: {}", conf.env_path().display());

    let route = detect_route(&conf);
    let Some(route) = route else {
        println!("SKIP: no CLAUDE_CODE_OAUTH_TOKEN or ANTHROPIC_API_KEY configured");
        return Ok(());
    };
    println!("auth route: {route}");

    let mut failures = 0;
    match run_text_probe(&conf).await {
        Ok(()) => println!("PASS text round-trip"),
        Err(reason) => {
            failures += 1;
            println!("FAIL text round-trip: {reason}");
        }
    }
    match run_tool_call_probe(&conf).await {
        Ok(()) => println!("PASS tool-call round-trip"),
        Err(reason) => {
            failures += 1;
            println!("FAIL tool-call round-trip: {reason}");
        }
    }

    if failures > 0 {
        return Err(format!("{failures} claude-code smoke probe(s) failed").into());
    }
    Ok(())
}

/// `claude-code` resolves its token via the auth registry, which prefers
/// `CLAUDE_CODE_OAUTH_TOKEN`. Report which credential is actually present so the
/// log makes the exercised route obvious.
fn detect_route(conf: &TuraConfig) -> Option<&'static str> {
    if conf
        .get("CLAUDE_CODE_OAUTH_TOKEN")
        .filter(|value| !value.trim().is_empty())
        .is_some()
    {
        Some("oauth-subscription")
    } else if conf
        .get("ANTHROPIC_API_KEY")
        .filter(|value| !value.trim().is_empty())
        .is_some()
    {
        Some("api-key")
    } else {
        None
    }
}

fn provider() -> ProviderConfig {
    ProviderConfig {
        provider: "claude-code".to_string(),
        base_url: BASE_URL.to_string(),
        model: MODEL.to_string(),
        temperature: 0.0,
    }
}

async fn run_text_probe(conf: &TuraConfig) -> Result<(), String> {
    let messages = vec![json!({
        "role": "user",
        "content": "Reply with the single word OK and nothing else."
    })];
    let options = CallOptions {
        max_tokens: Some(64),
        ..Default::default()
    };

    let response = provider()
        .call(conf, messages, options)
        .await
        .map_err(|err| err.to_string())?;

    let text = response
        .content
        .as_str()
        .ok_or_else(|| format!("expected string content, got {}", response.content))?;
    if text.trim().is_empty() {
        return Err("empty text response".to_string());
    }
    require_usage(&response)?;
    Ok(())
}

async fn run_tool_call_probe(conf: &TuraConfig) -> Result<(), String> {
    let messages = vec![json!({
        "role": "user",
        "content": "Call the echo_answer tool exactly once with answer set to pong."
    })];
    let options = CallOptions {
        max_tokens: Some(256),
        tools: Some(vec![json!({
            "type": "function",
            "function": {
                "name": "echo_answer",
                "description": "Return a tiny answer for the claude-code smoke test.",
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

    let response = provider()
        .call(conf, messages, options)
        .await
        .map_err(|err| err.to_string())?;

    if !contains_echo_answer(&response.content) {
        return Err(format!(
            "response did not contain echo_answer tool call: {}",
            response.content
        ));
    }
    // The native path must surface the call in the OpenAI-shaped content the
    // runtime state machine reads, not only in the raw payload.
    let tool_calls = response
        .content
        .get("tool_calls")
        .and_then(Value::as_array)
        .ok_or_else(|| "content.tool_calls array missing".to_string())?;
    let first = tool_calls.first().ok_or("no tool calls present")?;
    if first.pointer("/function/name").and_then(Value::as_str) != Some("echo_answer") {
        return Err("first tool call was not echo_answer".to_string());
    }
    if first.pointer("/function/arguments/answer").and_then(Value::as_str) != Some("pong") {
        return Err("echo_answer arguments did not contain answer=pong".to_string());
    }

    let metrics = response.metrics.as_ref().ok_or("missing metrics")?;
    if metrics.tool_call_count == 0 {
        return Err("metrics did not record the tool call".to_string());
    }
    Ok(())
}

fn require_usage(response: &ProviderResponse) -> Result<(), String> {
    let metrics = response.metrics.as_ref().ok_or("missing metrics")?;
    if metrics.usage.input_tokens.unwrap_or(0) == 0 {
        return Err("usage input_tokens missing".to_string());
    }
    if metrics.usage.output_tokens.unwrap_or(0) == 0 {
        return Err("usage output_tokens missing".to_string());
    }
    Ok(())
}

fn contains_echo_answer(value: &Value) -> bool {
    match value {
        Value::Object(object) => {
            object
                .get("function")
                .and_then(|function| function.get("name"))
                .and_then(Value::as_str)
                .is_some_and(|name| name == "echo_answer")
                || object.values().any(contains_echo_answer)
        }
        Value::Array(items) => items.iter().any(contains_echo_answer),
        Value::String(text) => text.contains("echo_answer"),
        _ => false,
    }
}
