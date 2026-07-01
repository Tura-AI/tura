use gateway::session::config::DEFAULT_SESSION_AGENT;

#[test]
fn gateway_default_session_agent_is_thoughtful() {
    assert_eq!(DEFAULT_SESSION_AGENT, "thoughtful");
}
