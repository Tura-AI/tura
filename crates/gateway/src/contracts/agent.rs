use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Agent {
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

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct UpsertAgentRequest {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub config: Option<tura_agents::store::AgentConfig>,
    #[serde(default)]
    pub prompt: Option<String>,
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
