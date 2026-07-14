use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct SummaryResponse {
    pub summary: String,
}
