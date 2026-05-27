use serde_json::json;
use std::sync::Arc;
use tura_llm_rust::tura_conf::TuraConfig;
use tura_llm_rust::tura_llm::{CallOptions, Settings};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let settings = Settings::default().await?;
    let conf = TuraConfig::new(".env");

    println!("=== Test 1: OpenAI stream + tool call ===\n");
    test_openai_stream_tool_call(&settings, &conf).await?;

    println!("\n=== Test 2: Google stream + tool call (gemini-3-flash-preview) ===\n");
    test_google_stream_tool_call(&settings, &conf).await?;

    println!("\n=== All tests completed ===");
    Ok(())
}

async fn test_openai_stream_tool_call(
    settings: &Arc<Settings>,
    conf: &TuraConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    let route = &settings.tura_coder;

    let messages = vec![
        json!({"role": "user", "content": "What is your exact model name and version? For example: gpt-4, gemini-2.0-flash, etc."}),
    ];

    let options = CallOptions::default();

    match route.run(conf, messages, options).await {
        Ok(response) => {
            println!("Success! Content: {}", response.content);
        }
        Err(e) => {
            println!("Error: {}", e);
        }
    }
    Ok(())
}

async fn test_google_stream_tool_call(
    settings: &Arc<Settings>,
    conf: &TuraConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    let route = &settings.tura_general;

    let messages = vec![
        json!({"role": "user", "content": "What is your exact model name and version? For example: gpt-4, gemini-2.0-flash, etc."}),
    ];

    let options = CallOptions::default();

    match route.run(conf, messages, options).await {
        Ok(response) => {
            println!("Success! Content: {}", response.content);
        }
        Err(e) => {
            println!("Error: {}", e);
        }
    }
    Ok(())
}
