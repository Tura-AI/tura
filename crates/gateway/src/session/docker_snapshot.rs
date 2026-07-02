use std::process::Command;

use serde::{Deserialize, Serialize};

const MAX_CONTAINERS: usize = 32;

#[derive(Debug, Clone, Serialize)]
pub struct DockerSnapshot {
    pub available: bool,
    pub containers: Vec<DockerContainerInfo>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DockerContainerInfo {
    pub id: String,
    pub name: String,
    pub image: String,
    pub state: String,
    pub status: String,
    pub ports: String,
}

#[derive(Debug, Deserialize)]
struct DockerPsLine {
    #[serde(rename = "ID")]
    id: String,
    #[serde(rename = "Names")]
    names: String,
    #[serde(rename = "Image")]
    image: String,
    #[serde(rename = "State")]
    state: String,
    #[serde(rename = "Status")]
    status: String,
    #[serde(rename = "Ports")]
    ports: String,
}

pub fn collect_docker_snapshot() -> DockerSnapshot {
    let mut command = Command::new("docker");
    command.args(["ps", "--format", "{{json .}}"]);
    tura_path::process_hardening::hide_child_console_window(&mut command);
    let output = match command.output() {
        Ok(output) => output,
        Err(error) => {
            return DockerSnapshot {
                available: false,
                containers: Vec::new(),
                error: Some(format!("docker command unavailable: {error}")),
            };
        }
    };

    if !output.status.success() {
        return DockerSnapshot {
            available: false,
            containers: Vec::new(),
            error: Some(String::from_utf8_lossy(&output.stderr).trim().to_string()),
        };
    }

    let mut containers = String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| serde_json::from_str::<DockerPsLine>(line).ok())
        .map(|line| DockerContainerInfo {
            id: line.id,
            name: line.names,
            image: line.image,
            state: line.state,
            status: line.status,
            ports: line.ports,
        })
        .collect::<Vec<_>>();
    containers.sort_by(|left, right| left.name.cmp(&right.name).then(left.id.cmp(&right.id)));
    containers.truncate(MAX_CONTAINERS);

    DockerSnapshot {
        available: true,
        containers,
        error: None,
    }
}
