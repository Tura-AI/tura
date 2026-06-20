//! Gated live check for the terminal final-response request shape.
//!
//! It verifies the final turn keeps the same tool schema for prompt-cache
//! stability while using `tool_choice: "none"` to prevent another tool call.
//! Set `TURA_FINAL_TURN_CACHE_TOOL_CHOICE=none` to run the no-tool-choice
//! comparison variant.
//!
//! ```text
//! TURA_FINAL_TURN_CACHE_LIVE=1 cargo test -p provider --features live-tests \
//!     --test final_turn_cache_live -- --nocapture
//! ```

use serde_json::{json, Value};
use std::io;
use tura_llm_rust::tura_conf::TuraConfig;
use tura_llm_rust::tura_llm::{CallOptions, ProviderConfig, ProviderResponse};
use uuid::Uuid;

const FINAL_RESPONSE_INSTRUCTION: &str = "The task was marked done. Now send the user-facing assistant reply directly, without calling tools and without mentioning task_status, command_run, or internal status updates.";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    if std::env::var("TURA_FINAL_TURN_CACHE_LIVE").ok().as_deref() != Some("1") {
        println!("skipping final-turn cache live test; set TURA_FINAL_TURN_CACHE_LIVE=1");
        return Ok(());
    }

    let conf = TuraConfig::new(".env");
    if conf
        .get("OPENAI_API_KEY")
        .filter(|value| !value.trim().is_empty())
        .is_none()
    {
        println!("skipping final-turn cache live test; missing OPENAI_API_KEY");
        return Ok(());
    }

    let model = std::env::var("TURA_FINAL_TURN_CACHE_MODEL")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "gpt-5.5".to_string());
    let provider = ProviderConfig {
        provider: "openai".to_string(),
        base_url: "https://api.openai.com/v1".to_string(),
        model,
        temperature: 0.0,
    };
    let run_nonce = format!("final-turn-cache-live-{}", Uuid::new_v4().simple());
    let cache_key = format!("tura-final-turn-cache-live-{run_nonce}");
    let final_tool_choice = std::env::var("TURA_FINAL_TURN_CACHE_TOOL_CHOICE")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "auto".to_string());
    let tools = vec![echo_answer_tool()];
    let stable_prefix = stable_cache_prefix(&run_nonce);
    let first_messages = vec![
        json!({"role": "system", "content": stable_prefix}),
        json!({
            "role": "user",
            "content": "Call echo_answer exactly once with answer set to pong. Do not answer in prose."
        }),
    ];

    let first = provider
        .call(
            &conf,
            first_messages.clone(),
            call_options(tools.clone(), Some(json!("auto")), cache_key.clone()),
        )
        .await?;
    let tool_call = first_tool_call(&first).ok_or_else(|| {
        io::Error::other(format!(
            "first response did not contain echo_answer tool call: {}",
            first.raw
        ))
    })?;

    let mut final_messages = first_messages;
    final_messages.push(tool_call.clone());
    final_messages.push(json!({
        "type": "function_call_output",
        "call_id": tool_call
            .get("call_id")
            .and_then(Value::as_str)
            .unwrap_or("final-turn-cache-live-call"),
        "output": "{\"answer\":\"pong\"}"
    }));
    final_messages.push(json!({
        "role": "system",
        "content": FINAL_RESPONSE_INSTRUCTION
    }));

    let warm_response = provider
        .call(
            &conf,
            final_messages.clone(),
            call_options(
                tools.clone(),
                Some(json!(final_tool_choice.clone())),
                cache_key.clone(),
            ),
        )
        .await?;
    assert_no_final_tool_call(&warm_response, &final_tool_choice);
    let warm_cached = cached_input_tokens(&warm_response);

    let min_ratio = min_cache_ratio();
    let probe_attempts = probe_attempts();
    let mut last_response = None;
    for attempt in 1..=probe_attempts {
        let final_response = provider
            .call(
                &conf,
                final_messages.clone(),
                call_options(
                    tools.clone(),
                    Some(json!(final_tool_choice.clone())),
                    cache_key.clone(),
                ),
            )
            .await?;
        assert_no_final_tool_call(&final_response, &final_tool_choice);
        let final_metrics = final_response
            .metrics
            .as_ref()
            .ok_or_else(|| io::Error::other("final response did not include metrics"))?;
        let cached = final_metrics.usage.cached_input_tokens.unwrap_or(0);
        let input = final_metrics.usage.input_tokens.unwrap_or(0);
        let ratio = cache_ratio(cached, input).ok_or_else(|| {
            io::Error::other(format!(
                "final response did not report positive input_tokens: {final_metrics:#?}"
            ))
        })?;
        if cached > 0 && ratio >= min_ratio {
            println!(
                "PASS final-turn cache live: final_tool_choice={}, warm_cached_input_tokens={}, probe_attempts={}, cached_input_tokens={}, input_tokens={}, cache_ratio={:.2}%, output_tokens={:?}, tool_call_count={}, contains_tool_call={}",
                final_tool_choice,
                warm_cached,
                attempt,
                cached,
                input,
                ratio * 100.0,
                final_metrics.usage.output_tokens,
                final_metrics.tool_call_count,
                contains_tool_call(&final_response.raw)
            );
            return Ok(());
        }
        last_response = Some(final_response);
        std::thread::sleep(std::time::Duration::from_millis(750));
    }
    let final_response = last_response.expect("at least one probe attempt should run");
    let final_metrics = final_response
        .metrics
        .as_ref()
        .ok_or_else(|| io::Error::other("final response did not include metrics"))?;
    let cached = final_metrics.usage.cached_input_tokens.unwrap_or(0);
    let input = final_metrics.usage.input_tokens.unwrap_or(0);
    let ratio = cache_ratio(cached, input).ok_or_else(|| {
        io::Error::other(format!(
            "final response did not report positive input_tokens: {final_metrics:#?}"
        ))
    })?;
    assert!(
        cached > 0,
        "final response probe should hit prompt cache after explicit warm request; warm_cached_input_tokens={}; probe_attempts={}; metrics={:#?}; raw={}",
        warm_cached,
        probe_attempts,
        final_metrics,
        final_response.raw
    );
    assert!(
        ratio >= min_ratio,
        "final response probe prompt cache hit is too small after explicit warm request; warm_cached_input_tokens={}; probe_attempts={}; cached/input ratio {:.2}% < {:.2}%; cached_input_tokens={}; input_tokens={}; metrics={:#?}; raw={}",
        warm_cached,
        probe_attempts,
        ratio * 100.0,
        min_ratio * 100.0,
        cached,
        input,
        final_metrics,
        final_response.raw
    );
    Ok(())
}

fn call_options(
    tools: Vec<Value>,
    tool_choice: Option<Value>,
    prompt_cache_key: String,
) -> CallOptions {
    CallOptions {
        temperature: Some(0.0),
        max_tokens: Some(128),
        stream: Some(true),
        stream_options: Some(json!({"include_usage": true})),
        reasoning_effort: Some("low".to_string()),
        prompt_cache_key: Some(prompt_cache_key),
        tools: Some(tools),
        tool_choice,
        store: Some(false),
        ..Default::default()
    }
}

fn echo_answer_tool() -> Value {
    json!({
        "type": "function",
        "function": {
            "name": "echo_answer",
            "description": "Return a tiny answer for final-turn prompt-cache live tests.",
            "parameters": {
                "type": "object",
                "properties": {
                    "answer": { "type": "string", "enum": ["pong"] }
                },
                "required": ["answer"],
                "additionalProperties": false
            }
        }
    })
}

fn stable_cache_prefix(run_nonce: &str) -> String {
    let block = format!(
        "Stable prompt-cache prefix. run_nonce={run_nonce}. Keep this exact text available for the next request. "
    );
    format!(
        "{}\nYou are testing terminal final-response behavior. The final answer must be plain text.",
        block.repeat(900)
    )
}

fn first_tool_call(response: &ProviderResponse) -> Option<Value> {
    if let Some(output) = response.raw.get("output").and_then(Value::as_array) {
        if let Some(item) = output.iter().find(|item| is_echo_answer_tool_call(item)) {
            return Some(item.clone());
        }
    }
    find_tool_call(&response.raw)
}

fn find_tool_call(value: &Value) -> Option<Value> {
    match value {
        Value::Object(object) => {
            if is_echo_answer_tool_call(value) {
                return Some(value.clone());
            }
            if object
                .get("function")
                .and_then(|function| function.get("name"))
                .and_then(Value::as_str)
                .is_some_and(|name| name == "echo_answer")
            {
                return Some(value.clone());
            }
            object.values().find_map(find_tool_call)
        }
        Value::Array(items) => items.iter().find_map(find_tool_call),
        _ => None,
    }
}

fn cached_input_tokens(response: &ProviderResponse) -> u64 {
    response
        .metrics
        .as_ref()
        .and_then(|metrics| metrics.usage.cached_input_tokens)
        .unwrap_or(0)
}

fn cache_ratio(cached_input_tokens: u64, input_tokens: u64) -> Option<f64> {
    (input_tokens > 0).then_some(cached_input_tokens as f64 / input_tokens as f64)
}

fn min_cache_ratio() -> f64 {
    std::env::var("TURA_FINAL_TURN_CACHE_MIN_RATIO")
        .ok()
        .and_then(|value| value.trim().parse::<f64>().ok())
        .filter(|ratio| (0.0..=1.0).contains(ratio))
        .unwrap_or(0.80)
}

fn probe_attempts() -> usize {
    std::env::var("TURA_FINAL_TURN_CACHE_PROBE_ATTEMPTS")
        .ok()
        .and_then(|value| value.trim().parse::<usize>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(3)
}

fn assert_no_final_tool_call(response: &ProviderResponse, final_tool_choice: &str) {
    let tool_call_count = response
        .metrics
        .as_ref()
        .map(|metrics| metrics.tool_call_count)
        .unwrap_or(0);
    assert_eq!(
        tool_call_count, 0,
        "final turn should not produce tool calls; tool_choice={}; raw={}",
        final_tool_choice, response.raw
    );
    assert!(
        !contains_tool_call(&response.raw),
        "final response raw output should not include echo_answer tool call; tool_choice={}; raw={}",
        final_tool_choice,
        response.raw
    );
}

fn is_echo_answer_tool_call(value: &Value) -> bool {
    value
        .get("type")
        .and_then(Value::as_str)
        .is_some_and(|kind| kind == "function_call")
        && value
            .get("name")
            .and_then(Value::as_str)
            .is_some_and(|name| name == "echo_answer")
}

fn contains_tool_call(value: &Value) -> bool {
    find_tool_call(value).is_some()
}
