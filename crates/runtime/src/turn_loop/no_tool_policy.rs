pub(crate) fn no_tool_retry_limit() -> u8 {
    std::env::var("TURA_NO_TOOL_RETRY_LIMIT")
        .ok()
        .and_then(|value| value.trim().parse::<u8>().ok())
        .unwrap_or(20)
}
