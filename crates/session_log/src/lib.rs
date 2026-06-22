#![deny(clippy::unwrap_used)]
#![forbid(unsafe_code)]

pub mod checkpoint;
pub mod cli;
pub mod client;
pub mod client_protocol;
pub mod file_queue;
pub mod ipc;
pub mod path;
pub mod protocol;
pub mod queue;
pub mod service;
pub mod session_state;
pub mod store;

pub use checkpoint::{CheckpointType, CommandCheckpoint};
pub use protocol::{
    DeleteSessionRequest, DeleteWorkspaceRequest, GetSessionRequest, ListSessionRecordsRequest,
    ListSessionsRequest, Page, SessionLogCommand, SessionLogResponse, SessionRecord,
    SessionSnapshot, SessionSummary, UpsertSessionRequest, WorkspaceSummary,
};
pub use session_state::SessionState;
pub use store::SessionLogStore;
