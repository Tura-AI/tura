use chrono::Utc;

use crate::state_machine::runtime_management::{RuntimeId, RuntimeManagement};
use crate::state_machine::session_management::SessionManagement;

pub(crate) fn create_dummy_runtime(
    runtime_id: RuntimeId,
    session: &SessionManagement,
) -> RuntimeManagement {
    let now = Utc::now();
    let provider_name = crate::agent_router::coding_agent_provider_name();

    let runtime_provider_config = crate::state_machine::runtime_management::RuntimeProviderConfig {
        base: crate::state_machine::agent_management::ProviderConfig {
            tura_llm_name: provider_name.clone(),
            stream: true,
            temperature: 0.5,
            max_tokens: 0,
            tool_choice: crate::state_machine::agent_management::ToolChoice::Auto,
            time_out_ms: 120_000,
        },
        thinking: false,
        provider_name: provider_name.clone(),
        model_name: String::new(),
        provider_url_name: String::new(),
        llm_provider_name: provider_name,
    };

    let mut runtime = RuntimeManagement::new(
        runtime_id,
        session.session_id.clone(),
        session.session_id.clone(),
        runtime_provider_config,
        now,
    );

    let _ = runtime.finish_success(now, None);

    runtime
}
