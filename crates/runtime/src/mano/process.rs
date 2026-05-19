use chrono::Utc;
use tracing::{error, info};

use crate::agent_router::{activate_agents_by_session_type, initialize_agent_state_machine};
use crate::context::accumulate_message;
use crate::manas::{process_manas_internal, ManasInput};
use crate::mano::gateway_session::{load_persisted_gateway_session, persist_gateway_session};
use crate::mano::session_bootstrap::create_session_with_topic;
use crate::mano::{ManoOverrides, ManoProcessResult};
use crate::state_machine::session_management::{SessionInput, SessionManagement};
use chrono::SecondsFormat;
use std::collections::BTreeMap;
use std::fs;
use std::io::BufRead;
use std::io::BufReader;
use std::path::PathBuf;
use std::time::SystemTime;

pub struct OrchestrationConfig {
    pub redis_url: String,
    pub session_directory: Option<PathBuf>,
}

impl Default for OrchestrationConfig {
    fn default() -> Self {
        Self {
            redis_url: "redis://localhost:6379".to_string(),
            session_directory: None,
        }
    }
}

pub fn orchestrate(input: SessionInput) -> Result<ManoProcessResult, String> {
    orchestrate_with_config(input, OrchestrationConfig::default())
}

pub fn orchestrate_for_session(
    input: SessionInput,
    session_id: String,
) -> Result<ManoProcessResult, String> {
    orchestrate_with_config_and_session(input, OrchestrationConfig::default(), Some(session_id))
}

pub fn orchestrate_for_session_in_directory(
    input: SessionInput,
    session_id: String,
    session_directory: PathBuf,
) -> Result<ManoProcessResult, String> {
    orchestrate_with_config_and_session(
        input,
        OrchestrationConfig {
            session_directory: Some(session_directory),
            ..OrchestrationConfig::default()
        },
        Some(session_id),
    )
}

pub fn orchestrate_with_config(
    input: SessionInput,
    config: OrchestrationConfig,
) -> Result<ManoProcessResult, String> {
    orchestrate_with_config_and_session(input, config, None)
}

fn orchestrate_with_config_and_session(
    input: SessionInput,
    config: OrchestrationConfig,
    gateway_session_id: Option<String>,
) -> Result<ManoProcessResult, String> {
    let now = Utc::now();

    info!(
        user_input = %input.user_input,
        "starting orchestration"
    );

    let mut session =
        bootstrap_orchestration_session(input.clone(), &config, gateway_session_id.clone(), now)?;

    info!(
        session_id = %session.session_id,
        session_topic = %session.session_topic,
        "session created"
    );

    let mut agents = match activate_agents_by_session_type(&session) {
        Ok(a) => a,
        Err(e) => {
            error!(error = %e, "failed to activate agents");
            return Err(format!("failed to activate agents: {}", e));
        }
    };

    if let Err(e) = initialize_agent_state_machine(&mut agents, &session) {
        error!(error = %e, "failed to initialize agent state machine");
        return Err(format!("failed to initialize agent state machine: {}", e));
    }

    info!(
        session_id = %session.session_id,
        agent_count = agents.len(),
        "agents activated"
    );

    let initial_messages = initial_messages_for_session(&mut session)?;
    persist_gateway_session(&session)
        .map_err(|err| format!("failed to persist initial gateway session: {err}"))?;

    let mut session_clone = session.clone();

    let manas_input = ManasInput {
        agents: &mut agents,
        session: &mut session_clone,
        initial_messages,
        redis_url: &config.redis_url,
    };

    let manas_result =
        match process_manas_internal(manas_input, crate::manas::ManasOverrides::default()) {
            Ok(r) => r,
            Err(e) => {
                error!(error = %e, "manas processing failed");
                return Err(format!("manas processing failed: {}", e));
            }
        };

    info!(
        session_id = %manas_result.session.session_id,
        final_turn = manas_result.session.session_current_turn,
        final_state = ?manas_result.session.state,
        "orchestration completed"
    );

    Ok(ManoProcessResult {
        session: manas_result.session,
        agents: manas_result.agents,
    })
}

fn initial_messages_for_session(
    session: &mut SessionManagement,
) -> Result<Vec<serde_json::Value>, String> {
    if session.session_current_turn == 0 && !session_has_initial_user_message(session) {
        let permissions_message = serde_json::json!({
            "role": "developer",
            "content": permissions_instructions(),
        });
        let snapshot_message = serde_json::json!({
            "role": "user",
            "content": workspace_snapshot_message(&session.session_directory),
        });
        let environment_message = serde_json::json!({
            "role": "user",
            "content": environment_context_message(&session.session_directory),
        });
        let user_message = serde_json::json!({
            "role": "user",
            "content": session.input.user_input,
        });

        for message in [
            &permissions_message,
            &snapshot_message,
            &environment_message,
            &user_message,
        ] {
            let role = message
                .get("role")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("system");
            let content = message
                .get("content")
                .cloned()
                .unwrap_or_else(|| serde_json::Value::String(String::new()));
            accumulate_message(session, role, content)?;
        }

        return Ok(vec![
            permissions_message,
            snapshot_message,
            environment_message,
            user_message,
        ]);
    }

    Ok(vec![serde_json::json!({
        "role": "user",
        "content": session.input.user_input,
    })])
}

fn permissions_instructions() -> &'static str {
    "<permissions instructions>\nFilesystem sandboxing defines which files can be read or written. `sandbox_mode` is `danger-full-access`: No filesystem sandboxing - all commands are permitted. Network access is enabled.\nApproval policy is currently never. Do not provide the `sandbox_permissions` for any reason, commands will be rejected.\n</permissions instructions>"
}

fn environment_context_message(cwd: &std::path::Path) -> String {
    format!(
        "<environment_context>\n  <cwd>{}</cwd>\n  <shell>{}</shell>\n  <current_date>{}</current_date>\n  <timezone>{}</timezone>\n</environment_context>",
        cwd.display(),
        context_shell_name(),
        chrono::Local::now().format("%Y-%m-%d"),
        std::env::var("TZ").unwrap_or_else(|_| "Europe/Paris".to_string())
    )
}

fn context_shell_name() -> &'static str {
    match std::env::var("TURA_COMMAND_RUN_SHELL")
        .ok()
        .map(|value| value.trim().to_ascii_lowercase())
        .as_deref()
    {
        Some("bash") => "bash",
        Some("shell") | Some("shell_command") | Some("shll") | Some("shall") => {
            if cfg!(windows) {
                "powershell"
            } else {
                "bash"
            }
        }
        _ if cfg!(windows) => "powershell",
        _ => "bash",
    }
}

fn workspace_snapshot_message(cwd: &std::path::Path) -> String {
    let mut snapshot = WorkspaceSnapshot::new(cwd);
    collect_workspace_snapshot(cwd, cwd, 0, &mut snapshot);
    snapshot
        .entries
        .sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    format!(
        "<WORKSPACE_SNAPSHOT>\n{}\n</WORKSPACE_SNAPSHOT>",
        snapshot.text()
    )
}

struct WorkspaceSnapshot {
    cwd: PathBuf,
    total_files: usize,
    total_dirs: usize,
    suffix_counts: BTreeMap<String, usize>,
    entries: Vec<WorkspaceSnapshotEntry>,
}

impl WorkspaceSnapshot {
    fn new(cwd: &std::path::Path) -> Self {
        Self {
            cwd: cwd.to_path_buf(),
            total_files: 0,
            total_dirs: 0,
            suffix_counts: BTreeMap::new(),
            entries: Vec::new(),
        }
    }

    fn text(&self) -> String {
        let mut lines = vec![
            format!("cwd: {}", self.cwd.display()),
            "scan_depth: 2".to_string(),
            format!("total_files: {}", self.total_files),
            format!("total_dirs: {}", self.total_dirs),
            format!("suffix_counts: {}", self.suffix_counts_text()),
            "columns: modified_utc | lines | suffix | path".to_string(),
        ];
        lines.extend(self.entries.iter().map(|entry| {
            format!(
                "{} | {} | {} | {}",
                entry.modified_utc, entry.line_count, entry.suffix, entry.relative_path
            )
        }));
        lines.join("\n")
    }

    fn suffix_counts_text(&self) -> String {
        if self.suffix_counts.is_empty() {
            return "none".to_string();
        }
        self.suffix_counts
            .iter()
            .map(|(suffix, count)| format!("{suffix}={count}"))
            .collect::<Vec<_>>()
            .join(", ")
    }
}

struct WorkspaceSnapshotEntry {
    relative_path: String,
    modified_utc: String,
    line_count: String,
    suffix: String,
}

fn collect_workspace_snapshot(
    root: &std::path::Path,
    dir: &std::path::Path,
    depth: usize,
    snapshot: &mut WorkspaceSnapshot,
) {
    let Ok(read_dir) = fs::read_dir(dir) else {
        return;
    };
    for entry in read_dir.flatten() {
        let path = entry.path();
        let Ok(metadata) = entry.metadata() else {
            continue;
        };
        if metadata.is_dir() {
            snapshot.total_dirs += 1;
            if depth < 2 {
                collect_workspace_snapshot(root, &path, depth + 1, snapshot);
            }
            continue;
        }
        if !metadata.is_file() {
            continue;
        }
        snapshot.total_files += 1;
        let suffix = path
            .extension()
            .and_then(|extension| extension.to_str())
            .map(|extension| format!(".{extension}"))
            .unwrap_or_else(|| "(none)".to_string());
        *snapshot.suffix_counts.entry(suffix.clone()).or_insert(0) += 1;
        snapshot.entries.push(WorkspaceSnapshotEntry {
            relative_path: path
                .strip_prefix(root)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/"),
            modified_utc: metadata
                .modified()
                .map(format_system_time)
                .unwrap_or_else(|_| "unknown".to_string()),
            line_count: count_lines(&path)
                .map(|count| count.to_string())
                .unwrap_or_else(|| "unknown".to_string()),
            suffix,
        });
    }
}

fn format_system_time(time: SystemTime) -> String {
    let datetime: chrono::DateTime<Utc> = time.into();
    datetime.to_rfc3339_opts(SecondsFormat::Secs, true)
}

fn count_lines(path: &std::path::Path) -> Option<usize> {
    let file = fs::File::open(path).ok()?;
    let mut reader = BufReader::new(file);
    let mut count = 0usize;
    let mut buffer = Vec::new();
    loop {
        buffer.clear();
        let bytes = reader.read_until(b'\n', &mut buffer).ok()?;
        if bytes == 0 {
            break;
        }
        count += 1;
    }
    Some(count)
}

fn session_has_initial_user_message(session: &SessionManagement) -> bool {
    let input = session.input.user_input.trim();
    session.session_log.iter().any(|entry| {
        serde_json::from_str::<serde_json::Value>(entry)
            .ok()
            .is_some_and(|value| {
                value.get("role").and_then(serde_json::Value::as_str) == Some("user")
                    && value
                        .get("content")
                        .and_then(serde_json::Value::as_str)
                        .is_some_and(|content| content.trim() == input)
            })
    })
}

fn bootstrap_orchestration_session(
    input: SessionInput,
    config: &OrchestrationConfig,
    gateway_session_id: Option<String>,
    now: chrono::DateTime<Utc>,
) -> Result<crate::state_machine::session_management::SessionManagement, String> {
    if let Some(session_id) = gateway_session_id {
        if let Some(mut persisted) = config
            .session_directory
            .as_ref()
            .and_then(|directory| load_persisted_gateway_session(directory, &session_id))
        {
            persisted.prepare_for_new_user_turn(input, now);
            if let Some(directory) = config.session_directory.clone() {
                persisted.session_directory = directory;
            }
            persisted.session_id = session_id;
            return Ok(persisted);
        }

        let mut session = create_session_with_topic(input, config.session_directory.clone())
            .map_err(|e| {
                error!(error = %e, "failed to create session");
                format!("failed to create session: {}", e)
            })?;
        session.session_id = session_id;
        return Ok(session);
    }

    create_session_with_topic(input, config.session_directory.clone()).map_err(|e| {
        error!(error = %e, "failed to create session");
        format!("failed to create session: {}", e)
    })
}

pub fn process_from_user_internal(
    input: SessionInput,
    overrides: ManoOverrides,
) -> Result<ManoProcessResult, String> {
    let session = match overrides.session_factory {
        Some(session_factory) => session_factory(input)?,
        None => create_session_with_topic(input, None)?,
    };

    let agents = match overrides.manas_entry {
        Some(manas_entry) => manas_entry(&session)?,
        None => {
            let mut agts = activate_agents_by_session_type(&session)?;
            initialize_agent_state_machine(&mut agts, &session)?;
            agts
        }
    };

    Ok(ManoProcessResult { session, agents })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::build_messages_from_session;
    use crate::state_machine::session_management::{SessionInput, SessionManagement};
    use chrono::Utc;
    use std::fs;

    #[test]
    fn gateway_bootstrap_loads_persisted_session_before_creating_new_session() {
        let root = std::env::temp_dir().join(format!(
            "tura-gateway-bootstrap-{}",
            Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ));
        let session_id = "sess-existing".to_string();
        let sessions_dir = root.join(".tura").join("sessions");
        fs::create_dir_all(&sessions_dir).expect("test session dir");

        let old_input = SessionInput {
            user_input: "old prompt".to_string(),
            file_input: Vec::new(),
            agent: None,
            runtime_context: None,
        };
        let mut persisted = SessionManagement::new(
            session_id.clone(),
            "existing".to_string(),
            root.clone(),
            false,
            "coding".to_string(),
            old_input,
            "old prompt".to_string(),
            Utc::now(),
        );
        persisted.push_log("persisted-session-loaded", Utc::now());

        let record = serde_json::json!({
            "info": {
                "management": persisted,
            },
            "messages": [],
            "todos": [],
        });
        fs::write(
            sessions_dir.join(format!("{session_id}.json")),
            serde_json::to_string_pretty(&record).expect("record json"),
        )
        .expect("write persisted session");

        let next_input = SessionInput {
            user_input: "fix bug in the existing workspace".to_string(),
            file_input: Vec::new(),
            agent: None,
            runtime_context: None,
        };
        let session = bootstrap_orchestration_session(
            next_input.clone(),
            &OrchestrationConfig {
                redis_url: "redis://localhost:6379".to_string(),
                session_directory: Some(root.clone()),
            },
            Some(session_id.clone()),
            Utc::now(),
        )
        .expect("persisted gateway session should load");

        assert_eq!(session.session_id, session_id);
        assert_eq!(session.session_directory, root);
        assert_eq!(session.input, next_input);
        assert!(session
            .session_log
            .iter()
            .any(|entry| entry == "persisted-session-loaded"));

        let _ = fs::remove_dir_all(session.session_directory);
    }

    #[test]
    fn initial_messages_persist_workspace_file_snapshot_for_cache_reuse() {
        let root = std::env::temp_dir().join(format!(
            "tura-initial-snapshot-{}",
            Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ));
        fs::create_dir_all(root.join("src")).expect("test workspace should be created");
        fs::write(root.join("src").join("lib.rs"), "fn main() {}\n").expect("fixture should write");
        let input = SessionInput {
            user_input: "inspect this workspace".to_string(),
            file_input: Vec::new(),
            agent: None,
            runtime_context: None,
        };
        let mut session = SessionManagement::new(
            "snapshot-session".to_string(),
            "snapshot".to_string(),
            root.clone(),
            false,
            "coding".to_string(),
            input,
            "inspect this workspace".to_string(),
            Utc::now(),
        );

        let initial =
            initial_messages_for_session(&mut session).expect("initial messages should build");
        let replayed = build_messages_from_session(&session);

        let initial_snapshot = initial
            .iter()
            .find(|message| {
                message["content"]
                    .as_str()
                    .is_some_and(|content| content.contains("<WORKSPACE_SNAPSHOT>"))
            })
            .expect("initial messages should include workspace snapshot");
        assert!(initial_snapshot["content"]
            .as_str()
            .expect("snapshot content should be text")
            .contains("src/lib.rs"));
        assert!(replayed.iter().any(|message| message == initial_snapshot));

        let _ = fs::remove_dir_all(root);
    }
}
