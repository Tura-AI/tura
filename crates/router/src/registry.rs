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

pub fn binary_target_diagnostics(repo_root: &Path, binary_name: &str) -> String {
    let file_name = if cfg!(windows) {
        format!("{binary_name}.exe")
    } else {
        binary_name.to_string()
    };
    binary_target_candidates(repo_root, &file_name)
        .into_iter()
        .map(|path| path.display().to_string())
        .collect::<Vec<_>>()
        .join(", ")
}

fn binary_target_candidates(repo_root: &Path, file_name: &str) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(release_bin_dir) = std::env::var_os("TURA_RELEASE_BIN_DIR") {
        candidates.push(PathBuf::from(release_bin_dir).join(file_name));
    }
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

#[cfg(test)]
mod tests {
    use super::resolve_binary_target;

    struct EnvGuard {
        key: &'static str,
        previous: Option<std::ffi::OsString>,
    }

    impl EnvGuard {
        fn set(key: &'static str, value: &std::path::Path) -> Self {
            let previous = std::env::var_os(key);
            std::env::set_var(key, value);
            Self { key, previous }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            match self.previous.take() {
                Some(value) => std::env::set_var(self.key, value),
                None => std::env::remove_var(self.key),
            }
        }
    }

    #[test]
    fn binary_target_prefers_explicit_release_bin_dir() -> anyhow::Result<()> {
        let temp = tempfile::tempdir()?;
        let release_bin = temp.path().join("installed-release");
        std::fs::create_dir_all(&release_bin)?;
        let executable = if cfg!(windows) {
            "tura_runtime.exe"
        } else {
            "tura_runtime"
        };
        let expected = release_bin.join(executable);
        std::fs::write(&expected, b"test runtime worker")?;
        let _guard = EnvGuard::set("TURA_RELEASE_BIN_DIR", &release_bin);

        assert_eq!(
            resolve_binary_target(temp.path(), "tura_runtime").as_deref(),
            Some(expected.as_path())
        );

        Ok(())
    }
}
