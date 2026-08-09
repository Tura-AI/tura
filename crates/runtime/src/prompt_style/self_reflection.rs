use lifecycle::SessionManagement;

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
