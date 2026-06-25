use chrono::Utc;
use serde::Deserialize;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use crate::state_machine::session_management::SessionManagement;

pub const RUNTIME_PROMPT_MANUAL_RECORD_TYPE: &str = "runtime_prompt_manual";
pub const RUNTIME_PROMPT_COMMAND_RUN_CAPABILITY_RECORD_TYPE: &str =
    "runtime_prompt_command_run_capabilities";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimePromptManual {
    pub id: String,
    pub display_name: String,
    pub description: String,
    pub father_ids: Vec<String>,
    pub capabilities: Vec<String>,
    pub prompt: String,
}

#[derive(Debug, Deserialize)]
struct RuntimePromptIdentity {
    id: String,
    display_name: String,
    description: String,
    #[serde(default)]
    father_ids: Vec<String>,
    #[serde(default)]
    capabilities: Vec<String>,
}

struct EmbeddedRuntimePromptManual {
    identity_json: &'static str,
    prompt: &'static str,
}

include!(concat!(env!("OUT_DIR"), "/runtime_prompt_manuals.rs"));

pub fn available_manuals() -> Vec<RuntimePromptManual> {
    runtime_prompt_root()
        .and_then(|root| read_manuals_from_dir(&root).ok())
        .filter(|manuals| !manuals.is_empty())
        .unwrap_or_else(static_manuals)
}

pub fn valid_task_type_ids() -> Vec<String> {
    available_manuals()
        .into_iter()
        .map(|manual| manual.id)
        .collect()
}

pub fn normalize_task_type_ids<'a>(ids: impl IntoIterator<Item = &'a str>) -> Vec<String> {
    let manuals = available_manuals();
    let manuals_by_id = manuals
        .iter()
        .map(|manual| (manual.id.clone(), manual))
        .collect::<HashMap<_, _>>();
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for id in ids {
        let id = id.trim();
        if id.is_empty() {
            continue;
        }
        append_task_type_with_fathers(id, &manuals_by_id, &mut seen, &mut out, &mut Vec::new());
    }
    out
}

pub fn task_type_ids_from_value(value: &Value) -> Vec<String> {
    let Some(items) = value.as_array() else {
        return Vec::new();
    };
    normalize_task_type_ids(items.iter().filter_map(Value::as_str))
}

pub fn task_type_catalog_for_prompt() -> String {
    available_manuals()
        .into_iter()
        .map(|manual| format!("- \"{}\": \"{}\"", manual.id, manual.description))
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn task_type_catalog_for_schema_description() -> String {
    available_manuals()
        .into_iter()
        .map(|manual| format!("\"{}\": \"{}\"", manual.id, manual.description))
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn capabilities_for_task_type_ids(ids: &[String]) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for manual in manuals_for_task_type_ids(ids) {
        for capability in manual.capabilities {
            let capability = code_tools::commands::canonical_command(&capability);
            if capability == "command_run" || capability.is_empty() {
                continue;
            }
            if seen.insert(capability.clone()) {
                out.push(capability);
            }
        }
    }
    out
}

pub fn active_operation_manual_text(session: &SessionManagement) -> Option<String> {
    if !session.goal_mode {
        return None;
    }
    let manuals = manuals_for_task_type_ids(&session.task_type);
    if manuals.is_empty() {
        return None;
    }
    Some(
        manuals
            .into_iter()
            .map(|manual| manual.prompt.trim().to_string())
            .collect::<Vec<_>>()
            .join("\n\n"),
    )
}

pub fn append_missing_runtime_prompt_manuals(
    session: &mut SessionManagement,
    mut current_messages: Option<&mut Vec<Value>>,
) -> Result<bool, String> {
    if session.task_type.is_empty() {
        return Ok(false);
    }
    let manuals = manuals_for_task_type_ids(&session.task_type);
    let mut changed = false;
    for manual in manuals {
        if !runtime_prompt_manual_present_since_last_compact(session, &manual.id) {
            let content = manual.prompt.trim().to_string();
            if !content.is_empty() {
                let now = Utc::now();
                let message = serde_json::json!({
                    "role": "system",
                    "content": content,
                });
                if let Some(messages) = current_messages.as_deref_mut() {
                    messages.push(message);
                }
                let record = serde_json::json!({
                    "type": RUNTIME_PROMPT_MANUAL_RECORD_TYPE,
                    "task_type": manual.id.clone(),
                    "manual_name": manual.display_name.clone(),
                    "role": "system",
                    "content": content,
                    "created_at": now.timestamp_millis(),
                    "updated_at": now.timestamp_millis(),
                    "timestamp": now.to_rfc3339(),
                });
                session.push_log(
                    serde_json::to_string(&record).map_err(|err| err.to_string())?,
                    now,
                );
                changed = true;
            }
        }
        let Some((content, capabilities)) = command_run_capability_content(session, &manual) else {
            continue;
        };
        let now = Utc::now();
        let message = serde_json::json!({
            "role": "system",
            "content": content,
        });
        if let Some(messages) = current_messages.as_deref_mut() {
            messages.push(message);
        }
        let record = serde_json::json!({
            "type": RUNTIME_PROMPT_COMMAND_RUN_CAPABILITY_RECORD_TYPE,
            "task_type": manual.id.clone(),
            "manual_name": manual.display_name.clone(),
            "role": "system",
            "capabilities": capabilities,
            "content": content,
            "created_at": now.timestamp_millis(),
            "updated_at": now.timestamp_millis(),
            "timestamp": now.to_rfc3339(),
        });
        session.push_log(
            serde_json::to_string(&record).map_err(|err| err.to_string())?,
            now,
        );
        session.record_session_capabilities_at(capabilities.iter().map(String::as_str), now);
        changed = true;
    }
    Ok(changed)
}

pub fn append_runtime_prompt_manuals_after_compact(
    session: &mut SessionManagement,
) -> Result<bool, String> {
    append_missing_runtime_prompt_manuals(session, None)
}

fn runtime_prompt_manual_present_since_last_compact(session: &SessionManagement, id: &str) -> bool {
    runtime_prompt_record_present_since_last_compact(session, RUNTIME_PROMPT_MANUAL_RECORD_TYPE, id)
}

fn runtime_prompt_record_present_since_last_compact(
    session: &SessionManagement,
    record_type: &str,
    id: &str,
) -> bool {
    for entry in session.session_log.iter().rev() {
        let Ok(value) = serde_json::from_str::<Value>(entry) else {
            continue;
        };
        if value.get("type").and_then(Value::as_str) == Some("context_compaction") {
            return false;
        }
        if value.get("type").and_then(Value::as_str) == Some(record_type)
            && value.get("task_type").and_then(Value::as_str) == Some(id)
        {
            return true;
        }
    }
    false
}

fn command_run_capability_content(
    session: &SessionManagement,
    manual: &RuntimePromptManual,
) -> Option<(String, Vec<String>)> {
    let mut seen = HashSet::new();
    let mut capabilities = Vec::new();
    let mut command_lines = Vec::new();
    for capability in &manual.capabilities {
        let capability = code_tools::commands::canonical_command(capability);
        if capability == "command_run"
            || capability.is_empty()
            || session.has_session_capability(&capability)
            || !seen.insert(capability.clone())
        {
            continue;
        }
        let Some(line) =
            crate::manas::tool_catalog::command_run_command_format_line(&capability, false)
        else {
            continue;
        };
        capabilities.push(capability);
        command_lines.push(line);
    }
    if command_lines.is_empty() {
        return None;
    }
    Some((
        format!(
            "[runtime_prompt_command_run_capabilities]\nThe active `{}` Operation Manual adds these command_run commands. Treat this system message as an additional command_run command format extension for the current context.\nCommand line formats:\n{}",
            manual.id,
            command_lines.join("\n")
        ),
        capabilities,
    ))
}

fn manuals_for_task_type_ids(ids: &[String]) -> Vec<RuntimePromptManual> {
    let manuals = available_manuals();
    ids.iter()
        .filter_map(|id| manuals.iter().find(|manual| manual.id == *id).cloned())
        .collect()
}

fn append_task_type_with_fathers(
    id: &str,
    manuals_by_id: &HashMap<String, &RuntimePromptManual>,
    seen: &mut HashSet<String>,
    out: &mut Vec<String>,
    visiting: &mut Vec<String>,
) {
    let Some(manual) = manuals_by_id.get(id) else {
        return;
    };
    if visiting.iter().any(|item| item == id) {
        return;
    }
    visiting.push(id.to_string());
    for father_id in &manual.father_ids {
        append_task_type_with_fathers(father_id, manuals_by_id, seen, out, visiting);
    }
    visiting.pop();
    if seen.insert(id.to_string()) {
        out.push(id.to_string());
    }
}

fn runtime_prompt_root() -> Option<PathBuf> {
    std::env::var_os("TURA_RUNTIME_PROMPT_ROOT")
        .map(PathBuf::from)
        .or_else(|| {
            Some(
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("src")
                    .join("runtime_prompt"),
            )
        })
}

fn read_manuals_from_dir(root: &Path) -> Result<Vec<RuntimePromptManual>, String> {
    let entries = std::fs::read_dir(root).map_err(|err| {
        format!(
            "failed to read runtime prompt root {}: {err}",
            root.display()
        )
    })?;
    let mut manuals = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|err| err.to_string())?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let identity_path = path.join("prompt_identity.json");
        let prompt_path = path.join("prompt.md");
        let identity_text = std::fs::read_to_string(&identity_path)
            .map_err(|err| format!("failed to read {}: {err}", identity_path.display()))?;
        let identity: RuntimePromptIdentity = serde_json::from_str(&identity_text)
            .map_err(|err| format!("failed to parse {}: {err}", identity_path.display()))?;
        let prompt = std::fs::read_to_string(&prompt_path)
            .map_err(|err| format!("failed to read {}: {err}", prompt_path.display()))?;
        manuals.push(RuntimePromptManual {
            id: identity.id,
            display_name: identity.display_name,
            description: identity.description,
            father_ids: identity.father_ids,
            capabilities: identity.capabilities,
            prompt,
        });
    }
    manuals.sort_by(|left, right| left.id.cmp(&right.id));
    Ok(manuals)
}

fn static_manuals() -> Vec<RuntimePromptManual> {
    let mut manuals = EMBEDDED_RUNTIME_PROMPT_MANUALS
        .iter()
        .filter_map(|manual| {
            let identity =
                serde_json::from_str::<RuntimePromptIdentity>(manual.identity_json).ok()?;
            Some(RuntimePromptManual {
                id: identity.id,
                display_name: identity.display_name,
                description: identity.description,
                father_ids: identity.father_ids,
                capabilities: identity.capabilities,
                prompt: manual.prompt.to_string(),
            })
        })
        .collect::<Vec<_>>();
    manuals.sort_by(|left, right| left.id.cmp(&right.id));
    manuals
}

#[cfg(test)]
mod tests {
    use super::{
        append_missing_runtime_prompt_manuals, capabilities_for_task_type_ids,
        normalize_task_type_ids, RUNTIME_PROMPT_COMMAND_RUN_CAPABILITY_RECORD_TYPE,
        RUNTIME_PROMPT_MANUAL_RECORD_TYPE,
    };
    use crate::state_machine::session_management::{SessionInput, SessionManagement};
    use chrono::Utc;

    #[test]
    fn normalize_task_type_ids_expands_father_chain() {
        assert_eq!(
            normalize_task_type_ids(["interactive_and_3d"]),
            vec!["visual", "frontend", "interactive_and_3d"]
        );
        assert_eq!(
            normalize_task_type_ids(["frontend"]),
            vec!["visual", "frontend"]
        );
        assert_eq!(
            normalize_task_type_ids(["data_visualization"]),
            vec!["visual", "new_build", "data_visualization"]
        );
        assert_eq!(
            normalize_task_type_ids(["editorial"]),
            vec!["visual", "editorial"]
        );
    }

    #[test]
    fn capabilities_for_task_type_ids_include_father_manuals() {
        let ids = normalize_task_type_ids(["interactive_and_3d"]);

        assert_eq!(
            capabilities_for_task_type_ids(&ids),
            vec![
                "web_discover",
                "generate_media",
                "read_media",
                "apply_patch",
                code_tools::commands::active_shell_command_name()
            ]
        );

        let ids = normalize_task_type_ids(["data_visualization"]);
        assert_eq!(
            capabilities_for_task_type_ids(&ids),
            vec![
                "web_discover",
                "generate_media",
                "read_media",
                "apply_patch",
                code_tools::commands::active_shell_command_name()
            ]
        );

        let ids = normalize_task_type_ids(["editorial"]);
        assert_eq!(
            capabilities_for_task_type_ids(&ids),
            vec!["web_discover", "generate_media", "read_media"]
        );
    }

    #[test]
    fn append_missing_manuals_places_command_capabilities_after_manual() {
        let mut session = SessionManagement::new(
            "runtime-prompt-manual-test".to_string(),
            "runtime prompt manual test".to_string(),
            std::path::PathBuf::from("C:/workspace"),
            false,
            "coding".to_string(),
            SessionInput {
                user_input: "make a visual deck".to_string(),
                file_input: vec![],
                agent: None,
                runtime_context: None,
                planning_mode_override: None,
            },
            "make a visual deck".to_string(),
            Utc::now(),
        );
        session.task_type = normalize_task_type_ids(["visual"]);

        assert!(append_missing_runtime_prompt_manuals(&mut session, None)
            .expect("manuals should append"));

        let records = session
            .session_log
            .iter()
            .filter_map(|entry| serde_json::from_str::<serde_json::Value>(entry).ok())
            .filter_map(|value| {
                let record_type = value.get("type").and_then(serde_json::Value::as_str)?;
                matches!(
                    record_type,
                    RUNTIME_PROMPT_MANUAL_RECORD_TYPE
                        | RUNTIME_PROMPT_COMMAND_RUN_CAPABILITY_RECORD_TYPE
                )
                .then_some(value)
            })
            .collect::<Vec<_>>();

        assert_eq!(records.len(), 2);
        assert_eq!(
            records[0].get("type").and_then(serde_json::Value::as_str),
            Some(RUNTIME_PROMPT_MANUAL_RECORD_TYPE)
        );
        assert_eq!(
            records[1].get("type").and_then(serde_json::Value::as_str),
            Some(RUNTIME_PROMPT_COMMAND_RUN_CAPABILITY_RECORD_TYPE)
        );
        assert_eq!(
            records[1]
                .get("task_type")
                .and_then(serde_json::Value::as_str),
            Some("visual")
        );
        let content = records[1]
            .get("content")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        assert!(content.contains("[runtime_prompt_command_run_capabilities]"));
        assert!(content.contains("- read_media:"));
        assert!(content.contains("- generate_media:"));
    }

    #[test]
    fn append_missing_manuals_dedupes_capability_prompts_against_session_state() {
        let mut session = SessionManagement::new(
            "runtime-prompt-manual-dedupe".to_string(),
            "runtime prompt manual dedupe".to_string(),
            std::path::PathBuf::from("C:/workspace"),
            false,
            "coding".to_string(),
            SessionInput {
                user_input: "Build a 3D WebGL demo".to_string(),
                file_input: vec![],
                agent: None,
                runtime_context: None,
                planning_mode_override: None,
            },
            "Build a 3D WebGL demo".to_string(),
            Utc::now(),
        );
        session.record_session_capabilities(vec![
            "web_discover".to_string(),
            "apply_patch".to_string(),
            code_tools::commands::active_shell_command_name().to_string(),
        ]);
        session.task_type = normalize_task_type_ids(["visual"]);

        assert!(append_missing_runtime_prompt_manuals(&mut session, None)
            .expect("visual manual should append"));

        let records = command_run_capability_records(&session);
        assert_eq!(records.len(), 1);
        assert_eq!(
            records[0]
                .get("task_type")
                .and_then(serde_json::Value::as_str),
            Some("visual")
        );
        assert_eq!(
            records[0].get("capabilities"),
            Some(&serde_json::json!(["generate_media", "read_media"]))
        );
        let content = records[0]
            .get("content")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        assert!(!content.contains("- web_discover:"));
        assert!(session.has_session_capability("generate_media"));
        assert!(session.has_session_capability("read_media"));

        session.task_type = normalize_task_type_ids(["interactive_and_3d"]);

        assert!(append_missing_runtime_prompt_manuals(&mut session, None)
            .expect("3D manual should append"));

        let records = command_run_capability_records(&session);
        assert_eq!(
            records.len(),
            1,
            "interactive_and_3d reuses visual's media capabilities and must not inject a duplicate capability prompt"
        );
        assert_eq!(
            runtime_prompt_manual_log_ids(&session),
            vec!["visual", "frontend", "interactive_and_3d"]
        );
    }

    #[test]
    fn debug_manual_does_not_reinject_agent_shell_capability_prompt() {
        let mut session = SessionManagement::new(
            "runtime-prompt-manual-agent-capability-dedupe".to_string(),
            "runtime prompt manual agent capability dedupe".to_string(),
            std::path::PathBuf::from("C:/workspace"),
            false,
            "coding".to_string(),
            SessionInput {
                user_input: "fix duplicate prompt injection".to_string(),
                file_input: vec![],
                agent: None,
                runtime_context: None,
                planning_mode_override: None,
            },
            "fix duplicate prompt injection".to_string(),
            Utc::now(),
        );
        let active_shell = code_tools::commands::active_shell_command_name();
        session.record_session_capabilities(vec![active_shell.to_string()]);
        session.task_type = normalize_task_type_ids(["debug"]);

        assert!(append_missing_runtime_prompt_manuals(&mut session, None)
            .expect("debug manual should append"));

        assert!(
            command_run_capability_records(&session).is_empty(),
            "debug manual's shell capability is already active through the agent and must not be reinjected"
        );
        assert_eq!(runtime_prompt_manual_log_ids(&session), vec!["debug"]);
        assert!(session.has_session_capability(active_shell));
    }

    fn command_run_capability_records(session: &SessionManagement) -> Vec<serde_json::Value> {
        session
            .session_log
            .iter()
            .filter_map(|entry| serde_json::from_str::<serde_json::Value>(entry).ok())
            .filter(|value| {
                value.get("type").and_then(serde_json::Value::as_str)
                    == Some(RUNTIME_PROMPT_COMMAND_RUN_CAPABILITY_RECORD_TYPE)
            })
            .collect()
    }

    fn runtime_prompt_manual_log_ids(session: &SessionManagement) -> Vec<String> {
        session
            .session_log
            .iter()
            .filter_map(|entry| serde_json::from_str::<serde_json::Value>(entry).ok())
            .filter(|value| {
                value.get("type").and_then(serde_json::Value::as_str)
                    == Some(RUNTIME_PROMPT_MANUAL_RECORD_TYPE)
            })
            .filter_map(|value| {
                value
                    .get("task_type")
                    .and_then(serde_json::Value::as_str)
                    .map(ToString::to_string)
            })
            .collect()
    }
}
