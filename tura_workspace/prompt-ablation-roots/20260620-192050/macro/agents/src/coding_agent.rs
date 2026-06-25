#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CodingAgentToolChoice {
    Auto,
    Strict,
    Disable,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CodingAgentProviderConfig {
    pub tura_llm_name: String,
    pub default_model_tier: Option<String>,
    pub current_model: Option<String>,
    pub stream: bool,
    pub temperature: f32,
    pub max_tokens: u32,
    pub tool_choice: CodingAgentToolChoice,
    pub time_out_ms: u64,
}

pub struct CodingAgent;

impl CodingAgent {
    pub fn name() -> String {
        "thinking-planning".to_string()
    }

    pub fn provider() -> CodingAgentProviderConfig {
        CodingAgentProviderConfig {
            tura_llm_name: "thinking".to_string(),
            default_model_tier: Some("thinking".to_string()),
            current_model: None,
            stream: true,
            temperature: 0.2,
            max_tokens: 0,
            tool_choice: CodingAgentToolChoice::Auto,
            time_out_ms: 120_000,
        }
    }

    pub fn capabilities() -> Vec<String> {
        vec![
            "apply_patch".to_string(),
            "shells".to_string(),
            "generate_media".to_string(),
            "read_media".to_string(),
            "web_discover".to_string(),
            "compact_context".to_string(),
            "task_status".to_string(),
            "planning".to_string(),
        ]
    }

    pub fn prompts() -> Vec<String> {
        vec!["thinking-planning".to_string()]
    }
}
