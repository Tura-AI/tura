//! Web module - HTTP server for OpenCode-compatible API

pub mod server;

pub use server::{build_router, local_bind_addr, run_server};
