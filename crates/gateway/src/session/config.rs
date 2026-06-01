use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const TURA_DIR: &str = ".tura";
const CONFIG_FILE: &str = "config.conf";

pub const DEFAULT_SESSION_MODEL: &str = "codex/gpt-5.1-codex";
pub const DEFAULT_SESSION_PROVIDER: &str = "codex";
pub const DEFAULT_SESSION_MODEL_ID: &str = "gpt-5.1-codex";
pub const DEFAULT_SESSION_AGENT: &str = "coding_agent_planning";
pub const DEFAULT_SESSION_TYPE: &str = "coding";
pub const DEFAULT_SESSION_REASONING_EFFORT: &str = "low";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct TuraSessionConfig {
    pub language: Option<String>,
    pub model: Option<String>,
    pub active_provider: Option<String>,
    pub active_model: Option<String>,
    pub active_agent: Option<String>,
    pub session_type: Option<String>,
    pub model_variant: Option<String>,
    pub model_acceleration_enabled: Option<bool>,
    pub context_message_limit: Option<usize>,
    pub kill_processes_on_start: Option<bool>,
    pub validator_enabled: Option<bool>,
    pub force_multiple_tasks: Option<bool>,
    pub command_run_stall_guard_profile: Option<String>,
    pub command_run_stall_guard_check_secs: Option<u64>,
    pub command_run_stall_guard_identical_checks: Option<u8>,
    pub agent_avatar: Option<serde_json::Value>,
}

impl Default for TuraSessionConfig {
    fn default() -> Self {
        Self {
            language: None,
            model: Some(DEFAULT_SESSION_MODEL.to_string()),
            active_provider: Some(DEFAULT_SESSION_PROVIDER.to_string()),
            active_model: Some(DEFAULT_SESSION_MODEL_ID.to_string()),
            active_agent: Some(DEFAULT_SESSION_AGENT.to_string()),
            session_type: Some(DEFAULT_SESSION_TYPE.to_string()),
            model_variant: Some(DEFAULT_SESSION_REASONING_EFFORT.to_string()),
            model_acceleration_enabled: Some(true),
            context_message_limit: None,
            kill_processes_on_start: None,
            validator_enabled: None,
            force_multiple_tasks: None,
            command_run_stall_guard_profile: None,
            command_run_stall_guard_check_secs: None,
            command_run_stall_guard_identical_checks: None,
            agent_avatar: None,
        }
    }
}

impl TuraSessionConfig {
    pub fn merge(&mut self, next: TuraSessionConfig) {
        if next.model.is_some() {
            self.model = next.model;
        }
        if next.language.is_some() {
            self.language = next.language;
        }
        if next.active_provider.is_some() {
            self.active_provider = next.active_provider;
        }
        if next.active_model.is_some() {
            self.active_model = next.active_model;
        }
        if next.active_agent.is_some() {
            self.active_agent = next.active_agent;
        }
        if next.session_type.is_some() {
            self.session_type = next.session_type;
        }
        if next.model_variant.is_some() {
            self.model_variant = next.model_variant;
        }
        if next.model_acceleration_enabled.is_some() {
            self.model_acceleration_enabled = next.model_acceleration_enabled;
        }
        if next.context_message_limit.is_some() {
            self.context_message_limit = next.context_message_limit;
        }
        if next.kill_processes_on_start.is_some() {
            self.kill_processes_on_start = next.kill_processes_on_start;
        }
        if next.validator_enabled.is_some() {
            self.validator_enabled = next.validator_enabled;
        }
        if next.force_multiple_tasks.is_some() {
            self.force_multiple_tasks = next.force_multiple_tasks;
        }
        if next.command_run_stall_guard_profile.is_some() {
            self.command_run_stall_guard_profile = next.command_run_stall_guard_profile;
        }
        if next.command_run_stall_guard_check_secs.is_some() {
            self.command_run_stall_guard_check_secs = next.command_run_stall_guard_check_secs;
        }
        if next.command_run_stall_guard_identical_checks.is_some() {
            self.command_run_stall_guard_identical_checks =
                next.command_run_stall_guard_identical_checks;
        }
        if next.agent_avatar.is_some() {
            self.agent_avatar = next.agent_avatar;
        }
        self.fill_model_parts();
    }

    pub fn fill_model_parts(&mut self) {
        if self.model.is_none() {
            if let (Some(provider), Some(model)) = (&self.active_provider, &self.active_model) {
                self.model = Some(format!("{provider}/{model}"));
            }
        }
        if let Some(model) = &self.model {
            if let Some((provider, model_id)) = model.split_once('/') {
                if self.active_provider.is_none() {
                    self.active_provider = Some(provider.to_string());
                }
                if self.active_model.is_none() {
                    self.active_model = Some(model_id.to_string());
                }
            }
        }
    }

    pub fn command_run_stall_guard(&self) -> CommandRunStallGuardConfig {
        let profile = self
            .command_run_stall_guard_profile
            .as_deref()
            .unwrap_or(CommandRunStallGuardConfig::DEFAULT_PROFILE);
        let mut config = CommandRunStallGuardConfig::from_profile(profile);
        if let Some(check_secs) = self.command_run_stall_guard_check_secs {
            config.check_secs = check_secs.clamp(1, 300);
        }
        if let Some(identical_checks) = self.command_run_stall_guard_identical_checks {
            config.identical_checks = identical_checks.clamp(1, 60);
        }
        config
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct CommandRunStallGuardConfig {
    pub check_secs: u64,
    pub identical_checks: u8,
}

impl CommandRunStallGuardConfig {
    pub const DEFAULT_PROFILE: &'static str = "balanced_20s";

    pub fn from_profile(profile: &str) -> Self {
        match profile.trim() {
            "fast_10s" => Self {
                check_secs: 2,
                identical_checks: 5,
            },
            "patient_30s" => Self {
                check_secs: 10,
                identical_checks: 3,
            },
            "long_io_60s" => Self {
                check_secs: 15,
                identical_checks: 4,
            },
            _ => Self {
                check_secs: 5,
                identical_checks: 4,
            },
        }
    }

    pub fn stall_secs(self) -> u64 {
        self.check_secs.saturating_mul(self.identical_checks as u64)
    }
}

pub fn tura_dir(directory: impl AsRef<Path>) -> PathBuf {
    directory.as_ref().join(TURA_DIR)
}

pub fn sessions_dir(directory: impl AsRef<Path>) -> PathBuf {
    tura_dir(directory).join("sessions")
}

pub fn config_path(directory: impl AsRef<Path>) -> PathBuf {
    tura_dir(directory).join(CONFIG_FILE)
}

pub fn load_config(directory: impl AsRef<Path>) -> TuraSessionConfig {
    let path = config_path(directory);
    let Ok(content) = std::fs::read_to_string(path) else {
        return TuraSessionConfig::default();
    };
    parse_config(&content)
}

pub fn save_config(directory: impl AsRef<Path>, config: &TuraSessionConfig) -> Result<(), String> {
    let directory = directory.as_ref();
    std::fs::create_dir_all(tura_dir(directory)).map_err(|err| err.to_string())?;
    std::fs::write(config_path(directory), serialize_config(config)).map_err(|err| err.to_string())
}

pub fn merge_config(
    directory: impl AsRef<Path>,
    patch: TuraSessionConfig,
) -> Result<TuraSessionConfig, String> {
    let directory = directory.as_ref();
    let mut config = load_config(directory);
    config.merge(patch);
    save_config(directory, &config)?;
    Ok(config)
}

fn parse_config(content: &str) -> TuraSessionConfig {
    let mut values = BTreeMap::new();
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        values.insert(key.trim().to_string(), unquote(value.trim()));
    }

    let mut config = TuraSessionConfig {
        language: values.get("language").cloned(),
        model: values
            .get("model")
            .cloned()
            .or_else(|| Some(DEFAULT_SESSION_MODEL.to_string())),
        active_provider: values
            .get("active_provider")
            .cloned()
            .or_else(|| Some(DEFAULT_SESSION_PROVIDER.to_string())),
        active_model: values
            .get("active_model")
            .cloned()
            .or_else(|| Some(DEFAULT_SESSION_MODEL_ID.to_string())),
        active_agent: values
            .get("active_agent")
            .cloned()
            .or_else(|| Some(DEFAULT_SESSION_AGENT.to_string())),
        session_type: values
            .get("session_type")
            .cloned()
            .or_else(|| Some(DEFAULT_SESSION_TYPE.to_string())),
        model_variant: values
            .get("model_variant")
            .cloned()
            .or_else(|| Some(DEFAULT_SESSION_REASONING_EFFORT.to_string())),
        model_acceleration_enabled: values
            .get("model_acceleration_enabled")
            .and_then(|value| parse_bool(value)),
        context_message_limit: values
            .get("context_message_limit")
            .and_then(|value| value.parse::<usize>().ok())
            .filter(|value| *value > 0),
        kill_processes_on_start: values
            .get("kill_processes_on_start")
            .and_then(|value| parse_bool(value)),
        validator_enabled: values
            .get("validator_enabled")
            .and_then(|value| parse_bool(value)),
        force_multiple_tasks: values
            .get("force_multiple_tasks")
            .and_then(|value| parse_bool(value)),
        command_run_stall_guard_profile: values.get("command_run_stall_guard_profile").cloned(),
        command_run_stall_guard_check_secs: values
            .get("command_run_stall_guard_check_secs")
            .and_then(|value| value.parse::<u64>().ok())
            .filter(|value| *value > 0),
        command_run_stall_guard_identical_checks: values
            .get("command_run_stall_guard_identical_checks")
            .and_then(|value| value.parse::<u8>().ok())
            .filter(|value| *value > 0),
        agent_avatar: values
            .get("agent_avatar")
            .map(|value| parse_json_value(value)),
    };
    if config.model_acceleration_enabled.is_none() {
        config.model_acceleration_enabled = Some(true);
    }
    config.fill_model_parts();
    config
}

fn serialize_config(config: &TuraSessionConfig) -> String {
    let mut config = config.clone();
    config.fill_model_parts();
    let mut lines = Vec::new();
    push_line(&mut lines, "language", config.language.as_deref());
    push_line(&mut lines, "model", config.model.as_deref());
    push_line(
        &mut lines,
        "active_provider",
        config.active_provider.as_deref(),
    );
    push_line(&mut lines, "active_model", config.active_model.as_deref());
    push_line(&mut lines, "active_agent", config.active_agent.as_deref());
    push_line(&mut lines, "session_type", config.session_type.as_deref());
    push_line(&mut lines, "model_variant", config.model_variant.as_deref());
    if let Some(value) = config.model_acceleration_enabled {
        lines.push(format!("model_acceleration_enabled={value}"));
    }
    if let Some(value) = config.context_message_limit {
        lines.push(format!("context_message_limit={value}"));
    }
    if let Some(value) = config.kill_processes_on_start {
        lines.push(format!("kill_processes_on_start={value}"));
    }
    if let Some(value) = config.validator_enabled {
        lines.push(format!("validator_enabled={value}"));
    }
    if let Some(value) = config.force_multiple_tasks {
        lines.push(format!("force_multiple_tasks={value}"));
    }
    push_line(
        &mut lines,
        "command_run_stall_guard_profile",
        config.command_run_stall_guard_profile.as_deref(),
    );
    if let Some(value) = config.command_run_stall_guard_check_secs {
        lines.push(format!("command_run_stall_guard_check_secs={value}"));
    }
    if let Some(value) = config.command_run_stall_guard_identical_checks {
        lines.push(format!("command_run_stall_guard_identical_checks={value}"));
    }
    if let Some(value) = config.agent_avatar.as_ref() {
        let encoded = serialize_json_value(value);
        push_line(&mut lines, "agent_avatar", Some(&encoded));
    }
    lines.push(String::new());
    lines.join("\n")
}

fn push_line(lines: &mut Vec<String>, key: &str, value: Option<&str>) {
    let Some(value) = value else {
        return;
    };
    if value.trim().is_empty() {
        return;
    }
    lines.push(format!("{key}={}", quote(value)));
}

fn quote(value: &str) -> String {
    if value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '/' | '_' | '-' | '.' | ':'))
    {
        return value.to_string();
    }
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

fn unquote(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.len() < 2 || !trimmed.starts_with('"') || !trimmed.ends_with('"') {
        return trimmed.to_string();
    }
    let mut output = String::new();
    let mut escaped = false;
    for ch in trimmed[1..trimmed.len() - 1].chars() {
        if escaped {
            output.push(ch);
            escaped = false;
            continue;
        }
        if ch == '\\' {
            escaped = true;
            continue;
        }
        output.push(ch);
    }
    output
}

fn parse_bool(value: &str) -> Option<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "true" | "1" | "yes" | "on" => Some(true),
        "false" | "0" | "no" | "off" => Some(false),
        _ => None,
    }
}

fn parse_json_value(value: &str) -> serde_json::Value {
    serde_json::from_str(value).unwrap_or_else(|_| serde_json::Value::String(value.to_string()))
}

fn serialize_json_value(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(value) => value.clone(),
        value => serde_json::to_string(value).unwrap_or_default(),
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_config, serialize_config, CommandRunStallGuardConfig, TuraSessionConfig};

    #[test]
    fn command_run_stall_guard_defaults_to_balanced_profile() {
        let config = TuraSessionConfig::default();
        let guard = config.command_run_stall_guard();
        assert_eq!(guard.check_secs, 5);
        assert_eq!(guard.identical_checks, 4);
        assert_eq!(guard.stall_secs(), 20);
    }

    #[test]
    fn command_run_stall_guard_profiles_cover_supported_frequencies() {
        assert_eq!(
            CommandRunStallGuardConfig::from_profile("fast_10s"),
            CommandRunStallGuardConfig {
                check_secs: 2,
                identical_checks: 5
            }
        );
        assert_eq!(
            CommandRunStallGuardConfig::from_profile("balanced_20s"),
            CommandRunStallGuardConfig {
                check_secs: 5,
                identical_checks: 4
            }
        );
        assert_eq!(
            CommandRunStallGuardConfig::from_profile("patient_30s"),
            CommandRunStallGuardConfig {
                check_secs: 10,
                identical_checks: 3
            }
        );
        assert_eq!(
            CommandRunStallGuardConfig::from_profile("long_io_60s"),
            CommandRunStallGuardConfig {
                check_secs: 15,
                identical_checks: 4
            }
        );
    }

    #[test]
    fn command_run_stall_guard_round_trips_config_file() {
        let config = parse_config(
            r#"
command_run_stall_guard_profile=long_io_60s
command_run_stall_guard_check_secs=15
command_run_stall_guard_identical_checks=4
"#,
        );

        assert_eq!(
            config.command_run_stall_guard_profile.as_deref(),
            Some("long_io_60s")
        );
        assert_eq!(config.command_run_stall_guard().stall_secs(), 60);

        let serialized = serialize_config(&config);
        assert!(serialized.contains("command_run_stall_guard_profile=long_io_60s"));
        assert!(serialized.contains("command_run_stall_guard_check_secs=15"));
        assert!(serialized.contains("command_run_stall_guard_identical_checks=4"));
    }
}
