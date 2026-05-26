pub const TASK_STATUS: &str = r#"Task-management status.
Use command_run only while workspace work remains.
When the active task is complete and verified, call command_run with a task_status command whose status is done.
When user feedback or more information is needed, call command_run with a task_status command whose status is question.
If neither done nor question applies, continue the task with command_run instead of reporting status.
Do not rename task_summary during execution once it already exists, unless the user clearly changed the task."#;
