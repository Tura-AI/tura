use super::*;

pub async fn tui_action(payload: Option<Json<serde_json::Value>>) -> Json<serde_json::Value> {
    let payload = payload
        .map(|Json(payload)| payload)
        .unwrap_or(serde_json::Value::Null);
    let action = payload
        .get("action")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("submit-prompt");
    if matches!(
        action,
        "submit-prompt" | "append-prompt" | "execute-command"
    ) {
        let content = payload
            .get("prompt")
            .or_else(|| payload.get("input"))
            .or_else(|| payload.get("message"))
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("Prompt submitted")
            .to_string();
        let session_id = payload
            .get("sessionID")
            .or_else(|| payload.get("session_id"))
            .and_then(serde_json::Value::as_str)
            .map(str::to_string)
            .unwrap_or_else(|| {
                session_store()
                    .create_session(
                        global_store().get_current_directory(),
                        None,
                        None,
                        Some("coding".to_string()),
                        false,
                        false,
                        false,
                        None,
                        false,
                        false,
                    )
                    .id
            });
        let prompt_payload = serde_json::json!({
            "parts": [{
                "id": format!("tui-part-{}", uuid::Uuid::new_v4()),
                "type": "text",
                "text": content
            }]
        });
        session_store().clear_cancelled(&session_id);
        let _ = session_store().add_message(&session_id, SessionMessageRole::User, content);
        session_store().update_session_status(&session_id, SessionStatusMano::Busy);
        let session_id_for_task = session_id.clone();
        tokio::task::spawn_blocking(move || {
            run_mano_for_prompt(session_id_for_task, prompt_payload);
        });
        return Json(serde_json::json!({
            "ok": true,
            "action": action,
            "sessionID": session_id,
            "status": "submitted"
        }));
    }
    if action == "clear-prompt" {
        return Json(serde_json::json!({
            "ok": true,
            "action": action,
            "prompt": ""
        }));
    }
    Json(serde_json::json!({
        "ok": true,
        "action": action,
        "payload": payload,
    }))
}
