param(
  [switch]$NoBuild
)

$ErrorActionPreference = "Stop"
$repo = Resolve-Path (Join-Path $PSScriptRoot "..")
Set-Location $repo

if (-not $NoBuild) {
  cargo check -p code-tools
  cargo check -p code-tools-suite
  cargo check -p tura-llm-rust
}

cargo test -p code-tools --test command_run_current_flow -- --nocapture
cargo test -p code-tools-suite --lib tool_router::execute_tool::tests::tool_output_success_follows_current_style_command_run_results -- --nocapture
cargo test -p tura-llm-rust command_run_streaming --lib -- --nocapture
cargo test -p tura-llm-rust codex_event_tool_calls --lib -- --nocapture
