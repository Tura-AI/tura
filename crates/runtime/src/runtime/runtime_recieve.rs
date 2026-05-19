use crate::state_machine::agent_management::AgentId;
use crate::state_machine::runtime_management::RuntimeId;
use crate::state_machine::session_management::SessionId;
use std::path::PathBuf;

use super::types::{StreamChunkType, ToolCallData};

pub struct RuntimeReceiveInput {
    pub runtime_id: RuntimeId,
    pub session_id: SessionId,
    pub agent_id: AgentId,
    pub raw_stream_data: serde_json::Value,
}

pub struct ProcessedStreamData {
    pub text_chunks: Vec<String>,
    pub tool_calls: Vec<ToolCallData>,
    pub reasoning_chunks: Vec<String>,
}

pub async fn runtime_recieve(input: RuntimeReceiveInput) -> Result<ProcessedStreamData, String> {
    let mut processed = ProcessedStreamData {
        text_chunks: Vec::new(),
        tool_calls: Vec::new(),
        reasoning_chunks: Vec::new(),
    };

    process_stream_data(&input.raw_stream_data, &mut processed)?;

    Ok(processed)
}

pub async fn execute_runtime_stream_event(
    event: tura_llm_rust::ProviderStreamEvent,
    session_directory: PathBuf,
) -> Option<serde_json::Value> {
    match event {
        tura_llm_rust::ProviderStreamEvent::CommandRunCommandReady { command, .. } => Some(
            code_tools::command_run::execute_streamed_command_value(command, session_directory)
                .await,
        ),
        tura_llm_rust::ProviderStreamEvent::ProviderOutputStarted => None,
    }
}

pub async fn execute_runtime_stream_command_batch(
    commands: Vec<serde_json::Value>,
    session_directory: PathBuf,
) -> Option<serde_json::Value> {
    if commands.is_empty() {
        return None;
    }
    Some(
        code_tools::command_run::execute_async_value(
            serde_json::json!({ "commands": commands }),
            session_directory,
        )
        .await,
    )
}

pub fn command_run_stream_event_command(
    event: tura_llm_rust::ProviderStreamEvent,
) -> Option<serde_json::Value> {
    match event {
        tura_llm_rust::ProviderStreamEvent::CommandRunCommandReady { command, .. } => Some(command),
        tura_llm_rust::ProviderStreamEvent::ProviderOutputStarted => None,
    }
}

fn process_stream_data(
    raw_data: &serde_json::Value,
    processed: &mut ProcessedStreamData,
) -> Result<(), String> {
    if let Some(array) = raw_data.as_array() {
        for item in array {
            process_single_chunk(item, processed)?;
        }
    } else if raw_data.as_object().is_some() {
        process_single_chunk(raw_data, processed)?;
    } else if let Some(text) = raw_data.as_str() {
        processed.text_chunks.push(text.to_string());
    }

    Ok(())
}

fn process_single_chunk(
    chunk: &serde_json::Value,
    processed: &mut ProcessedStreamData,
) -> Result<(), String> {
    let chunk_type = determine_chunk_type(chunk);

    match chunk_type {
        StreamChunkType::Text => {
            if let Some(text) = extract_text_content(chunk) {
                processed.text_chunks.push(text);
            }
        }
        StreamChunkType::ToolCall => {
            if let Some(tool_call) = extract_tool_call(chunk) {
                processed.tool_calls.push(tool_call);
            }
        }
        StreamChunkType::Reasoning => {
            if let Some(reasoning) = extract_reasoning_content(chunk) {
                processed.reasoning_chunks.push(reasoning);
            }
        }
        StreamChunkType::Done | StreamChunkType::Error => {}
    }

    Ok(())
}

fn determine_chunk_type(chunk: &serde_json::Value) -> StreamChunkType {
    if let Some(type_field) = chunk.get("type").and_then(|t| t.as_str()) {
        match type_field {
            "text" => return StreamChunkType::Text,
            "tool_call" | "function_call" | "tool_use" => return StreamChunkType::ToolCall,
            "reasoning" | "thinking" => return StreamChunkType::Reasoning,
            "done" | "stop" => return StreamChunkType::Done,
            "error" => return StreamChunkType::Error,
            _ => {}
        }
    }

    if chunk.get("tool_calls").is_some()
        || chunk.get("function_call").is_some()
        || chunk.get("function_call_arguments").is_some()
    {
        return StreamChunkType::ToolCall;
    }

    if chunk.get("reasoning").is_some() || chunk.get("thinking").is_some() {
        return StreamChunkType::Reasoning;
    }

    StreamChunkType::Text
}

fn extract_text_content(chunk: &serde_json::Value) -> Option<String> {
    if let Some(text) = chunk.get("text").and_then(|t| t.as_str()) {
        return Some(text.to_string());
    }
    if let Some(content) = chunk.get("content").and_then(|c| c.as_str()) {
        return Some(content.to_string());
    }
    if let Some(delta) = chunk.get("delta").and_then(|d| d.as_str()) {
        return Some(delta.to_string());
    }
    if let Some(delta_obj) = chunk.get("delta").and_then(|d| d.as_object()) {
        if let Some(text) = delta_obj.get("text").and_then(|t| t.as_str()) {
            return Some(text.to_string());
        }
    }
    None
}

fn extract_tool_call(chunk: &serde_json::Value) -> Option<ToolCallData> {
    let tool_name = chunk
        .get("tool_calls")
        .and_then(|tc| tc.as_array())
        .and_then(|arr| arr.first())
        .and_then(|tc| tc.get("function"))
        .and_then(|f| f.get("name"))
        .and_then(|n| n.as_str())
        .or_else(|| {
            chunk
                .get("function_call")
                .and_then(|fc| fc.get("name"))
                .and_then(|n| n.as_str())
        })
        .or_else(|| chunk.get("name").and_then(|n| n.as_str()))?
        .to_string();

    let arguments = chunk
        .get("tool_calls")
        .and_then(|tc| tc.as_array())
        .and_then(|arr| arr.first())
        .and_then(|tc| tc.get("function"))
        .and_then(|f| f.get("arguments"))
        .cloned()
        .or_else(|| {
            chunk
                .get("function_call")
                .and_then(|fc| fc.get("arguments"))
                .cloned()
        })
        .or_else(|| chunk.get("arguments").cloned())
        .unwrap_or(serde_json::Value::Null);

    Some(ToolCallData {
        tool_name,
        arguments,
    })
}

fn extract_reasoning_content(chunk: &serde_json::Value) -> Option<String> {
    chunk
        .get("reasoning")
        .and_then(|r| r.as_str())
        .map(|s| s.to_string())
        .or_else(|| {
            chunk
                .get("thinking")
                .and_then(|t| t.as_str())
                .map(|s| s.to_string())
        })
}

pub async fn enqueue_tool_calls(
    tool_calls: &[ToolCallData],
    session_id: &SessionId,
    redis_url: &str,
) -> Result<(), String> {
    if tool_calls.is_empty() {
        return Ok(());
    }

    let client = redis::Client::open(redis_url)
        .map_err(|e| format!("failed to create redis client: {}", e))?;

    let mut con = client
        .get_multiplexed_async_connection()
        .await
        .map_err(|e| format!("failed to get redis connection: {}", e))?;

    let queue_key = format!("tool_router:queue:{}", session_id);

    for tool_call in tool_calls {
        let payload = serde_json::to_string(tool_call)
            .map_err(|e| format!("failed to serialize tool call: {}", e))?;

        redis::cmd("RPUSH")
            .arg(&queue_key)
            .arg(&payload)
            .query_async::<_, ()>(&mut con)
            .await
            .map_err(|e| format!("failed to enqueue tool call: {}", e))?;
    }

    Ok(())
}
