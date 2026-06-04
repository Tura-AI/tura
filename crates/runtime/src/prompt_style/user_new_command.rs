pub const USER_NEW_COMMAND: &str = r#"User new command received while this task is already running.
Treat the commands below as the latest user guidance for the current main task and any active child task.
Do not restart from scratch unless the user explicitly asks for that. Adjust the next action, tool choice, and final answer to honor these commands."#;
