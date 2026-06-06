use std::{fs, path::PathBuf};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.parent())
        .expect("router crate should be under crates/router")
        .to_path_buf()
}

fn read(path: &str) -> String {
    fs::read_to_string(repo_root().join(path)).expect(path)
}

#[test]
fn router_ipc_has_supervision_methods_but_no_session_db_data_call() {
    let main = read("crates/router/src/main.rs");
    for method in [
        "health_check",
        "session_db.lifecycle.start",
        "session_db.lifecycle.status",
        "session_db.lifecycle.restart",
        "execution.enqueue_turn",
        "execution.cancel_turn",
        "execution.get_status",
        "execution.kill_session_workers",
        "execution.shutdown",
    ] {
        assert!(main.contains(method), "router IPC missing {method}");
    }
    assert!(
        !main.contains("\"session_db.call\"") && !main.contains("\"session-log\""),
        "router must not expose session DB read/write data calls"
    );
    assert!(
        !read("crates/router/src/lib.rs").contains("session_log_forward"),
        "router library must not export the old session_log_forward bridge"
    );
}

#[test]
fn gateway_uses_router_enqueue_and_direct_session_db_client() {
    let session_api = read("crates/gateway/src/api/session.rs");
    assert!(
        session_api.contains("RouterClient::global()")
            && session_api.contains("enqueue_turn")
            && session_api.contains("persist_session_ack"),
        "gateway prompt path must persist then enqueue through router client"
    );
    assert!(
        !session_api.contains("TURA_ROLE\", \"runtime_worker")
            && !session_api.contains("TURA_ROLE=runtime_worker"),
        "gateway session API must not spawn runtime workers directly"
    );

    let gateway_bin = read("crates/gateway/src/bin/gateway.rs");
    assert!(
        gateway_bin.contains("SessionDbClient::discover()?.call(command)")
            && !gateway_bin.contains("session_log_forward"),
        "gateway session-log compatibility CLI must use direct SessionDbClient"
    );
}

#[test]
fn runtime_and_gateway_session_db_calls_use_session_db_service_cli() {
    for path in [
        "crates/gateway/src/session_db_client.rs",
        "crates/runtime/src/session_log_client.rs",
    ] {
        let source = read(path);
        assert!(
            source.contains("session-db-call"),
            "{path} must call the session_db service endpoint"
        );
        assert!(
            source.contains("CREATE_NO_WINDOW"),
            "{path} must hide child process windows on Windows"
        );
        assert!(
            !source.contains("SessionLogStore::open_default"),
            "{path} must not open PostgreSQL directly"
        );
    }
}

#[test]
fn session_db_service_replays_durable_queue_on_startup() {
    let store = read("crates/session_log/src/store.rs");
    let service = read("crates/session_log/src/service.rs");
    assert!(
        store.contains("pub fn replay_pending_write_queue")
            && store.contains("FROM session_write_queue")
            && store.contains("WHERE status = 'pending'")
            && service.contains("store.replay_pending_write_queue()?"),
        "session_db service must replay the durable write queue during startup"
    );
    assert!(
        store.contains("pub fn mark_running_sessions_interrupted")
            && service.contains("store.mark_running_sessions_interrupted()?"),
        "session_db service startup must mark non-reattachable running work interrupted"
    );
}

#[test]
fn runtime_acks_streamed_command_checkpoints_through_session_db() {
    let protocol = read("crates/session_log/src/protocol.rs");
    let runtime_client = read("crates/runtime/src/session_log_client.rs");
    let runtime_call = read("crates/runtime/src/runtime/call_runtime.rs");
    assert!(
        protocol.contains("ApplyCommandCheckpoint(Box<CommandCheckpoint>)")
            && runtime_client.contains("pub fn apply_command_checkpoint")
            && runtime_call.contains("ack_streamed_command_checkpoint")
            && runtime_call.contains("session_db command checkpoint ACK failed"),
        "runtime streamed command results must ACK durable command checkpoints through session_db"
    );
}

#[test]
fn playwright_lite_tura_usage_reports_final_cumulative_phase() {
    let script =
        read("tests/business/frontend-playwright/react_ops_board_playwright_repair_lite.mjs");
    assert!(
        script.contains("reporting_mode: useFinalPhaseUsage ? \"final_phase_cumulative\"")
            && script.contains("agentKind(agentId).startsWith(\"tura-\") && second.total > 0"),
        "Playwright Lite must not double count Tura cumulative phase usage"
    );
}
