//! Global API handlers (health, config, events)

use crate::api::types::*;
use crate::mock::global_store;
use crate::session::session_store;
use axum::{
    response::sse::{Event as SseEvent, KeepAlive, Sse},
    Json,
};
use std::collections::HashMap;
use std::convert::Infallible;
use std::time::Duration;

// ============================================================================
// Health
// ============================================================================

pub async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        healthy: true,
        version: env!("CARGO_PKG_VERSION").to_string(),
    })
}

// ============================================================================
// Config
// ============================================================================

pub async fn get_config() -> Json<Config> {
    Json(global_store().get_config())
}

pub async fn patch_config(Json(payload): Json<ConfigPatch>) -> Json<Config> {
    Json(global_store().update_config(payload))
}

// ============================================================================
// Global Events (SSE)
// ============================================================================

pub async fn global_event() -> Sse<impl futures::Stream<Item = Result<SseEvent, Infallible>>> {
    let state = EventStreamState {
        first: true,
        seen_messages: seen_message_counts(),
    };
    let stream = futures::stream::unfold(state, |mut state| async move {
        loop {
            let event = if state.first {
                state.first = false;
                Some(GlobalEvent::ServerConnected {
                    properties: std::collections::HashMap::new(),
                })
            } else {
                session_store()
                    .pop_event()
                    .or_else(|| scan_message_events(&mut state.seen_messages))
            };

            if let Some(event) = event {
                let directory = event_directory(&event);
                let data = serde_json::json!({
                    "directory": directory,
                    "payload": event,
                });
                let item = SseEvent::default().data(data.to_string());
                return Some((Ok(item), state));
            }

            tokio::time::sleep(Duration::from_millis(250)).await;
        }
    });

    Sse::new(stream).keep_alive(KeepAlive::default())
}

struct EventStreamState {
    first: bool,
    seen_messages: HashMap<String, usize>,
}

fn seen_message_counts() -> HashMap<String, usize> {
    session_store()
        .list_sessions()
        .into_iter()
        .map(|session| {
            let count = session_store().get_messages(&session.id).len();
            (session.id, count)
        })
        .collect()
}

fn scan_message_events(seen: &mut HashMap<String, usize>) -> Option<GlobalEvent> {
    for session in session_store().list_sessions() {
        let messages = session_store().get_messages(&session.id);
        let count = messages.len();
        let previous = seen.entry(session.id.clone()).or_insert(0);
        if count <= *previous {
            continue;
        }

        let message = messages.get(*previous).cloned()?;
        *previous += 1;
        return Some(GlobalEvent::MessageUpdated {
            properties: MessageUpdatedProperties {
                session_id: session.id,
                info: crate::api::session::api_message_from_store(message),
            },
        });
    }

    None
}

fn event_directory(event: &GlobalEvent) -> String {
    let session_id = match event {
        GlobalEvent::SessionCreated { properties } => {
            return properties.info.directory.clone().unwrap_or_default()
        }
        GlobalEvent::SessionUpdated { properties } => {
            return properties.info.directory.clone().unwrap_or_default()
        }
        GlobalEvent::SessionDeleted { properties } => {
            return properties.info.directory.clone().unwrap_or_default()
        }
        GlobalEvent::SessionStatus { properties } => Some(properties.session_id.as_str()),
        GlobalEvent::MessageUpdated { properties } => Some(properties.session_id.as_str()),
        GlobalEvent::MessageRemoved { properties } => Some(properties.session_id.as_str()),
        GlobalEvent::MessagePartDelta { properties } => Some(properties.session_id.as_str()),
        GlobalEvent::MessagePartUpdated { properties } => Some(properties.session_id.as_str()),
        GlobalEvent::TodoUpdated { properties } => {
            properties.get("sessionID").and_then(|value| value.as_str())
        }
        GlobalEvent::ServerInstanceDisposed { properties } => return properties.directory.clone(),
        GlobalEvent::ProjectUpdated { properties } => return properties.worktree.clone(),
        GlobalEvent::ServerConnected { .. } => return "global".to_string(),
    };

    session_id
        .and_then(|id| session_store().get_session(id))
        .and_then(|session| session.directory)
        .unwrap_or_else(|| "global".to_string())
}

pub async fn sync_event() -> Json<SyncEvent> {
    Json(SyncEvent::SessionUpdated {
        properties: global_store().get_or_create_session(),
    })
}

// ============================================================================
// Global Dispose
// ============================================================================

pub async fn dispose() -> Json<bool> {
    Json(true)
}

// ============================================================================
// Global Upgrade
// ============================================================================

pub async fn upgrade(Json(_payload): Json<UpgradeRequest>) -> Json<UpgradeResponse> {
    Json(UpgradeResponse {
        success: false,
        version: Some(env!("CARGO_PKG_VERSION").to_string()),
        error: Some("Self-upgrade is not implemented by this gateway build.".to_string()),
    })
}
