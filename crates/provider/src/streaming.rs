use std::time::{Duration, Instant};

use futures_util::StreamExt;

use crate::tura_llm::TuraError;

pub fn provider_first_output_timeout() -> Duration {
    provider_timeout_from_env(
        "TURA_PROVIDER_FIRST_OUTPUT_TIMEOUT_MS",
        crate::tura_llm::provider_latency_timeouts().first_output_timeout_ms,
    )
}

pub fn provider_idle_output_timeout() -> Duration {
    provider_timeout_from_env(
        "TURA_PROVIDER_IDLE_OUTPUT_TIMEOUT_MS",
        crate::tura_llm::provider_latency_timeouts().idle_output_timeout_ms,
    )
}

/// Upper bound for reading a full non-streaming response body. The headers may
/// arrive promptly (passing [`send_provider_request_first_response`]) while the
/// upstream holds the connection open during a long reasoning phase; without
/// this bound `resp.json()` can hang indefinitely. Honors
/// `TURA_PROVIDER_TOTAL_TIMEOUT_MS`.
pub fn provider_total_timeout() -> Duration {
    provider_timeout_from_env(
        "TURA_PROVIDER_TOTAL_TIMEOUT_MS",
        crate::tura_llm::provider_latency_timeouts().total_timeout_ms,
    )
}

/// Await a non-streaming response body future under [`provider_total_timeout`]
/// so a stalled upstream cannot block the call forever.
pub async fn read_provider_response_body<T, F>(future: F) -> Result<T, TuraError>
where
    F: std::future::Future<Output = Result<T, reqwest::Error>>,
{
    let limit = provider_total_timeout();
    match tokio::time::timeout(limit, future).await {
        Ok(Ok(value)) => Ok(value),
        Ok(Err(err)) => Err(TuraError::Network {
            message: err.to_string(),
        }),
        Err(_) => Err(TuraError::Network {
            message: format!(
                "provider response body timed out after {} ms (no complete body received)",
                limit.as_millis()
            ),
        }),
    }
}

pub async fn send_provider_request_first_response(
    request: reqwest::RequestBuilder,
) -> Result<reqwest::Response, TuraError> {
    let limit = provider_first_output_timeout();
    match tokio::time::timeout(limit, request.send()).await {
        Ok(Ok(response)) => Ok(response),
        Ok(Err(err)) => Err(TuraError::Network {
            message: err.to_string(),
        }),
        Err(_) => Err(provider_timeout_error(false, limit)),
    }
}

pub async fn next_provider_stream_chunk<S>(
    stream: &mut S,
    saw_output: bool,
    last_output_at: Instant,
) -> Result<Option<S::Item>, TuraError>
where
    S: futures_util::Stream + Unpin,
{
    let limit = if saw_output {
        provider_idle_output_timeout()
    } else {
        provider_first_output_timeout()
    };
    let elapsed = last_output_at.elapsed();
    if elapsed >= limit {
        return Err(provider_timeout_error(saw_output, limit));
    }
    match tokio::time::timeout(limit - elapsed, stream.next()).await {
        Ok(next) => Ok(next),
        Err(_) => Err(provider_timeout_error(saw_output, limit)),
    }
}

pub fn provider_timeout_error(saw_output: bool, limit: Duration) -> TuraError {
    let phase = if saw_output {
        "new provider output"
    } else {
        "first provider output"
    };
    TuraError::Network {
        message: format!(
            "provider stream timed out waiting for {phase} after {} ms",
            limit.as_millis()
        ),
    }
}

fn provider_timeout_from_env(name: &str, default_ms: u64) -> Duration {
    std::env::var(name)
        .ok()
        .and_then(|value| value.trim().parse::<u64>().ok())
        .filter(|value| *value > 0)
        .map(Duration::from_millis)
        .unwrap_or_else(|| Duration::from_millis(default_ms))
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use futures_util::stream;

    use super::{next_provider_stream_chunk, provider_timeout_error};

    #[test]
    fn timeout_error_names_first_and_idle_phases() {
        let first = provider_timeout_error(false, Duration::from_millis(7)).to_string();
        let idle = provider_timeout_error(true, Duration::from_millis(9)).to_string();

        assert!(first.contains("first provider output"));
        assert!(first.contains("7 ms"));
        assert!(idle.contains("new provider output"));
        assert!(idle.contains("9 ms"));
    }

    #[tokio::test]
    async fn next_provider_stream_chunk_returns_available_chunk() {
        let mut items = stream::iter([Ok::<_, std::io::Error>("hello")]);
        let next = next_provider_stream_chunk(&mut items, false, Instant::now())
            .await
            .expect("stream chunk result");

        assert_eq!(next.expect("chunk").expect("ok"), "hello");
    }
}
