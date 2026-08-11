use gateway::api::session::get_session_config_value;

#[test]
fn unconfigured_session_api_returns_a_loadable_default_agent() {
    let config = get_session_config_value(None);
    let agent_id = config
        .active_agent
        .expect("default session config should select an agent");
    let agent = gateway::api::agent::get_agent_value(agent_id.clone())
        .expect("default session agent should load through the gateway registry");

    assert_eq!(agent.summary.id, agent_id);
    assert!(agent.config.report_to_user);
}
