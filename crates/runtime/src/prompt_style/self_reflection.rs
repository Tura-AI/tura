use crate::state_machine::session_management::SessionManagement;

use super::runtime_prompt_manual::active_manual_display_names;

pub fn self_reflection_tail_prompt(session: &SessionManagement) -> String {
    let manuals = active_manual_display_names(session);
    let manual_list = if manuals.is_empty() {
        "active Operation Manual(s)".to_string()
    } else {
        manuals.join(", ")
    };

    format!(
        "Before running the next `command_run` batch or providing the final answer, review the {manual_list} and complete the required `Self Reflection` to ensure you are still aligned with the user's goal and the manual(s). ***Even if you think you have found the answer, confirm the complete chain before making any judgment; do not settle on a conclusion while the full source-to-output path is still incomplete.*** If you notice any mistakes, stop now and correct the previous mistaks before continuing."
    )
}

#[cfg(test)]
mod tests {
    use super::self_reflection_tail_prompt;
    use crate::prompt_style::runtime_prompt_manual::normalize_task_type_ids;
    use crate::state_machine::session_management::{SessionInput, SessionManagement};

    #[test]
    fn tail_prompt_lists_active_manual_display_names_once() {
        let mut session = session();
        session.task_type = normalize_task_type_ids(["interactive_and_3d", "visual"]);

        let prompt = self_reflection_tail_prompt(&session);

        assert!(
            prompt.contains(
                "Visual Operation Manual, Frontend Operation Manual, Interactive and 3D Operation Manual"
            ),
            "{prompt}"
        );
        assert_eq!(prompt.matches("Visual Operation Manual").count(), 1);
        assert!(prompt.contains("complete the required `Self Reflection`"));
        assert!(prompt.contains("confirm the complete chain before making any judgment"));
        assert!(prompt.contains("full source-to-output path is still incomplete"));
        assert!(prompt.contains("correct the previous mistaks before continuing"));
    }

    #[test]
    fn tail_prompt_has_fallback_without_task_type() {
        let session = session();

        let prompt = self_reflection_tail_prompt(&session);

        assert!(prompt.contains("review the active Operation Manual(s)"));
    }

    fn session() -> SessionManagement {
        SessionManagement::new(
            "self-reflection-tail".to_string(),
            "self reflection tail".to_string(),
            std::path::PathBuf::from("C:/workspace"),
            false,
            "coding".to_string(),
            SessionInput {
                user_input: "work".to_string(),
                file_input: vec![],
                agent: None,
                runtime_context: None,
                planning_mode_override: None,
            },
            "work".to_string(),
            chrono::Utc::now(),
        )
    }
}
