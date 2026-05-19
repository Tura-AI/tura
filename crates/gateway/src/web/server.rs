//! Web HTTP server using Axum

use crate::api;
use axum::{
    routing::{delete, get, patch, post, put},
    Router,
};
use std::net::SocketAddr;
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
        .route("/global/dispose", post(api::global::dispose))
        .route("/global/upgrade", post(api::global::upgrade))
        // Auth
        .route("/auth/{providerID}", put(api::provider::set_auth))
        .route("/auth/{providerID}", delete(api::provider::remove_auth))
        .route(
            "/auth/callback",
            get(api::provider::oauth_redirect_callback),
        )
        // Config
        .route("/config", get(api::global::get_config))
        .route("/config", patch(api::global::patch_config))
        .route("/config/providers", get(api::misc::get_config_providers))
        // Project
        .route("/project", get(api::project::list_projects))
        .route("/project/current", get(api::project::get_current_project))
        .route("/project/{projectID}", get(api::project::get_project))
        .route("/project/{projectID}", patch(api::project::update_project))
        .route("/project/git/init", post(api::project::git_init_project))
        // Experimental
        .route(
            "/experimental/worktree",
            post(api::project::create_worktree),
        )
        .route(
            "/experimental/worktree/reset",
            post(api::project::reset_worktree),
        )
        // Session
        .route("/session", get(api::session::list_sessions))
        .route("/session", post(api::session::create_session))
        .route("/session/config", get(api::session::get_session_config))
        .route("/session/config", patch(api::session::patch_session_config))
        .route("/session/status", get(api::session::session_status))
        .route("/session/{sessionID}", get(api::session::get_session))
        .route("/session/{sessionID}", patch(api::session::update_session))
        .route("/session/{sessionID}", delete(api::session::delete_session))
        .route(
            "/session/{sessionID}/abort",
            post(api::session::abort_session),
        )
        .route(
            "/session/{sessionID}/status",
            post(api::session::update_session_status_for_runtime),
        )
        .route(
            "/session/{sessionID}/child",
            post(api::session::register_child_session),
        )
        .route(
            "/session/{sessionID}/children",
            get(api::session::session_children),
        )
        .route(
            "/session/{sessionID}/user-commands",
            get(api::session::session_user_commands)
                .post(api::session::append_session_user_command),
        )
        .route(
            "/session/{sessionID}/command",
            post(api::session::session_command),
        )
        .route(
            "/session/{sessionID}/diff",
            get(api::session::get_session_diff),
        )
        .route(
            "/session/{sessionID}/fork",
            post(api::session::fork_session),
        )
        .route(
            "/session/{sessionID}/init",
            post(api::session::create_session),
        )
        .route(
            "/session/{sessionID}/message",
            get(api::session::list_messages),
        )
        .route(
            "/session/{sessionID}/message",
            post(api::session::send_message),
        )
        .route(
            "/session/{sessionID}/message/agent",
            post(api::session::send_agent_message),
        )
        .route(
            "/session/{sessionID}/message/{messageID}",
            get(api::session::get_message),
        )
        .route(
            "/session/{sessionID}/message/{messageID}/part/{partID}",
            get(api::session::get_message_part),
        )
        .route(
            "/session/{sessionID}/permissions/{permissionID}",
            get(api::session::list_session_permission_by_id),
        )
        .route(
            "/session/{sessionID}/permissions/{permissionID}/reply",
            get(api::session::get_permission_reply),
        )
        .route(
            "/session/{sessionID}/permissions",
            get(api::session::list_permissions).post(api::session::create_permission),
        )
        .route(
            "/session/{sessionID}/prompt_async",
            post(api::session::prompt_async),
        )
        .route(
            "/session/{sessionID}/revert",
            post(api::session::revert_session),
        )
        .route(
            "/session/{sessionID}/share",
            post(api::session::share_session),
        )
        .route(
            "/session/{sessionID}/shell",
            post(api::session::session_shell),
        )
        .route(
            "/session/{sessionID}/summarize",
            post(api::session::summarize_session),
        )
        .route("/session/{sessionID}/todo", get(api::session::get_todos))
        .route(
            "/session/{sessionID}/todo",
            post(api::session::update_todos),
        )
        .route(
            "/session/{sessionID}/unrevert",
            post(api::session::unrevert_session),
        )
        .route("/file", get(api::file::list_files))
        .route("/file", post(api::file::write_file))
        .route("/file/content", get(api::file::get_file_content))
        .route("/file/content", post(api::file::write_file))
        .route("/file/status", get(api::file::get_file_status))
        // Find
        .route("/find", get(api::file::find_files))
        .route("/find/file", get(api::file::find_files))
        .route("/find/symbol", get(api::file::find_symbols))
        // Provider
        .route("/provider", get(api::provider::list_providers))
        .route("/provider/auth", get(api::provider::provider_auth))
        .route(
            "/provider/model/validate",
            post(api::provider::validate_model),
        )
        .route(
            "/provider/{providerID}/oauth/authorize",
            post(api::provider::oauth_authorize),
        )
        .route(
            "/provider/{providerID}/oauth/callback",
            post(api::provider::oauth_callback),
        )
        // Permission
        .route("/permission", get(api::misc::list_permissions))
        .route(
            "/permission/{requestID}/reply",
            post(api::session::reply_permission),
        )
        // Question
        .route("/question", get(api::misc::list_questions))
        .route(
            "/question/{requestID}/reject",
            post(api::misc::reject_question),
        )
        .route(
            "/question/{requestID}/reply",
            post(api::misc::reply_question),
        )
        // PTY
        .route("/pty", get(api::pty::list_pty))
        .route("/pty", post(api::pty::create_pty))
        .route("/pty/{ptyID}", get(api::pty::get_pty))
        .route("/pty/{ptyID}", put(api::pty::update_pty))
        .route("/pty/{ptyID}", delete(api::pty::delete_pty))
        .route("/pty/{ptyID}/connect", get(api::pty::pty_connect))
        // MCP
        .route("/mcp", get(api::mcp::list_mcp_servers))
        .route("/mcp/{name}/connect", post(api::mcp::mcp_connect))
        .route("/mcp/{name}/disconnect", post(api::mcp::mcp_disconnect))
        .route("/mcp/{name}/tool/{tool}", post(api::mcp::mcp_call_tool))
        .route("/mcp/{name}/resource", get(api::mcp::mcp_read_resource))
        .route("/mcp/{name}/auth", post(api::mcp::mcp_auth))
        .route(
            "/mcp/{name}/auth/authenticate",
            post(api::mcp::mcp_authenticate),
        )
        .route(
            "/mcp/{name}/auth/callback",
            get(api::mcp::mcp_auth_callback),
        )
        // Agent
        .route("/agent", get(api::misc::list_agents))
        // Command
        .route("/command", get(api::misc::list_commands))
        .route("/command", post(api::misc::execute_command))
        // VCS
        .route("/vcs", get(api::misc::get_vcs_info))
        .route("/vcs/diff", get(api::misc::get_vcs_diff))
        // LSP
        .route("/lsp", get(api::misc::get_lsp_status))
        .route("/service/status", get(api::misc::get_service_status))
        .route(
            "/service/process/{pid}/stop",
            post(api::misc::stop_service_process),
        )
        // Skill
        .route("/skill", get(api::misc::list_skills))
        .route("/plugin", get(api::misc::list_plugins))
        // Path
        .route("/path", get(api::misc::get_paths))
        // Formatter
        .route("/formatter", post(api::misc::format_code))
        // Log
        .route("/log", post(api::misc::write_log))
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
        .route("/experimental/console", get(api::misc::list_agents))
        .route("/experimental/console/orgs", get(api::misc::list_agents))
        .route(
            "/experimental/console/switch",
            post(api::misc::console_switch),
        )
        .route("/experimental/resource", get(api::mcp::list_mcp_resources))
        .route("/experimental/session", get(api::session::list_sessions))
        .route("/experimental/tool", get(api::misc::list_agents))
        .route("/experimental/tool/ids", get(api::misc::list_agents))
        .route("/experimental/workspace", get(api::misc::list_agents))
        .route("/experimental/workspace/{id}", get(api::misc::list_agents))
        .route(
            "/experimental/directory-picker",
            post(api::misc::open_directory_picker),
        )
        // Instance
        .route("/instance/dispose", post(api::global::dispose))
        .layer(cors)
}

fn build_oauth_callback_router() -> Router {
    Router::new()
        .route(
            "/auth/callback",
            get(api::provider::oauth_redirect_callback),
        )
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

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let router = build_router();

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

    let addr = SocketAddr::from(([0, 0, 0, 0], OAUTH_CALLBACK_PORT));
    let listener = match tokio::net::TcpListener::bind(addr).await {
        Ok(listener) => listener,
        Err(error) => {
            eprintln!("⚠️ OpenAI OAuth callback server not started on http://{addr}: {error}");
            return;
        }
    };

    println!("🔐 OpenAI OAuth callback listening on http://{addr}/auth/callback");
    tokio::spawn(async move {
        if let Err(error) = axum::serve(listener, build_oauth_callback_router()).await {
            eprintln!("OpenAI OAuth callback server stopped: {error}");
        }
    });
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
