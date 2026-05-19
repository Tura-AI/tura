pub(super) const PLANNING_TOOL: &str = "planning";
pub(super) const COMMAND_RUN_TOOL: &str = "command_run";
pub(super) const APPLY_DIFF_TOOL: &str = "apply_patch";
pub(super) const DELETE_FILE_TOOL: &str = "rm";
pub(super) const WRITE_FILE_TOOL: &str = "tee";
pub(super) const CODING_AGENT_CONTEXT_EXCLUDED_TOOLS: &[&str] = &[];
pub(super) const BATCH_INPUT_TOOLS: &[&str] = &["apply_patch", "apply_diff"];
pub(super) const DISABLE_GATEWAY_CALLBACKS_ENV: &str = "TURA_DISABLE_GATEWAY_CALLBACKS";
pub(super) const FORCE_PLANNING_ENV: &str = "TURA_FORCE_PLANNING";
pub(super) const FORCE_EXECUTE_TOOLS_PLANNING_ENV: &str = "TURA_FORCE_EXECUTE_TOOLS_PLANNING";
pub(super) const DISABLE_PLANNING_GATE_ENV: &str = "TURA_DISABLE_PLANNING_GATE";
pub(super) const DISABLE_EXECUTE_TOOLS_PLANNING_ENV: &str =
    "TURA_DISABLE_EXECUTE_TOOLS_PLANNING_GATE";
pub(super) const DISABLE_PLANNING_TOOL_ENV: &str = "TURA_DISABLE_PLANNING_TOOL";
pub(super) const DISABLE_EXECUTE_TOOLS_TOOL_ENV: &str = "TURA_DISABLE_EXECUTE_TOOLS_TOOL";
pub(super) const PROJECT_ROOT_ENV: &str = "TURA_PROJECT_ROOT";
pub(super) const APPROVAL_POLICY_ENV: &str = "TURA_APPROVAL_POLICY";
pub(super) const PERMISSION_WAIT_SECONDS_ENV: &str = "TURA_PERMISSION_WAIT_SECONDS";
