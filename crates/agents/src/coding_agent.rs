#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CodingAgentToolChoice {
    Auto,
    Strict,
    Disable,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CodingAgentProviderConfig {
    pub tura_llm_name: String,
    pub stream: bool,
    pub temperature: f32,
    pub max_tokens: u32,
    pub tool_choice: CodingAgentToolChoice,
    pub time_out_ms: u64,
}

pub struct CodingAgent;

impl CodingAgent {
    pub fn name() -> String {
        "coding_agent".to_string()
    }

    pub fn provider() -> CodingAgentProviderConfig {
        CodingAgentProviderConfig {
            tura_llm_name: "tura_coder".to_string(),
            stream: false,
            temperature: 0.2,
            max_tokens: 0,
            tool_choice: CodingAgentToolChoice::Auto,
            time_out_ms: 120_000,
        }
    }

    pub fn capabilities() -> Vec<String> {
        vec!["command_run".to_string()]
    }

    pub fn prompts() -> Vec<String> {
        vec!["coding_agent".to_string()]
    }
}
