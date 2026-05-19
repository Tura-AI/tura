pub const TASK_CONTINUITY: &str = r#"Task continuity reminder.
Do not drift away from the original user task.
Each child task must receive only its own scoped task instruction, goal, deliverable, dependencies, and needed tool or capability.
Use command_run for workspace actions. Only end the user turn with normal assistant text after the requested work and verification are complete."#;
