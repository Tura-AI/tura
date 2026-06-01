//! Router 注册表：agent 定义解析（spec 下发）。
//!
//! 注册表为内存态，启动时从静态定义装载——不引入数据库、不要求固定端口。
//! 边界：router 持有 agent 元数据与解析，agent loop 仍归 runtime。
//! agent 与 command 注册管理归 router；runtime 只消费 router 下发的 spec。

pub mod agent;
pub mod command;
pub mod persona;

use std::path::{Path, PathBuf};

pub use agent::AgentRegistry;
#[allow(unused_imports)]
pub use agent::AgentSpec;
pub use command::CommandRegistry;
pub use persona::PersonaRegistry;

/// 注册表集合，挂在 router AppState 上。
#[derive(Clone, Debug, Default)]
pub struct Registry {
    pub agents: AgentRegistry,
    pub commands: CommandRegistry,
    pub personas: PersonaRegistry,
}

impl Registry {
    pub fn from_static() -> Self {
        Self {
            agents: AgentRegistry::from_static(),
            commands: CommandRegistry::default(),
            personas: PersonaRegistry::from_static(),
        }
    }
}

/// 从注册表解析二进制 target（替代写死的发行目录路径）。
/// 优先发行目录，回退 debug 目录。
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
