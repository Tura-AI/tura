use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AboutInfoResponse {
    pub release_version: String,
    pub system: AboutSystemInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AboutSystemInfo {
    pub operating_system: String,
    pub os_version: String,
    pub architecture: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AboutStarOutcome {
    Starred,
    Opened,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AboutStarResponse {
    pub outcome: AboutStarOutcome,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AboutOpenTarget {
    ReportBug,
    Contribute,
    Contact,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AboutOpenRequest {
    pub target: AboutOpenTarget,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AboutOpenResponse {
    pub opened: bool,
    pub target: AboutOpenTarget,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AboutUpdate {
    pub current_version: String,
    pub latest_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AboutUpdateCheckResponse {
    pub update: Option<AboutUpdate>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AboutUpdateInstallRequest {
    pub version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AboutUpdateInstallResponse {
    pub scheduled: bool,
    pub version: String,
}
