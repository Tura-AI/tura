use std::{
    io::{BufRead, BufReader, Write},
    net::{SocketAddr, TcpStream},
    path::Path,
    time::{Duration, Instant},
};

use crate::session_log_client::SessionLogClient;
use lifecycle::SessionManagement;

pub(crate) fn load_persisted_gateway_session(
    directory: &Path,
    session_id: &str,
) -> Result<Option<SessionManagement>, String> {
    ensure_session_db_owner_for_persisted_reads();
    let snapshot = SessionLogClient::discover()
        .map_err(|err| format!("failed to discover session_log: {err}"))?
        .get_session(session_id.to_string())
        .map_err(|err| format!("failed to load persisted session {session_id}: {err}"))?;
    let Some(snapshot) = snapshot else {
        return Ok(None);
    };
    if normalize_workspace(&snapshot.workspace) != normalize_workspace(&directory.to_string_lossy())
    {
        return Ok(None);
    }
    let mut management = decode_persisted_management(session_id, snapshot.management)?;
    let context = SessionLogClient::discover()
        .map_err(|err| format!("failed to discover session_log: {err}"))?
        .read_context_slice(
            session_id.to_string(),
            management.context_tokens.limit.max(1),
        )?;
    let window_from_sequence = context
        .records
        .first()
        .map(|record| record.sequence)
        .unwrap_or(context.next_sequence);
    if window_from_sequence < context.retained_from_sequence {
        return Err(format!(
            "persisted context for {session_id} starts before canonical retention"
        ));
    }
    if context
        .records
        .iter()
        .enumerate()
        .any(|(index, record)| record.sequence != window_from_sequence.saturating_add(index as u64))
    {
        return Err(format!(
            "persisted context for {session_id} contains a sequence gap"
        ));
    }
    management.session_log = context
        .records
        .into_iter()
        .map(|record| record.raw_record)
        .collect();
    management.session_log_retention.omitted_entries = window_from_sequence;
    Ok(Some(management))
}

fn decode_persisted_management(
    session_id: &str,
    value: serde_json::Value,
) -> Result<SessionManagement, String> {
    serde_json::from_value(value)
        .map_err(|err| format!("invalid persisted SessionManagement for {session_id}: {err}"))
}

fn normalize_workspace(value: &str) -> String {
    let normalized = value.replace('\\', "/");
    let trimmed = normalized.trim_end_matches('/');
    if trimmed.is_empty() && normalized.starts_with('/') {
        "/".to_string()
    } else if trimmed.is_empty() {
        normalized
    } else {
        trimmed.to_string()
    }
}

fn ensure_session_db_owner_for_persisted_reads() {
    if session_log_contract::client::service_is_running() {
        return;
    }
    for addr in router_addrs_for_current_home() {
        let _ = router_health_check(addr.trim());
        if wait_for_session_db_service(Duration::from_secs(10)) {
            return;
        }
    }
}

fn router_addrs_for_current_home() -> Vec<String> {
    let mut addrs = Vec::new();
    if let Ok(addr) = std::env::var("TURA_ROUTER_ADDR") {
        let addr = addr.trim();
        if !addr.is_empty() {
            addrs.push(addr.to_string());
        }
    }
    if let Some(addr) = router_addr_from_file() {
        if !addrs.iter().any(|existing| existing == &addr) {
            addrs.push(addr);
        }
    }
    addrs
}

fn router_addr_from_file() -> Option<String> {
    let path = session_log_contract::client::default_db_dir().join("router.addr");
    let raw = std::fs::read_to_string(path).ok()?;
    let value: serde_json::Value = serde_json::from_str(raw.trim()).ok()?;
    let version = value
        .get("version")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    if !version.is_empty() && version != tura_path::instance_version() {
        return None;
    }
    value
        .get("addr")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|addr| !addr.is_empty())
        .map(str::to_string)
}

fn wait_for_session_db_service(timeout: Duration) -> bool {
    let started = Instant::now();
    while started.elapsed() < timeout {
        if session_log_contract::client::service_is_running() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    session_log_contract::client::service_is_running()
}

fn router_health_check(addr: &str) -> Result<(), String> {
    let socket: SocketAddr = addr
        .parse()
        .map_err(|err| format!("invalid router address {addr:?}: {err}"))?;
    let stream = TcpStream::connect_timeout(&socket, Duration::from_secs(2))
        .map_err(|err| format!("failed to connect to router at {addr}: {err}"))?;
    stream
        .set_read_timeout(Some(Duration::from_secs(11)))
        .map_err(|err| format!("failed to set router read timeout: {err}"))?;
    stream
        .set_write_timeout(Some(Duration::from_secs(10)))
        .map_err(|err| format!("failed to set router write timeout: {err}"))?;
    let request = serde_json::json!({
        "request_id": "runtime-session-bootstrap-health",
        "kind": "health_check",
        "method": "health_check",
        "payload": {},
        "deadline_ms": 10_000,
    });
    let mut writer = stream
        .try_clone()
        .map_err(|err| format!("failed to clone router stream: {err}"))?;
    writer
        .write_all(format!("{request}\n").as_bytes())
        .map_err(|err| format!("failed to write router health check: {err}"))?;
    writer
        .flush()
        .map_err(|err| format!("failed to flush router health check: {err}"))?;

    let mut line = String::new();
    BufReader::new(stream)
        .read_line(&mut line)
        .map_err(|err| format!("failed to read router health response: {err}"))?;
    let response: serde_json::Value = serde_json::from_str(line.trim())
        .map_err(|err| format!("invalid router health response: {err}"))?;
    if response
        .get("ok")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false)
    {
        Ok(())
    } else {
        Err(response
            .get("error")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("router health check failed")
            .to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::{decode_persisted_management, router_addr_from_file, router_health_check};
    use chrono::Utc;
    use lifecycle::SessionState;
    use lifecycle::{SessionInput, SessionManagement};
    use std::io::{BufRead, BufReader, Write};
    use std::net::TcpListener;
    use std::path::PathBuf;
    use std::thread;

    struct EnvGuard {
        previous: Vec<(&'static str, Option<std::ffi::OsString>)>,
    }

    impl EnvGuard {
        fn set_home(home: &std::path::Path) -> Self {
            let keys = ["TURA_HOME", "SESSION_LOG_DB_ROOT", "TURA_DB_ROOT"];
            let previous = keys
                .iter()
                .map(|key| (*key, std::env::var_os(key)))
                .collect::<Vec<_>>();
            std::env::set_var("TURA_HOME", home);
            std::env::remove_var("SESSION_LOG_DB_ROOT");
            std::env::remove_var("TURA_DB_ROOT");
            Self { previous }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            for (key, value) in self.previous.drain(..) {
                match value {
                    Some(value) => std::env::set_var(key, value),
                    None => std::env::remove_var(key),
                }
            }
        }
    }

    fn persisted_management(state: SessionState) -> serde_json::Value {
        let now = Utc::now();
        serde_json::to_value(SessionManagement::new(
            "persisted-session".to_string(),
            "Persisted".to_string(),
            PathBuf::from("C:/workspace"),
            false,
            "coding".to_string(),
            SessionInput {
                user_input: "resume".to_string(),
                file_input: Vec::new(),
                agent: None,
                runtime_context: None,
                planning_mode_override: None,
            },
            "resume".to_string(),
            now,
        ))
        .map(|mut value| {
            value["state"] = serde_json::to_value(state).expect("state should serialize");
            value
        })
        .expect("management should serialize")
    }

    #[test]
    fn interrupted_management_decodes_without_losing_history() {
        let value = persisted_management(SessionState::Interrupted);
        let decoded = decode_persisted_management("persisted-session", value)
            .expect("interrupted state should be a first-class FSM state");

        assert_eq!(decoded.state, SessionState::Interrupted);
        assert_eq!(decoded.session_id, "persisted-session");
    }

    #[test]
    fn invalid_internal_state_returns_an_error_instead_of_none() {
        let mut value = persisted_management(SessionState::Running);
        value["state"] = serde_json::json!("Running");

        let error = decode_persisted_management("persisted-session", value)
            .expect_err("PascalCase internal state should not be accepted");

        assert!(error.contains("persisted-session"));
        assert!(error.contains("invalid persisted SessionManagement"));
    }

    #[test]
    fn router_health_check_uses_router_ipc_contract() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("test router listener should bind");
        let addr = listener.local_addr().expect("listener should have address");
        let handle = thread::spawn(move || {
            let (stream, _) = listener.accept().expect("client should connect");
            let mut line = String::new();
            BufReader::new(stream.try_clone().expect("clone stream"))
                .read_line(&mut line)
                .expect("read health request");
            let request: serde_json::Value =
                serde_json::from_str(line.trim()).expect("health request should be json");
            assert_eq!(request["kind"], "health_check");
            assert_eq!(request["method"], "health_check");
            let mut writer = stream;
            writer
                .write_all(
                    br#"{"request_id":"runtime-session-bootstrap-health","ok":true,"payload":{"status":"ok"}}"#,
                )
                .expect("write response");
            writer.write_all(b"\n").expect("write newline");
        });

        router_health_check(&addr.to_string()).expect("health check should succeed");
        handle.join().expect("test router thread should finish");
    }

    #[test]
    fn router_addr_from_file_reads_current_home_endpoint() {
        let temp = tempfile::tempdir().expect("temp home");
        let _env = EnvGuard::set_home(temp.path());
        let db_dir = session_log_contract::client::default_db_dir();
        std::fs::create_dir_all(&db_dir).expect("db dir");
        std::fs::write(
            db_dir.join("router.addr"),
            serde_json::json!({
                "addr": "127.0.0.1:34567",
                "version": tura_path::instance_version(),
            })
            .to_string(),
        )
        .expect("router addr");

        assert_eq!(router_addr_from_file().as_deref(), Some("127.0.0.1:34567"));
    }
}
