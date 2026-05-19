use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};
use tracing::{error, info, warn};

use crate::utils::cli;

#[derive(Debug, Clone)]
pub struct PreparedService {
    pub service_name: String,
    pub executable_path: PathBuf,
    pub is_rust: bool,
}

pub async fn prepare_service(service_dir: &Path) -> Result<PreparedService> {
    let service_name = service_dir
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or_else(|| anyhow!("invalid service directory name"))?
        .to_string();

    let cargo_toml = service_dir.join("Cargo.toml");
    if cargo_toml.exists() {
        let exe = build_rust_service(service_dir).await?;
        return Ok(PreparedService {
            service_name,
            executable_path: exe,
            is_rust: true,
        });
    }

    let direct_bin = service_dir.join(executable_name(&service_name));
    if direct_bin.exists() {
        return Ok(PreparedService {
            service_name,
            executable_path: direct_bin,
            is_rust: false,
        });
    }

    Err(anyhow!(
        "service directory does not contain Cargo.toml or executable: {}",
        service_dir.display()
    ))
}

async fn build_rust_service(service_dir: &Path) -> Result<PathBuf> {
    info!(dir = %service_dir.display(), "rust service detected, building release binary");

    let service_name = service_dir
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or_else(|| anyhow!("invalid service directory name"))?;
    let existing = rust_service_executable_candidates(service_dir, service_name)
        .into_iter()
        .find(|path| path.exists());

    if let Err(err) = cli::run_command(service_dir, "cargo build --release").await {
        error!(
            dir = %service_dir.display(),
            error = %err,
            "cargo build failed"
        );
        if let Some(path) = existing {
            warn!(
                dir = %service_dir.display(),
                executable = %path.display(),
                "using existing service executable after build failure"
            );
            return Ok(path);
        }
        return Err(anyhow!("service build failed"));
    }

    for path in rust_service_executable_candidates(service_dir, service_name) {
        if path.exists() {
            return Ok(path);
        }
    }

    Err(anyhow!(
        "built service executable not found for service '{}'",
        service_name
    ))
}

fn rust_service_executable_candidates(service_dir: &Path, service_name: &str) -> Vec<PathBuf> {
    let mut candidates = vec![service_dir
        .join("target")
        .join("release")
        .join(executable_name(service_name))];

    if let Some(path) = service_dir.parent().and_then(|p| p.parent()).map(|root| {
        root.join("target")
            .join("release")
            .join(executable_name(service_name))
    }) {
        candidates.push(path);
    }

    candidates
}

fn executable_name(name: &str) -> String {
    if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_string()
    }
}
