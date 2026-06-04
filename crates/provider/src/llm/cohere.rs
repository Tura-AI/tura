use serde_json::{json, Value};

use crate::streaming::send_provider_request_first_response;
use crate::tura_llm::{default_client, TuraError};

pub async fn embed(
    base_url: &str,
    model: &str,
    api_key: &str,
    text: &str,
) -> Result<Vec<f32>, TuraError> {
    let client = default_client(api_key)?;
    let url = format!("{}/embed", base_url.trim_end_matches('/'));
    let payload = json!({
        "model": model,
        "texts": [text],
        "input_type": "search_document",
        "embedding_types": ["float"],
    });
    let resp = send_provider_request_first_response(client.post(url).json(&payload)).await?;
    let status = resp.status();
    let data: Value = resp.json().await.map_err(|e| TuraError::Network {
        message: e.to_string(),
    })?;
    if !status.is_success() {
        return Err(TuraError::HttpStatus {
            status: status.as_u16(),
            body: data.to_string(),
        });
    }
    let embedding = data
        .pointer("/embeddings/float/0")
        .or_else(|| data.pointer("/embeddings/0"))
        .and_then(Value::as_array)
        .ok_or_else(|| TuraError::ProviderRequest {
            provider: "cohere".into(),
            message: "missing embedding vector".into(),
        })?;
    Ok(embedding
        .iter()
        .filter_map(Value::as_f64)
        .map(|v| v as f32)
        .collect())
}
