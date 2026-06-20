//! Agent registry owned by the router.
//!
//! The router owns agent discovery, resolution, and spec delivery. The runtime
//! activates `AgentManagement` from the delivered spec and runs the MANAS loop.
//! The router does not own prompt assembly, provider formatting, or agent loops.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use tura_agents::store::{discover_agents, project_root_from_env_or_cwd};

/// Resolved agent spec delivered from the router to a runtime worker.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AgentSpec {
    pub agent_name: String,
    /// Default provider id; credential truth and OAuth handling stay in provider.
    pub provider: String,
    pub capabilities: Vec<String>,
    /// Session types and topics that select this agent.
    pub session_types: Vec<String>,
    pub validator_enabled: bool,
    #[serde(default)]
    pub default_config: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config: Option<tura_agents::store::AgentConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentCatalogItem {
    pub name: String,
    pub description: String,
    pub mode: String,
    pub native: bool,
    pub hidden: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<AgentModel>,
    #[serde(default)]
    pub options: HashMap<String, serde_json::Value>,
    pub permission: PermissionRuleset,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentModel {
    #[serde(rename = "providerID")]
    pub provider_id: String,
    #[serde(rename = "modelID")]
    pub model_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionRuleset {
    pub allow: Vec<String>,
    pub deny: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpsertAgentRequest {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub config: Option<tura_agents::store::AgentConfig>,
    #[serde(default)]
    pub prompt: Option<String>,
}

#[derive(Clone, Debug)]
struct AgentDefinition {
    agent_name: &'static str,
    aliases: &'static [&'static str],
    provider: &'static str,
    capabilities: &'static [&'static str],
    session_types: &'static [&'static str],
    validator_enabled: bool,
}

const AGENT_TABLE: &[AgentDefinition] = &[
    AgentDefinition {
        agent_name: "coding_agent",
        aliases: &["coding", "programming", "development", "testing"],
        provider: "anthropic",
        capabilities: &["command_run", "file_edit", "code_search"],
        session_types: &["coding", "programming", "development", "testing"],
        validator_enabled: false,
    },
    AgentDefinition {
        agent_name: "general_agent",
        aliases: &["general"],
        provider: "anthropic",
        capabilities: &["command_run", "web_discover"],
        session_types: &["general"],
        validator_enabled: false,
    },
];

const DEFAULT_AGENT_INDEX: usize = 1;

/// In-memory agent registry loaded from static and dynamic definitions.
#[derive(Clone, Debug)]
pub struct AgentRegistry {
    name_index: HashMap<String, usize>,
    session_type_index: HashMap<String, usize>,
    dynamic_specs: HashMap<String, AgentSpec>,
}

impl AgentRegistry {
    pub fn from_static() -> Self {
        let mut name_index = HashMap::new();
        let mut session_type_index = HashMap::new();
        let mut dynamic_specs = HashMap::new();
        for (index, def) in AGENT_TABLE.iter().enumerate() {
            name_index.insert(def.agent_name.to_string(), index);
            for alias in def.aliases {
                name_index.insert((*alias).to_string(), index);
            }
            for session_type in def.session_types {
                session_type_index.insert((*session_type).to_string(), index);
            }
        }
        for agent in discover_agents(&project_root_from_env_or_cwd()) {
            let spec = AgentSpec {
                agent_name: agent.summary.id.clone(),
                provider: agent
                    .summary
                    .provider
                    .unwrap_or_else(|| "default".to_string()),
                capabilities: agent.summary.capabilities,
                session_types: vec![agent.summary.id.clone()],
                validator_enabled: agent
                    .config
                    .validator
                    .get("need_validator")
                    .and_then(serde_json::Value::as_bool)
                    .unwrap_or(false),
                default_config: agent.config.default_config,
                config: Some(agent.config),
            };
            dynamic_specs.insert(agent.summary.id.to_ascii_lowercase(), spec.clone());
            for alias in agent.summary.aliases {
                dynamic_specs.insert(alias.to_ascii_lowercase(), spec.clone());
            }
        }
        Self {
            name_index,
            session_type_index,
            dynamic_specs,
        }
    }

    /// Resolve by explicit agent name.
    pub fn resolve_by_name(&self, name: &str) -> Option<AgentSpec> {
        let key = name.trim().to_ascii_lowercase();
        if let Some(spec) = self.dynamic_specs.get(&key) {
            return Some(spec.clone());
        }
        self.name_index
            .get(&key)
            .map(|index| spec_from(&AGENT_TABLE[*index]))
    }

    /// Resolve by session type or topic when no explicit agent is selected.
    pub fn resolve_by_session_type(&self, session_type: &str) -> AgentSpec {
        let key = session_type.trim().to_ascii_lowercase();
        if let Some(spec) = self.dynamic_specs.get(&key) {
            return spec.clone();
        }
        let index = self
            .session_type_index
            .get(&key)
            .copied()
            .unwrap_or(DEFAULT_AGENT_INDEX);
        spec_from(&AGENT_TABLE[index])
    }

    /// Resolve by explicit agent first, then fall back to session type.
    pub fn resolve(&self, agent: Option<&str>, session_type: Option<&str>) -> AgentSpec {
        if let Some(agent) = agent {
            if let Some(spec) = self.resolve_by_name(agent) {
                return spec;
            }
        }
        self.resolve_by_session_type(session_type.unwrap_or("general"))
    }

    pub fn list_catalog(&self) -> Vec<AgentCatalogItem> {
        discover_agents(&project_root_from_env_or_cwd())
            .into_iter()
            .map(catalog_item_from_stored_agent)
            .collect()
    }

    pub fn get_stored(&self, agent_id: &str) -> Option<tura_agents::store::StoredAgent> {
        tura_agents::store::load_agent(&project_root_from_env_or_cwd(), agent_id)
    }

    pub fn upsert(
        &self,
        agent_id: Option<String>,
        payload: UpsertAgentRequest,
    ) -> Result<tura_agents::store::StoredAgent, String> {
        let project_root = project_root_from_env_or_cwd();
        let agent_id = agent_id
            .or(payload.id)
            .or_else(|| {
                payload
                    .config
                    .as_ref()
                    .map(|config| config.agent_name.clone())
            })
            .ok_or_else(|| "agent id is required".to_string())?;
        let mut config = payload.config.unwrap_or(
            tura_agents::store::load_agent(&project_root, &agent_id)
                .map(|agent| agent.config)
                .unwrap_or(tura_agents::store::default_agent_config(
                    &project_root,
                    &agent_id,
                )?),
        );
        config.agent_name = agent_id;
        tura_agents::store::save_dynamic_agent(&project_root, &config, payload.prompt.as_deref())
    }

    pub fn delete(&self, agent_id: &str) -> Result<bool, String> {
        tura_agents::store::delete_dynamic_agent(&project_root_from_env_or_cwd(), agent_id)
    }
}

impl Default for AgentRegistry {
    fn default() -> Self {
        Self::from_static()
    }
}

fn spec_from(def: &AgentDefinition) -> AgentSpec {
    AgentSpec {
        agent_name: def.agent_name.to_string(),
        provider: def.provider.to_string(),
        capabilities: def.capabilities.iter().map(|s| s.to_string()).collect(),
        session_types: def.session_types.iter().map(|s| s.to_string()).collect(),
        validator_enabled: def.validator_enabled,
        default_config: true,
        config: None,
    }
}

fn catalog_item_from_stored_agent(agent: tura_agents::store::StoredAgent) -> AgentCatalogItem {
    let mut options = HashMap::new();
    options.insert(
        "source".to_string(),
        serde_json::json!(agent.summary.source),
    );
    options.insert("path".to_string(), serde_json::json!(agent.summary.path));
    options.insert(
        "aliases".to_string(),
        serde_json::json!(agent.summary.aliases),
    );
    options.insert(
        "capabilities".to_string(),
        serde_json::json!(agent.summary.capabilities),
    );
    options.insert(
        "default_config".to_string(),
        serde_json::json!(agent.config.default_config),
    );
    AgentCatalogItem {
        name: agent.summary.id,
        description: agent.summary.description,
        mode: "primary".to_string(),
        native: agent.summary.source == tura_agents::store::AgentSource::Static,
        hidden: agent.summary.hidden,
        model: None,
        options,
        permission: PermissionRuleset {
            allow: vec!["*".to_string()],
            deny: vec![],
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_session_type_to_coding() {
        let registry = AgentRegistry::from_static();
        let spec = registry.resolve_by_session_type("coding");
        assert!(spec.agent_name == "coding_agent" || spec.agent_name == "thinking-planning");
    }

    #[test]
    fn falls_back_to_general() {
        let registry = AgentRegistry::from_static();
        let spec = registry.resolve(Some("nonexistent"), Some("unknown_topic"));
        assert_eq!(spec.agent_name, "general_agent");
    }

    #[test]
    fn explicit_agent_takes_priority() {
        let registry = AgentRegistry::from_static();
        let spec = registry.resolve(Some("coding"), Some("general"));
        assert!(spec.agent_name == "coding_agent" || spec.agent_name == "thinking-planning");
    }

    #[test]
    fn static_agent_registry_exposes_expected_capabilities() {
        let registry = AgentRegistry::from_static();
        let spec = registry
            .resolve_by_name("coding")
            .expect("coding alias should resolve");
        assert!(spec.agent_name == "coding_agent" || spec.agent_name == "thinking-planning");
        assert!(
            spec.capabilities.contains(&"command_run".to_string())
                || spec.capabilities.contains(&"shells".to_string())
        );
        assert!(
            spec.capabilities.contains(&"file_edit".to_string())
                || spec.capabilities.contains(&"apply_patch".to_string())
        );
        assert!(
            spec.session_types.contains(&"coding".to_string())
                || spec.agent_name == "thinking-planning"
        );
    }
}
