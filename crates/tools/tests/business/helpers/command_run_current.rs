pub(crate) use code_tools::command_run;
pub(crate) use code_tools::commands;
pub(crate) use code_tools::runtime::file_locks::{self, Access};
pub(crate) use code_tools::runtime::tool::{
    FunctionToolOutput, ToolCall, ToolContext, ToolError, ToolPayload, ToolRouter, ToolRuntimeEvent,
};
pub(crate) use serde_json::{json, Value};
pub(crate) use std::collections::BTreeSet;
pub(crate) use std::ffi::OsString;
pub(crate) use std::fs;
pub(crate) use std::io::{Read, Write};
pub(crate) use std::net::TcpListener;
pub(crate) use std::path::{Path, PathBuf};
pub(crate) use std::sync::{Arc, Barrier};
pub(crate) use std::thread;
pub(crate) use std::time::{Duration, Instant};
use tokio::sync::Mutex;

pub(crate) static ENV_LOCK: Mutex<()> = Mutex::const_new(());

pub(crate) async fn env_lock() -> tokio::sync::MutexGuard<'static, ()> {
    ENV_LOCK.lock().await
}

pub(crate) fn env_lock_blocking() -> tokio::sync::MutexGuard<'static, ()> {
    ENV_LOCK.blocking_lock()
}

pub(crate) fn restore_env_var(name: &str, value: Option<OsString>) {
    if let Some(value) = value {
        std::env::set_var(name, value);
    } else {
        std::env::remove_var(name);
    }
}

pub(crate) fn temp_workspace(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "tura-command-run-current-flow-{name}-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).expect("create temp workspace");
    path
}

pub(crate) fn single_quoted_powershell_path(path: &Path) -> String {
    path.to_string_lossy().replace('\'', "''")
}

pub(crate) fn single_quoted_posix_path(path: &Path) -> String {
    path.to_string_lossy().replace('\'', r#"'\''"#)
}

pub(crate) fn find_ffmpeg() -> Option<String> {
    if let Ok(path) = std::env::var("FFMPEG_PATH") {
        if !path.trim().is_empty() && PathBuf::from(&path).exists() {
            return Some(path);
        }
    }
    if let Some(path_var) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&path_var) {
            let candidate = dir.join(if cfg!(windows) {
                "ffmpeg.exe"
            } else {
                "ffmpeg"
            });
            if candidate.exists() {
                return Some(candidate.display().to_string());
            }
        }
    }
    let output = std::process::Command::new("python")
        .arg("-c")
        .arg("import imageio_ffmpeg; print(imageio_ffmpeg.get_ffmpeg_exe())")
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if !path.is_empty() && PathBuf::from(&path).exists() {
        Some(path)
    } else {
        None
    }
}

#[cfg(unix)]
pub(crate) fn zsh_available() -> bool {
    std::process::Command::new("zsh")
        .arg("-c")
        .arg("print -r -- zsh-ok")
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}
