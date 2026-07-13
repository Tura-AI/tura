//! Web HTTP server using Axum

use crate::api;
use axum::{
    routing::{get, patch, post, put},
    Router,
};
use std::future::Future;
use std::net::{Ipv4Addr, SocketAddr};
use std::path::{Path, PathBuf};
use tower_http::cors::{Any, CorsLayer};
use tower_http::services::{ServeDir, ServeFile};
use tracing_subscriber;

// ============================================================================
// App State
// ============================================================================

#[derive(Clone)]
pub struct AppState {
    // Add shared state here if needed
}

// ============================================================================
// Build Router
// ============================================================================

pub fn build_router() -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let router = Router::new()
        // Global
        .route("/global/health", get(api::global::health))
        .route("/event", get(api::global::global_event))
        .route("/model_config", get(api::global::get_tura_config))
        .route("/model_config", put(api::global::put_tura_config))
        .route("/about", get(api::about::get_about))
        .route("/about/star", post(api::about::star_repository))
        .route("/about/open", post(api::about::open_target))
        .route("/about/update/check", get(api::about::check_update))
        .route("/about/update/install", post(api::about::install_update))
        // Multica-compatible product surface
        .route("/api/config", get(api::product::public_config))
        .route("/api/me", get(api::product::current_user))
        .route("/api/me", patch(api::product::patch_current_user))
        .route("/api/workspaces", get(api::product::list_workspaces))
        .route("/api/issues", get(api::product::list_issues))
        .route(
            "/api/issues/quick-create",
            post(api::product::quick_create_issue),
        )
        .route("/api/issues/{issueID}", patch(api::product::patch_issue))
        .route("/api/projects", get(api::product::list_product_projects))
        .route("/api/agents", get(api::product::list_product_agents))
        // Auth
        .route("/auth/{providerID}", put(api::provider::set_auth))
        .route(
            "/auth/callback",
            get(api::provider::oauth_redirect_callback),
        )
        // Config
        .route("/config", get(api::global::get_config))
        .route("/config", patch(api::global::patch_config))
        // Project
        .route("/project", get(api::project::list_projects))
        .route("/project/current", get(api::project::get_current_project))
        .route(
            "/project/workspace/create",
            post(api::project::create_named_workspace),
        )
        .route(
            "/project/workspace/default",
            post(api::project::use_default_workspace),
        )
        .route(
            "/project/workspace/select-local",
            post(api::project::select_local_workspace),
        )
        // Session
        .route("/session", get(api::session::list_sessions))
        .route("/session", post(api::session::create_session))
        .route("/session/config", get(api::session::get_session_config))
        .route("/session/config", patch(api::session::patch_session_config))
        .route(
            "/session-log/workspaces",
            get(api::session_log::session_log_workspaces),
        )
        .route(
            "/session-log/sessions",
            get(api::session_log::session_log_sessions),
        )
        .route(
            "/session-log/{sessionID}/records",
            get(api::session_log::session_log_records),
        )
        .route(
            "/session/{sessionID}",
            get(api::session::get_session)
                .patch(api::session::update_session)
                .delete(api::session::delete_session),
        )
        .route(
            "/session/{sessionID}/fork",
            post(api::session::fork_session),
        )
        .route(
            "/session/{sessionID}/task-management",
            patch(api::session::update_session_task_management),
        )
        .route(
            "/session/{sessionID}/abort",
            post(api::session::abort_session),
        )
        .route(
            "/session/{sessionID}/message",
            get(api::session::list_messages),
        )
        .route(
            "/session/{sessionID}/events",
            get(api::global::session_event),
        )
        .route(
            "/session/{sessionID}/prompt_async",
            post(api::session::prompt_async),
        )
        .route("/file", get(api::file::list_files))
        .route("/file/content", get(api::file::get_file_content))
        .route("/file/media", get(api::file::get_file_media))
        .route("/file/input", post(api::file::save_input_file))
        .route("/file/open", post(api::file::open_file))
        .route("/file/open-location", post(api::file::open_file_location))
        // Provider
        .route("/provider", get(api::provider::list_providers))
        .route(
            "/provider/model/validate",
            post(api::provider::validate_model),
        )
        .route("/provider/auth", get(api::provider::provider_auth))
        .route(
            "/provider/{providerID}/auth/status",
            get(api::provider::provider_auth_status),
        )
        .route(
            "/provider/{providerID}/auth/validate",
            post(api::provider::provider_auth_validate),
        )
        .route(
            "/provider/{providerID}/auth/logout",
            post(api::provider::provider_auth_logout),
        )
        .route(
            "/provider/{providerID}/oauth/authorize",
            post(api::provider::oauth_authorize),
        )
        .route(
            "/provider/{providerID}/oauth/callback",
            get(api::provider::oauth_callback_info).post(api::provider::oauth_callback),
        )
        // Agent
        .route(
            "/agent",
            get(api::agent::list_agents).post(api::agent::create_agent),
        )
        .route(
            "/agent/{agentID}",
            get(api::agent::get_agent)
                .put(api::agent::update_agent)
                .patch(api::agent::update_agent)
                .delete(api::agent::delete_agent),
        )
        // Persona
        .route(
            "/persona",
            get(api::persona::list_personas).post(api::persona::create_persona),
        )
        .route(
            "/persona/{personaID}",
            get(api::persona::get_persona)
                .put(api::persona::update_persona)
                .patch(api::persona::update_persona)
                .delete(api::persona::delete_persona),
        )
        // Command
        .route("/command", get(api::command::list_commands))
        .route("/command", post(api::command::execute_command))
        .route("/tool", get(api::tool::list_tools))
        .route("/tool/{toolID}", get(api::tool::get_tool))
        .route("/tool/{toolID}", patch(api::tool::patch_tool))
        .route("/tool/{toolID}/config", get(api::tool::get_tool_config))
        .route("/tool/{toolID}/config", patch(api::tool::patch_tool_config))
        .route("/service/status", get(api::service::get_service_status))
        // Path
        .route("/path", get(api::path::get_paths));

    // Serve the packaged web GUI (Vite SPA build) as a fallback for any path
    // the API routes above don't claim. The API uses explicit, non-root paths,
    // so static assets (`/`, `/index.html`, `/assets/...`) and client-side
    // routes (`/:workspace/...`) all fall through to here. Unknown deep paths
    // are rewritten to index.html so SPA routing works on hard reloads.
    let router = match gui_dist_dir() {
        Some(dir) => {
            let index = dir.join("index.html");
            println!("🖥️ Serving web GUI from {}", dir.display());
            router.fallback_service(ServeDir::new(dir).fallback(ServeFile::new(index)))
        }
        None => router,
    };

    router.layer(cors)
}

/// Resolve the directory holding the built web GUI (`index.html` + `assets/`).
///
/// Honors `TURA_GUI_DIST` first, then release-style `tura_gui/` next to the gateway
/// executable, then repository development build locations. Returns `None`
/// when no built GUI is present so the gateway runs as a pure API server.
fn gui_dist_dir() -> Option<PathBuf> {
    gui_dist_candidates()
        .into_iter()
        .find(|dir| dir.join("index.html").is_file())
}

fn gui_dist_candidates() -> Vec<PathBuf> {
    gui_dist_candidates_for(
        std::env::var_os("TURA_GUI_DIST").map(PathBuf::from),
        std::env::current_exe().ok(),
        std::env::var_os("TURA_PROJECT_ROOT").map(PathBuf::from),
        std::env::current_dir().ok(),
    )
}

fn gui_dist_candidates_for(
    explicit: Option<PathBuf>,
    exe_path: Option<PathBuf>,
    project_root: Option<PathBuf>,
    current_dir: Option<PathBuf>,
) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(dir) = explicit {
        candidates.push(dir);
    }
    if let Some(exe_dir) = exe_path.as_deref().and_then(Path::parent) {
        candidates.push(exe_dir.join("tura_gui"));
    }
    if let Some(root) = project_root {
        candidates.push(root.join("apps").join("gui").join("app").join("dist"));
        candidates.push(root.join("tura_gui"));
    }
    if let Some(cwd) = current_dir {
        candidates.push(cwd.join("tura_gui"));
    }
    candidates
}

fn build_oauth_callback_router() -> Router {
    Router::new()
        .route(
            "/auth/callback",
            get(api::provider::oauth_redirect_callback),
        )
        .route("/callback", get(api::provider::oauth_redirect_callback))
        .route("/global/health", get(api::global::health))
}

// ============================================================================
// Run Server
// ============================================================================

pub async fn run_server(port: u16) -> Result<(), Box<dyn std::error::Error>> {
    run_server_until_shutdown(port, std::future::pending::<()>()).await
}

pub async fn run_server_until_shutdown(
    port: u16,
    shutdown: impl Future<Output = ()> + Send + 'static,
) -> Result<(), Box<dyn std::error::Error>> {
    let startup_started = std::time::Instant::now();

    tracing_subscriber::fmt()
        .with_env_filter("gateway=debug,tower_http=debug")
        .init();

    let addr = local_bind_addr(port);
    let router = build_router();
    api::session::start_task_scheduler();
    api::provider::start_provider_auth_scheduler();

    println!("🚀 Gateway server starting on http://{addr}");
    println!("📡 Health check: http://{addr}/global/health");

    start_openai_oauth_callback_server(port).await;

    let listener = tokio::net::TcpListener::bind(addr).await?;
    let gateway_url = format!("http://{addr}");
    if let Err(error) =
        tura_path::write_active_gateway_url_for_home(tura_path::instance_home(), &gateway_url)
    {
        eprintln!("gateway failed to write active URL {gateway_url}: {error}");
    }
    println!(
        "⏱️ Gateway startup ready in {:.2}s",
        startup_started.elapsed().as_secs_f64()
    );
    axum::serve(listener, router)
        .with_graceful_shutdown(shutdown)
        .await?;

    Ok(())
}

async fn start_openai_oauth_callback_server(main_port: u16) {
    const OAUTH_CALLBACK_PORT: u16 = 1455;
    if main_port == OAUTH_CALLBACK_PORT {
        return;
    }

    let addr = local_bind_addr(OAUTH_CALLBACK_PORT);
    let listener = match tokio::net::TcpListener::bind(addr).await {
        Ok(listener) => listener,
        Err(error) => {
            eprintln!("⚠️ OpenAI OAuth callback server not started on http://{addr}: {error}");
            return;
        }
    };

    println!("🔐 OAuth callback listening on http://{addr}/auth/callback");
    tokio::spawn(async move {
        if let Err(error) = axum::serve(listener, build_oauth_callback_router()).await {
            eprintln!("OAuth callback server stopped: {error}");
        }
    });
}

pub fn local_bind_addr(port: u16) -> SocketAddr {
    SocketAddr::from((Ipv4Addr::LOCALHOST, port))
}

// ============================================================================
// Main entry point for standalone server
// ============================================================================

#[tokio::main]
pub async fn main() {
    let port = std::env::var("PORT")
        .ok()
        .and_then(|value| value.trim().parse::<u16>().ok())
        .unwrap_or_else(|| tura_path::default_gateway_port_for_build_kind(tura_path::build_kind()));

    if let Err(error) = run_server(port).await {
        eprintln!("gateway server stopped with error: {error}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::gui_dist_candidates_for;
    use std::path::PathBuf;

    #[test]
    fn gui_dist_candidates_cover_release_and_repo_builds() {
        let candidates = gui_dist_candidates_for(
            Some(PathBuf::from("explicit")),
            Some(PathBuf::from("target/release/tura_gateway")),
            Some(PathBuf::from("repo")),
            Some(PathBuf::from("cwd")),
        );

        assert_eq!(
            candidates,
            vec![
                PathBuf::from("explicit"),
                PathBuf::from("target/release/tura_gui"),
                PathBuf::from("repo/apps/gui/app/dist"),
                PathBuf::from("repo/tura_gui"),
                PathBuf::from("cwd/tura_gui"),
            ]
        );
    }
}
