#![deny(clippy::unwrap_used)]
#![deny(unsafe_code)]

mod app;
mod cli;
mod daemon;
mod front_lifecycle;
mod ipc;
mod ipc_handlers;
mod process_info;
mod runtime_dispatch;
mod runtime_utils;
mod services;
mod shutdown;

#[cfg(test)]
pub(crate) use app::build_state;
pub(crate) use app::AppState;
pub(crate) use runtime_dispatch::{dispatch_run_agent_with_runtime_slot, RunAgentRequest};

fn main() -> anyhow::Result<()> {
    tura_path::process_hardening::harden_current_process("router");
    let command = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "serve".to_string());
    cli::run_router_command(&command)
}
