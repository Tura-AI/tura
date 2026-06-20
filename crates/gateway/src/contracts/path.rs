use serde::{Deserialize, Serialize};

// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathResponse {
    pub home: String,
    pub state: String,
    pub config: String,
    pub worktree: String,
    pub directory: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PathParams {
    pub directory: Option<String>,
}
