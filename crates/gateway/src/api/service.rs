use crate::api::registry;
use axum::Json;

pub async fn get_service_status() -> Json<ServiceStatusResponse> {
    let session_directory = crate::mock::global_store()
        .get_current_directory()
        .or_else(|| {
            std::env::current_dir()
                .ok()
                .map(|path| path.display().to_string())
        });
    let session_processes = session_directory.as_deref().map(|directory| {
        crate::session::process_snapshot::collect_session_process_snapshot(std::path::Path::new(
            directory,
        ))
    });
    let mut response = ServiceStatusResponse {
        mano: ServiceHealth {
            status: "connected".to_string(),
            url: None,
            error: None,
        },
        router: ServiceHealth {
            status: "checking".to_string(),
            url: None,
            error: None,
        },
        session_processes,
        docker: crate::session::docker_snapshot::collect_docker_snapshot(),
    };

    if registry::router_binary_path().exists() {
        response.router.status = "available".to_string();
    } else {
        response.router.status = "error".to_string();
        response.router.error = Some("router CLI binary not found".to_string());
    }

    Json(response)
}

#[derive(Debug, serde::Serialize)]
pub struct ServiceStatusResponse {
    pub mano: ServiceHealth,
    pub router: ServiceHealth,
    pub session_processes: Option<crate::session::process_snapshot::SessionProcessSnapshot>,
    pub docker: crate::session::docker_snapshot::DockerSnapshot,
}

#[derive(Debug, serde::Serialize)]
pub struct ServiceHealth {
    pub status: String,
    pub url: Option<String>,
    pub error: Option<String>,
}
