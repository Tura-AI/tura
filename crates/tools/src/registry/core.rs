pub const CORE_COMMANDS: &[&str] = &[
    "shell_command",
    "bash",
    "zsh",
    "apply_patch",
    "task_status",
    "compact_context",
    "planning",
];

pub fn is_core_command(command_id: &str) -> bool {
    CORE_COMMANDS.contains(&command_id)
}
