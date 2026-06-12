use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

pub const AGENTS_DIR: &str = "agents/src";
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
    let root = project_root.join(AGENTS_DIR);
    let Ok(entries) = fs::read_dir(&root) else {
        return Vec::new();
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        if let Some(agent) = load_agent_at(project_root, &path) {
            let key = agent.summary.id.to_ascii_lowercase();
            agents.entry(key).or_insert(agent);
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
    Ok(project_root.join(AGENTS_DIR).join(id))
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
    load_agent_at(project_root, &agent_dir)
        .ok_or_else(|| format!("failed to reload agent {}", config.agent_name))
}

pub fn delete_dynamic_agent(project_root: &Path, agent_id: &str) -> Result<bool, String> {
    let mut canonical_id = None;
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
        canonical_id = Some(agent.summary.id);
    }
    let agent_dir = dynamic_agent_path(project_root, canonical_id.as_deref().unwrap_or(agent_id))?;
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
            "persona_directory": "personas/src/tura"
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

fn load_agent_at(project_root: &Path, directory: &Path) -> Option<StoredAgent> {
    let config_path = directory.join(AGENT_CONFIG_FILE);
    if !config_path.exists() {
        return None;
    }
    let content = fs::read_to_string(&config_path).ok()?;
    let config: AgentConfig = serde_json::from_str(&content).ok()?;
    let prompt = fs::read_to_string(directory.join(AGENT_PROMPT_FILE)).ok();
    let source = if config.default_config {
        AgentSource::Static
    } else {
        AgentSource::Dynamic
    };
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
        "zsh",
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

#[cfg(test)]
mod tests {
    use super::*;

    fn project() -> tempfile::TempDir {
        let temp = tempfile::tempdir().expect("temp project");
        fs::create_dir_all(temp.path().join(AGENTS_DIR)).expect("agents dir");
        temp
    }

    fn test_config(root: &Path, name: &str) -> AgentConfig {
        let mut config = default_agent_config(root, name).expect("default config");
        config.description = Some("Does useful work".to_string());
        config.aliases = vec!["helper".to_string(), "coder".to_string()];
        config
    }

    #[test]
    fn default_agent_config_normalizes_id_and_declares_runtime_contract() {
        let temp = project();

        let config = default_agent_config(temp.path(), "  Custom-Agent  ").expect("config");

        assert_eq!(config.agent_name, "custom-agent");
        assert_eq!(
            config.agent_directory,
            PathBuf::from("agents/src/custom-agent")
        );
        assert_eq!(config.description.as_deref(), Some("Custom Tura agent"));
        assert_eq!(config.report_to_user, true);
        assert_eq!(config.default_config, false);
        assert_eq!(config.provider["tura_llm_name"], "flagship_thinking");
        assert_eq!(config.provider["tool_choice"], "Auto");
        assert!(config
            .agent_capabilities
            .iter()
            .any(|capability| capability["capability_name"] == "command_run"));
        assert!(config
            .agent_capabilities
            .iter()
            .any(|capability| capability["capability_name"] == "web_discover"));
        assert_eq!(config.validator["need_validator"], false);
    }

    #[test]
    fn dynamic_agent_path_rejects_empty_traversal_and_non_identifier_ids() {
        let temp = project();
        for invalid in [
            "",
            "  ",
            ".",
            "..",
            "../x",
            "a/b",
            r"a\b",
            "has space",
            "中文",
        ] {
            let error = dynamic_agent_path(temp.path(), invalid)
                .expect_err("invalid agent id should be rejected");
            assert!(error.contains("invalid agent id"), "{error}");
        }

        assert_eq!(
            dynamic_agent_path(temp.path(), " Agent_01 ").expect("valid path"),
            temp.path().join("agents/src/agent_01")
        );
    }

    #[test]
    fn save_dynamic_agent_writes_config_prompt_and_loads_summary() {
        let temp = project();
        let config = test_config(temp.path(), "coding-helper");

        let saved = save_dynamic_agent(temp.path(), &config, Some("Use careful tests."))
            .expect("save agent");

        assert_eq!(saved.summary.id, "coding-helper");
        assert_eq!(saved.summary.name, "Coding Helper");
        assert_eq!(saved.summary.description, "Does useful work");
        assert_eq!(saved.summary.source, AgentSource::Dynamic);
        assert_eq!(
            saved.summary.path,
            PathBuf::from("agents/src/coding-helper")
        );
        assert_eq!(saved.summary.aliases, vec!["helper", "coder"]);
        assert_eq!(saved.summary.provider.as_deref(), Some("flagship_thinking"));
        assert!(saved
            .summary
            .capabilities
            .iter()
            .any(|capability| capability == "apply_patch"));
        assert_eq!(saved.prompt.as_deref(), Some("Use careful tests."));

        let config_path = temp
            .path()
            .join("agents/src/coding-helper")
            .join(AGENT_CONFIG_FILE);
        assert!(config_path.exists());
        assert!(temp
            .path()
            .join("agents/src/coding-helper")
            .join(AGENT_PROMPT_FILE)
            .exists());
    }

    #[test]
    fn discover_agents_sorts_by_id_and_load_agent_matches_alias_case_insensitively() {
        let temp = project();
        save_dynamic_agent(temp.path(), &test_config(temp.path(), "Zulu"), None)
            .expect("save zulu");
        save_dynamic_agent(temp.path(), &test_config(temp.path(), "Alpha"), None)
            .expect("save alpha");

        let discovered = discover_agents(temp.path());
        let ids = discovered
            .iter()
            .map(|agent| agent.summary.id.as_str())
            .collect::<Vec<_>>();

        assert_eq!(ids, vec!["alpha", "zulu"]);
        assert_eq!(
            load_agent(temp.path(), "HELPER")
                .expect("alias load")
                .summary
                .id,
            "alpha"
        );
        assert!(load_agent(temp.path(), "missing").is_none());
    }

    #[test]
    fn discover_agents_skips_missing_and_malformed_configs() {
        let temp = project();
        fs::create_dir_all(temp.path().join("agents/src/no-config")).expect("no config dir");
        fs::create_dir_all(temp.path().join("agents/src/bad-json")).expect("bad json dir");
        fs::write(
            temp.path()
                .join("agents/src/bad-json")
                .join(AGENT_CONFIG_FILE),
            "{not-json",
        )
        .expect("bad config");
        save_dynamic_agent(temp.path(), &test_config(temp.path(), "valid"), None)
            .expect("valid agent");

        let discovered = discover_agents(temp.path());

        assert_eq!(discovered.len(), 1);
        assert_eq!(discovered[0].summary.id, "valid");
    }

    #[test]
    fn save_dynamic_agent_adds_prompt_binding_when_missing() {
        let temp = project();
        let mut config = test_config(temp.path(), "promptless");
        config.agent_prompt.clear();

        let saved = save_dynamic_agent(temp.path(), &config, None).expect("save");

        assert_eq!(saved.config.agent_prompt.len(), 1);
        assert_eq!(saved.config.agent_prompt[0]["agent_prompt"], "promptless");
        assert_eq!(
            saved.config.agent_prompt[0]["prompt_directory"]
                .as_str()
                .map(|value| value.replace('\\', "/")),
            Some("agents/src/promptless".to_string())
        );
    }

    #[test]
    fn delete_dynamic_agent_is_idempotent_for_missing_agents_and_removes_existing() {
        let temp = project();

        assert_eq!(
            delete_dynamic_agent(temp.path(), "missing").expect("missing delete"),
            false
        );
        save_dynamic_agent(temp.path(), &test_config(temp.path(), "remove-me"), None)
            .expect("save");

        assert_eq!(
            delete_dynamic_agent(temp.path(), "remove-me").expect("delete"),
            true
        );
        assert!(load_agent(temp.path(), "remove-me").is_none());
        assert_eq!(
            delete_dynamic_agent(temp.path(), "remove-me").expect("second delete"),
            false
        );
    }

    #[test]
    fn delete_dynamic_agent_rejects_default_config_even_when_user_requests_alias() {
        let temp = project();
        let mut config = test_config(temp.path(), "built-in");
        config.default_config = true;
        config.aliases = vec!["builtin".to_string()];
        save_dynamic_agent(temp.path(), &config, None).expect("save default");

        let error = delete_dynamic_agent(temp.path(), "BUILTIN")
            .expect_err("default config should be protected");

        assert!(error.contains("default_config and cannot be deleted"));
        assert!(load_agent(temp.path(), "built-in").is_some());
    }

    #[test]
    fn source_and_summary_defaults_derive_from_config_fields() {
        let temp = project();
        let mut config = test_config(temp.path(), "summary_case");
        config.description = None;
        config.aliases.clear();
        config.agent_capabilities = vec![
            serde_json::json!({"capability_name":"shell_command"}),
            serde_json::json!({"ignored":"missing-name"}),
        ];
        config.provider = serde_json::json!({});

        let saved = save_dynamic_agent(temp.path(), &config, None).expect("save");

        assert_eq!(saved.summary.name, "Summary Case");
        assert_eq!(saved.summary.description, "Tura agent summary_case");
        assert_eq!(saved.summary.capabilities, vec!["shell_command"]);
        assert_eq!(saved.summary.provider, None);
        assert!(!saved.summary.hidden);
    }
}
