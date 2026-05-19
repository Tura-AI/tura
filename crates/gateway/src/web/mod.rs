//! Web module - HTTP server for OpenCode-compatible API

pub mod server;

pub use server::{build_router, run_server};
