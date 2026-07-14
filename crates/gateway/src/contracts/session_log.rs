use serde::Deserialize;

#[derive(Debug, Clone, Default, Deserialize)]
pub struct SessionLogListParams {
    pub workspace: Option<String>,
    #[serde(default)]
    pub page: u64,
    #[serde(default = "default_session_log_page_size")]
    pub page_size: u64,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct SessionLogRecordsParams {
    #[serde(default)]
    pub page: u64,
    #[serde(default = "default_session_log_page_size")]
    pub page_size: u64,
}

pub(crate) fn default_session_log_page_size() -> u64 {
    50
}
