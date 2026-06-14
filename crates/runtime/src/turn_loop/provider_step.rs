use chrono::Utc;

use crate::manas::{user_visible_runtime_output_text, user_visible_runtime_text};
use crate::provider_flow::usage::runtime_cache_diagnostics;
use crate::state_machine::runtime_management::RuntimeManagement;
use crate::state_machine::session_management::SessionManagement;
use crate::{gateway_events::stream_agent_message_id, gateway_events::stream_agent_part_id};

pub(crate) fn accumulate_session_from_runtime(
    session: &mut SessionManagement,
    runtime: &RuntimeManagement,
    publish_runtime_text: bool,
) -> Result<(), String> {
    let now = Utc::now();

    if let Some(usage) = &runtime.usage {
        session.push_log(
            serde_json::json!({
                "type": "runtime_usage",
                "runtime_id": runtime.runtime_id,
                "usage": usage,
                "status": format!("{:?}", runtime.call_result_status),
                "cache_diagnostics": runtime_cache_diagnostics(runtime),
                "timestamp": now.to_rfc3339(),
            })
            .to_string(),
            now,
        );
    }

    if !publish_runtime_text {
        return Ok(());
    }

    let visible_text = user_visible_runtime_text(&runtime.text).or_else(|| {
        runtime
            .output
            .as_ref()
            .and_then(user_visible_runtime_output_text)
    });

    if let Some(content) = visible_text {
        session.push_log(
            serde_json::json!({
                "id": stream_agent_message_id(&runtime.runtime_id),
                "role": "assistant",
                "content": content,
                "part_id": stream_agent_part_id(&runtime.runtime_id),
                "runtime_id": runtime.runtime_id,
                "created_at": now.timestamp_millis(),
                "updated_at": now.timestamp_millis(),
                "timestamp": now.to_rfc3339(),
            })
            .to_string(),
            now,
        );
    }

    Ok(())
}
