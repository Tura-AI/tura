use serde::{Deserialize, Serialize};

// ============================================================================

#[derive(Debug, Clone, Deserialize)]
pub struct ListFilesQuery {
    pub directory: Option<String>,
    pub path: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FileInfo {
    pub name: String,
    pub path: String,
    #[serde(rename = "type")]
    pub file_type: String,
    pub absolute: String,
    pub ignored: bool,
    pub git_status: Option<String>,
    pub size_bytes: Option<u64>,
    pub modified_at: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FileContentQuery {
    pub path: String,
    pub directory: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileContentResponse {
    #[serde(rename = "type")]
    pub content_type: String,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub encoding: Option<String>,
    #[serde(rename = "mimeType", skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FileOpenResponse {
    pub path: String,
    pub opened: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FileInputSaveRequest {
    pub name: String,
    pub content: String,
    pub encoding: String,
    #[serde(rename = "mimeType")]
    pub mime_type: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FileInputSaveQuery {
    pub directory: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FileInputSaveResponse {
    pub path: String,
    pub absolute: String,
    pub name: String,
    #[serde(rename = "mimeType", skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
    pub size_bytes: u64,
}
