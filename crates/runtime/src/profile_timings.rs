use serde_json::{Map, Value};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

pub(crate) fn enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        std::env::var("TURA_PROFILE_TURN_TIMINGS")
            .or_else(|_| std::env::var("TURA_PROFILE_TIMINGS"))
            .ok()
            .map(|value| {
                let value = value.trim().to_ascii_lowercase();
                !matches!(value.as_str(), "" | "0" | "false" | "off" | "no")
            })
            .unwrap_or(false)
    })
}

pub(crate) fn byte_sizes_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        std::env::var("TURA_PROFILE_TURN_TIMING_BYTES")
            .or_else(|_| std::env::var("TURA_PROFILE_TIMING_BYTES"))
            .ok()
            .map(|value| {
                let value = value.trim().to_ascii_lowercase();
                !matches!(value.as_str(), "" | "0" | "false" | "off" | "no")
            })
            .unwrap_or(false)
    })
}

pub(crate) fn log_elapsed(label: &str, start: Instant, fields: Value) {
    if !enabled() {
        return;
    }
    log_duration(label, start.elapsed(), fields);
}

pub(crate) fn log_duration(label: &str, elapsed: Duration, fields: Value) {
    if !enabled() {
        return;
    }
    eprintln!(
        "TURA_PROFILE_TIMING {}",
        event_payload(label, Some(elapsed), fields)
    );
}

pub(crate) fn json_bytes(value: &Value) -> usize {
    if !byte_sizes_enabled() {
        return 0;
    }
    serde_json::to_vec(value)
        .map(|bytes| bytes.len())
        .unwrap_or(0)
}

pub(crate) fn json_vec_bytes(values: &[Value]) -> usize {
    if !byte_sizes_enabled() {
        return 0;
    }
    serde_json::to_vec(values)
        .map(|bytes| bytes.len())
        .unwrap_or(0)
}

fn event_payload(label: &str, elapsed: Option<Duration>, fields: Value) -> Value {
    let mut payload = match fields {
        Value::Object(fields) => fields,
        other => {
            let mut fields = Map::new();
            fields.insert("fields".to_string(), other);
            fields
        }
    };
    payload.insert("label".to_string(), Value::String(label.to_string()));
    if let Some(elapsed) = elapsed {
        payload.insert(
            "elapsed_us".to_string(),
            Value::Number((elapsed.as_micros() as u64).into()),
        );
        payload.insert(
            "elapsed_ms".to_string(),
            Value::Number((elapsed.as_millis() as u64).into()),
        );
    }
    Value::Object(payload)
}
