//! Runtime worker lifecycle policy owned by router.
//!
//! Default TTL is zero, so workers are expected to exit when a session turn
//! returns to waiting/idle.

pub const MAX_ACTIVE_RUNTIME_WORKERS: usize = 16;
pub const RUNTIME_WORKER_IDLE_TTL_SECS: u64 = 0;
pub const MAX_IDLE_RUNTIME_WORKERS: usize = 0;
