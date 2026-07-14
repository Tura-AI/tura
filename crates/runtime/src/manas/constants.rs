pub(crate) const PLANNING_TOOL: &str = "planning";
pub(crate) const COMMAND_RUN_TOOL: &str = "command_run";
pub(crate) const TASK_STATUS_COMMAND: &str = "task_status";
pub(super) const DISABLE_PLANNING_TOOL_ENV: &str = "TURA_DISABLE_PLANNING_TOOL";
pub(super) const DISABLE_EXECUTE_TOOLS_TOOL_ENV: &str = "TURA_DISABLE_EXECUTE_TOOLS_TOOL";
pub(super) const PROJECT_ROOT_ENV: &str = "TURA_PROJECT_ROOT";
pub(crate) const APPROVAL_POLICY_ENV: &str = "TURA_APPROVAL_POLICY";
pub(crate) const GATEWAY_CALLBACKS_ENV: &str = "TURA_GATEWAY_CALLBACKS";

pub(crate) fn gateway_callbacks_disabled() -> bool {
    std::env::var(GATEWAY_CALLBACKS_ENV)
        .ok()
        .as_deref()
        .is_some_and(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "0" | "false" | "no" | "off" | "disabled"
            )
        })
}
