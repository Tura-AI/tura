use std::sync::atomic::Ordering;

use crate::app::AppState;
use crate::daemon::unpublish_router_addr;

pub(crate) fn start_idle_shutdown_monitor(state: AppState) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_millis(250));
        loop {
            interval.tick().await;
            if state.shutdown.load(Ordering::SeqCst) {
                return;
            }
            let active_runtime_workers = state.manager.count_workers_with_prefix("runtime_worker:");
            let active_sessions = state.execution.active_session_count();
            if state
                .lifecycle
                .should_shutdown_idle(active_runtime_workers, active_sessions)
            {
                unpublish_router_addr();
                let stopped = state
                    .manager
                    .stop_workers_with_prefix("runtime_worker:")
                    .await;
                state.session_db.stop();
                mark_router_shutting_down(&state);
                eprintln!("router idle shutdown: stopped {stopped} runtime workers and session_db");
                return;
            }
        }
    });
}

pub(crate) fn mark_router_shutting_down(state: &AppState) {
    state.shutdown.store(true, Ordering::SeqCst);
    unpublish_router_addr();
}
