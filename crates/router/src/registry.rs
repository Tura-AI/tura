//! Router registry for resolving agent, command, and persona specs.
//!
//! The registry is in-memory and loaded from static definitions at startup.
//! Runtime code consumes resolved specs; router owns metadata lookup.

pub mod agent;
pub mod command;
pub mod persona;
pub mod tools;

use std::path::{Path, PathBuf};

pub use agent::AgentRegistry;
#[allow(unused_imports)]
pub use agent::AgentSpec;
pub use command::CommandRegistry;
pub use persona::PersonaRegistry;
pub use tools::ToolRegistry;

/// Registry bundle attached to router AppState.
#[derive(Clone, Debug, Default)]
pub struct Registry {
    pub agents: AgentRegistry,
    pub commands: CommandRegistry,
    pub personas: PersonaRegistry,
    #[allow(dead_code)]
    pub tools: ToolRegistry,
}

impl Registry {
    pub fn from_static() -> Self {
        Self {
            agents: AgentRegistry::from_static(),
            commands: CommandRegistry,
            personas: PersonaRegistry::from_static(),
            tools: ToolRegistry::discover(default_repo_root()),
        }
    }
}

fn default_repo_root() -> PathBuf {
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

/// Resolve a binary target from release first, then debug.
pub fn resolve_binary_target(repo_root: &Path, binary_name: &str) -> Option<PathBuf> {
    let file_name = if cfg!(windows) {
        format!("{binary_name}.exe")
    } else {
        binary_name.to_string()
    };
    let release = repo_root.join("target").join("release").join(&file_name);
    if release.exists() {
        return Some(release);
    }
    let debug = repo_root.join("target").join("debug").join(&file_name);
    if debug.exists() {
        return Some(debug);
    }
    None
}
