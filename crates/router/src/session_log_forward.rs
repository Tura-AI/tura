use session_log::SessionLogResponse;

pub fn handle_session_log_json(raw: &str) -> anyhow::Result<SessionLogResponse> {
    session_log::cli::handle_raw_command(raw)
}
