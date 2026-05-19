pub fn session_processes(process_snapshot: &str, docker_snapshot: &str) -> String {
    format!(
        "Session process snapshot:\n{process_snapshot}\n\nRunning Docker containers snapshot (sanitized JSON):\n{docker_snapshot}\n\nUse this to understand foreground commands, background services, LSP workers, and Docker containers already running for the session. Do not restart a healthy service just because it is long-lived; inspect its pid/location/logs or use the background service path when appropriate. Docker details are read-only context and intentionally omit environment variables, mounts, and full container commands."
    )
}
