use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExternalCommandEnvelope {
    pub kind: String,
    #[serde(default)]
    pub payload: Value,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExternalCommandResponse {
    pub ok: bool,
    #[serde(default)]
    pub output: Value,
    #[serde(default)]
    pub success: bool,
    #[serde(default)]
    pub stderr: String,
    #[serde(default)]
    pub exit_code: i32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExternalCommandExecutePayload {
    #[serde(default)]
    pub arguments: Value,
    #[serde(default)]
    pub session_dir: String,
    #[serde(default)]
    pub call_id: String,
    #[serde(default)]
    pub config: Value,
}
