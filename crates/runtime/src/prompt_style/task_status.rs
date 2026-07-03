/// Fixed reminder injected when the model needs to settle task state.
pub const TASK_STATUS: &str = "Reminder: task_status only updates internal task state; it is never a substitute for the user-visible assistant message. When there is no task-state update to record--no status transition, no task_group/task_type correction, no required Operation Manual change, and no compact_context checkpoint--a task_status call is usually unnecessary, but it is not forbidden. Keep task_group available as the few-word internal code work area for the active task, not as a concrete task detail, progress report, completion summary, or user reply. Correct task_group examples: PDF editing, storefront frontend, order settlement service. Wrong task_group examples: Create a slide deck about the fall of Constantinople in 1453, Add cart button animation, Check order system logs. Use task_status `task_type` to update the complete set of prompt and Operation Manual types needed by the current task. Update it as soon as you identify the task type. Keep each task_group paired with one corresponding task_type set, and update task_group and task_type together. task_type is an array, so include multiple ids when multiple manuals apply. When the task needs another Operation Manual, add that manual's id to task_type. Use task_status `doing` only when the task cannot be completed without additional command_run calls. Before changing task_status `status` to `done` or `question`, first send the normal assistant-channel natural language reply containing the actual answer, explanation, completion summary, blocker, or question for the user; then call task_status in the same assistant response. For simple questions, greetings, acknowledgements, or ordinary conversation, answer naturally in the assistant channel. Usually avoid task_status unless the conversation actually changes task state or needs required user input, but a no-change task_status call is allowed when an explicit internal state touch is useful. Do not mark `doing` for ordinary conversation. If any required or reasonably runnable verification failed, timed out, was skipped, or could not start, continue working to fix the environment or implementation and rerun it. Mark `done` only after the task is complete, verified, and every media file you plan to send or show to the user has been read and inspected with read_media. Use task_status `compact_context` to create a context checkpoint when a meaningful phase is complete, when most previous context is no longer relevant to the next task, or when the active context reaches the 260,000 tokens hard cap. Only use task_status with compact_context when the new task no longer depends on the current main context and a handoff is needed. The user will receive all conversation from the current task and any previous summary; include only details not already covered by that conversation or prior summary. The compact_context handoff should preserve current user goal and Operation Manual, still-relevant user requirements and preferences, workflow rules that must continue, completed and incomplete work, key decisions and constraints, deliverables, file paths, validation standards, reference docs, relevant command results, directory requirements, and exactly what to do next. Keep compact_context concise and structured; do not exceed 10 sentences. If the current environment truly cannot run the verification after reasonable setup effort, clearly explain the blocker to the user and mark `question`.";

pub const STARTUP_TASK_STATE_GATE: &str = "The current session state has no task_type. Before any apply_patch command or write-producing shell command, define task_type based on the current context and the user's request, and include task_group in the same update. You may do non-writing work such as searches, file reads, or tests in the same command_run batch as that task_status update.";

pub fn task_status_prompt(require_startup_task_state: bool) -> String {
    let catalog = super::runtime_prompt_manual::task_type_catalog_for_prompt();
    let task_status = if require_startup_task_state {
        format!("{STARTUP_TASK_STATE_GATE} {TASK_STATUS}")
    } else {
        TASK_STATUS.to_string()
    };
    if catalog.trim().is_empty() {
        return task_status;
    }
    format!(
        "{task_status}\n\nAvailable `task_type` values:\n{catalog}",
        catalog = catalog.trim()
    )
}

pub fn task_status_schema(require_startup_task_state: bool) -> String {
    let Ok(mut schema) =
        serde_json::from_str::<serde_json::Value>(code_tools::commands::task_status::SCHEMA)
    else {
        return code_tools::commands::task_status::SCHEMA.to_string();
    };
    let ids = super::runtime_prompt_manual::valid_task_type_ids();
    if !ids.is_empty() {
        if let Some(items) = schema.pointer_mut("/properties/task_type/items") {
            items["enum"] =
                serde_json::Value::Array(ids.into_iter().map(serde_json::Value::String).collect());
        }
    }
    let catalog = super::runtime_prompt_manual::task_type_catalog_for_schema_description();
    if let Some(task_type) = schema.pointer_mut("/properties/task_type") {
        let startup = if require_startup_task_state {
            format!(" {STARTUP_TASK_STATE_GATE}")
        } else {
            String::new()
        };
        task_type["description"] = serde_json::Value::String(format!(
            "Complete set of prompt and Operation Manual types needed by the current task.{startup} Update task_type as soon as the task type is identified, keep it paired with task_group, and include multiple ids when multiple manuals apply. Available task types: {catalog}",
            catalog = catalog.trim()
        ));
    }
    schema.to_string()
}

pub fn planning_objective_context(objective: &str) -> String {
    format!(
        r#"Continue working toward the active thread user goal and Operation Manual.

The objective below is the active task scope. Verify the progress first. Treat it together with any active Operation Manual as the user goal and Operation Manual, and follow them exactly while continuing to obey higher-priority system, developer, and tool instructions.

<untrusted_objective>
{objective}
</untrusted_objective>

Avoid repeating work that is already done. Choose the next concrete action toward the user goal and Operation Manual."#,
        objective = objective.trim()
    )
}

pub fn no_tool_retry(objective: &str, operation_manual: Option<&str>) -> String {
    super::PromptBuilder::new()
        .part("Continue working toward the active thread user goal and Operation Manual.")
        .section("active_goal", objective)
        .optional_section("active_operation_manual", operation_manual)
        .part("The active_goal above and active_operation_manual, when present, are the user goal and Operation Manual for this retry. Do not infer the objective from the last user message, prompt-style scaffolding, context-management reminders, or task_status retry text. Use active_operation_manual as reflective execution guidance that strengthens completion checks; it does not replace or narrow the active_goal.")
        .part("If you need to set or correct the internal code work area, call command_run with task_status task_group. Use a broad area such as PDF editing, storefront frontend, or order settlement service, not a concrete action such as Add cart button animation. If more command_run calls are required to complete the task, call command_run with task_status status doing. If user feedback, missing information, permissions, credentials, or keys are required, first send the user-facing assistant reply with the question or blocker, then call command_run with task_status status question in the same assistant response. If the task is complete, verified, and every media file you plan to send or show has been read and inspected with read_media, first send the user-facing assistant completion summary, then call command_run with task_status status done in the same assistant response. Use task_status compact_context only when a new task no longer depends on the current main context and include only handoff details not already present in the current task conversation or prior summary.")
        .render()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_status_prompt_requires_task_type_before_writes_when_state_needs_it() {
        let prompt = task_status_prompt(true);

        assert!(prompt.contains(STARTUP_TASK_STATE_GATE), "{prompt}");
        assert!(
            prompt.contains("Before any apply_patch command"),
            "{prompt}"
        );
        assert!(prompt.contains("You may do non-writing work"), "{prompt}");
        assert!(prompt.contains("Available `task_type` values:"));
    }

    #[test]
    fn task_status_prompt_omits_startup_gate_when_state_already_has_type() {
        let prompt = task_status_prompt(false);

        assert!(!prompt.contains(STARTUP_TASK_STATE_GATE), "{prompt}");
        assert!(
            prompt.contains("a task_status call is usually unnecessary"),
            "{prompt}"
        );
        assert!(prompt.contains("but it is not forbidden"), "{prompt}");
        assert!(
            prompt.contains(
                "a no-change task_status call is allowed when an explicit internal state touch is useful"
            ),
            "{prompt}"
        );
        assert!(prompt.contains("Available `task_type` values:"));
    }

    #[test]
    fn task_status_schema_repeats_startup_rule_when_state_needs_it() {
        let schema: serde_json::Value = serde_json::from_str(&task_status_schema(true))
            .expect("task_status schema should parse");
        let description = schema["properties"]["task_type"]["description"]
            .as_str()
            .expect("task_type description should be a string");

        assert!(
            description.contains(STARTUP_TASK_STATE_GATE),
            "{description}"
        );
        assert!(schema["properties"]["task_type"]["items"]["enum"].is_array());
    }

    #[test]
    fn task_status_schema_omits_startup_rule_when_state_already_has_type() {
        let schema: serde_json::Value = serde_json::from_str(&task_status_schema(false))
            .expect("task_status schema should parse");
        let description = schema["properties"]["task_type"]["description"]
            .as_str()
            .expect("task_type description should be a string");

        assert!(
            !description.contains(STARTUP_TASK_STATE_GATE),
            "{description}"
        );
    }
}
