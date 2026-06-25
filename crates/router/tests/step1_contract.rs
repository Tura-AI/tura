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
    let handlers = read("crates/router/src/ipc_handlers.rs");
    for method in [
        "health_check",
        "session_db.lifecycle.start",
        "session_db.lifecycle.status",
        "session_db.lifecycle.restart",
        "lifecycle.front_heartbeat",
        "lifecycle.status",
        "execution.enqueue_turn",
        "execution.cancel_turn",
        "execution.probe_sessions",
        "execution.get_status",
        "execution.kill_session_workers",
        "execution.shutdown",
    ] {
        assert!(handlers.contains(method), "router IPC missing {method}");
    }
    assert!(
        !handlers.contains("\"session_db.call\"") && !handlers.contains("\"session-log\""),
        "router must not expose session DB read/write data calls"
    );
    assert!(
        !handlers.contains("session-db-call"),
        "router must not expose a one-shot session DB process path"
    );
    assert!(
        !read("crates/router/src/lib.rs").contains("session_log_forward"),
        "router library must not export the old session_log_forward bridge"
    );
}

#[test]
fn gateway_uses_router_enqueue_and_direct_session_db_client() {
    let session_api = read("crates/gateway/src/api/session_prompt.rs");
    assert!(
        session_api.contains("RouterClient::global()") && session_api.contains("enqueue_turn"),
        "gateway prompt path must enqueue through router client"
    );
    assert!(
        !session_api.contains("persist_session_ack")
            && !session_api.contains("file_queue::enqueue_command"),
        "gateway prompt path must not write session DB before enqueue"
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
fn runtime_session_db_client_uses_file_queue_without_one_shot_processes() {
    let path = "crates/runtime/src/session_log_client.rs";
    let source = read(path);
    assert!(
        source.contains("file_queue::enqueue_command")
            && source.contains("ipc::call_service")
            && !source.contains("SessionLogStore::open_default")
            && !source.contains("file_queue::drain_queue"),
        "{path} must enqueue writes and read only through the session_db socket"
    );
    let forbidden_direct_env = ["TURA_SESSION_DB_ALLOW", "DIRECT"].join("_");
    for forbidden in [
        "session-db-call",
        "Command::new",
        "wait_with_timeout",
        "kill_process_tree",
        "CREATE_NO_WINDOW",
        "router_binary",
        "resolve_router_binary",
    ] {
        assert!(
            !source.contains(forbidden),
            "{path} must not keep one-shot session DB process flow: {forbidden}"
        );
    }
    assert!(
        !source.contains(&forbidden_direct_env),
        "{path} must not keep one-shot session DB process flow: {forbidden_direct_env}"
    );
}

#[test]
fn gateway_session_db_client_is_read_only_without_one_shot_processes() {
    let path = "crates/gateway/src/session_db_client.rs";
    let source = read(path);
    assert!(
        source.contains("gateway session_db client is read-only; write command rejected")
            && source.contains("ipc::call_service")
            && source.contains("fn is_read_command")
            && !source.contains("file_queue::enqueue_command")
            && !source.contains("SessionLogStore::open_default")
            && !source.contains("file_queue::drain_queue"),
        "{path} must be a read-only session_db socket client"
    );
    let forbidden_direct_env = ["TURA_SESSION_DB_ALLOW", "DIRECT"].join("_");
    for forbidden in [
        "session-db-call",
        "Command::new",
        "wait_with_timeout",
        "kill_process_tree",
        "CREATE_NO_WINDOW",
        "router_binary",
        "resolve_router_binary",
    ] {
        assert!(
            !source.contains(forbidden),
            "{path} must not keep one-shot session DB process flow: {forbidden}"
        );
    }
    assert!(
        !source.contains(&forbidden_direct_env),
        "{path} must not keep one-shot session DB process flow: {forbidden_direct_env}"
    );
}

#[test]
fn session_db_service_replays_durable_queue_on_startup() {
    let store = read("crates/session_log/src/store.rs");
    let store_queue = read("crates/session_log/src/store/queue.rs");
    let store_write = read("crates/session_log/src/store/write.rs");
    let service = read("crates/session_log/src/service.rs");
    assert!(
        store.contains("mod queue;")
            && store_queue.contains("pub fn replay_pending_write_queue")
            && store_queue.contains("FROM session_write_queue")
            && store_queue.contains("WHERE status = 'pending'")
            && service.contains("store.replay_pending_write_queue()?"),
        "session_db service must replay the durable write queue during startup"
    );
    assert!(
        service.contains("file_queue::drain_queue(&store, 1000)"),
        "session_db service must own draining the file-backed runtime write queue"
    );
    assert!(
        store.contains("mod write;")
            && store_write.contains("pub fn mark_running_sessions_interrupted")
            && store_write.contains("pub fn mark_stale_running_sessions_interrupted")
            && service.contains("store.mark_running_sessions_interrupted()?"),
        "session_db service startup must mark non-reattachable running work interrupted"
    );
}

#[test]
fn runtime_acks_streamed_command_checkpoints_through_session_db() {
    let protocol = read("crates/session_log/src/protocol.rs");
    let runtime_client = read("crates/runtime/src/session_log_client.rs");
    let checkpointing = read("crates/runtime/src/provider_flow/checkpointing.rs");
    let command_streaming = read("crates/runtime/src/provider_flow/command_run_streaming.rs");
    assert!(
        protocol.contains("ApplyCommandCheckpoint(Box<CommandCheckpoint>)")
            && runtime_client.contains("pub fn apply_command_checkpoint")
            && checkpointing.contains("checkpoint_streamed_command_finished")
            && command_streaming.contains("checkpointing::streamed_command_finished")
            && command_streaming.contains("session_db command checkpoint ACK failed"),
        "runtime streamed command results must ACK durable command checkpoints through session_db"
    );
}

#[test]
fn gui_dev_gateway_is_parent_owned_and_refuses_unknown_port_owner() {
    let vite = read("apps/gui/app/vite.config.ts");
    assert!(
        vite.contains("ownedGatewayChild")
            && vite.contains("server.httpServer?.once(\"close\"")
            && vite.contains("killOwnedGateway()"),
        "GUI dev must keep an owned gateway child and kill it when Vite closes"
    );
    assert!(
        !vite.contains("detached: true") && !vite.contains("child.unref()"),
        "GUI dev gateway must not be detached or unrefed"
    );
    assert!(
        vite.contains("canBindGatewayUrl") && vite.contains("unknown or foreign process"),
        "GUI dev must fail rather than spawn when the gateway port is occupied by an unknown process"
    );
}
