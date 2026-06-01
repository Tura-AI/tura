//! Multi-agent concurrent + recursive dispatch mechanism test (task #16).
//!
//! Verifies that `child_dispatch` drives child sub-sessions through the
//! **CLI subprocess** channel (stdin/stdout JSON, no URL/HTTP):
//! 1. Two top-level child agents dispatched concurrently.
//! 2. Each child recurses once at level 2 to spawn a grandchild (depth=2).
//! 3. Summaries collected and the concatenated payload is checked.
//!
//! `TURA_ROUTER_BIN` is pointed at the in-package `mock_router_for_test`
//! binary, so the test exercises the real concurrent + recursive + summary
//! contract deterministically — without depending on a live LLM or gateway.

use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard, OnceLock};

use code_tools_suite::manas::child_dispatch::{
    dispatch_child_agent, dispatch_child_agents_concurrent, ChildAgentRequest,
};

fn mock_router_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_mock_router_for_test"))
}

/// Process-wide env mutex: cargo test threads share process env, so each test case must serialize its env writes.
fn env_lock() -> MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn reset_mock_env() {
    std::env::set_var("TURA_ROUTER_BIN", mock_router_bin());
    std::env::remove_var("MOCK_RECURSE_TO_DEPTH");
    std::env::remove_var("MOCK_AGENT_SUMMARY");
    std::env::remove_var("MOCK_FAIL");
}

#[test]
fn single_child_dispatch_returns_summary() {
    let _guard = env_lock();
    reset_mock_env();
    std::env::set_var("MOCK_AGENT_SUMMARY", "solo-summary");

    let result = dispatch_child_agent(&ChildAgentRequest {
        agent: "solo".to_string(),
        prompt: "do thing".to_string(),
        directory: None,
        parent_session_id: "parent-A".to_string(),
        depth: 1,
    })
    .expect("dispatch ok");

    assert!(result.ok, "raw={}", result.raw);
    assert_eq!(result.agent, "solo");
    assert!(
        result.summary.contains("solo-summary"),
        "summary missing payload: {}",
        result.summary
    );
}

#[test]
fn concurrent_dispatch_returns_both_summaries() {
    let _guard = env_lock();
    reset_mock_env();

    let requests = vec![
        ChildAgentRequest {
            agent: "agentA".to_string(),
            prompt: "task A".to_string(),
            directory: None,
            parent_session_id: "parent-root".to_string(),
            depth: 1,
        },
        ChildAgentRequest {
            agent: "agentB".to_string(),
            prompt: "task B".to_string(),
            directory: None,
            parent_session_id: "parent-root".to_string(),
            depth: 1,
        },
    ];

    let results = dispatch_child_agents_concurrent(requests);
    assert_eq!(results.len(), 2);

    let agents: Vec<String> = results
        .iter()
        .map(|r| {
            let summary = r.as_ref().expect("ok").clone_summary();
            summary
        })
        .collect();
    assert!(agents.iter().any(|s| s.contains("agentA")));
    assert!(agents.iter().any(|s| s.contains("agentB")));
}

#[test]
fn recursive_dispatch_2_levels_returns_one_summary() {
    let _guard = env_lock();
    reset_mock_env();
    // Top-level agent at depth=1 recurses to depth=2 and folds the grandchild summary back in.
    std::env::set_var("MOCK_RECURSE_TO_DEPTH", "2");
    std::env::set_var("MOCK_AGENT_SUMMARY", "lead-summary");

    let result = dispatch_child_agent(&ChildAgentRequest {
        agent: "lead".to_string(),
        prompt: "split and recurse".to_string(),
        directory: None,
        parent_session_id: "parent-recurse".to_string(),
        depth: 1,
    })
    .expect("dispatch ok");

    assert!(result.ok);
    assert!(
        result.summary.contains("lead-summary"),
        "missing lead summary: {}",
        result.summary
    );
    assert!(
        result.summary.contains("child=["),
        "missing child marker: {}",
        result.summary
    );
    assert!(
        result.summary.contains("depth2"),
        "missing depth-2 child: {}",
        result.summary
    );
}

// Tiny extension: lets the test capture a copy of the summary string.
trait CloneSummary {
    fn clone_summary(&self) -> String;
}
impl CloneSummary for code_tools_suite::manas::child_dispatch::ChildAgentSummary {
    fn clone_summary(&self) -> String {
        format!("{}::{}", self.agent, self.summary)
    }
}
