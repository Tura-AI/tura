# Tura Router

Router owns command registration metadata, CLI forwarding, and managed process
lifecycle. It does not own `command_run` implementation logic; `command_run`
executes in the Rust `code-tools` crate through runtime.

This version keeps `command_run` as the only coding-agent visible tool. Router
metadata may resolve internal command ids such as `shell_command`, `bash`, and
`apply_patch`, but those handlers live under `crates/tools/src/commands`.
