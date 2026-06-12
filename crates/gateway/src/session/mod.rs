//! Session module
//!
//! Provides session management using the mano state machine.
//!
//! Files:
//! - manager.rs: Session creation and state machine operations
//! - store.rs: Session persistence and retrieval

pub mod config;
pub mod docker_snapshot;
pub mod manager;
pub mod process_snapshot;
pub mod store;

pub use manager::{SessionInfo, SessionManager, SessionStatus};
pub use store::{session_store, Message, MessagePart, MessageRole, SessionStore};
