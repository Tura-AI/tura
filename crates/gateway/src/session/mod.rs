//! Session module
//!
//! Provides API projection caching backed by the Session lifecycle service.
//!
//! Files:
//! - manager.rs: Session projection construction and API metadata
//! - store.rs: Projection caching and typed Session service access

pub mod config;
pub mod docker_snapshot;
pub mod manager;
pub mod process_snapshot;
pub mod store;

pub use manager::{SessionInfo, SessionManager};
pub use store::{session_store, Message, MessagePart, MessageRole, SessionStore};
