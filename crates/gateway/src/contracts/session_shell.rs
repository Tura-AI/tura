use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize)]
pub struct ShellRequest {
    pub input: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ShellResponse {
    pub output: String,
}
