/// Short reminder injected by the runtime loop when the model keeps doing
/// workspace work without ever settling the task state. The full how-to and
/// examples live in the `task_status` command prompt; this is only the nudge.
pub const TASK_STATUS: &str = "Reminder: settle the task state with a task_status command. Do not keep re-running verification or read-only commands in place of marking `done` or `question`.";
