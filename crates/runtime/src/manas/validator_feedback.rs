//! Validator reliability feedback: report command_run tool-call success/failure
//! into the `alaya` registry (only for commands prefixed with `py:`).
//! Failure-tolerant: if `alaya` is unreachable, the main flow is unaffected.

use crate::state_machine::runtime_management::RuntimeManagement;

use super::constants::COMMAND_RUN_TOOL;

pub(super) fn apply_validator_reliability_feedback(runtime: &RuntimeManagement) {
    for record in &runtime.tool_call {
        let Some(success) = record.validator_reported_success else {
            continue;
        };
        if record.tool_called_name != COMMAND_RUN_TOOL {
            continue;
        }
        let Some(commands) = record
            .tool_called_input
            .get("commands")
            .and_then(|value| value.as_array())
        else {
            continue;
        };
        for command in commands {
            let Some(label) = command.get("command").and_then(|value| value.as_str()) else {
                continue;
            };
            if !label.trim_start().starts_with("py:") {
                continue;
            }
            let tool_name = registry_tool_name_for_command_label(label);
            let note = if success {
                None
            } else {
                Some(format!(
                    "validator reported failure for runtime {} command {}",
                    runtime.runtime_id, label
                ))
            };
            let _ = call_alaya_registry_reliability(
                "command-run-auto",
                &tool_name,
                success,
                note.as_deref(),
            );
        }
    }
}

fn call_alaya_registry_reliability(
    service_id: &str,
    tool_name: &str,
    success: bool,
    note: Option<&str>,
) -> Result<(), String> {
    let root = project_root_for_alaya().ok_or_else(|| "project root not found".to_string())?;
    let exe = alaya_executable_for_feedback(&root)
        .ok_or_else(|| "alaya executable not found".to_string())?;
    let mut command = std::process::Command::new(exe);
    command
        .args([
            "registry",
            "update-reliability",
            "--service-id",
            service_id,
            "--tool-name",
            tool_name,
            "--success",
            if success { "true" } else { "false" },
        ])
        .env("TURA_PROJECT_ROOT", &root);
    if let Some(note) = note {
        command.args(["--note", note]);
    }
    let status = command.status().map_err(|err| err.to_string())?;
    status
        .success()
        .then_some(())
        .ok_or_else(|| "alaya update-reliability failed".to_string())
}

fn registry_tool_name_for_command_label(command: &str) -> String {
    let mut out = command
        .trim()
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect::<String>();
    while out.contains("__") {
        out = out.replace("__", "_");
    }
    let out = out.trim_matches('_').to_string();
    if out.is_empty() {
        "command_run_tool".to_string()
    } else {
        out
    }
}

fn alaya_executable_for_feedback(root: &std::path::Path) -> Option<std::path::PathBuf> {
    let exe_name = if cfg!(windows) {
        "alaya_memory_server.exe"
    } else {
        "alaya_memory_server"
    };
    [
        root.join("target")
            .join("alaya-service-target")
            .join("debug")
            .join(exe_name),
        root.join("target").join("debug").join(exe_name),
    ]
    .into_iter()
    .find(|path| path.exists())
}

fn project_root_for_alaya() -> Option<std::path::PathBuf> {
    if let Ok(root) = std::env::var("TURA_PROJECT_ROOT") {
        let path = std::path::PathBuf::from(root);
        if path
            .join("services")
            .join("alaya")
            .join("Cargo.toml")
            .exists()
        {
            return Some(path);
        }
    }
    for start in [std::env::current_dir().ok(), std::env::current_exe().ok()]
        .into_iter()
        .flatten()
    {
        for candidate in start.ancestors() {
            if candidate
                .join("services")
                .join("alaya")
                .join("Cargo.toml")
                .exists()
            {
                return Some(candidate.to_path_buf());
            }
        }
    }
    None
}
