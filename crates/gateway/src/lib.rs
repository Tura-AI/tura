#![deny(clippy::unwrap_used)]
#![forbid(unsafe_code)]

pub mod api;
pub mod channel;
pub mod handler;
pub mod media;
pub mod mock;
pub mod process_lock;
pub mod router_client;
pub mod router_process;
pub mod runtime;
pub mod session;
pub mod session_db_client;
pub mod simple_runtime;
pub mod types;
pub mod web;

pub use channel::ChannelSender;
pub use handler::ProcessedMessageHandler;
pub use media::GatewayMediaProcessor;
pub use runtime::GatewayRuntime;
pub use session::{session_store, SessionInfo, SessionManager, SessionStatus, SessionStore};
pub use simple_runtime::{SimpleGatewayRuntime, SimpleMessageHandler};
pub use types::*;

#[cfg(test)]
pub(crate) mod test_support {
    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    static CURRENT_DIRECTORY_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    pub(crate) fn env_lock() -> std::sync::MutexGuard<'static, ()> {
        ENV_LOCK.lock().unwrap_or_else(|error| error.into_inner())
    }

    pub(crate) fn current_directory_lock() -> std::sync::MutexGuard<'static, ()> {
        CURRENT_DIRECTORY_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner())
    }
}
