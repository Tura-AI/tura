use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

pub const DYNAMIC_AGENTS_DIR: &str = "agents";
pub const STATIC_AGENTS_DIR: &str = "agents/src";
pub const AGENT_CONFIG_FILE: &str = "agent_config.json";
pub const AGENT_PROMPT_FILE: &str = "prompt.md";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentConfig {
    pub agent_name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub aliases: Vec<String>,
    #[serde(default)]
    pub icon_emoji: Option<String>,
    pub agent_directory: PathBuf,
    #[serde(default)]
    pub parent_agent_id: Option<String>,
    #[serde(default = "default_report_to_user")]
    pub report_to_user: bool,
    #[serde(default)]
    pub default_config: bool,
    pub provider: serde_json::Value,
    #[serde(default)]
    pub agent_persona: Vec<serde_json::Value>,
    #[serde(default)]
    pub agent_prompt: Vec<serde_json::Value>,
    #[serde(default)]
    pub agent_capabilities: Vec<serde_json::Value>,
    pub validator: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentSummary {
    pub id: String,
    pub name: String,
    pub description: String,
    pub source: AgentSource,
    pub path: PathBuf,
    pub aliases: Vec<String>,
    pub capabilities: Vec<String>,
    pub provider: Option<String>,
    pub hidden: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AgentSource {
    Dynamic,
    Static,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredAgent {
    pub summary: AgentSummary,
    pub config: AgentConfig,
    pub prompt: Option<String>,
}

fn default_report_to_user() -> bool {
    true
}

pub fn discover_agents(project_root: &Path) -> Vec<StoredAgent> {
    let mut agents = BTreeMap::<String, StoredAgent>::new();
    for (source, root) in agent_roots(project_root) {
        let Ok(entries) = fs::read_dir(&root) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            if let Some(agent) = load_agent_at(project_root, &path, source) {
                let key = agent.summary.id.to_ascii_lowercase();
                agents.entry(key).or_insert(agent);
            }
        }
    }
    agents.into_values().collect()
}

pub fn load_agent(project_root: &Path, agent_id: &str) -> Option<StoredAgent> {
    let normalized = normalize_agent_id(agent_id);
    discover_agents(project_root).into_iter().find(|agent| {
        agent.summary.id.eq_ignore_ascii_case(&normalized)
            || agent
                .summary
                .aliases
                .iter()
                .any(|alias| alias.eq_ignore_ascii_case(&normalized))
    })
}

pub fn dynamic_agent_path(project_root: &Path, agent_id: &str) -> Result<PathBuf, String> {
    let id = normalize_agent_id(agent_id);
    if id.is_empty()
        || id.contains('/')
        || id.contains('\\')
        || id == "."
        || id == ".."
        || id
            .chars()
            .any(|ch| !(ch.is_ascii_alphanumeric() || ch == '_' || ch == '-'))
    {
        return Err(format!("invalid agent id: {agent_id}"));
    }
    Ok(project_root.join(DYNAMIC_AGENTS_DIR).join(id))
}

pub fn save_dynamic_agent(
    project_root: &Path,
    config: &AgentConfig,
    prompt: Option<&str>,
) -> Result<StoredAgent, String> {
    let agent_dir = dynamic_agent_path(project_root, &config.agent_name)?;
    fs::create_dir_all(&agent_dir).map_err(|err| {
        format!(
            "failed to create agent directory {}: {err}",
            agent_dir.display()
        )
    })?;

    let mut config = config.clone();
    config.agent_directory = path_relative_to(project_root, &agent_dir);
    if config.agent_prompt.is_empty() {
        config.agent_prompt.push(serde_json::json!({
            "agent_prompt": config.agent_name,
            "prompt_directory": config.agent_directory,
        }));
    }

    let encoded = serde_json::to_string_pretty(&config)
        .map_err(|err| format!("failed to encode agent config: {err}"))?;
    fs::write(agent_dir.join(AGENT_CONFIG_FILE), encoded).map_err(|err| {
        format!(
            "failed to write agent config {}: {err}",
            agent_dir.join(AGENT_CONFIG_FILE).display()
        )
    })?;
    if let Some(prompt) = prompt {
        fs::write(agent_dir.join(AGENT_PROMPT_FILE), prompt).map_err(|err| {
            format!(
                "failed to write agent prompt {}: {err}",
                agent_dir.join(AGENT_PROMPT_FILE).display()
            )
        })?;
    }
    load_agent_at(project_root, &agent_dir, AgentSource::Dynamic)
        .ok_or_else(|| format!("failed to reload agent {}", config.agent_name))
}

pub fn delete_dynamic_agent(project_root: &Path, agent_id: &str) -> Result<bool, String> {
    if let Some(agent) = load_agent(project_root, agent_id) {
        if agent.config.default_config {
            return Err(format!(
                "agent {} is a default_config and cannot be deleted",
                agent.summary.id
            ));
        }
        if agent.summary.source == AgentSource::Static {
            return Err(format!(
                "agent {} is static and cannot be deleted",
                agent.summary.id
            ));
        }
    }
    let agent_dir = dynamic_agent_path(project_root, agent_id)?;
    if !agent_dir.exists() {
        return Ok(false);
    }
    fs::remove_dir_all(&agent_dir)
        .map_err(|err| format!("failed to delete agent {}: {err}", agent_dir.display()))?;
    Ok(true)
}

pub fn default_agent_config(project_root: &Path, agent_id: &str) -> Result<AgentConfig, String> {
    let agent_dir = dynamic_agent_path(project_root, agent_id)?;
    let relative_dir = path_relative_to(project_root, &agent_dir);
    Ok(AgentConfig {
        agent_name: normalize_agent_id(agent_id),
        description: Some("Custom Tura agent".to_string()),
        aliases: Vec::new(),
        icon_emoji: Some("🧭".to_string()),
        agent_directory: relative_dir.clone(),
        parent_agent_id: None,
        report_to_user: true,
        default_config: false,
        provider: serde_json::json!({
            "tura_llm_name": "flagship_thinking",
            "stream": true,
            "temperature": 0.2,
            "max_tokens": 0,
            "tool_choice": "Auto",
            "time_out_ms": 120000
        }),
        agent_persona: vec![serde_json::json!({
            "persona_name": "tura",
            "persona_directory": "personas/src/tura/prompt"
        })],
        agent_prompt: vec![serde_json::json!({
            "agent_prompt": normalize_agent_id(agent_id),
            "prompt_directory": relative_dir
        })],
        agent_capabilities: default_capabilities(),
        validator: serde_json::json!({
            "need_validator": false,
            "validator_name": null
        }),
    })
}

pub fn project_root_from_env_or_cwd() -> PathBuf {
    if let Ok(root) = std::env::var("TURA_PROJECT_ROOT") {
        let root = PathBuf::from(root);
        if root.exists() {
            return root;
        }
    }

    let current = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    for candidate in current.ancestors() {
        if candidate.join("Cargo.toml").exists() && candidate.join("crates").exists() {
            return candidate.to_path_buf();
        }
    }
    current
}

fn agent_roots(project_root: &Path) -> [(AgentSource, PathBuf); 2] {
    [
        (AgentSource::Dynamic, project_root.join(DYNAMIC_AGENTS_DIR)),
        (AgentSource::Static, project_root.join(STATIC_AGENTS_DIR)),
    ]
}

fn load_agent_at(
    project_root: &Path,
    directory: &Path,
    source: AgentSource,
) -> Option<StoredAgent> {
    let config_path = directory.join(AGENT_CONFIG_FILE);
    if !config_path.exists() {
        return None;
    }
    let content = fs::read_to_string(&config_path).ok()?;
    let config: AgentConfig = serde_json::from_str(&content).ok()?;
    let prompt = fs::read_to_string(directory.join(AGENT_PROMPT_FILE)).ok();
    let summary = summary_from_config(project_root, directory, source, &config);
    Some(StoredAgent {
        summary,
        config,
        prompt,
    })
}

fn summary_from_config(
    project_root: &Path,
    directory: &Path,
    source: AgentSource,
    config: &AgentConfig,
) -> AgentSummary {
    let capabilities = config
        .agent_capabilities
        .iter()
        .filter_map(|item| {
            item.get("capability_name")
                .and_then(serde_json::Value::as_str)
        })
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    let provider = config
        .provider
        .get("tura_llm_name")
        .and_then(serde_json::Value::as_str)
        .map(ToString::to_string);
    let name = config.agent_name.clone();
    AgentSummary {
        id: name.clone(),
        name: display_name(&name),
        description: config
            .description
            .clone()
            .unwrap_or_else(|| format!("Tura agent {name}")),
        source,
        path: path_relative_to(project_root, directory),
        aliases: config.aliases.clone(),
        capabilities,
        provider,
        hidden: false,
    }
}

fn display_name(agent_id: &str) -> String {
    agent_id
        .split(['_', '-'])
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => format!("{}{}", first.to_ascii_uppercase(), chars.as_str()),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn default_capabilities() -> Vec<serde_json::Value> {
    [
        "command_run",
        "apply_patch",
        "shell_command",
        "read_media",
        "web_discover",
        "compact_context",
        "task_status",
    ]
    .into_iter()
    .map(|capability_name| {
        serde_json::json!({
            "capability_name": capability_name,
            "capability_directory": "crates/tools/src"
        })
    })
    .collect()
}

fn normalize_agent_id(agent_id: &str) -> String {
    agent_id.trim().to_ascii_lowercase()
}

fn path_relative_to(root: &Path, path: &Path) -> PathBuf {
    path.strip_prefix(root).unwrap_or(path).to_path_buf()
}
