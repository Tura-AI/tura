//! Web HTTP server using Axum

use crate::api;
use axum::{
    routing::{get, patch, post, put},
    Router,
};
use std::net::{Ipv4Addr, SocketAddr};
use tower_http::cors::{Any, CorsLayer};
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

    Router::new()
        // Global
        .route("/global/health", get(api::global::health))
        .route("/global/event", get(api::global::global_event))
        .route("/event", get(api::global::global_event))
        .route("/global/sync-event", get(api::global::sync_event))
        .route("/global/config", get(api::global::get_config))
        .route("/global/config", patch(api::global::patch_config))
        .route("/model_config", get(api::global::get_tura_config))
        .route("/model_config", put(api::global::put_tura_config))
        .route("/gui_config", get(api::global::get_gui_config))
        .route("/gui_config", put(api::global::put_gui_config))
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
        .route("/session/{sessionID}", patch(api::session::update_session))
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
            "/session/{sessionID}/message/agent",
            post(api::session::send_agent_message),
        )
        .route(
            "/session/{sessionID}/prompt_async",
            post(api::session::prompt_async),
        )
        .route(
            "/session/{sessionID}/user-commands",
            get(api::session::session_user_commands)
                .post(api::session::append_session_user_command),
        )
        .route("/file", get(api::file::list_files))
        .route("/file/content", get(api::file::get_file_content))
        .route("/file/open", post(api::file::open_file))
        .route("/file/open-location", post(api::file::open_file_location))
        // Provider
        .route("/provider", get(api::provider::list_providers))
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
        .route("/path", get(api::path::get_paths))
        // TUI compatibility routes
        .route("/tui/submit-prompt", post(api::session::tui_action))
        .route("/tui/select-session", post(api::session::create_session))
        .route("/tui/append-prompt", post(api::session::tui_action))
        .route("/tui/clear-prompt", post(api::session::tui_action))
        .route("/tui/control/next", post(api::session::tui_action))
        .route("/tui/control/response", post(api::session::tui_action))
        .route("/tui/execute-command", post(api::session::tui_action))
        .route("/tui/open-help", post(api::session::tui_action))
        .route("/tui/open-models", post(api::session::tui_action))
        .route("/tui/open-sessions", post(api::session::tui_action))
        .route("/tui/open-themes", post(api::session::tui_action))
        .route("/tui/publish", post(api::session::tui_action))
        .route("/tui/show-toast", post(api::session::tui_action))
        // Experimental
        .route("/experimental/session", get(api::session::list_sessions))
        .layer(cors)
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
    let startup_started = std::time::Instant::now();

    tracing_subscriber::fmt()
        .with_env_filter("gateway=debug,tower_http=debug")
        .init();

    let addr = local_bind_addr(port);
    let router = build_router();
    api::session::start_task_scheduler();
    api::provider::start_provider_auth_scheduler();

    println!("🚀 Gateway server starting on http://{}", addr);
    println!("📡 Health check: http://{}/global/health", addr);

    start_openai_oauth_callback_server(port).await;

    let listener = tokio::net::TcpListener::bind(addr).await?;
    println!(
        "⏱️ Gateway startup ready in {:.2}s",
        startup_started.elapsed().as_secs_f64()
    );
    axum::serve(listener, router).await?;

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
        .unwrap_or_else(|_| "4096".to_string())
        .parse::<u16>()
        .unwrap_or(4096);

    run_server(port).await.expect("Server error");
}
