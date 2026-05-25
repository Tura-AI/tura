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
        // Multica-compatible product surface
        .route("/api/config", get(api::product::public_config))
        .route("/api/me", get(api::product::current_user))
        .route("/api/me", patch(api::product::patch_current_user))
        .route("/api/workspaces", get(api::product::list_workspaces))
        .route("/api/workspaces", post(api::product::create_workspace))
        .route(
            "/api/workspaces/{workspaceID}",
            get(api::product::get_workspace),
        )
        .route(
            "/api/workspaces/{workspaceID}",
            patch(api::product::patch_workspace),
        )
        .route(
            "/api/workspaces/{workspaceID}/members",
            get(api::product::list_workspace_members),
        )
        .route("/api/issues", get(api::product::list_issues))
        .route("/api/issues", post(api::product::create_issue))
        .route("/api/issues/grouped", get(api::product::grouped_issues))
        .route("/api/issues/search", get(api::product::search_issues))
        .route(
            "/api/issues/quick-create",
            post(api::product::quick_create_issue),
        )
        .route(
            "/api/issues/batch-update",
            post(api::product::batch_update_issues),
        )
        .route("/api/issues/{issueID}", get(api::product::get_issue))
        .route("/api/issues/{issueID}", patch(api::product::patch_issue))
        .route(
            "/api/issues/{issueID}/comments",
            get(api::product::list_issue_comments),
        )
        .route(
            "/api/issues/{issueID}/timeline",
            get(api::product::issue_timeline),
        )
        .route(
            "/api/issues/{issueID}/active-task",
            get(api::product::issue_active_task),
        )
        .route(
            "/api/issues/{issueID}/task-runs",
            get(api::product::issue_task_runs),
        )
        .route(
            "/api/issues/{issueID}/usage",
            get(api::product::issue_usage),
        )
        .route("/api/projects", get(api::product::list_product_projects))
        .route("/api/projects", post(api::product::create_product_project))
        .route(
            "/api/projects/search",
            get(api::product::search_product_projects),
        )
        .route(
            "/api/projects/{projectID}",
            get(api::product::get_product_project),
        )
        .route(
            "/api/projects/{projectID}",
            patch(api::product::patch_product_project),
        )
        .route("/api/agents", get(api::product::list_product_agents))
        .route("/api/agent-templates", get(api::product::agent_templates))
        .route("/api/runtimes", get(api::product::list_runtimes))
        .route("/api/skills", get(api::product::list_product_skills))
        .route("/api/autopilots", get(api::product::list_autopilots))
        .route("/api/chat/sessions", get(api::product::list_chat_sessions))
        .route("/api/inbox", get(api::product::list_inbox))
        .route(
            "/api/inbox/unread-count",
            get(api::product::inbox_unread_count),
        )
        .route(
            "/api/dashboard/usage/daily",
            get(api::product::dashboard_usage_daily),
        )
        .route(
            "/api/dashboard/usage/by-agent",
            get(api::product::dashboard_usage_by_agent),
        )
        .route(
            "/api/dashboard/usage/agent-runtime",
            get(api::product::dashboard_agent_runtime),
        )
        .route(
            "/api/agent-task-snapshot",
            get(api::product::agent_task_snapshot),
        )
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
        .route("/file/open", post(api::file::open_file))
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
            "/provider/{providerID}/validate",
            post(api::provider::provider_auth_validate),
        )
        .route(
            "/provider/{providerID}/auth/status",
            get(api::provider::provider_auth_status),
        )
        .route(
            "/provider/{providerID}/auth/refresh",
            post(api::provider::provider_auth_refresh),
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
