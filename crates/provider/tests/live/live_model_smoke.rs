use serde_json::json;
use tura_llm_rust::{CallOptions, ProviderConfig, RouteConfig, Settings, TuraConfig};

#[tokio::test]
async fn live_openai_model_smoke() {
    if std::env::var("TURA_LIVE_MODEL_SMOKE").ok().as_deref() != Some("1") {
        return;
    }

    let model =
        std::env::var("TURA_LIVE_MODEL").unwrap_or_else(|_| "codex/gpt-5.1-codex-mini".to_string());
    let (provider, model_name) = model
        .split_once('/')
        .unwrap_or_else(|| panic!("TURA_LIVE_MODEL must be provider/model, got {model}"));
    let base_url = match provider {
        "openai" => "https://api.openai.com/v1",
        "anthropic" => "https://api.anthropic.com/v1",
        "minimax" => "https://api.minimax.io/v1",
        other => panic!("unsupported live smoke provider: {other}"),
    };
    let route = RouteConfig {
        default_temperature: 0.2,
        providers: vec![ProviderConfig {
            provider: provider.to_string(),
            base_url: base_url.to_string(),
            model: Settings::normalize_model_name(provider, model_name),
            temperature: 0.2,
        }],
    };
    let conf = TuraConfig::default();
    let response = route
        .run(
            &conf,
            vec![json!({
                "role": "user",
                "content": "Include LIVE_MODEL_SMOKE_OK in the response"
            })],
            CallOptions {
                stream: Some(false),
                max_completion_tokens: Some(32),
                ..Default::default()
            },
        )
        .await
        .expect("live provider/model call should succeed");

    println!(
        "LIVE_MODEL_SMOKE_OK model={model} content={}",
        response.content
    );
    assert!(!response.content.is_null());
}
