use serde_json::json;
use session_log_contract::{GetSessionRequest, ServiceEndpoint, SessionLogCommand};

#[test]
fn session_database_command_and_endpoint_shapes_are_stable() {
    assert_eq!(
        serde_json::to_value(SessionLogCommand::GetSession(GetSessionRequest {
            session_id: "session-1".to_string(),
        }))
        .expect("get-session command"),
        json!({ "command": "get_session", "session_id": "session-1" })
    );
    assert_eq!(
        serde_json::to_value(ServiceEndpoint {
            addr: "127.0.0.1:40123".to_string(),
            version: "0.1.0".to_string(),
        })
        .expect("endpoint"),
        json!({ "addr": "127.0.0.1:40123", "version": "0.1.0" })
    );
}
