use chrono::Utc;

use crate::state_machine::runtime_management::{RuntimeId, RuntimeManagement};
use crate::state_machine::session_management::SessionManagement;

pub(crate) fn create_dummy_runtime(
    runtime_id: RuntimeId,
    session: &SessionManagement,
) -> Result<RuntimeManagement, String> {
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

    runtime.mark_called(now)?;
    runtime.mark_waiting_first_token()?;
    runtime.finish_success(now, None)?;

    Ok(runtime)
}

#[cfg(test)]
mod tests {
    use super::create_dummy_runtime;
    use crate::state_machine::runtime_management::{RuntimeCallResultStatus, RuntimeState};
    use crate::state_machine::session_management::{SessionInput, SessionManagement};
    use chrono::Utc;
    use std::path::PathBuf;

    #[test]
    fn dummy_final_runtime_follows_runtime_fsm_before_reporting_success() {
        let session = SessionManagement::new(
            "session-final-runtime".to_string(),
            "final runtime session".to_string(),
            PathBuf::from("C:/workspace/final-runtime"),
            false,
            "coding".to_string(),
            SessionInput {
                user_input: "finish the session".to_string(),
                file_input: Vec::new(),
                agent: None,
                runtime_context: None,
                planning_mode_override: None,
            },
            "finish the session".to_string(),
            Utc::now(),
        );

        let runtime = create_dummy_runtime("runtime-final".to_string(), &session)
            .expect("dummy final runtime should use valid runtime transitions");

        assert_eq!(runtime.runtime_id, "runtime-final");
        assert_eq!(runtime.session_id, session.session_id);
        assert_eq!(runtime.agent_id, session.session_id);
        assert_eq!(runtime.state, RuntimeState::Finished);
        assert_eq!(
            runtime.call_result_status,
            RuntimeCallResultStatus::Succeeded
        );
        assert!(
            runtime.called_at.is_some(),
            "final runtime should pass through Dispatching"
        );
        assert!(
            runtime.call_finished_at.is_some(),
            "final runtime should record completion time"
        );
        assert!(
            runtime.first_token_at.is_none(),
            "dummy final runtime should not invent a first token"
        );
    }
}
