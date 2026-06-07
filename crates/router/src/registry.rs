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

/// Resolve a sibling service binary for the current router profile first.
pub fn resolve_binary_target(repo_root: &Path, binary_name: &str) -> Option<PathBuf> {
    let file_name = if cfg!(windows) {
        format!("{binary_name}.exe")
    } else {
        binary_name.to_string()
    };
    binary_target_candidates(repo_root, &file_name)
        .into_iter()
        .find(|path| path.exists())
}

fn binary_target_candidates(repo_root: &Path, file_name: &str) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Ok(current_exe) = std::env::current_exe() {
        if let Some(directory) = current_exe.parent() {
            candidates.push(directory.join(file_name));
        }
    }
    candidates.push(repo_root.join("bin").join(file_name));
    candidates.push(repo_root.join("target").join("debug").join(file_name));
    candidates.push(repo_root.join("target").join("release").join(file_name));
    candidates
}
