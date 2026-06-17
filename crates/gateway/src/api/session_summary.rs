use super::*;
use crate::contracts::SummaryResponse;

pub async fn summarize_session(Path(session_id): Path<String>) -> Json<SummaryResponse> {
    let messages = session_store().get_frontend_messages(&session_id);
    let message_count = messages.len();
    let mut user_count = 0;
    let mut assistant_count = 0;
    let mut tool_count = 0;
    let mut snippets = Vec::new();

    for message in messages.iter().rev().take(8).rev() {
        match message.role {
            SessionMessageRole::User => user_count += 1,
            SessionMessageRole::Assistant => assistant_count += 1,
            SessionMessageRole::System => {}
        }
        for part in &message.parts {
            if part.tool.is_some() {
                tool_count += 1;
            }
            let text = part
                .text
                .as_deref()
                .or(part.content.as_deref())
                .map(str::trim)
                .filter(|value| !value.is_empty());
            if let Some(text) = text {
                snippets.push(format!(
                    "{}: {}",
                    match message.role {
                        SessionMessageRole::User => "User",
                        SessionMessageRole::Assistant => "Assistant",
                        SessionMessageRole::System => "System",
                    },
                    truncate_summary_text(text, 180)
                ));
            }
        }
    }

    let summary = if snippets.is_empty() {
        format!(
            "Session {session_id} has {message_count} stored messages and no textual summary content yet."
        )
    } else {
        format!(
            "Session {session_id}: {message_count} messages ({user_count} user, {assistant_count} assistant), {tool_count} tool parts. Recent context:\n{}",
            snippets.join("\n")
        )
    };

    Json(SummaryResponse { summary })
}
